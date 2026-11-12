//! iOS Window implementation using UIWindow and UIViewController.
//!
//! iOS windows are fundamentally different from desktop windows:
//! - Always fullscreen (or split-screen on iPad)
//! - No title bar or window chrome
//! - Touch-based input
//! - Safe area insets for notch/home indicator
//!
//! The window is backed by a UIWindow containing a UIViewController
//! whose view hosts a CAMetalLayer.

use super::events::*;
use super::text_input::TextInputView;
use super::{CallbackSlot, IosDisplay, platform::IosPlatformState};
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, ProtocolObject};
use objc2::{
    ClassType, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{
    NSArray, NSNotification, NSNotificationCenter, NSObjectProtocol, NSSet, NSValue,
};
use objc2_metal::MTLTexture as _;
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};
use objc2_ui_kit::{
    NSValueUIGeometryExtensions, UIKeyboardFrameEndUserInfoKey,
    UIKeyboardWillChangeFrameNotification, UIKeyboardWillHideNotification, UISceneActivationState,
    UIStatusBarStyle, UIWindowDidBecomeHiddenNotification, UIWindowDidBecomeKeyNotification,
    UIWindowDidBecomeVisibleNotification, UIWindowDidResignKeyNotification,
};
use objc2_ui_kit::{
    UICoordinateSpace, UICornerConfiguration, UICornerRadius, UIEvent, UIGestureRecognizer,
    UIGestureRecognizerDelegate, UIPanGestureRecognizer, UIRectCorner, UIRectEdge, UIScreen,
    UIScreenEdgePanGestureRecognizer, UIScrollTypeMask, UITouch, UITraitCollection,
    UITraitEnvironment, UIUserInterfaceLayoutDirection, UIUserInterfaceStyle, UIView,
    UIViewAutoresizing, UIViewController, UIWindow, UIWindowScene,
};
use open_gpui::{
    AnyWindowHandle, BackGestureEvent, BackGesturePhase, Bounds, Capslock, Corners, DevicePixels,
    DisplayId, Edges, FrameRequestSource, GpuSpecs, Modifiers, NativePointerCancelReservation,
    Pixels, PlatformAtlas, PlatformDisplay, PlatformFrameSignal, PlatformInput,
    PlatformInputCallback, PlatformInputCallbackSlot, PlatformInputHandler,
    PlatformInputHandlerSlot, PlatformNativeWindowRetirementOutcome,
    PlatformPresentationShutdownOutcome, PlatformWindow, PlatformWindowActiveStatusObservation,
    PlatformWindowCommand, PlatformWindowCommandDispatcher, PlatformWindowCommandOutcome,
    PlatformWindowInteractionQuiescence, PlatformWindowPresentOutcome, Point,
    PreparedPlatformPresentationShutdown, PromptButton, PromptLevel, RequestFrameOptions, Scene,
    Size, TextInputConfiguration, TextInputStateChange, TouchEvent, TouchId, TouchPhase,
    WindowAppearance, WindowBackgroundAppearance, WindowBounds, WindowControlArea,
    WindowCreationFacts, WindowInsets, WindowParams, WindowPresentationShutdownTicket,
    WindowVisibility, px, size,
};
use open_gpui_apple::{main_thread_marker, metal_renderer::MetalRenderer};
use parking_lot::Mutex;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, UiKitDisplayHandle, UiKitWindowHandle};
use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    ptr::NonNull,
    rc::{Rc, Weak},
    sync::Arc,
    time::{Duration, Instant},
};

const KEYBOARD_DISMISS_DISTANCE: Pixels = px(24.);

mod frame_demand;
mod lifecycle;
mod native_window;

use frame_demand::FrameDemand;
use native_window::NativeWindow;

#[derive(Clone, Copy)]
struct KeyboardDismissTouch {
    id: TouchId,
    start_position: Point<Pixels>,
}

/// Global storage for the current status bar style.
/// 0 = default (dark content), 1 = light content.
/// Accessed from the main thread only.
static STATUS_BAR_STYLE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

#[derive(Default)]
pub(super) struct WindowReference(pub(super) RefCell<Weak<IosWindowState>>);

impl WindowReference {
    pub(super) fn with_window<R>(
        &self,
        callback: impl FnOnce(&Rc<IosWindowState>) -> R,
    ) -> Option<R> {
        let window = self.0.borrow().upgrade()?;
        // Keep callback storage and native objects alive even if the callback
        // synchronously closes the platform window.
        Some(callback(&window))
    }
}

define_class!(
    #[unsafe(super = UIViewController)]
    #[thread_kind = MainThreadOnly]
    #[name = "GPUIViewController"]
    #[ivars = WindowReference]
    struct ViewController;

    impl ViewController {
        #[unsafe(method(preferredStatusBarStyle))]
        fn preferred_status_bar_style(&self) -> UIStatusBarStyle {
            let style = STATUS_BAR_STYLE.load(std::sync::atomic::Ordering::Relaxed);
            if style == 1 {
                UIStatusBarStyle::LightContent
            } else {
                UIStatusBarStyle::DarkContent
            }
        }

        #[unsafe(method(viewDidLayoutSubviews))]
        fn view_did_layout_subviews(&self) {
            unsafe {
                let _: () = msg_send![super(self), viewDidLayoutSubviews];
            }
            self.ivars().with_window(IosWindowState::handle_layout_change);
        }
    }
);

impl ViewController {
    fn new(main_thread: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(main_thread).set_ivars(WindowReference::default());
        unsafe { msg_send![super(this), init] }
    }
}

/// Set the iOS status bar content style (light or dark text/icons).
///
/// This updates the stored style and asks the root view controller
/// to re-query `preferredStatusBarStyle`.
pub fn set_status_bar_style(style: crate::StatusBarContentStyle) {
    use crate::StatusBarContentStyle;

    let value = match style {
        StatusBarContentStyle::Light => 1,
        StatusBarContentStyle::Dark => 0,
    };
    STATUS_BAR_STYLE.store(value, std::sync::atomic::Ordering::Relaxed);

    // This public free function predates explicit platform ownership.
    // Keep its single-application behavior without making runtime events global.
    if let Some(platform) = super::application::current_platform() {
        platform.application.with_windows(|window| {
            window.view_controller.setNeedsStatusBarAppearanceUpdate();
        });
    }
}

