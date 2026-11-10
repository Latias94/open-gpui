//! Native process-main-thread gate, run on macOS with a GUI session and Metal device:
//! `cargo run -p open-gpui-macos --example native_windowing -- <mode>`.
//!
//! Run each mode in a fresh process: `default`, `windowed`, `headless`, `accessory`, `direct`,
//! `activity`. The activity mode exercises the real headless platform acquisition/drop path.
//! The first four enter the real GPUIApplication run loop and launch delegate. `direct` only
//! checks stored policy and native resource refusal without launching the application. Neither
//! path creates a Core App or qualifies Core retirement, external event pumping, or Dock pixels.
//! `headless` selects Accessory on a switchable platform. Fixed-headless refusal is checked without
//! entering its separate CFRunLoop. This probe stops AppKit directly and does not qualify quit.

#[cfg(target_os = "macos")]
fn main() {
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
    use objc2::{MainThreadMarker, rc::Retained, runtime::AnyObject};
    use objc2_app_kit::NSApplicationActivationPolicy;
    use objc2_foundation::{
        NSDate, NSNotification, NSNotificationCenter, NSPoint, NSRunLoop, NSString,
    };
    use open_gpui::{
        ActivationPolicy, Bounds, Empty, GraphicalEnvironment, Platform, PlatformWindow, Result,
        Task, WindowBounds, WindowHandle, WindowId, WindowKind, WindowParams, WindowingRequest,
        point, px, size,
    };
    use open_gpui_macos::MacPlatform;
    use raw_window_handle::RawWindowHandle;
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
        let callbacks = Rc::new(Cell::new(0));
        let callback_dropped = Rc::new(Cell::new(false));
        window.on_active_status_change(Box::new({
            let platform = Rc::downgrade(platform);
            let window = Rc::downgrade(&window);
            let callbacks = callbacks.clone();
            let lifetime = ObserveCallbackDrop(callback_dropped.clone());
            move |observation| {
                let _lifetime = &lifetime;
                assert!(MainThreadMarker::new().is_some());
                let platform = platform.upgrade().expect("the running platform");
                let window = window.upgrade().expect("the live platform window");
                let _ = platform.background_executor();
                let _ = window.is_active();
                if observation.exact_native_positive() {
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
            platform.activate(true);
            unsafe {
                let _: () =
                    objc2::msg_send![&*native_window, makeKeyAndOrderFront: None::<&AnyObject>];
            }
            wait_for_native(
                platform,
                || is_active(app) && window.is_active() && callbacks.get() > 0,
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
            platform.activate(true);
            wait_for_native(
                platform,
                || is_active(app),
                "explicit activation without a window must succeed",
            )
            .await;
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

    pub(super) fn run() {
        let _main_thread =
            MainThreadMarker::new().expect("native windowing must run on the process main thread");
        let mode = std::env::args().nth(1).unwrap_or_else(|| "default".into());
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
