//! Native process-main-thread gate, run on macOS with a GUI session and Metal device:
//! Build with `cargo build -p open-gpui-macos --features test-support,runtime_shaders --examples`.
//! Launch with `python3 scripts/run_macos_native_probe.py target/debug/examples/native_windowing -- <mode>`.
//! The app bundle and LaunchServices identity are required for activation on macOS 27.
//!
//! Run each mode in a fresh process: `default`, `windowed`, `headless`, `accessory`, `direct`,
//! `activity`, `sheet`, `native-close`, `native-close-panic`.
//! The activity mode exercises the real headless platform acquisition/drop path.
//! The first four enter the real GPUIApplication run loop and launch delegate. `direct` only
//! checks stored policy and native resource refusal without launching the application. Neither
//! path creates a Core App or qualifies Core retirement, external event pumping, or Dock pixels.
//! `headless` selects Accessory on a switchable platform. Fixed-headless refusal is checked without
//! entering its separate CFRunLoop. This probe stops AppKit directly and does not qualify quit.
//! The separate `sheet` mode creates a Core App and checks explicit-owner modal sheets, nested
//! presentation, retirement, hidden creation, submitted scenes, titlebar visibility and debug JSON.
//! Its input controls use AppKit event dispatch; physical input and fullscreen pixels remain manual.
//! `native-close` checks exact renderer-shutdown receipts and repeated/reentrant native closes.
//! `native-close-panic` also injects typed query, checkout-drop, frame/resize retirement,
//! and terminal callback panics.

#[cfg(target_os = "macos")]
fn main() {
    let inject_close_panics = std::env::args().nth(1).as_deref() == Some("native-close-panic");
    let panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if inject_close_panics && macos::is_expected_close_panic(info.payload()) {
            return;
        }
        panic_hook(info);
        std::process::exit(1);
    }));
    macos::run();
}

#[cfg(not(target_os = "macos"))]
fn main() -> std::process::ExitCode {
    eprintln!("native_windowing is unsupported on this platform; no native evidence");
    std::process::ExitCode::FAILURE
}