define_class!(
    #[unsafe(super = UIView)]
    #[thread_kind = MainThreadOnly]
    #[name = "GPUIMetalView"]
    #[ivars = WindowReference]
    struct MetalView;

    unsafe impl NSObjectProtocol for MetalView {}

    impl MetalView {
        #[unsafe(method(layerClass))]
        fn layer_class() -> &'static AnyClass {
            CAMetalLayer::class()
        }

        #[unsafe(method(gpuiSubmittedDrawableSize))]
        fn submitted_drawable_size(&self) -> CGSize {
            self.ivars()
                .with_window(|window| window.submitted_drawable_size.get())
                .unwrap_or(CGSize::ZERO)
        }

        #[unsafe(method(gpuiSubmissionSerial))]
        fn submission_serial(&self) -> u64 {
            self.ivars()
                .with_window(|window| window.submission_serial.get())
                .unwrap_or(0)
        }

        // This callback also covers iOS 15/16, before UIKit's trait-registration API.
        #[unsafe(method(traitCollectionDidChange:))]
        fn trait_collection_did_change(&self, previous: Option<&UITraitCollection>) {
            unsafe {
                let _: () = msg_send![super(self), traitCollectionDidChange: previous];
                if self
                    .traitCollection()
                    .hasDifferentColorAppearanceComparedToTraitCollection(previous)
                {
                    self.ivars()
                        .with_window(|window| window.notify_appearance_changed());
                }
            }
        }

        #[unsafe(method(touchesBegan:withEvent:))]
        fn touches_began(&self, touches: &NSSet<UITouch>, event: Option<&UIEvent>) {
            self.handle_touches(touches, event);
        }

        #[unsafe(method(touchesMoved:withEvent:))]
        fn touches_moved(&self, touches: &NSSet<UITouch>, event: Option<&UIEvent>) {
            self.handle_touches(touches, event);
        }

        #[unsafe(method(touchesEnded:withEvent:))]
        fn touches_ended(&self, touches: &NSSet<UITouch>, event: Option<&UIEvent>) {
            self.handle_touches(touches, event);
        }

        #[unsafe(method(touchesCancelled:withEvent:))]
        fn touches_cancelled(&self, touches: &NSSet<UITouch>, event: Option<&UIEvent>) {
            self.handle_touches(touches, event);
        }

        #[unsafe(method(handleScroll:))]
        fn handle_scroll(&self, recognizer: &UIPanGestureRecognizer) {
            self.ivars()
                .with_window(|window| window.handle_scroll(recognizer));
        }

        #[unsafe(method(handleBackGesture:))]
        fn handle_back_gesture(&self, recognizer: &UIScreenEdgePanGestureRecognizer) {
            self.ivars()
                .with_window(|window| window.handle_back_gesture(recognizer));
        }
    }

    unsafe impl UIGestureRecognizerDelegate for MetalView {
        // Only the back-gesture recognizer uses this view as its delegate.
        #[unsafe(method(gestureRecognizerShouldBegin:))]
        fn gesture_recognizer_should_begin(&self, recognizer: &UIGestureRecognizer) -> bool {
            self.ivars()
                .with_window(|window| window.begin_back_gesture(recognizer))
                .unwrap_or(false)
        }
    }
);

