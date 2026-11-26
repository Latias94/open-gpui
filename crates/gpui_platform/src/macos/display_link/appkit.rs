use super::frame_state::{FrameLinkGeneration, FrameLinkState};
use objc2::{
    ClassType, DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send,
    rc::Retained, sel,
};
use objc2_app_kit::NSView;
use objc2_foundation::{NSObject, NSObjectProtocol, NSRunLoop, NSRunLoopCommonModes};
use objc2_quartz_core::CADisplayLink;
use open_gpui::scheduler::Instant;
use open_gpui::{FrameRequestSource, PlatformFrameSignal, RequestFrameOptions};
use std::cell::{Cell, RefCell};

struct DisplayLinkTargetIvars(Box<dyn Fn()>);

define_class!(
    // SAFETY: NSObject has no additional subclassing requirements.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "GPUIAppKitDisplayLinkTarget"]
    #[ivars = DisplayLinkTargetIvars]
    struct DisplayLinkTarget;

    unsafe impl NSObjectProtocol for DisplayLinkTarget {}

    impl DisplayLinkTarget {
        #[unsafe(method(displayLinkDidFire:))]
        fn display_link_did_fire(&self, _link: &CADisplayLink) {
            let target = self.retain();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                (target.ivars().0)();
            }));
            if let Err(panic) = result {
                std::mem::forget(panic);
                log::error!("isolated a panic from an AppKit display-link target");
            }
        }
    }
);

impl DisplayLinkTarget {
    fn new(main_thread: MainThreadMarker, callback: impl Fn() + 'static) -> Retained<Self> {
        let this = Self::alloc(main_thread).set_ivars(DisplayLinkTargetIvars(Box::new(callback)));
        // SAFETY: NSObject's init is its designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}

struct AppKitFrameSource {
    link: Retained<CADisplayLink>,
    _target: Retained<DisplayLinkTarget>,
}

impl AppKitFrameSource {
    fn new(view: &NSView, main_thread: MainThreadMarker, callback: impl Fn() + 'static) -> Self {
        let target = DisplayLinkTarget::new(main_thread, callback);
        let link =
            unsafe { view.displayLinkWithTarget_selector(&target, sel!(displayLinkDidFire:)) };
        unsafe { link.addToRunLoop_forMode(&NSRunLoop::mainRunLoop(), NSRunLoopCommonModes) };
        Self {
            link,
            _target: target,
        }
    }
}

impl Drop for AppKitFrameSource {
    fn drop(&mut self) {
        self.link.invalidate();
    }
}

/// Schedules WGPU window frames from a display link associated with the AppKit view.
#[derive(Default)]
pub(crate) struct AppKitFrames {
    source: RefCell<Option<AppKitFrameSource>>,
    state: RefCell<FrameLinkState>,
    signal: PlatformFrameSignal,
    require_presentation: Cell<bool>,
    force_render: Cell<bool>,
}

impl AppKitFrames {
    pub fn is_available() -> bool {
        NSView::class().responds_to(sel!(displayLinkWithTarget:selector:))
    }

    pub fn start(
        &self,
        view: &NSView,
        main_thread: MainThreadMarker,
        callback: impl Fn(FrameLinkGeneration) + 'static,
    ) {
        self.stop();
        let generation = self.state.borrow_mut().attach();
        let source = AppKitFrameSource::new(view, main_thread, move || callback(generation));
        *self.source.borrow_mut() = Some(source);
    }

    pub fn is_attached(&self) -> bool {
        self.state.borrow().is_attached()
    }

    /// Returns the native pause state, or `None` while no source can be observed.
    pub fn is_paused(&self) -> Option<bool> {
        self.source
            .try_borrow()
            .ok()
            .and_then(|source| source.as_ref().map(|source| source.link.isPaused()))
    }

    /// Returns whether a frame was requested since the current update began, without consuming it.
    pub fn is_woken(&self) -> bool {
        self.state.borrow().is_woken()
    }

    pub fn wake(&self) {
        self.request_frame(RequestFrameOptions {
            signal_at: PlatformFrameSignal::capture(Instant::now),
            signal_source: FrameRequestSource::LocalSchedule,
            ..Default::default()
        });
    }

    pub fn request_frame(&self, options: RequestFrameOptions) {
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
            source.link.setPaused(false);
        }
    }

    pub fn begin_frame(
        &self,
        generation: FrameLinkGeneration,
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

    pub fn end_frame(&self, generation: FrameLinkGeneration, retry_pending: bool) -> bool {
        let pause = self.state.borrow_mut().end_frame(generation, retry_pending);
        if pause && let Some(source) = self.source.borrow().as_ref() {
            source.link.setPaused(true);
        }
        pause
    }

    pub fn stop(&self) {
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
    fn diagnostic_reads_preserve_pending_requests_and_wakes() {
        let frames = AppKitFrames::default();
        let generation = frames.state.borrow_mut().attach();
        let signal_at = PlatformFrameSignal::capture(Instant::now);
        frames.request_frame(RequestFrameOptions {
            require_presentation: true,
            force_render: true,
            signal_at,
            signal_source: FrameRequestSource::LocalSchedule,
        });
        for _ in 0..2 {
            assert!(frames.is_attached());
            assert!(frames.is_woken());
            assert_eq!(frames.is_paused(), None);
        }
        let options = frames.begin_frame(generation, None).unwrap();
        assert!(options.require_presentation);
        assert!(options.force_render);
        assert_eq!(options.signal_at, signal_at);
        if signal_at.is_some() {
            assert_eq!(options.signal_source, FrameRequestSource::LocalSchedule);
        }
        assert!(!frames.is_woken());

        frames.wake();
        for _ in 0..2 {
            assert!(frames.is_woken());
            assert_eq!(frames.is_paused(), None);
        }
        assert!(!frames.end_frame(generation, false));
        let next = frames.begin_frame(generation, None).unwrap();
        assert!(!next.require_presentation);
        assert!(!next.force_render);
        assert!(!frames.is_woken());
        assert!(frames.end_frame(generation, false));

        frames.stop();
        assert!(!frames.is_attached());
        assert!(!frames.is_woken());
        assert_eq!(frames.is_paused(), None);
    }

    #[test]
    fn pending_requests_keep_requirements_and_the_earliest_signal_pair() {
        let frames = AppKitFrames::default();
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
        let frames = AppKitFrames::default();
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