#[cfg(target_os = "macos")]
mod macos {
    use block2::RcBlock;
    use futures::{
        FutureExt as _,
        channel::oneshot,
        future::{Either, select},
    };
    use objc2::{MainThreadMarker, MainThreadOnly as _, rc::Retained, runtime::AnyObject};
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSWindow, NSWindowButton};
    use objc2_foundation::{
        NSDate, NSNotification, NSNotificationCenter, NSPoint, NSRunLoop, NSString,
    };
    use open_gpui::{
        ActivationPolicy, App, Application, AsyncApp, Bounds, Context, Empty, GraphicalEnvironment,
        IntoElement, MouseButton, Platform, PlatformWindow, Render, Result, Task, Window,
        WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowId, WindowKind,
        WindowOptions, WindowParams, WindowPresentationShutdownTicket, WindowingRequest, div,
        point, prelude::*, px, rgb, size,
    };
    use open_gpui_macos::MacPlatform;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::{
        cell::Cell,
        pin::pin,
        ptr::NonNull,
        rc::Rc,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        time::{Duration, Instant},
    };

    type LaunchObservations = Arc<Mutex<Vec<(&'static str, NSApplicationActivationPolicy, bool)>>>;

    struct NativeNotifications {
        center: Retained<NSNotificationCenter>,
        observers: Vec<Retained<AnyObject>>,
        launches: LaunchObservations,
        activations: Arc<AtomicUsize>,
    }

    impl NativeNotifications {
        fn new(app: &AnyObject) -> Self {
            let mut notifications = Self {
                center: NSNotificationCenter::defaultCenter(),
                observers: Vec::new(),
                launches: Arc::default(),
                activations: Arc::default(),
            };
            for name in [
                "NSApplicationWillFinishLaunchingNotification",
                "NSApplicationDidFinishLaunchingNotification",
                "NSApplicationDidBecomeActiveNotification",
            ] {
                let launches = notifications.launches.clone();
                let activations = notifications.activations.clone();
                let block = RcBlock::new(move |notification: NonNull<NSNotification>| {
                    let notification = unsafe { notification.as_ref() };
                    let app = notification.object().expect("an application notification");
                    if name == "NSApplicationDidBecomeActiveNotification" {
                        activations.fetch_add(1, Ordering::Relaxed);
                    } else {
                        launches.lock().unwrap().push((
                            name,
                            native_policy(&app),
                            MainThreadMarker::new().is_some(),
                        ));
                    }
                });
                let name = NSString::from_str(name);
                let observer = unsafe {
                    objc2::msg_send![&*notifications.center,
                        addObserverForName: &*name,
                        object: app,
                        queue: None::<&AnyObject>,
                        usingBlock: &*block
                    ]
                };
                notifications.observers.push(observer);
            }
            notifications
        }
    }

    impl Drop for NativeNotifications {
        fn drop(&mut self) {
            for observer in self.observers.drain(..) {
                unsafe { self.center.removeObserver(&observer) };
            }
        }
    }

    struct ObserveCallbackDrop(Rc<Cell<bool>>);

    impl Drop for ObserveCallbackDrop {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum NativeClosePanic {
        QueryCallback,
        QueryCheckoutDrop,
        QueryRetirementDrop,
        FrameCallbackDrop,
        ResizeCallbackDrop,
        TerminalCallback,
        TerminalCallbackDrop,
    }

    pub(super) fn is_expected_close_panic(payload: &(dyn std::any::Any + Send)) -> bool {
        payload.is::<NativeClosePanic>()
    }

    struct InjectClosePanicOnDrop {
        panic: NativeClosePanic,
        drops: Rc<Cell<usize>>,
    }

    impl Drop for InjectClosePanicOnDrop {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
            std::panic::panic_any(self.panic);
        }
    }

    fn native_policy(app: &AnyObject) -> NSApplicationActivationPolicy {
        unsafe { objc2::msg_send![app, activationPolicy] }
    }

    fn is_active(app: &AnyObject) -> bool {
        unsafe { objc2::msg_send![app, isActive] }
    }

    fn settle(task: Task<Result<()>>) -> Result<()> {
        task.now_or_never()
            .expect("Mac windowing applies synchronously")
    }

    fn windowed() -> WindowingRequest {
        WindowingRequest::Windowed(GraphicalEnvironment::detect())
    }

    async fn wait_for_native(
        platform: &MacPlatform,
        mut condition: impl FnMut() -> bool,
        reason: &str,
    ) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(MainThreadMarker::new().is_some());
            if objc2::rc::autoreleasepool(|_| condition()) {
                return;
            }
            assert!(Instant::now() < deadline, "{reason}");
            platform
                .background_executor()
                .timer(Duration::from_millis(10))
                .await;
        }
    }

    async fn drain_main_queue(platform: &MacPlatform) {
        assert!(MainThreadMarker::new().is_some());
        let (sender, receiver) = oneshot::channel();
        dispatch2::DispatchQueue::main().exec_async(move || {
            assert!(MainThreadMarker::new().is_some());
            let _ = sender.send(());
        });
        let receiver = pin!(receiver);
        let timeout = pin!(platform.background_executor().timer(Duration::from_secs(5)));
        match select(receiver, timeout).await {
            Either::Left((completed, _)) => completed.expect("the main-queue marker must run"),
            Either::Right(_) => panic!("main-queue cleanup must complete"),
        }
        assert!(MainThreadMarker::new().is_some());
    }

    fn assert_mode(platform: &MacPlatform, app: &AnyObject, regular: bool, launched: bool) {
        assert_eq!(platform.graphical_environment().is_some(), regular);
        if launched {
            assert_eq!(
                native_policy(app),
                if regular {
                    NSApplicationActivationPolicy::Regular
                } else {
                    NSApplicationActivationPolicy::Accessory
                }
            );
        }
    }

    fn open_native_window(platform: &MacPlatform) -> Rc<dyn PlatformWindow> {
        let bounds = Bounds::new(point(px(40.0), px(40.0)), size(px(256.0), px(160.0)));
        Rc::from(
            platform
                .open_window(
                    WindowHandle::<Empty>::new(WindowId::from(1)).into(),
                    WindowParams {
                        window_bounds: WindowBounds::Windowed(bounds),
                        bounds,
                        titlebar: Some(Default::default()),
                        kind: WindowKind::Normal,
                        is_movable: true,
                        app_owns_titlebar_drag: false,
                        is_resizable: true,
                        is_minimizable: true,
                        accepts_pointer_input: true,
                        focus_on_appearing: false,
                        activation_policy: Default::default(),
                        transient_for: None,
                        show: false,
                        provisional_session: None,
                        icon: None,
                        display_id: None,
                        window_min_size: None,
                        tabbing_identifier: None,
                        use_display_p3: false,
                    },
                )
                .expect("a native window requires a macOS GUI session and Metal device"),
        )
    }

    async fn exercise_window(platform: &Rc<MacPlatform>, app: &AnyObject, launched: bool) {
        assert!(MainThreadMarker::new().is_some());
        let (window, retained_view, native_window) = objc2::rc::autoreleasepool(|_| {
            let window = open_native_window(platform);
            let handle = window.window_handle().expect("the native window handle");
            let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
                panic!("macOS must expose an AppKit view");
            };
            let view = unsafe { Retained::retain(handle.ns_view.cast::<AnyObject>().as_ptr()) }
                .expect("the platform window owns its native view");
            let native_window: Retained<AnyObject> = unsafe { objc2::msg_send![&*view, window] };
            (window, view, native_window)
        });
        let typed_window = native_window
            .downcast_ref::<NSWindow>()
            .expect("the native view belongs to an NSWindow");
        assert!(
            typed_window
                .childWindows()
                .is_none_or(|children| children.is_empty())
        );
        let content = typed_window
            .contentView()
            .expect("normal AppKit content view");
        window.set_background_appearance(WindowBackgroundAppearance::Blurred);
        assert_eq!(
            window.background_appearance(),
            WindowBackgroundAppearance::Blurred
        );
        typed_window.setContentView(None);
        window.set_background_appearance(WindowBackgroundAppearance::Blurred);
        assert_eq!(
            window.background_appearance(),
            WindowBackgroundAppearance::Transparent,
        );
        typed_window.setContentView(Some(&content));
        window.set_background_appearance(WindowBackgroundAppearance::Blurred);
        assert_eq!(
            window.background_appearance(),
            WindowBackgroundAppearance::Blurred
        );
        window.set_background_appearance(WindowBackgroundAppearance::Opaque);
        drop(content);
        let callbacks = Rc::new(Cell::new(0));
        let callback_dropped = Rc::new(Cell::new(false));
        let activation_request_in_flight = Rc::new(Cell::new(false));
        window.on_active_status_change(Box::new({
            let platform = Rc::downgrade(platform);
            let window = Rc::downgrade(&window);
            let callbacks = callbacks.clone();
            let activation_request_in_flight = activation_request_in_flight.clone();
            let native_window = Retained::as_ptr(&native_window);
            let lifetime = ObserveCallbackDrop(callback_dropped.clone());
            move |observation| {
                let _lifetime = &lifetime;
                assert!(MainThreadMarker::new().is_some());
                assert!(
                    !activation_request_in_flight.get(),
                    "application activation callbacks must follow the native request"
                );
                let platform = platform.upgrade().expect("the running platform");
                let window = window.upgrade().expect("the live platform window");
                let _ = platform.background_executor();
                let _ = window.is_active();
                eprintln!("native activation callback: {observation:?}");
                if observation.exact_native_positive() {
                    let app = NSApplication::sharedApplication(MainThreadMarker::new().unwrap());
                    let key_window: *const AnyObject =
                        unsafe { objc2::msg_send![&*app, keyWindow] };
                    assert_eq!(key_window, native_window);
                    callbacks.set(callbacks.get() + 1);
                }
            }
        }));
        let refusal = settle(platform.request_windowing(WindowingRequest::Headless)).unwrap_err();
        assert!(refusal.to_string().contains("native window resources"));
        assert_mode(platform, app, true, launched);
        platform.set_activation_policy(ActivationPolicy::Accessory);
        assert!(settle(platform.request_windowing(windowed())).is_err());
        assert_mode(platform, app, false, launched);
        assert!(window.platform_facts().accepts_activation);
        platform.set_activation_policy(ActivationPolicy::Regular);
        if launched {
            activation_request_in_flight.set(true);
            unsafe {
                let _: () =
                    objc2::msg_send![&*native_window, makeKeyAndOrderFront: None::<&AnyObject>];
            }
            platform.activate(true);
            activation_request_in_flight.set(false);
            let mut last_activation = None;
            wait_for_native(
                platform,
                || {
                    let activation = (
                        is_active(app),
                        typed_window.isKeyWindow(),
                        window.is_active(),
                        callbacks.get(),
                    );
                    if last_activation != Some(activation) {
                        eprintln!(
                            "native activation (application, native key, observed key, callbacks): {activation:?}"
                        );
                        last_activation = Some(activation);
                    }
                    activation.0 && activation.2 && activation.3 > 0
                },
                "explicit activation must reach the exact window and its callback",
            )
            .await;
        }
        objc2::rc::autoreleasepool(|_| {
            drop(window);
            assert!(callback_dropped.get());
            assert!(settle(platform.request_windowing(WindowingRequest::Headless)).is_err());
        });
        drain_main_queue(platform).await;
        objc2::rc::autoreleasepool(|_| {
            assert!(settle(platform.request_windowing(WindowingRequest::Headless)).is_err());
            let before = callbacks.get();
            unsafe {
                let _: () =
                    objc2::msg_send![&*native_window, windowDidBecomeKey: None::<&AnyObject>];
            }
            assert_eq!(callbacks.get(), before);
            drop(native_window);
        });
        assert!(settle(platform.request_windowing(WindowingRequest::Headless)).is_err());
        drop(retained_view);
        wait_for_native(
            platform,
            || settle(platform.request_windowing(WindowingRequest::Headless)).is_ok(),
            "fully released native windows must permit a later windowing request",
        )
        .await;
        assert_mode(platform, app, false, launched);
        settle(platform.request_windowing(windowed())).unwrap();
    }

    async fn exercise_live(
        platform: &Rc<MacPlatform>,
        app: &AnyObject,
        launched: bool,
        regular: bool,
    ) {
        assert!(MainThreadMarker::new().is_some());
        let text_system = platform.text_system();
        let native_before = native_policy(app);
        assert_mode(platform, app, regular, launched);
        let duplicate = settle(platform.request_windowing(if regular {
            windowed()
        } else {
            WindowingRequest::Headless
        }))
        .unwrap_err();
        assert_eq!(
            duplicate.to_string(),
            if regular {
                "already windowed"
            } else {
                "already headless"
            }
        );
        let fixed = MacPlatform::new(true);
        fixed.set_initial_windowing(WindowingRequest::Headless);
        fixed.set_activation_policy(ActivationPolicy::Regular);
        assert!(fixed.graphical_environment().is_none());
        for request in [windowed(), WindowingRequest::Headless] {
            let refusal = settle(fixed.request_windowing(request)).unwrap_err();
            assert!(refusal.to_string().contains("a platform created headless"));
        }
        assert_eq!(native_policy(app), native_before);

        if launched {
            // AppKit may refuse activation without a visible window. The positive
            // activation control runs below with a real ordered native window.
            platform.hide();
            wait_for_native(
                platform,
                || !is_active(app),
                "the isolated activation control must become inactive",
            )
            .await;
        }
        let activation_notifications = NativeNotifications::new(app);
        for policy in [ActivationPolicy::Accessory, ActivationPolicy::Regular] {
            platform.set_activation_policy(policy);
            assert_mode(platform, app, policy == ActivationPolicy::Regular, launched);
        }
        let rejected_validator_dropped = Rc::new(Cell::new(false));
        let validated = platform.request_windowing_with_validation(
            WindowingRequest::Headless,
            Box::new({
                let platform = Rc::downgrade(platform);
                let lifetime = ObserveCallbackDrop(rejected_validator_dropped.clone());
                move || {
                    let _lifetime = &lifetime;
                    assert!(
                        platform
                            .upgrade()
                            .unwrap()
                            .graphical_environment()
                            .is_some()
                    );
                    Err(anyhow::anyhow!("native probe validation refusal"))
                }
            }),
        );
        assert!(rejected_validator_dropped.get());
        assert!(
            settle(validated)
                .unwrap_err()
                .to_string()
                .contains("native probe validation refusal")
        );
        assert_mode(platform, app, true, launched);
        for _ in 0..2 {
            let validator_calls = Rc::new(Cell::new(0));
            let request = platform.request_windowing_with_validation(
                WindowingRequest::Headless,
                Box::new({
                    let validator_calls = validator_calls.clone();
                    move || {
                        validator_calls.set(validator_calls.get() + 1);
                        Ok(())
                    }
                }),
            );
            assert_eq!(validator_calls.get(), 1);
            settle(request).unwrap();
            assert_mode(platform, app, false, launched);
            assert!(settle(platform.request_windowing(WindowingRequest::Headless)).is_err());
            settle(platform.request_windowing(windowed())).unwrap();
            assert_mode(platform, app, true, launched);
            assert!(settle(platform.request_windowing(windowed())).is_err());
        }
        drop(platform.request_windowing(WindowingRequest::Headless));
        assert_mode(platform, app, false, launched);
        settle(platform.request_windowing(windowed())).unwrap();
        drain_main_queue(platform).await;
        if launched {
            assert!(!is_active(app));
            assert_eq!(
                activation_notifications.activations.load(Ordering::Relaxed),
                0
            );
        } else {
            assert_eq!(native_policy(app), native_before);
        }
        exercise_window(platform, app, launched).await;
        if launched {
            assert!(activation_notifications.activations.load(Ordering::Relaxed) > 0);
        }
        assert!(Arc::ptr_eq(&text_system, &platform.text_system()));
    }

    fn run_direct(platform: &Rc<MacPlatform>, app: &AnyObject, regular: bool) {
        let mut probe = pin!(exercise_live(platform, app, false, regular));
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            assert!(MainThreadMarker::new().is_some());
            if objc2::rc::autoreleasepool(|_| probe.as_mut().now_or_never()).is_some() {
                return;
            }
            assert!(Instant::now() < deadline, "the direct probe must complete");
            objc2::rc::autoreleasepool(|_| {
                NSRunLoop::mainRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.01));
            });
        }
    }

    fn stop_application(app: &AnyObject) {
        unsafe {
            let _: () = objc2::msg_send![app, stop: None::<&AnyObject>];
            let event: Retained<AnyObject> = objc2::msg_send![objc2::class!(NSEvent),
                otherEventWithType: 15usize,
                location: NSPoint::new(0.0, 0.0),
                modifierFlags: 0usize,
                timestamp: 0.0f64,
                windowNumber: 0isize,
                context: None::<&AnyObject>,
                subtype: 0i16,
                data1: 0isize,
                data2: 0isize
            ];
            let _: () = objc2::msg_send![app, postEvent: &*event, atStart: true];
        }
    }

    struct SheetProbe(Rc<Cell<usize>>);

    impl Render for SheetProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let clicks = self.0.clone();
            div()
                .size_full()
                .bg(rgb(0x345678))
                .on_mouse_down(MouseButton::Left, move |_, _, _| {
                    clicks.set(clicks.get() + 1);
                })
        }
    }

    fn open_sheet_probe(
        cx: &mut App,
        options: WindowOptions,
    ) -> (
        WindowHandle<SheetProbe>,
        Retained<NSWindow>,
        Rc<Cell<usize>>,
    ) {
        let clicks = Rc::new(Cell::new(0));
        let mut native_window = None;
        let handle = cx
            .open_window(options, |window, cx| {
                let handle = HasWindowHandle::window_handle(window).unwrap();
                let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
                    panic!("macOS must expose an AppKit view");
                };
                let native: Retained<NSWindow> = unsafe {
                    objc2::msg_send![handle.ns_view.cast::<AnyObject>().as_ptr(), window]
                };
                assert!(!native.isVisible(), "root construction must stay hidden");
                assert!(native.sheetParent().is_none());
                assert!(native.parentWindow().is_none());
                native_window = Some(native);
                cx.new(|_| SheetProbe(clicks.clone()))
            })
            .expect("Core window creation requires a macOS GUI session and Metal device");
        (handle, native_window.unwrap(), clicks)
    }

    async fn wait_for_scene(
        platform: &MacPlatform,
        cx: &mut AsyncApp,
        window: WindowHandle<SheetProbe>,
    ) {
        let deadline = Instant::now() + Duration::from_secs(5);
        wait_for_native(
            platform,
            || {
                let facts = window.update(cx, |_, window, _| window.presentation_facts());
                let submitted = facts.as_ref().is_ok_and(|facts| {
                    facts.native_visible
                        && facts.present_submitted_generation.is_some()
                        && facts.non_empty_presented_generation.is_some()
                });
                if !submitted && Instant::now() >= deadline {
                    let geometry = window.update(cx, |_, window, _| {
                        (
                            window.bounds(),
                            window.viewport_size(),
                            window.rendered_frame_revision(),
                        )
                    });
                    eprintln!(
                        "SHEET scene timeout: window={:?}, facts={facts:?}, geometry={geometry:?}",
                        window.window_id()
                    );
                    let windows = cx.update(|cx| cx.debug_state()["windows"].clone());
                    eprintln!("SHEET native frame sources: {windows}");
                }
                submitted
            },
            "a visible probe must submit a non-empty scene through the original frame journal",
        )
        .await;
    }

    fn assert_sheet(parent: &NSWindow, child: &NSWindow) {
        assert!(child.isSheet() && child.isVisible());
        assert!(
            child
                .sheetParent()
                .is_some_and(|actual| std::ptr::eq(&*actual, parent))
        );
        assert!(
            parent
                .attachedSheet()
                .is_some_and(|actual| std::ptr::eq(&*actual, child))
        );
        assert_eq!(parent.sheets().len(), 1);
    }

    fn send_native_click(window: &NSWindow) {
        let app = NSApplication::sharedApplication(window.mtm());
        let bounds = window.contentView().unwrap().bounds();
        for event_type in [1usize, 2usize] {
            let event: Retained<objc2_app_kit::NSEvent> = unsafe {
                objc2::msg_send![objc2::class!(NSEvent),
                    mouseEventWithType: event_type,
                    location: NSPoint::new(bounds.size.width / 2.0, bounds.size.height / 2.0),
                    modifierFlags: 0usize,
                    timestamp: 0.0f64,
                    windowNumber: window.windowNumber(),
                    context: None::<&AnyObject>,
                    eventNumber: event_type as isize,
                    clickCount: 1isize,
                    pressure: if event_type == 1 { 1.0f32 } else { 0.0f32 }
                ]
            };
            app.sendEvent(&event);
        }
    }

    async fn exercise_sheets(platform: &Rc<MacPlatform>, cx: &mut AsyncApp) {
        eprintln!("SHEET stage=owner-main-control");
        let (owner, native_owner, owner_clicks) =
            cx.update(|cx| open_sheet_probe(cx, WindowOptions::default()));
        platform.activate(true);
        wait_for_scene(platform, cx, owner).await;
        send_native_click(&native_owner);
        drain_main_queue(platform).await;
        assert_eq!(
            owner_clicks.get(),
            1,
            "the parent input control must be live"
        );

        let (unrelated, native_unrelated, unrelated_clicks) = cx.update(|cx| {
            open_sheet_probe(
                cx,
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(420.0), px(80.0)),
                        size(px(320.0), px(240.0)),
                    ))),
                    ..Default::default()
                },
            )
        });
        wait_for_scene(platform, cx, unrelated).await;
        assert!(native_unrelated.attachedSheet().is_none());
        // Establish the main-window control independently of asynchronous OS activation.
        native_unrelated.makeMainWindow();
        let app = NSApplication::sharedApplication(native_unrelated.mtm());
        eprintln!(
            "SHEET main control: active={}, can_main={}, is_main={}, target={}, main={:?}, key={:?}",
            app.isActive(),
            native_unrelated.canBecomeMainWindow(),
            native_unrelated.isMainWindow(),
            native_unrelated.windowNumber(),
            app.mainWindow().map(|window| window.windowNumber()),
            app.keyWindow().map(|window| window.windowNumber()),
        );
        let main_deadline = Instant::now() + Duration::from_secs(5);
        wait_for_native(
            platform,
            || {
                let is_main = native_unrelated.isMainWindow();
                if !is_main && Instant::now() >= main_deadline {
                    eprintln!(
                        "SHEET main control timeout: active={}, can_main={}, target={}, main={:?}, key={:?}",
                        app.isActive(), native_unrelated.canBecomeMainWindow(),
                        native_unrelated.windowNumber(), app.mainWindow().map(|window| window.windowNumber()),
                        app.keyWindow().map(|window| window.windowNumber()),
                    );
                }
                is_main
            },
            "unrelated main-window control",
        )
        .await;
        eprintln!("SHEET stage=explicit-owner-modal-nested-close");
        let (dialog, native_dialog, dialog_clicks) = cx.update(|cx| {
            let owner = cx.transient_window_owner(owner.into()).unwrap();
            open_sheet_probe(
                cx,
                WindowOptions {
                    kind: WindowKind::Dialog,
                    transient_for: Some(owner),
                    ..Default::default()
                },
            )
        });
        wait_for_scene(platform, cx, dialog).await;
        assert_sheet(&native_owner, &native_dialog);
        assert!(
            native_unrelated.attachedSheet().is_none(),
            "mainWindow cannot replace the explicit owner"
        );
        send_native_click(&native_dialog);
        send_native_click(&native_owner);
        drain_main_queue(platform).await;
        assert_eq!(
            dialog_clicks.get(),
            1,
            "the sheet must receive native input"
        );
        assert_eq!(
            owner_clicks.get(),
            1,
            "the modal sheet must block its parent"
        );

        let (nested, native_nested, _) = cx.update(|cx| {
            let owner = cx.transient_window_owner(owner.into()).unwrap();
            open_sheet_probe(
                cx,
                WindowOptions {
                    kind: WindowKind::Dialog,
                    transient_for: Some(owner),
                    ..Default::default()
                },
            )
        });
        wait_for_scene(platform, cx, nested).await;
        assert_sheet(&native_owner, &native_dialog);
        assert_sheet(&native_dialog, &native_nested);
        let _nested_activation = nested
            .update(cx, |_, window, _| window.activate_window())
            .unwrap();
        drain_main_queue(platform).await;
        assert_sheet(&native_dialog, &native_nested);
        send_native_click(&native_dialog);
        drain_main_queue(platform).await;
        assert_eq!(
            dialog_clicks.get(),
            1,
            "a nested sheet must block its parent sheet"
        );

        let close_deadline = Instant::now() + Duration::from_secs(5);
        // AppKit sheets have no close button, so performClose: does not reach native close.
        // Send close directly to exercise terminal native cleanup and Core close ingress.
        native_nested.close();
        wait_for_native(
            platform,
            || {
                let sheet_parent_none = native_nested.sheetParent().is_none();
                let not_visible = !native_nested.isVisible();
                let core_retired = !cx.update(|cx| cx.windows().contains(&nested.into()));
                let closed = sheet_parent_none && not_visible && core_retired;
                if !closed && Instant::now() >= close_deadline {
                    eprintln!(
                        "NATIVE SHEET CLOSE timeout after close selector: sheet_parent_none={sheet_parent_none}, not_visible={not_visible}, core_retired={core_retired}, sheet_parent_window_number={:?}",
                        native_nested.sheetParent().map(|parent| parent.windowNumber()),
                    );
                }
                closed
            },
            "native sheet close must retire only the child",
        )
        .await;
        assert_sheet(&native_owner, &native_dialog);
        send_native_click(&native_dialog);
        drain_main_queue(platform).await;
        assert_eq!(
            dialog_clicks.get(),
            2,
            "ending a nested sheet must restore parent input"
        );

        eprintln!("SHEET stage=owner-retirement-and-transient-controls");
        owner
            .update(cx, |_, window, cx| window.remove_window(cx))
            .unwrap();
        wait_for_native(
            platform,
            || {
                !native_owner.isVisible()
                    && native_dialog.sheetParent().is_none()
                    && native_dialog.isVisible()
            },
            "owner retirement must detach a committed sheet without closing its Core window",
        )
        .await;
        assert!(cx.update(|cx| cx.windows().contains(&dialog.into())));
        assert!(native_owner.sheets().is_empty());
        let _standalone_activation = dialog
            .update(cx, |_, window, _| window.activate_window())
            .unwrap();
        drain_main_queue(platform).await;
        assert!(native_dialog.sheetParent().is_none());
        assert!(native_unrelated.attachedSheet().is_none());
        dialog
            .update(cx, |_, window, cx| window.remove_window(cx))
            .unwrap();
        wait_for_native(
            platform,
            || !native_dialog.isVisible(),
            "standalone dialog retirement",
        )
        .await;

        let (standalone, native_standalone, _) = cx.update(|cx| {
            open_sheet_probe(
                cx,
                WindowOptions {
                    kind: WindowKind::Dialog,
                    ..Default::default()
                },
            )
        });
        wait_for_scene(platform, cx, standalone).await;
        assert!(!native_standalone.isSheet() && native_standalone.sheetParent().is_none());
        standalone
            .update(cx, |_, window, cx| window.remove_window(cx))
            .unwrap();

        let (ordinary, native_ordinary, _) = cx.update(|cx| {
            let owner = cx.transient_window_owner(unrelated.into()).unwrap();
            open_sheet_probe(
                cx,
                WindowOptions {
                    transient_for: Some(owner),
                    ..Default::default()
                },
            )
        });
        wait_for_scene(platform, cx, ordinary).await;
        assert!(!native_ordinary.isSheet() && native_ordinary.sheetParent().is_none());
        assert!(
            native_ordinary
                .parentWindow()
                .is_some_and(|parent| std::ptr::eq(&*parent, &*native_unrelated))
        );
        ordinary
            .update(cx, |_, window, cx| window.remove_window(cx))
            .unwrap();
        drain_main_queue(platform).await;

        for (child_owned_by_parent, native_close) in [(false, true), (false, false), (true, false)]
        {
            eprintln!(
                "SHEET stage=parent-retirement explicit_owner_is_parent={child_owned_by_parent} native_close={native_close}"
            );
            let (parent, native_parent, _) = cx.update(|cx| {
                let owner = cx.transient_window_owner(unrelated.into()).unwrap();
                open_sheet_probe(
                    cx,
                    WindowOptions {
                        kind: WindowKind::Dialog,
                        transient_for: Some(owner),
                        ..Default::default()
                    },
                )
            });
            wait_for_scene(platform, cx, parent).await;
            let (child, native_child, child_clicks) = cx.update(|cx| {
                let owner = cx
                    .transient_window_owner(if child_owned_by_parent {
                        parent.into()
                    } else {
                        unrelated.into()
                    })
                    .unwrap();
                open_sheet_probe(
                    cx,
                    WindowOptions {
                        kind: WindowKind::Dialog,
                        transient_for: Some(owner),
                        ..Default::default()
                    },
                )
            });
            wait_for_scene(platform, cx, child).await;
            assert_sheet(&native_unrelated, &native_parent);
            assert_sheet(&native_parent, &native_child);
            if native_close {
                native_parent.close();
            } else {
                parent
                    .update(cx, |_, window, cx| window.remove_window(cx))
                    .unwrap();
            }
            wait_for_native(
                platform,
                || {
                    !native_parent.isVisible()
                        && native_child.isVisible()
                        && if child_owned_by_parent {
                            native_child.sheetParent().is_none()
                        } else {
                            native_child
                                .sheetParent()
                                .is_some_and(|parent| std::ptr::eq(&*parent, &*native_unrelated))
                        }
                        && !cx.update(|cx| cx.windows().contains(&parent.into()))
                },
                "a retiring sheet parent must preserve the surviving dialog's exact explicit owner",
            )
            .await;
            assert!(cx.update(|cx| cx.windows().contains(&child.into())));
            assert!(native_parent.sheets().is_empty());
            if child_owned_by_parent {
                assert!(!native_child.isSheet());
                assert!(native_child.parentWindow().is_none());
                assert!(native_unrelated.sheets().is_empty());
            } else {
                assert_sheet(&native_unrelated, &native_child);
            }
            let clicks_before = unrelated_clicks.get();
            send_native_click(&native_child);
            send_native_click(&native_unrelated);
            drain_main_queue(platform).await;
            assert_eq!(
                child_clicks.get(),
                1,
                "the surviving dialog must accept input"
            );
            assert_eq!(
                unrelated_clicks.get(),
                clicks_before + usize::from(child_owned_by_parent),
                "only retiring the explicit owner may release modal blocking"
            );
            child
                .update(cx, |_, window, cx| window.remove_window(cx))
                .unwrap();
            wait_for_native(
                platform,
                || native_unrelated.sheets().is_empty() && !native_child.isVisible(),
                "the surviving dialog must retire without orphan sheets",
            )
            .await;
            let clicks_before = unrelated_clicks.get();
            send_native_click(&native_unrelated);
            drain_main_queue(platform).await;
            assert_eq!(unrelated_clicks.get(), clicks_before + 1);
        }

        eprintln!("SHEET stage=hidden-submission-and-activation");
        let (hidden, native_hidden, _) = cx.update(|cx| {
            let owner = cx.transient_window_owner(unrelated.into()).unwrap();
            open_sheet_probe(
                cx,
                WindowOptions {
                    kind: WindowKind::Dialog,
                    transient_for: Some(owner),
                    show: false,
                    ..Default::default()
                },
            )
        });
        drain_main_queue(platform).await;
        assert!(!native_hidden.isVisible() && native_hidden.sheetParent().is_none());
        assert!(native_unrelated.attachedSheet().is_none());
        // Activation admission requires the first candidate to have committed. A main-queue
        // marker alone does not establish renderer submission for a newly created hidden window.
        wait_for_native(
            platform,
            || {
                hidden
                    .update(cx, |_, window, _| window.presentation_facts())
                    .is_ok_and(|facts| {
                        !facts.native_visible && facts.non_empty_presented_generation.is_some()
                    })
            },
            "the hidden dialog must submit its first candidate before activation",
        )
        .await;
        let hidden_activation = hidden
            .update(cx, |_, window, _| window.activate_window())
            .unwrap();
        drain_main_queue(platform).await;
        eprintln!(
            "SHEET hidden activation: ticket={:?}, visible={}, sheet_parent={:?}, owner_visible={}, owner_active_space={}",
            hidden_activation.snapshot(),
            native_hidden.isVisible(),
            native_hidden
                .sheetParent()
                .map(|window| window.windowNumber()),
            native_unrelated.isVisible(),
            native_unrelated.isOnActiveSpace(),
        );
        wait_for_scene(platform, cx, hidden).await;
        assert_sheet(&native_unrelated, &native_hidden);
        hidden
            .update(cx, |_, window, cx| window.remove_window(cx))
            .unwrap();
        wait_for_native(
            platform,
            || native_unrelated.sheets().is_empty(),
            "hidden-then-activated sheet cleanup",
        )
        .await;

        eprintln!("SHEET stage=titlebar-and-debug-json");
        let close = native_unrelated
            .standardWindowButton(NSWindowButton::CloseButton)
            .unwrap();
        let titlebar = unsafe { close.superview().and_then(|view| view.superview()) }.unwrap();
        for visible in [false, true] {
            unrelated
                .update(cx, |_, window, _| {
                    window.set_native_titlebar_visibility(visible)
                })
                .unwrap();
            assert_eq!(titlebar.isHidden(), !visible);
        }
        let debug = cx.update(|cx| cx.debug_state());
        let window_debug = debug["windows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|window| window["id"].as_u64() == Some(unrelated.window_id().as_u64()))
            .unwrap();
        assert!(window_debug["platform"]["frame_source"].is_object());
        assert!(window_debug["platform"]["renderer"].is_object());
        assert_eq!(
            window_debug["platform"]["window_number"].as_i64(),
            Some(native_unrelated.windowNumber() as i64)
        );
        unrelated
            .update(cx, |_, window, cx| window.remove_window(cx))
            .unwrap();
        drain_main_queue(platform).await;
        assert!(cx.update(|cx| cx.windows().is_empty()));
    }

    async fn exercise_native_close(platform: &Rc<MacPlatform>, panic: Option<NativeClosePanic>) {
        let window = open_native_window(platform);
        let raw = window
            .window_handle()
            .expect("the live native window handle");
        let RawWindowHandle::AppKit(raw) = raw.as_raw() else {
            panic!("macOS must expose an AppKit view");
        };
        // SAFETY: The live platform window owns this view and its NSWindow.
        let view =
            unsafe { Retained::retain(raw.ns_view.cast::<objc2_app_kit::NSView>().as_ptr()) }
                .expect("the platform window owns its native view");
        let native = view
            .window()
            .expect("the native view belongs to its window");
        let layer: Retained<AnyObject> = unsafe { objc2::msg_send![&*view, makeBackingLayer] };
        let metal: objc2::runtime::Bool =
            unsafe { objc2::msg_send![&*layer, isKindOfClass: objc2::class!(CAMetalLayer)] };
        assert!(
            metal.as_bool(),
            "the live renderer must supply its Metal layer"
        );
        drop(layer);
        native.makeKeyAndOrderFront(None);
        drain_main_queue(platform).await;
        native.contentView().unwrap().setWantsLayer(true);

        let retirement_drops = Rc::new(Cell::new(0));
        if panic.is_some() {
            wait_for_native(
                platform,
                || native.isVisible(),
                "the close-panic probe must begin with a visible native window",
            )
            .await;
            assert_eq!(window.debug_state()["native_close_phase"], "open");
        }

        if panic == Some(NativeClosePanic::TerminalCallback) {
            let calls = Rc::new(Cell::new(0));
            window.on_should_close(Box::new({
                let calls = calls.clone();
                move || {
                    calls.set(calls.get() + 1);
                    if calls.get() == 1 {
                        std::panic::panic_any(NativeClosePanic::QueryCallback);
                    }
                    true
                }
            }));
            let should_close: objc2::runtime::Bool =
                unsafe { objc2::msg_send![&*native, windowShouldClose: &*native] };
            assert!(
                !should_close.as_bool(),
                "a panicking close query must veto close"
            );
            assert!(native.isVisible());
            let should_close: objc2::runtime::Bool =
                unsafe { objc2::msg_send![&*native, windowShouldClose: &*native] };
            assert!(
                should_close.as_bool(),
                "the callback checkout must be restored"
            );
            assert_eq!(calls.get(), 2);

            let checkout_drops = Rc::new(Cell::new(0));
            let replacement_calls = Rc::new(Cell::new(0));
            window.on_should_close(Box::new({
                let window = Rc::downgrade(&window);
                let replacement_calls = replacement_calls.clone();
                let guard = InjectClosePanicOnDrop {
                    panic: NativeClosePanic::QueryCheckoutDrop,
                    drops: checkout_drops.clone(),
                };
                move || {
                    let _guard = &guard;
                    window.upgrade().unwrap().on_should_close(Box::new({
                        let calls = replacement_calls.clone();
                        move || {
                            calls.set(calls.get() + 1);
                            true
                        }
                    }));
                    true
                }
            }));
            let should_close: objc2::runtime::Bool =
                unsafe { objc2::msg_send![&*native, windowShouldClose: &*native] };
            assert!(
                !should_close.as_bool(),
                "a callback checkout destructor panic must veto close"
            );
            assert_eq!(checkout_drops.get(), 1);
            assert!(native.isVisible());
            let should_close: objc2::runtime::Bool =
                unsafe { objc2::msg_send![&*native, windowShouldClose: &*native] };
            assert!(should_close.as_bool());
            assert_eq!(replacement_calls.get(), 1);
            assert_eq!(window.debug_state()["native_close_phase"], "open");

            let combined_drops = Rc::new(Cell::new(0));
            let combined_replacement_calls = Rc::new(Cell::new(0));
            window.on_should_close(Box::new({
                let window = Rc::downgrade(&window);
                let replacement_calls = combined_replacement_calls.clone();
                let guard = InjectClosePanicOnDrop {
                    panic: NativeClosePanic::QueryCheckoutDrop,
                    drops: combined_drops.clone(),
                };
                move || {
                    let _guard = &guard;
                    window.upgrade().unwrap().on_should_close(Box::new({
                        let calls = replacement_calls.clone();
                        move || {
                            calls.set(calls.get() + 1);
                            true
                        }
                    }));
                    std::panic::panic_any(NativeClosePanic::QueryCallback);
                }
            }));
            let should_close: objc2::runtime::Bool =
                unsafe { objc2::msg_send![&*native, windowShouldClose: &*native] };
            assert!(!should_close.as_bool());
            assert!(native.isVisible());
            assert_eq!(combined_drops.get(), 1);
            let should_close: objc2::runtime::Bool =
                unsafe { objc2::msg_send![&*native, windowShouldClose: &*native] };
            assert!(should_close.as_bool());
            assert_eq!(combined_replacement_calls.get(), 1);

            window.on_should_close(Box::new({
                let guard = InjectClosePanicOnDrop {
                    panic: NativeClosePanic::QueryRetirementDrop,
                    drops: retirement_drops.clone(),
                };
                move || {
                    let _guard = &guard;
                    true
                }
            }));
        }

        // Preparation only claims the authority; the native close must perform its effects.
        let ticket = WindowPresentationShutdownTicket::new(WindowId::from(1), 1);
        let shutdown = window.prepare_presentation_shutdown(ticket.clone());
        assert!(!shutdown.snapshot().quiesced());
        let callbacks = Rc::new(Cell::new(0));
        let callback_drops = Rc::new(Cell::new(0));
        let frame_drops = Rc::new(Cell::new(0));
        let resize_drops = Rc::new(Cell::new(0));
        if panic.is_some() {
            window.on_request_frame(Box::new({
                let guard = InjectClosePanicOnDrop {
                    panic: NativeClosePanic::FrameCallbackDrop,
                    drops: frame_drops.clone(),
                };
                move |_| {
                    let _guard = &guard;
                }
            }));
            window.on_resize(Box::new({
                let guard = InjectClosePanicOnDrop {
                    panic: NativeClosePanic::ResizeCallbackDrop,
                    drops: resize_drops.clone(),
                };
                move |_, _| {
                    let _guard = &guard;
                }
            }));
        }
        window.on_close(Box::new({
            let native = native.clone();
            let ticket = ticket.clone();
            let callbacks = callbacks.clone();
            let frame_drops = frame_drops.clone();
            let resize_drops = resize_drops.clone();
            let guard = (panic == Some(NativeClosePanic::TerminalCallbackDrop)).then(|| {
                InjectClosePanicOnDrop {
                    panic: NativeClosePanic::TerminalCallbackDrop,
                    drops: callback_drops.clone(),
                }
            });
            move || {
                let _guard = &guard;
                let snapshot = ticket.snapshot();
                assert!(snapshot.quiesced() && snapshot.native_terminal());
                assert!(!snapshot.protocol_violation());
                assert_eq!(frame_drops.get(), usize::from(panic.is_some()));
                assert_eq!(resize_drops.get(), usize::from(panic.is_some()));
                callbacks.set(callbacks.get() + 1);
                // A native callback may synchronously request the same close again.
                native.close();
                if panic == Some(NativeClosePanic::TerminalCallback) {
                    std::panic::panic_any(NativeClosePanic::TerminalCallback);
                }
            }
        }));
        native.close();
        wait_for_native(
            platform,
            || {
                shutdown.snapshot().native_terminal()
                    && callbacks.get() == 1
                    && (panic.is_none()
                        || (!native.isVisible()
                            && window.debug_state()["native_close_phase"] == "closed"))
            },
            "native close must quiesce the exact renderer before one terminal callback",
        )
        .await;
        for _ in 0..2 {
            native.close();
        }
        let snapshot = shutdown.snapshot();
        assert!(snapshot.quiesced() && snapshot.native_terminal());
        assert!(!snapshot.protocol_violation());
        assert_eq!(callbacks.get(), 1);
        if let Some(panic) = panic {
            assert_eq!(snapshot.window_id(), WindowId::from(1));
            assert_eq!(snapshot.generation(), 1);
            assert!(
                !native.isVisible(),
                "superclass close must finish after a callback panic"
            );
            assert_eq!(window.debug_state()["native_close_phase"], "closed");
            assert_eq!(
                retirement_drops.get(),
                usize::from(panic == NativeClosePanic::TerminalCallback),
            );
            assert_eq!(
                callback_drops.get(),
                usize::from(panic == NativeClosePanic::TerminalCallbackDrop),
            );
        }
        drop(window);
        drain_main_queue(platform).await;
        assert_eq!(frame_drops.get(), usize::from(panic.is_some()));
        assert_eq!(resize_drops.get(), usize::from(panic.is_some()));
        view.setNeedsLayout(true);
        view.layoutSubtreeIfNeeded();
        view.setNeedsDisplay(true);
        view.displayIfNeeded();
        let layer: Retained<AnyObject> = unsafe { objc2::msg_send![&*view, makeBackingLayer] };
        let metal: objc2::runtime::Bool =
            unsafe { objc2::msg_send![&*layer, isKindOfClass: objc2::class!(CAMetalLayer)] };
        assert!(
            !metal.as_bool(),
            "retired native close must not restore a Metal surface"
        );
        if panic.is_none() {
            println!(
                "native close gate passed: quiesced, terminal, no protocol violation, callback exactly once"
            );
        }
    }

    pub(super) fn run() {
        let _main_thread =
            MainThreadMarker::new().expect("native windowing must run on the process main thread");
        let mode = std::env::args().nth(1).unwrap_or_else(|| "default".into());
        if mode == "sheet" || mode == "native-close" || mode == "native-close-panic" {
            struct SheetLog {
                errors: AtomicUsize,
                query_panics: AtomicUsize,
                surface_quiescence_panics: AtomicUsize,
                terminal_panics: AtomicUsize,
                expect_close_panics: std::sync::atomic::AtomicBool,
            }
            impl log::Log for SheetLog {
                fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
                    metadata.target().starts_with("open_gpui_macos::window")
                        || metadata.target() == "open_gpui::scene"
                }
                fn log(&self, record: &log::Record<'_>) {
                    if self.enabled(record.metadata()) {
                        eprintln!("SHEET native: {}", record.args());
                        if record.level() == log::Level::Error {
                            if self.expect_close_panics.load(Ordering::Relaxed) {
                                match record.args().to_string().as_str() {
                                    "isolated a panic from a macOS window close query at the native callback boundary" =>
                                    {
                                        self.query_panics.fetch_add(1, Ordering::Relaxed);
                                        return;
                                    }
                                    "isolated a panic from a macOS presentation surface quiescence at the native callback boundary" =>
                                    {
                                        self.surface_quiescence_panics
                                            .fetch_add(1, Ordering::Relaxed);
                                        return;
                                    }
                                    "isolated a panic from a macOS native window close at the native callback boundary" =>
                                    {
                                        self.terminal_panics.fetch_add(1, Ordering::Relaxed);
                                        return;
                                    }
                                    _ => {}
                                }
                            }
                            self.errors.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                fn flush(&self) {}
            }
            static NATIVE_LOG: SheetLog = SheetLog {
                errors: AtomicUsize::new(0),
                query_panics: AtomicUsize::new(0),
                surface_quiescence_panics: AtomicUsize::new(0),
                terminal_panics: AtomicUsize::new(0),
                expect_close_panics: std::sync::atomic::AtomicBool::new(false),
            };
            let close_panic_mode = mode == "native-close-panic";
            NATIVE_LOG
                .expect_close_panics
                .store(close_panic_mode, Ordering::Relaxed);
            log::set_logger(&NATIVE_LOG).unwrap();
            log::set_max_level(log::LevelFilter::Debug);
            let platform = Rc::new(MacPlatform::new(false));
            Application::with_platform(platform.clone()).run(move |cx| {
                // Platform::run creates GPUIApplication and installs its native platform ivars.
                let app: Retained<AnyObject> =
                    unsafe { objc2::msg_send![objc2::class!(NSApplication), sharedApplication] };
                let gpui_class = objc2::runtime::AnyClass::get(c"GPUIApplication").unwrap();
                let is_gpui: objc2::runtime::Bool =
                    unsafe { objc2::msg_send![&*app, isKindOfClass: gpui_class] };
                assert!(is_gpui.as_bool(), "the running app must inherit GPUIApplication");
                cx.spawn(async move |cx| {
                    if mode == "sheet" {
                        exercise_sheets(&platform, cx).await;
                        println!("native windowing sheet gate passed; physical input and fullscreen pixels remain manual");
                    } else if mode == "native-close-panic" {
                        for panic in [NativeClosePanic::TerminalCallback, NativeClosePanic::TerminalCallbackDrop] {
                            exercise_native_close(&platform, Some(panic)).await;
                        }
                    } else {
                        exercise_native_close(&platform, None).await;
                    }
                    assert_eq!(NATIVE_LOG.errors.load(Ordering::Relaxed), 0, "native close must not violate the renderer shutdown protocol");
                    stop_application(&app);
                }).detach();
            });
            if close_panic_mode {
                assert_eq!(NATIVE_LOG.errors.load(Ordering::Relaxed), 0);
                assert_eq!(NATIVE_LOG.query_panics.load(Ordering::Relaxed), 3);
                assert_eq!(
                    NATIVE_LOG.surface_quiescence_panics.load(Ordering::Relaxed),
                    2,
                );
                assert_eq!(NATIVE_LOG.terminal_panics.load(Ordering::Relaxed), 2);
                println!(
                    "native windowing native-close-panic gate passed: query veto and checkout recovery, frame and resize retirement, terminal callbacks and drops isolated, exact shutdown, native close, retired layer fallback"
                );
            }
            return;
        }
        if mode == "activity" {
            let (guard, abandoned_task) = objc2::rc::autoreleasepool(|_| {
                let platform = MacPlatform::new(true);
                let guard = futures::executor::block_on(
                    platform.prevent_idle_sleep("native platform idle sleep activity"),
                )
                .expect("native idle-sleep activity acquisition");
                let abandoned_task = platform.prevent_idle_sleep("abandoned native activity");
                (guard, abandoned_task)
            });
            drop(abandoned_task);
            drop(guard);
            println!("native platform idle-sleep consumer completed");
            return;
        }
        assert!(matches!(
            mode.as_str(),
            "default" | "windowed" | "headless" | "accessory" | "direct"
        ));
        objc2::rc::autoreleasepool(|_| {
            let platform = Rc::new(MacPlatform::new(false));
            platform.set_window_appearance(None);
            let app: Retained<AnyObject> =
                unsafe { objc2::msg_send![objc2::class!(NSApplication), sharedApplication] };
            assert_eq!(app.class().name().to_str().unwrap(), "GPUIApplication");
            let native_before = native_policy(&app);
            let regular = match mode.as_str() {
                "headless" => {
                    platform.set_initial_windowing(WindowingRequest::Headless);
                    false
                }
                "accessory" => {
                    platform.set_activation_policy(ActivationPolicy::Accessory);
                    false
                }
                "windowed" => {
                    platform.set_initial_windowing(windowed());
                    true
                }
                _ => true,
            };
            assert_eq!(native_policy(&app), native_before);
            let notifications = NativeNotifications::new(&app);
            if mode == "direct" {
                run_direct(&platform, &app, regular);
                assert!(notifications.launches.lock().unwrap().is_empty());
                let delegate: Option<Retained<AnyObject>> =
                    unsafe { objc2::msg_send![&*app, delegate] };
                assert!(delegate.is_none());
            } else {
                let launched = Rc::new(Cell::new(false));
                let launch_callback_dropped = Rc::new(Cell::new(false));
                platform.run(Box::new({
                    let platform = platform.clone();
                    let app = app.clone();
                    let launched = launched.clone();
                    let lifetime = ObserveCallbackDrop(launch_callback_dropped.clone());
                    move || {
                        let _lifetime = &lifetime;
                        assert!(!launched.replace(true));
                        assert_mode(&platform, &app, regular, true);
                        let delegate: Retained<AnyObject> =
                            unsafe { objc2::msg_send![&*app, delegate] };
                        assert_eq!(
                            delegate.class().name().to_str().unwrap(),
                            "GPUIApplicationDelegate"
                        );
                        platform
                            .foreground_executor()
                            .spawn(async move {
                                exercise_live(&platform, &app, true, regular).await;
                                stop_application(&app);
                            })
                            .detach();
                    }
                }));
                assert!(launched.get() && launch_callback_dropped.get());
                let launches = notifications.launches.lock().unwrap();
                assert_eq!(launches.len(), 2);
                assert!(launches.iter().all(|(_, _, main_thread)| *main_thread));
                let (_, before_finish, _) = launches
                    .iter()
                    .find(|(name, _, _)| *name == "NSApplicationWillFinishLaunchingNotification")
                    .expect("the real application launch notification");
                assert_eq!(
                    *before_finish,
                    if regular {
                        native_before
                    } else {
                        NSApplicationActivationPolicy::Accessory
                    }
                );
                let native_after_run = native_policy(&app);
                platform.set_activation_policy(ActivationPolicy::Accessory);
                assert!(platform.graphical_environment().is_none());
                assert_eq!(native_policy(&app), native_after_run);
            }
            drop(notifications);
            let weak = Rc::downgrade(&platform);
            drop(platform);
            assert!(weak.upgrade().is_none());
        });
        println!("native windowing {mode} gate passed; Core App and external-loop scope excluded");
    }
}