impl MetalView {
    fn new(frame: CGRect, main_thread: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(main_thread).set_ivars(WindowReference::default());
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    fn handle_touches(&self, touches: &NSSet<UITouch>, event: Option<&UIEvent>) {
        for touch in touches {
            self.ivars()
                .with_window(|window| window.handle_touch(&touch, event));
        }
    }
}

pub(crate) struct IosWindow {
    state: Rc<IosWindowState>,
}

impl std::ops::Deref for IosWindow {
    type Target = IosWindowState;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

#[allow(clippy::type_complexity)]
pub(crate) struct IosWindowState {
    platform: Weak<IosPlatformState>,
    foreground_executor: open_gpui::ForegroundExecutor,
    background_executor: open_gpui::BackgroundExecutor,
    handle: AnyWindowHandle,
    creation_facts: WindowCreationFacts,
    closed: Cell<bool>,
    interaction_quiesced: Rc<Cell<bool>>,
    initial_presentation_completed: Cell<bool>,
    presentation_shutdown: RefCell<Option<WindowPresentationShutdownTicket>>,
    frame_demand: FrameDemand,
    local_frame_scheduled: Cell<bool>,
    drawable_size: Cell<Size<DevicePixels>>,
    renderer_size: Cell<Size<DevicePixels>>,
    submitted_drawable_size: Cell<CGSize>,
    submission_serial: Cell<u64>,
    insets: RefCell<WindowInsets>,
    native_observers: RefCell<Vec<Retained<objc2::runtime::ProtocolObject<dyn NSObjectProtocol>>>>,
    /// The UIWindow object
    window: Retained<UIWindow>,
    /// The UIViewController
    view_controller: Retained<ViewController>,
    /// The Metal-backed UIView
    view: Retained<MetalView>,
    /// The hidden text input view for keyboard input
    text_input_view: Retained<TextInputView>,
    /// Current bounds in pixels
    bounds: Cell<Bounds<Pixels>>,
    /// Scale factor
    scale_factor: Cell<f32>,
    /// Input handler for text input
    input_handler: PlatformInputHandlerSlot,
    request_frame_callback: CallbackSlot<Box<dyn Fn(RequestFrameOptions)>>,
    /// Callback for input events
    input_callback: PlatformInputCallbackSlot,
    /// Callback for active status changes
    active_status_callback: CallbackSlot<Box<dyn Fn(PlatformWindowActiveStatusObservation)>>,
    visibility: Cell<WindowVisibility>,
    visibility_callback: CallbackSlot<Box<dyn FnMut(WindowVisibility)>>,
    /// Callback for hover status changes (not really applicable on iOS)
    hover_status_callback: CallbackSlot<Box<dyn Fn(bool)>>,
    /// Callback for resize events
    resize_callback: CallbackSlot<Box<dyn Fn(Size<Pixels>, f32)>>,
    display_id: Cell<DisplayId>,
    display_changed_callback: CallbackSlot<Box<dyn FnMut()>>,
    /// Callback for move events (not applicable on iOS)
    moved_callback: CallbackSlot<Box<dyn Fn()>>,
    /// Callback for should close
    should_close_callback: CallbackSlot<Box<dyn FnMut() -> bool>>,
    /// Callback for hit test
    hit_test_callback: CallbackSlot<Box<dyn FnMut() -> Option<WindowControlArea>>>,
    /// Callback for close
    close_callback: CallbackSlot<Box<dyn FnOnce()>>,
    /// Callback for appearance changes
    appearance_changed_callback: CallbackSlot<Box<dyn Fn()>>,
    insets_changed_callback: CallbackSlot<Box<dyn FnMut(WindowInsets)>>,
    keyboard_dismiss_touch: Cell<Option<KeyboardDismissTouch>>,
    /// Where the back gesture an element claimed started, while it runs.
    back_gesture_start: Cell<Option<Point<Pixels>>>,
    keyboard_height: Cell<f32>,
    /// Current mouse position (from touch)
    mouse_position: Cell<Point<Pixels>>,
    /// Current modifiers
    modifiers: Cell<Modifiers>,
    renderer: Mutex<MetalRenderer>,
}

impl IosWindow {
    #[allow(deprecated)] // Window construction can precede scene connection.
    pub fn new(
        handle: AnyWindowHandle,
        params: WindowParams,
        platform: &Rc<IosPlatformState>,
    ) -> anyhow::Result<Self> {
        let main_thread = main_thread_marker().expect("UIKit requires the main thread");
        // Create UIWindow
        let window_scene = platform.application.window_scene();
        let window_scene = window_scene.as_deref();
        let (screen_bounds_cg, scale) = if let Some(scene) = window_scene {
            let screen = scene.screen();
            let bounds = scene.coordinateSpace().bounds();
            (bounds, screen.scale())
        } else {
            let screen = UIScreen::mainScreen(main_thread);
            let bounds = screen.bounds();
            (bounds, screen.scale())
        };
        let interaction_quiesced = Rc::new(Cell::new(false));
        let window = NativeWindow::new(
            screen_bounds_cg,
            window_scene,
            interaction_quiesced.clone(),
            main_thread,
        );

        let display = IosDisplay::from_screen(window.screen());
        anyhow::ensure!(
            params
                .display_id
                .is_none_or(|display_id| display_id == display.id()),
            "the requested iOS display does not own the connected UIKit scene"
        );

        let metal_frame = CGRect::new(CGPoint::ZERO, screen_bounds_cg.size);
        let view = MetalView::new(metal_frame, main_thread);

        let layer = view.layer();
        layer.setContentsScale(scale);
        let mut renderer = MetalRenderer::from_layer(
            platform.instance_buffer_pool.clone(),
            layer
                .downcast_ref::<CAMetalLayer>()
                .ok_or_else(|| anyhow::anyhow!("UIKit view must have a CAMetalLayer"))?,
            false,
        )?;
        let pixel_w = (screen_bounds_cg.size.width * scale) as i32;
        let pixel_h = (screen_bounds_cg.size.height * scale) as i32;
        renderer.update_drawable_size(size(DevicePixels(pixel_w), DevicePixels(pixel_h)));
        renderer.set_display_link_attached(true);

        // Auto-resize the Metal view when the parent view changes size
        // (e.g. rotation). UIViewAutoresizingFlexibleWidth | UIViewAutoresizingFlexibleHeight
        view.setAutoresizingMask(
            UIViewAutoresizing::FlexibleWidth | UIViewAutoresizing::FlexibleHeight,
        );

        // Enable user interaction on the Metal view for touch handling
        view.setUserInteractionEnabled(params.accepts_pointer_input);
        view.setMultipleTouchEnabled(true);

        // UIKit has no public API for the display's corner radius, but since
        // iOS 26 a view filling the window with a container-concentric corner
        // configuration resolves its radii to the display's. This only sets the
        // layer's corner radius; without `masksToBounds` the Metal drawable is
        // not clipped.
        if reports_corner_radii(&view) {
            view.setCornerConfiguration(&UICornerConfiguration::configurationWithRadius(
                &UICornerRadius::containerConcentricRadius(),
            ));
        }

        // Trackpad and mouse-wheel scrolling reaches UIKit apps only through a
        // pan recognizer that accepts scroll types. With no allowed touch types
        // it never claims finger touches, so those keep reaching `touchesBegan`
        // and GPUI's own recognizer.
        let scroll_target: &AnyObject = &view;
        // SAFETY: `MetalView` implements `handleScroll:` and owns the recognizer.
        let scroll_recognizer = unsafe {
            UIPanGestureRecognizer::initWithTarget_action(
                UIPanGestureRecognizer::alloc(main_thread),
                Some(scroll_target),
                Some(sel!(handleScroll:)),
            )
        };
        scroll_recognizer.setAllowedScrollTypesMask(UIScrollTypeMask::All);
        scroll_recognizer.setAllowedTouchTypes(&NSArray::new());
        scroll_recognizer.setCancelsTouchesInView(false);
        view.addGestureRecognizer(&scroll_recognizer);

        // The leading-edge swipe is iOS's back gesture. The delegate lets it
        // begin only when an element claims it; it then cancels the touches it
        // took over, so GPUI's own recognizer abandons any pan it had started.
        let back_target: &AnyObject = &view;
        // SAFETY: `MetalView` implements `handleBackGesture:` and owns the recognizer.
        let back_recognizer = unsafe {
            UIScreenEdgePanGestureRecognizer::initWithTarget_action(
                UIScreenEdgePanGestureRecognizer::alloc(main_thread),
                Some(back_target),
                Some(sel!(handleBackGesture:)),
            )
        };
        back_recognizer.setEdges(
            if view.effectiveUserInterfaceLayoutDirection()
                == UIUserInterfaceLayoutDirection::RightToLeft
            {
                UIRectEdge::Right
            } else {
                UIRectEdge::Left
            },
        );
        back_recognizer.setDelegate(Some(ProtocolObject::from_ref(&*view)));
        view.addGestureRecognizer(&back_recognizer);

        let view_controller = ViewController::new(main_thread);
        view_controller.setView(Some(&view));
        window.setRootViewController(Some(&view_controller));

        // Create a hidden text input view for keyboard handling.
        // Uses our custom GPUITextInputView which implements UIKeyInput
        // so iOS actually routes keyboard text to us.
        let text_input_frame = CGRect::new(CGPoint::ZERO, CGSize::new(1.0, 1.0));
        let text_input_view = TextInputView::new(text_input_frame, main_thread);
        text_input_view.setAlpha(0.01);
        text_input_view.setUserInteractionEnabled(true);
        view.addSubview(&text_input_view);

        let state = IosWindowState {
            platform: Rc::downgrade(platform),
            foreground_executor: platform.foreground_executor.clone(),
            background_executor: platform.background_executor.clone(),
            handle,
            creation_facts: WindowCreationFacts {
                show: params.show,
                focus_on_appearing: params.focus_on_appearing,
                transient_for: None,
            },
            closed: Cell::new(false),
            interaction_quiesced,
            initial_presentation_completed: Cell::new(false),
            presentation_shutdown: RefCell::new(None),
            frame_demand: FrameDemand::default(),
            local_frame_scheduled: Cell::new(false),
            drawable_size: Cell::new(size(DevicePixels(pixel_w), DevicePixels(pixel_h))),
            renderer_size: Cell::new(size(DevicePixels(pixel_w), DevicePixels(pixel_h))),
            submitted_drawable_size: Cell::new(CGSize::ZERO),
            submission_serial: Cell::new(0),
            insets: RefCell::new(WindowInsets::default()),
            native_observers: RefCell::default(),
            display_id: Cell::new(IosDisplay::from_screen(window.screen()).id()),
            display_changed_callback: CallbackSlot::default(),
            window,
            view_controller,
            view,
            text_input_view,
            bounds: Cell::new(Bounds::new(
                Point::default(),
                size(
                    px(screen_bounds_cg.size.width as f32),
                    px(screen_bounds_cg.size.height as f32),
                ),
            )),
            scale_factor: Cell::new(scale as f32),
            input_handler: PlatformInputHandlerSlot::default(),
            request_frame_callback: CallbackSlot::default(),
            input_callback: PlatformInputCallbackSlot::default(),
            active_status_callback: CallbackSlot::default(),
            visibility: Cell::new(WindowVisibility::Hidden),
            visibility_callback: CallbackSlot::default(),
            hover_status_callback: CallbackSlot::default(),
            resize_callback: CallbackSlot::default(),
            moved_callback: CallbackSlot::default(),
            should_close_callback: CallbackSlot::default(),
            hit_test_callback: CallbackSlot::default(),
            close_callback: CallbackSlot::default(),
            appearance_changed_callback: CallbackSlot::default(),
            insets_changed_callback: CallbackSlot::default(),
            keyboard_dismiss_touch: Cell::new(None),
            back_gesture_start: Cell::new(None),
            keyboard_height: Cell::new(0.),
            mouse_position: Cell::new(Point::default()),
            modifiers: Cell::new(Modifiers::default()),
            renderer: Mutex::new(renderer),
        };

        Ok(Self {
            state: Rc::new(state),
        })
    }

