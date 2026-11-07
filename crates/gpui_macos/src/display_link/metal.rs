use super::metal_state::{MetalFrameGeneration, MetalFrameState};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send,
    rc::Retained,
    runtime::{AnyClass, ProtocolObject},
};
use objc2_foundation::{NSObject, NSObjectProtocol, NSRunLoop, NSRunLoopCommonModes};
use objc2_quartz_core::{
    CAMetalDisplayLink, CAMetalDisplayLinkDelegate, CAMetalDisplayLinkUpdate, CAMetalLayer,
};
use open_gpui::{FrameRequestSource, PlatformFrameSignal, RequestFrameOptions};
use open_gpui_scheduler::Instant;
use std::cell::{Cell, RefCell};

struct MetalDisplayLinkDelegateIvars {
    callback: Box<dyn Fn(&CAMetalDisplayLinkUpdate)>,
}

define_class!(
    // SAFETY: NSObject has no additional subclassing requirements.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = MetalDisplayLinkDelegateIvars]
    struct MetalDisplayLinkDelegate;

    unsafe impl NSObjectProtocol for MetalDisplayLinkDelegate {}

    unsafe impl CAMetalDisplayLinkDelegate for MetalDisplayLinkDelegate {
        #[unsafe(method(metalDisplayLink:needsUpdate:))]
        fn needs_update(&self, _link: &CAMetalDisplayLink, update: &CAMetalDisplayLinkUpdate) {
            // The callback may destroy its frame source, releasing our owner's retain.
            let delegate = self.retain();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                (delegate.ivars().callback)(update);
            }));
            if let Err(panic) = result {
                std::mem::forget(panic);
                log::error!("isolated a panic from a macOS Metal display-link delegate");
            }
        }
    }
);