    pub(crate) fn register(&self) {
        if let Some(platform) = self.platform.upgrade() {
            platform.application.register_window(&self.state);
        }

        let window = Rc::downgrade(&self.state);
        *self.view_controller.ivars().0.borrow_mut() = window.clone();
        *self.view.ivars().0.borrow_mut() = window.clone();
        self.text_input_view.set_window(window);

        self.state.register_native_observers();
        self.state.handle_layout_change();
    }
}

impl IosWindowState {
    pub(super) fn handle(&self) -> AnyWindowHandle {
        self.handle
    }

    pub(super) fn is_active(&self) -> bool {
        !self.closed.get()
            && !self.interaction_quiesced.get()
            && self.window.isKeyWindow()
            && self.window.windowScene().is_some_and(|scene| {
                scene.activationState() == UISceneActivationState::ForegroundActive
            })
    }

    pub(super) fn attach_to_scene(self: &Rc<Self>, scene: &UIWindowScene) {
        if self.closed.get() || self.presentation_shutdown.borrow().is_some() {
            return;
        }
        self.window.setWindowScene(Some(scene));
        if self.initial_presentation_completed.get() && self.creation_facts.show {
            if self.creation_facts.focus_on_appearing {
                self.window.makeKeyAndVisible();
            } else {
                self.window.setHidden(false);
            }
        }
        self.handle_layout_change();
        self.notify_active_status_change(true);
    }

    fn register_native_observers(self: &Rc<Self>) {
        unsafe {
            let notification_center = NSNotificationCenter::defaultCenter();

            let window = Rc::downgrade(self);
            let frame_change_block =
                block2::RcBlock::new(move |notification: NonNull<NSNotification>| {
                    let Some(user_info) = notification.as_ref().userInfo() else {
                        return;
                    };
                    let Some(frame_value) = user_info.objectForKey(UIKeyboardFrameEndUserInfoKey)
                    else {
                        return;
                    };
                    let Some(frame_value) = frame_value.downcast_ref::<NSValue>() else {
                        return;
                    };
                    let frame = frame_value.CGRectValue();
                    if let Some(window) = window.upgrade() {
                        window.set_keyboard_height(frame.size.height as f32);
                    }
                });

            let window = Rc::downgrade(self);
            let hide_block = block2::RcBlock::new(move |_notification: NonNull<NSNotification>| {
                if let Some(window) = window.upgrade() {
                    window.set_keyboard_height(0.);
                }
            });

            let frame_observer = notification_center.addObserverForName_object_queue_usingBlock(
                Some(UIKeyboardWillChangeFrameNotification),
                None,
                None,
                &frame_change_block,
            );
            let hide_observer = notification_center.addObserverForName_object_queue_usingBlock(
                Some(UIKeyboardWillHideNotification),
                None,
                None,
                &hide_block,
            );
            self.native_observers
                .borrow_mut()
                .extend([frame_observer, hide_observer]);

            let window = Rc::downgrade(self);
            let active_block = block2::RcBlock::new(move |_: NonNull<NSNotification>| {
                if let Some(window) = window.upgrade() {
                    window.notify_active_status_change(true);
                }
            });
            let window = Rc::downgrade(self);
            let visibility_block = block2::RcBlock::new(move |_: NonNull<NSNotification>| {
                if let Some(window) = window.upgrade() {
                    window.refresh_native_visibility();
                }
            });
            for name in [
                UIWindowDidBecomeKeyNotification,
                UIWindowDidResignKeyNotification,
            ] {
                let observer = notification_center.addObserverForName_object_queue_usingBlock(
                    Some(name),
                    Some(&self.window),
                    None,
                    &active_block,
                );
                self.native_observers.borrow_mut().push(observer);
            }
            for name in [
                UIWindowDidBecomeVisibleNotification,
                UIWindowDidBecomeHiddenNotification,
            ] {
                let observer = notification_center.addObserverForName_object_queue_usingBlock(
                    Some(name),
                    Some(&self.window),
                    None,
                    &visibility_block,
                );
                self.native_observers.borrow_mut().push(observer);
            }
        }
    }

    /// Delivers a UIKit touch through GPUI's platform-neutral touch API.
    pub fn handle_touch(&self, touch: &UITouch, _event: Option<&UIEvent>) {
        // GPUI timestamps delivery itself. Replaying UIKit's historical coalesced
        // samples here would make them appear simultaneous and distort fling velocity.
        let id = touch_id(touch);
        self.handle_touch_sample(touch, id);
    }

    fn handle_touch_sample(&self, touch: &UITouch, id: TouchId) {
        if self.interaction_quiesced.get() || self.closed.get() {
            return;
        }
        let position = touch_location_in_view(touch, &self.view);
        self.mouse_position.set(position);

        let event = TouchEvent {
            id,
            phase: touch_phase(touch),
            position,
            predicted_position: None,
            force: touch_force(touch),
        };
        self.handle_keyboard_dismiss_touch(&event);
        if event.phase == TouchPhase::Cancelled {
            match self
                .input_callback
                .reserve_reentrant_touch_cancel(event.clone())
            {
                NativePointerCancelReservation::Reserved
                | NativePointerCancelReservation::ApplicationGone
                | NativePointerCancelReservation::IngressClosed
                | NativePointerCancelReservation::RetiredSlot => return,
                _ => {}
            }
        }
        self.input_callback.dispatch(PlatformInput::Touch(event));
    }

    /// Delivers one step of a trackpad or mouse-wheel scroll gesture.
    fn handle_scroll(&self, recognizer: &UIPanGestureRecognizer) {
        if self.interaction_quiesced.get() || self.closed.get() {
            return;
        }
        let Some(event) = scroll_event(recognizer, &self.view) else {
            return;
        };
        self.mouse_position.set(event.position);
        self.modifiers.set(event.modifiers);
        self.input_callback.dispatch(PlatformInput::Scroll(event));
    }

    /// Offers a starting back gesture to GPUI, returning whether an element
    /// claimed it.
    fn begin_back_gesture(&self, recognizer: &UIGestureRecognizer) -> bool {
        if self.interaction_quiesced.get() || self.closed.get() {
            return false;
        }
        let location = recognizer.locationInView(Some(&self.view));
        let start_position = Point::new(px(location.x as f32), px(location.y as f32));
        let claimed = self
            .input_callback
            .dispatch(PlatformInput::BackGesture(BackGestureEvent {
                phase: BackGesturePhase::Started,
                start_position,
                progress: 0.,
            }))
            .default_prevented;
        self.back_gesture_start
            .set(claimed.then_some(start_position));
        claimed
    }

    /// Delivers a step of a claimed back gesture.
    fn handle_back_gesture(&self, recognizer: &UIScreenEdgePanGestureRecognizer) {
        if self.interaction_quiesced.get() || self.closed.get() {
            return;
        }
        let Some(start_position) = self.back_gesture_start.get() else {
            return;
        };
        let Some(event) = back_event(recognizer, &self.view, start_position) else {
            return;
        };
        if matches!(
            event.phase,
            BackGesturePhase::Committed | BackGesturePhase::Cancelled
        ) {
            self.back_gesture_start.set(None);
        }
        self.input_callback
            .dispatch(PlatformInput::BackGesture(event));
    }