impl MetalDisplayLinkDelegate {
    fn new(
        main_thread: MainThreadMarker,
        callback: impl Fn(&CAMetalDisplayLinkUpdate) + 'static,
    ) -> Retained<Self> {
        let this = Self::alloc(main_thread).set_ivars(MetalDisplayLinkDelegateIvars {
            callback: Box::new(callback),
        });
        // SAFETY: NSObject's init is its designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}

/// A main-run-loop source of Metal drawables on macOS 14 and later.
pub(crate) struct MetalFrameSource {
    link: Retained<CAMetalDisplayLink>,
    _delegate: Retained<MetalDisplayLinkDelegate>,
    _main_thread: MainThreadMarker,
}

impl MetalFrameSource {
    pub(crate) fn is_available() -> bool {
        AnyClass::get(c"CAMetalDisplayLink").is_some()
    }

    fn new(
        layer: &CAMetalLayer,
        main_thread: MainThreadMarker,
        callback: impl Fn(&CAMetalDisplayLinkUpdate) + 'static,
    ) -> Option<Self> {
        if !Self::is_available() {
            return None;
        }
        let delegate = MetalDisplayLinkDelegate::new(main_thread, callback);
        let link = CAMetalDisplayLink::initWithMetalLayer(CAMetalDisplayLink::alloc(), layer);
        link.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        // The default range follows the display and system power policy:
        // https://developer.apple.com/documentation/quartzcore/cametaldisplaylink/preferredframeraterange
        // SAFETY: The marker confines this source to the main thread. The main
        // run loop outlives it, and Drop invalidates it before releasing its delegate.
        unsafe { link.addToRunLoop_forMode(&NSRunLoop::mainRunLoop(), NSRunLoopCommonModes) };
        Some(Self {
            link,
            _delegate: delegate,
            _main_thread: main_thread,
        })
    }

    fn set_paused(&self, paused: bool) {
        self.link.setPaused(paused);
    }
}

impl Drop for MetalFrameSource {
    fn drop(&mut self) {
        self.link.invalidate();
    }
}

/// Shares a window's Metal link with its foreground frame waker.
#[derive(Default)]
pub(crate) struct MetalFrames {
    source: RefCell<Option<MetalFrameSource>>,
    state: RefCell<MetalFrameState>,
    signal: PlatformFrameSignal,
    require_presentation: Cell<bool>,
    force_render: Cell<bool>,
}

impl MetalFrames {
    pub(crate) fn start(
        &self,
        layer: &CAMetalLayer,
        main_thread: MainThreadMarker,
        callback: impl Fn(MetalFrameGeneration, &CAMetalDisplayLinkUpdate) + 'static,
    ) -> bool {
        self.stop();
        let generation = self.state.borrow_mut().attach();
        let source = MetalFrameSource::new(layer, main_thread, move |update| {
            callback(generation, update)
        });
        let attached = source.is_some();
        *self.source.borrow_mut() = source;
        if !attached {
            self.state.borrow_mut().retire();
        }
        attached
    }

    pub(crate) fn is_attached(&self) -> bool {
        self.state.borrow().is_attached()
    }

    pub(crate) fn is_current(&self, generation: MetalFrameGeneration) -> bool {
        self.state.borrow().is_current(generation)
    }

    pub(crate) fn wake(&self) {
        self.request_frame(RequestFrameOptions {
            signal_at: PlatformFrameSignal::capture(Instant::now),
            signal_source: FrameRequestSource::LocalSchedule,
            ..Default::default()
        });
    }

    pub(crate) fn request_frame(&self, options: RequestFrameOptions) {
        if let Some(at) = options.signal_at {
            self.signal.record(at, options.signal_source);
        }
        self.require_presentation
            .set(self.require_presentation.get() || options.require_presentation);
        self.force_render
            .set(self.force_render.get() || options.force_render);
        self.state.borrow_mut().wake();
        if let Ok(source) = self.source.try_borrow()
            && let Some(source) = source.as_ref()
        {
            source.set_paused(false);
        }
    }

    pub(crate) fn begin_frame(
        &self,
        generation: MetalFrameGeneration,
        signal_at: Option<Instant>,
    ) -> Option<RequestFrameOptions> {
        if !self.state.borrow_mut().begin_frame(generation) {
            return None;
        }
        if let Some(at) = signal_at {
            self.signal.record(at, FrameRequestSource::NativeCallback);
        }
        let (signal_at, signal_source) = self.signal.take().map_or(
            (None, FrameRequestSource::NativeCallback),
            |(at, source)| (Some(at), source),
        );
        Some(RequestFrameOptions {
            require_presentation: self.require_presentation.replace(false),
            force_render: self.force_render.replace(false),
            signal_at,
            signal_source,
        })
    }

    pub(crate) fn end_frame(&self, generation: MetalFrameGeneration, used_drawable: bool) -> bool {
        let pause = self.state.borrow_mut().end_frame(generation, used_drawable);
        if pause && let Some(source) = self.source.borrow().as_ref() {
            source.set_paused(true);
        }
        pause
    }

    pub(crate) fn stop(&self) {
        self.state.borrow_mut().retire();
        let source = self.source.borrow_mut().take();
        drop(source);
        self.signal.take();
        self.require_presentation.set(false);
        self.force_render.set(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn pending_requests_keep_requirements_and_the_earliest_signal_pair() {
        let frames = MetalFrames::default();
        let generation = frames.state.borrow_mut().attach();
        let first = Instant::now() + Duration::from_secs(1);
        frames.request_frame(RequestFrameOptions {
            require_presentation: true,
            signal_at: Some(first),
            signal_source: FrameRequestSource::LocalSchedule,
            ..Default::default()
        });
        frames.request_frame(RequestFrameOptions {
            force_render: true,
            signal_at: Some(first + Duration::from_millis(1)),
            signal_source: FrameRequestSource::NativeCallback,
            ..Default::default()
        });
        let options = frames
            .begin_frame(generation, Some(first + Duration::from_millis(2)))
            .unwrap();
        assert!(options.require_presentation);
        assert!(options.force_render);
        if PlatformFrameSignal::capture(|| first).is_some() {
            assert_eq!(options.signal_at, Some(first));
            assert_eq!(options.signal_source, FrameRequestSource::LocalSchedule);
        } else {
            assert_eq!(options.signal_at, None);
        }
        assert!(frames.end_frame(generation, false));
        let next = frames.begin_frame(generation, None).unwrap();
        assert!(!next.require_presentation);
        assert!(!next.force_render);
        assert_eq!(next.signal_at, None);
    }

    #[test]
    fn stopped_source_cannot_deliver_old_requirements_to_its_replacement() {
        let frames = MetalFrames::default();
        let old = frames.state.borrow_mut().attach();
        frames.request_frame(RequestFrameOptions {
            require_presentation: true,
            force_render: true,
            signal_at: Some(Instant::now()),
            signal_source: FrameRequestSource::LocalSchedule,
        });
        frames.stop();
        let current = frames.state.borrow_mut().attach();
        assert!(frames.begin_frame(old, Some(Instant::now())).is_none());
        let options = frames.begin_frame(current, None).unwrap();
        assert!(!options.require_presentation);
        assert!(!options.force_render);
        assert_eq!(options.signal_at, None);
        assert!(!frames.end_frame(old, false));
        frames.wake();
        assert!(!frames.end_frame(current, false));
    }
}