    fn handle_keyboard_dismiss_touch(&self, event: &TouchEvent) {
        if self.keyboard_height.get() <= 0. {
            self.keyboard_dismiss_touch.set(None);
            return;
        }

        match event.phase {
            TouchPhase::Started if self.keyboard_dismiss_touch.get().is_none() => {
                self.keyboard_dismiss_touch.set(Some(KeyboardDismissTouch {
                    id: event.id,
                    start_position: event.position,
                }));
            }
            TouchPhase::Moved => {
                let Some(touch) = self
                    .keyboard_dismiss_touch
                    .get()
                    .filter(|touch| touch.id == event.id)
                else {
                    return;
                };
                let delta = event.position - touch.start_position;
                if delta.y >= KEYBOARD_DISMISS_DISTANCE && delta.y.abs() > delta.x.abs() * 1.25 {
                    self.keyboard_dismiss_touch.set(None);
                    self.hide_keyboard();
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                if self
                    .keyboard_dismiss_touch
                    .get()
                    .is_some_and(|touch| touch.id == event.id)
                {
                    self.keyboard_dismiss_touch.set(None);
                }
            }
            TouchPhase::Started => {}
        }
    }

    pub(super) fn request_frame(&self) {
        let Some(options) = self.frame_demand.take(self.surface_is_ready()) else {
            return;
        };
        self.request_frame_callback.with(|callback| {
            callback(options);
        });
    }

    pub(super) fn has_pending_frame(&self) -> bool {
        self.frame_demand.is_ready(self.surface_is_ready())
    }

    fn queue_frame(self: &Rc<Self>, mut options: RequestFrameOptions) {
        if self.closed.get() || self.presentation_shutdown.borrow().is_some() {
            return;
        }
        if options.signal_at.is_none() {
            options.signal_at = PlatformFrameSignal::capture(Instant::now);
            options.signal_source = FrameRequestSource::LocalSchedule;
        }
        self.frame_demand.queue(options);
        self.wake_frame_source();
    }

    fn surface_is_ready(&self) -> bool {
        let size = self.drawable_size.get();
        !self.closed.get()
            && self.presentation_shutdown.borrow().is_none()
            && self.visibility.get() == WindowVisibility::Visible
            && !self.window.isHidden()
            && size.width.0 > 0
            && size.height.0 > 0
    }

    fn wake_frame_source(self: &Rc<Self>) {
        if !self.has_pending_frame() {
            return;
        }
        let Some(platform) = self.platform.upgrade() else {
            return;
        };
        if platform.application.wake_frame_source() || self.local_frame_scheduled.replace(true) {
            return;
        }
        let window = Rc::downgrade(self);
        platform
            .foreground_executor
            .spawn(async move {
                if let Some(window) = window.upgrade() {
                    window.local_frame_scheduled.set(false);
                    window.request_frame();
                }
            })
            .detach();
    }

    /// Query the safe area insets from the UIView.
    ///
    /// Returns `(top, bottom, left, right)` in logical points.
    /// These represent the areas occupied by system UI (status bar,
    /// home indicator, camera notch) that content should avoid.
    fn safe_area_insets(&self) -> (f32, f32, f32, f32) {
        let insets = self.view.safeAreaInsets();
        (
            insets.top as f32,
            insets.bottom as f32,
            insets.left as f32,
            insets.right as f32,
        )
    }

    fn current_insets(&self) -> WindowInsets {
        let (top, bottom, left, right) = self.safe_area_insets();
        WindowInsets {
            safe_area: Edges {
                top: px(top),
                right: px(right),
                bottom: px(bottom),
                left: px(left),
            },
            ime: Edges {
                bottom: px(self.keyboard_height.get()),
                ..Default::default()
            },
            corner_radii: self.display_corner_radii(),
        }
    }

    fn display_corner_radii(&self) -> Corners<Pixels> {
        if !reports_corner_radii(&self.view) {
            return Corners::default();
        }
        let radius = |corner| px(self.view.effectiveRadiusForCorner(corner) as f32);
        Corners {
            top_left: radius(UIRectCorner::TopLeft),
            top_right: radius(UIRectCorner::TopRight),
            bottom_right: radius(UIRectCorner::BottomRight),
            bottom_left: radius(UIRectCorner::BottomLeft),
        }
    }

    fn notify_insets_changed(&self) {
        if self.closed.get() {
            return;
        }
        let insets = self.current_insets();
        if *self.insets.borrow() == insets {
            return;
        }
        *self.insets.borrow_mut() = insets.clone();
        self.insets_changed_callback
            .with(|callback| callback(insets));
    }

    fn notify_appearance_changed(&self) {
        if self.closed.get() {
            return;
        }
        self.appearance_changed_callback.with(|callback| callback());
    }

    fn set_keyboard_height(&self, height: f32) {
        let height = height.max(0.);
        if (self.keyboard_height.get() - height).abs() <= 0.5 {
            return;
        }
        self.keyboard_height.set(height);
        if height <= 0. {
            self.keyboard_dismiss_touch.set(None);
        }
        self.notify_insets_changed();
    }

    pub fn show_keyboard(&self) {
        if !self.closed.get() && !self.interaction_quiesced.get() {
            self.text_input_view.set_keyboard_visible(true);
        }
    }

    pub fn hide_keyboard(&self) {
        self.text_input_view.set_keyboard_visible(false);
    }

    pub(super) fn with_input_handler<R>(
        &self,
        callback: impl FnOnce(&mut PlatformInputHandler) -> R,
    ) -> Option<R> {
        if self.interaction_quiesced.get() || self.closed.get() {
            return None;
        }
        self.input_handler
            .with_handler(|handler| {
                (!self.interaction_quiesced.get() && !self.closed.get()).then(|| callback(handler))
            })
            .flatten()
    }

    /// Notify the window when its UIKit scene becomes active or inactive.
    pub fn notify_active_status_change(&self, is_active: bool) {
        if self.closed.get() {
            return;
        }
        let is_active = is_active && self.is_active();
        log::info!("GPUI iOS: Window active status changed to: {}", is_active);

        self.active_status_callback.with(|callback| {
            callback(PlatformWindowActiveStatusObservation::new(
                is_active, is_active,
            ))
        });
    }

    fn refresh_native_visibility(self: &Rc<Self>) {
        let visibility = if self
            .platform
            .upgrade()
            .is_some_and(|platform| platform.application.is_foreground())
        {
            WindowVisibility::Visible
        } else {
            WindowVisibility::Hidden
        };
        self.notify_visibility_change(visibility);
    }

    pub(crate) fn notify_visibility_change(self: &Rc<Self>, visibility: WindowVisibility) {
        if self.closed.get() {
            return;
        }
        let visibility = if self.window.isHidden() {
            WindowVisibility::Hidden
        } else {
            visibility
        };
        if self.visibility.replace(visibility) != visibility {
            self.visibility_callback
                .with(|callback| callback(visibility));
        }
        self.wake_frame_source();
    }

    pub fn handle_layout_change(self: &Rc<Self>) {
        if self.closed.get() || self.presentation_shutdown.borrow().is_some() {
            return;
        }
        let view_bounds = self.view.bounds();
        let display = IosDisplay::from_screen(self.window.screen());
        let display_changed = self.display_id.replace(display.id()) != display.id();
        let scale = display.scale() as f64;

        let width = view_bounds.size.width as f32;
        let height = view_bounds.size.height as f32;
        let new_scale = scale as f32;

        let old_bounds = self.bounds.get();
        let old_scale = self.scale_factor.get();

        let new_size = size(px(width), px(height));
        let resized = old_bounds.size != new_size || old_scale != new_scale;

        log::info!(
            "GPUI iOS: Layout changed — {:?} @{:.1}x → {:?} @{:.1}x",
            old_bounds.size,
            old_scale,
            new_size,
            new_scale,
        );

        // Update stored bounds (in logical pixels, matching GPUI convention).
        let new_bounds = Bounds {
            origin: Default::default(),
            size: new_size,
        };
        self.bounds.set(new_bounds);
        self.scale_factor.set(new_scale);
        // Update the Metal layer's contentsScale so the drawable has the
        // correct pixel dimensions.
        self.view.layer().setContentsScale(scale);

        let pixel_width = (width * new_scale) as i32;
        let pixel_height = (height * new_scale) as i32;
        self.drawable_size
            .set(size(DevicePixels(pixel_width), DevicePixels(pixel_height)));
        if display_changed {
            self.display_changed_callback.with(|callback| callback());
        }
        if resized {
            self.resize_callback
                .with(|callback| callback(new_size, new_scale));
        }
        self.notify_insets_changed();
        self.wake_frame_source();
    }
}

/// Whether UIKit can resolve corner configurations, which arrived in iOS 26.
fn reports_corner_radii(view: &UIView) -> bool {
    view.respondsToSelector(sel!(effectiveRadiusForCorner:))
}

impl Drop for IosWindow {
    fn drop(&mut self) {
        if self.closed.get() {
            return;
        }
        let ticket = self
            .presentation_shutdown
            .borrow()
            .clone()
            .unwrap_or_else(|| {
                WindowPresentationShutdownTicket::new(self.handle.window_id(), u64::MAX)
            });
        *self.presentation_shutdown.borrow_mut() = Some(ticket.clone());
        self.state.quiesce_interaction();
        self.frame_demand.clear();
        let state = self.state.clone();
        let background = self.background_executor.clone();
        self.foreground_executor
            .spawn(async move {
                loop {
                    if state.quiesce_presentation(&ticket)
                        == PlatformPresentationShutdownOutcome::Quiesced
                        && state.retire_native(&ticket)
                            == PlatformNativeWindowRetirementOutcome::Accepted
                    {
                        break;
                    }
                    background.timer(Duration::from_millis(16)).await;
                }
            })
            .detach();
    }
}

impl HasWindowHandle for IosWindow {
    fn window_handle(
        &self,
    ) -> std::result::Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError>
    {
        let view = NonNull::new(Retained::as_ptr(&self.view).cast_mut().cast::<c_void>())
            .ok_or(raw_window_handle::HandleError::Unavailable)?;
        let handle = UiKitWindowHandle::new(view);
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(handle.into()) })
    }
}

impl HasDisplayHandle for IosWindow {
    fn display_handle(
        &self,
    ) -> std::result::Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError>
    {
        let handle = UiKitDisplayHandle::new();
        Ok(unsafe { raw_window_handle::DisplayHandle::borrow_raw(handle.into()) })
    }
}

impl PlatformWindow for IosWindow {
    fn command_dispatcher(&self) -> PlatformWindowCommandDispatcher {
        let state = Rc::downgrade(&self.state);
        PlatformWindowCommandDispatcher::new(move |command| {
            let Some(state) = state.upgrade() else {
                return PlatformWindowCommandOutcome::WindowClosed;
            };
            state.dispatch_command(command)
        })
    }

    fn interaction_quiescence(&self) -> PlatformWindowInteractionQuiescence {
        let state = Rc::downgrade(&self.state);
        PlatformWindowInteractionQuiescence::new(move || {
            if let Some(state) = state.upgrade() {
                state.quiesce_interaction();
            }
        })
    }

    fn prepare_presentation_shutdown(
        &self,
        shutdown: WindowPresentationShutdownTicket,
    ) -> PreparedPlatformPresentationShutdown {
        if shutdown.snapshot().window_id() == self.handle.window_id() {
            let mut current = self.presentation_shutdown.borrow_mut();
            if current.is_none() {
                *current = Some(shutdown.clone());
            }
        }
        let state = self.state.clone();
        PreparedPlatformPresentationShutdown::new(shutdown, move |ticket| {
            state.quiesce_presentation(ticket)
        })
    }

    fn retire_native_window(
        &self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> PlatformNativeWindowRetirementOutcome {
        self.state.retire_native(shutdown)
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.bounds.get()
    }

    fn is_maximized(&self) -> bool {
        true // iOS windows are always "maximized"
    }

    fn window_bounds(&self) -> WindowBounds {
        WindowBounds::Fullscreen(self.bounds.get())
    }

    fn content_size(&self) -> Size<Pixels> {
        self.bounds.get().size
    }

    fn prepare_frame(&self) -> bool {
        if self.closed.get() || self.presentation_shutdown.borrow().is_some() {
            return false;
        }
        let insets = self.current_insets();
        let insets_changed = *self.insets.borrow() != insets;
        *self.insets.borrow_mut() = insets;
        let size = self.drawable_size.get();
        let resized = self.renderer_size.replace(size) != size;
        if resized {
            self.renderer.lock().update_drawable_size(size);
        }
        insets_changed || resized
    }

    fn scale_factor(&self) -> f32 {
        self.scale_factor.get()
    }

    fn appearance(&self) -> WindowAppearance {
        match unsafe { self.view.traitCollection().userInterfaceStyle() } {
            UIUserInterfaceStyle::Dark => WindowAppearance::Dark,
            _ => WindowAppearance::Light,
        }
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(Rc::new(IosDisplay::from_screen(self.window.screen())))
    }

    fn on_display_changed(&self, callback: Box<dyn FnMut()>) {
        self.display_changed_callback.set(callback);
    }

    fn mouse_position(&self) -> Point<Pixels> {
        self.mouse_position.get()
    }

    fn modifiers(&self) -> Modifiers {
        self.modifiers.get()
    }

    fn set_cursor_style(&self, _style: open_gpui::CursorStyle) {}

    fn capslock(&self) -> Capslock {
        // Would need to check UIKeyModifierFlags
        Capslock { on: false }
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        if self.interaction_quiesced.get() {
            return;
        }
        self.text_input_view
            .set_input_field(Some(input_handler.focus_id()));
        self.input_handler.set(input_handler);
        self.text_input_view.refresh_keyboard();
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        let handler = self.input_handler.take();
        self.text_input_view.refresh_keyboard();
        handler
    }

    fn take_input_handler_for_frame(&mut self) -> Option<PlatformInputHandler> {
        self.input_handler.take_for_frame()
    }

    #[cfg(any(test, feature = "test-support"))]
    fn input_handler_slot_for_test(&self) -> Option<PlatformInputHandlerSlot> {
        Some(self.input_handler.clone())
    }

    fn prompt(
        &self,
        _level: PromptLevel,
        _msg: &str,
        _detail: Option<&str>,
        _answers: &[PromptButton],
    ) -> Option<futures::channel::oneshot::Receiver<usize>> {
        None
    }

    fn creation_facts(&self) -> WindowCreationFacts {
        self.creation_facts.clone()
    }

    fn is_visible(&self) -> bool {
        !self.closed.get() && !self.window.isHidden()
    }

    fn accepts_pointer_input(&self) -> bool {
        !self.interaction_quiesced.get() && self.view.isUserInteractionEnabled()
    }

    fn is_active(&self) -> bool {
        self.state.is_active()
    }

    fn is_hovered(&self) -> bool {
        // Hover isn't really applicable on iOS
        false
    }

    fn set_title(&mut self, _title: &str) {
        // iOS apps don't have window titles
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        WindowBackgroundAppearance::Opaque
    }

    fn set_background_appearance(&self, _background_appearance: WindowBackgroundAppearance) {
        // Could adjust view background color
    }

    fn is_fullscreen(&self) -> bool {
        true
    }

    fn request_frame(&self, options: RequestFrameOptions) {
        self.state.queue_frame(options);
    }

    fn schedule_frame(&self) {
        self.state.queue_frame(RequestFrameOptions::default());
    }

    fn frame_waker(&self) -> Option<Rc<dyn Fn()>> {
        let state = Rc::downgrade(&self.state);
        Some(Rc::new(move || {
            if let Some(state) = state.upgrade() {
                state.queue_frame(RequestFrameOptions::default());
            }
        }))
    }

    fn on_request_frame(&self, callback: Box<dyn Fn(RequestFrameOptions)>) {
        self.request_frame_callback.set(callback);
        self.state.queue_frame(RequestFrameOptions::default());
    }

    fn on_input(&self, callback: PlatformInputCallback) {
        self.input_callback.set(callback);
    }

    fn on_active_status_change(
        &self,
        callback: Box<dyn Fn(PlatformWindowActiveStatusObservation)>,
    ) {
        self.active_status_callback.set(callback);
    }

    fn visibility(&self) -> WindowVisibility {
        self.visibility.get()
    }

    fn on_visibility_change(&self, callback: Box<dyn FnMut(WindowVisibility)>) {
        self.visibility_callback.set(callback);
    }

    fn on_hover_status_change(&self, callback: Box<dyn Fn(bool)>) {
        self.hover_status_callback.set(callback);
    }

    fn on_resize(&self, callback: Box<dyn Fn(Size<Pixels>, f32)>) {
        self.resize_callback.set(callback);
    }

    fn on_moved(&self, callback: Box<dyn Fn()>) {
        self.moved_callback.set(callback);
    }

    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        self.should_close_callback.set(callback);
    }

    fn on_hit_test_window_control(&self, callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
        self.hit_test_callback.set(callback);
    }

    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        self.close_callback.set(callback);
    }

    fn on_appearance_changed(&self, callback: Box<dyn Fn()>) {
        self.appearance_changed_callback.set(callback);
    }

    fn draw(&self, scene: &Scene) -> PlatformWindowPresentOutcome {
        if self.closed.get() || self.presentation_shutdown.borrow().is_some() {
            return PlatformWindowPresentOutcome::Rejected;
        }
        if !self.surface_is_ready() {
            return PlatformWindowPresentOutcome::Deferred;
        }
        let mut renderer = self.renderer.lock();
        let Some(layer) = renderer.layer() else {
            return PlatformWindowPresentOutcome::Rejected;
        };
        let Some(drawable) = layer.nextDrawable() else {
            return PlatformWindowPresentOutcome::Deferred;
        };
        let texture = drawable.texture();
        let extent = CGSize::new(texture.width() as f64, texture.height() as f64);
        drop(renderer.set_frame_drawable(Some(drawable)));
        let outcome = renderer.draw(scene);
        drop(renderer.set_frame_drawable(None));
        if outcome == PlatformWindowPresentOutcome::Submitted {
            self.submitted_drawable_size.set(extent);
            self.submission_serial.set(
                self.submission_serial
                    .get()
                    .checked_add(1)
                    .expect("iOS submission serial overflowed"),
            );
        }
        outcome
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.renderer.lock().sprite_atlas().clone()
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        false
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        None
    }

    fn update_ime_position(&self, _bounds: Bounds<Pixels>) {
        // iOS handles IME positioning automatically
    }

    fn insets(&self) -> WindowInsets {
        self.insets.borrow().clone()
    }

    fn on_insets_changed(&self, callback: Box<dyn FnMut(WindowInsets)>) {
        self.insets_changed_callback.set(callback);
    }

    fn show_soft_keyboard(&self) {
        self.show_keyboard();
    }

    fn hide_soft_keyboard(&self) {
        self.hide_keyboard();
    }

    fn text_input_state_changed(&self, change: TextInputStateChange) {
        self.text_input_view.state_changed(change);
    }

    fn window_controls(&self) -> open_gpui::WindowControls {
        open_gpui::WindowControls {
            fullscreen: false,
            maximize: false,
            minimize: false,
            window_menu: false,
        }
    }

    #[cfg(any(test, feature = "test-support", feature = "bench-support"))]
    fn render_to_image(&self, scene: &Scene) -> anyhow::Result<image::RgbaImage> {
        self.renderer.lock().render_to_image(scene)
    }

    fn set_text_input_configuration(&mut self, configuration: TextInputConfiguration) {
        self.text_input_view.set_configuration(&configuration);
    }
}

#[cfg(test)]
mod tests {
    use super::CallbackSlot;

    #[test]
    fn callback_is_unavailable_during_dispatch_and_restored_afterward() {
        let slot = CallbackSlot::default();
        slot.set(1);
        assert_eq!(
            slot.with(|value| {
                *value += 1;
                assert!(
                    slot.with(|_| panic!("reentered the same handler"))
                        .is_none()
                );
                *value
            }),
            Some(2)
        );
        assert_eq!(slot.with(|value| *value), Some(2));
    }

    #[test]
    fn callback_replacement_during_dispatch_is_preserved() {
        let slot = CallbackSlot::default();
        slot.set(1);
        slot.with(|_| {
            slot.set(2);
            slot.with(|value| *value += 1);
        });
        assert_eq!(slot.take(), Some(3));
    }

    #[test]
    fn withdrawing_an_in_flight_handler_does_not_restore_it() {
        let slot = CallbackSlot::default();
        slot.set(1);
        slot.with(|_| assert!(slot.take().is_none()));
        assert!(
            slot.with(|_| panic!("restored a withdrawn handler"))
                .is_none()
        );
    }

    #[test]
    fn removing_a_replacement_does_not_resurrect_the_previous_handler() {
        let slot = CallbackSlot::default();
        slot.set(1);
        slot.with(|_| {
            slot.set(2);
            assert_eq!(slot.take(), Some(2));
        });
        assert!(slot.take().is_none());
    }
}
