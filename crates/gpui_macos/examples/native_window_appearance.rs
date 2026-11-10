//! Native AppKit boundary probe, run with
//! `cargo run -p open-gpui-macos --example native_window_appearance` on macOS.
//!
//! Requires a macOS GUI session and Metal device. The opaque window handles identify backend
//! windows only; this probe does not create a Core App or qualify titlebar pixels.

#[cfg(target_os = "macos")]
fn main() {
    macos::run();
}

#[cfg(not(target_os = "macos"))]
fn main() -> std::process::ExitCode {
    eprintln!("native_window_appearance is unsupported on this platform; no native evidence");
    std::process::ExitCode::FAILURE
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::{MainThreadMarker, rc::Retained, runtime::AnyObject};
    use objc2_app_kit::{
        NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSAppearanceNameVibrantDark,
        NSAppearanceNameVibrantLight,
    };
    use objc2_foundation::{NSDate, NSRunLoop};
    use open_gpui::{
        Bounds, Empty, Platform, PlatformWindow, WindowAppearance, WindowBounds, WindowHandle,
        WindowId, WindowKind, WindowParams, point, px, size,
    };
    use open_gpui_macos::MacPlatform;
    use raw_window_handle::RawWindowHandle;
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::{Duration, Instant},
    };

    struct RestoreApplicationAppearance {
        app: Retained<AnyObject>,
        original: Option<Retained<NSAppearance>>,
        _main_thread: MainThreadMarker,
    }

    impl RestoreApplicationAppearance {
        fn new(main_thread: MainThreadMarker) -> Self {
            let app: Retained<AnyObject> =
                unsafe { objc2::msg_send![objc2::class!(NSApplication), sharedApplication] };
            let original = unsafe { objc2::msg_send![&*app, appearance] };
            Self {
                app,
                original,
                _main_thread: main_thread,
            }
        }
    }

    impl Drop for RestoreApplicationAppearance {
        fn drop(&mut self) {
            unsafe {
                let _: () = objc2::msg_send![&*self.app, setAppearance: self.original.as_deref()];
            }
        }
    }

    fn effective_appearance(object: &AnyObject) -> Retained<NSAppearance> {
        unsafe { objc2::msg_send![object, effectiveAppearance] }
    }

    fn open_native_window(platform: &MacPlatform, id: u64) -> Rc<dyn PlatformWindow> {
        let bounds = Bounds::new(point(px(40.0), px(40.0)), size(px(256.0), px(160.0)));
        let window = platform
            .open_window(
                WindowHandle::<Empty>::new(WindowId::from(id)).into(),
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
                },
            )
            .expect("a native window requires a macOS GUI session and Metal device");
        Rc::from(window)
    }

    fn native_objects(window: &dyn PlatformWindow) -> (Retained<AnyObject>, Retained<AnyObject>) {
        let handle = window.window_handle().expect("native window handle");
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            panic!("macOS windows must expose an AppKit view");
        };
        let view = unsafe { Retained::retain(handle.ns_view.cast::<AnyObject>().as_ptr()) }
            .expect("the platform window owns its native view");
        let window: Option<Retained<AnyObject>> = unsafe { objc2::msg_send![&*view, window] };
        (view, window.expect("the native view belongs to a window"))
    }

    fn wait_for_native(mut condition: impl FnMut() -> bool, reason: &str) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !condition() {
            assert!(Instant::now() < deadline, "{reason}");
            objc2::rc::autoreleasepool(|_| {
                NSRunLoop::mainRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.01));
            });
        }
    }

    #[derive(Debug)]
    struct AppearanceObservation {
        main_thread: bool,
        window: WindowAppearance,
        application: WindowAppearance,
    }

    struct ObserveCallbackDrop(Rc<Cell<bool>>);

    impl Drop for ObserveCallbackDrop {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }

    pub(super) fn run() {
        let main_thread = MainThreadMarker::new()
            .expect("the native appearance probe must run on the process main thread");
        objc2::rc::autoreleasepool(|_| {
            let restore = RestoreApplicationAppearance::new(main_thread);
            let platform = Rc::new(MacPlatform::new(false));
            platform.set_window_appearance(None);
            let override_appearance: Option<Retained<NSAppearance>> =
                unsafe { objc2::msg_send![&*restore.app, appearance] };
            assert!(override_appearance.is_none());
            let system = platform.window_appearance();
            let system_appearance = effective_appearance(&restore.app);

            let window = open_native_window(&platform, 1);
            let (view, native_window) = native_objects(window.as_ref());
            let window_override: Option<Retained<NSAppearance>> =
                unsafe { objc2::msg_send![&*native_window, appearance] };
            assert!(window_override.is_none());
            assert_eq!(
                effective_appearance(&native_window).name(),
                system_appearance.name()
            );
            assert_eq!(window.appearance(), platform.window_appearance());

            let observations = Rc::new(RefCell::new(Vec::<AppearanceObservation>::new()));
            let callback_dropped = Rc::new(Cell::new(false));
            window.on_appearance_changed(Box::new({
                let observations = observations.clone();
                let window = Rc::downgrade(&window);
                let platform = Rc::downgrade(&platform);
                let callback_drop = ObserveCallbackDrop(callback_dropped.clone());
                move || {
                    let _callback_drop = &callback_drop;
                    let (Some(window), Some(platform)) = (window.upgrade(), platform.upgrade())
                    else {
                        return;
                    };
                    let _ = platform.background_executor();
                    let observation = AppearanceObservation {
                        main_thread: MainThreadMarker::new().is_some(),
                        window: window.appearance(),
                        application: platform.window_appearance(),
                    };
                    observations.borrow_mut().push(observation);
                }
            }));

            platform.set_window_appearance(Some(WindowAppearance::Dark));
            let later_window = open_native_window(&platform, 2);
            let (later_view, later_native_window) = native_objects(later_window.as_ref());
            assert_eq!(later_window.appearance(), WindowAppearance::Dark);
            let later_override: Option<Retained<NSAppearance>> =
                unsafe { objc2::msg_send![&*later_native_window, appearance] };
            assert!(later_override.is_none());

            for (expected, name) in unsafe {
                [
                    (WindowAppearance::Light, NSAppearanceNameAqua),
                    (WindowAppearance::VibrantDark, NSAppearanceNameVibrantDark),
                    (WindowAppearance::VibrantLight, NSAppearanceNameVibrantLight),
                    (WindowAppearance::Dark, NSAppearanceNameDarkAqua),
                ]
            } {
                let previous_callbacks = observations.borrow().len();
                platform.set_window_appearance(Some(expected));
                let native_override: Option<Retained<NSAppearance>> =
                    unsafe { objc2::msg_send![&*restore.app, appearance] };
                assert_eq!(
                    native_override
                        .expect("explicit application appearance")
                        .name()
                        .as_ref(),
                    name
                );
                assert_eq!(platform.window_appearance(), expected);
                assert_eq!(window.appearance(), expected);
                assert_eq!(later_window.appearance(), expected);
                assert_eq!(effective_appearance(&native_window).name().as_ref(), name);
                assert_eq!(
                    effective_appearance(&later_native_window).name().as_ref(),
                    name
                );
                wait_for_native(
                    || {
                        let _ = effective_appearance(&view);
                        observations.borrow().len() > previous_callbacks
                    },
                    "the existing appearance callback must observe the application override",
                );
                let observations = observations.borrow();
                let latest = observations.last().unwrap();
                assert_eq!(latest.window, expected);
                assert_eq!(latest.application, expected);
                assert!(
                    observations
                        .iter()
                        .all(|observation| observation.main_thread)
                );
            }

            let light = NSAppearance::appearanceNamed(unsafe { NSAppearanceNameAqua }).unwrap();
            unsafe {
                let _: () = objc2::msg_send![&*native_window, setAppearance: Some(&*light)];
            }
            platform.set_window_appearance(Some(WindowAppearance::VibrantDark));
            assert_eq!(window.appearance(), WindowAppearance::Light);
            assert_eq!(later_window.appearance(), WindowAppearance::VibrantDark);
            unsafe {
                let _: () = objc2::msg_send![&*native_window, setAppearance: None::<&NSAppearance>];
            }
            assert_eq!(window.appearance(), WindowAppearance::VibrantDark);

            let opposite = match system {
                WindowAppearance::Dark | WindowAppearance::VibrantDark => WindowAppearance::Light,
                WindowAppearance::Light | WindowAppearance::VibrantLight => WindowAppearance::Dark,
            };
            platform.set_window_appearance(Some(opposite));
            let _ = effective_appearance(&view);
            let before_clear = observations.borrow().len();
            platform.set_window_appearance(None);
            let cleared: Option<Retained<NSAppearance>> =
                unsafe { objc2::msg_send![&*restore.app, appearance] };
            assert!(cleared.is_none());
            let effective = effective_appearance(&restore.app);
            assert_eq!(
                effective_appearance(&native_window).name(),
                effective.name()
            );
            assert_eq!(
                effective_appearance(&later_native_window).name(),
                effective.name()
            );
            assert_eq!(window.appearance(), platform.window_appearance());
            assert_eq!(later_window.appearance(), platform.window_appearance());
            wait_for_native(
                || {
                    let _ = effective_appearance(&view);
                    observations.borrow().len() > before_clear
                },
                "clearing the override must deliver the inherited appearance change",
            );
            assert_eq!(
                observations.borrow().last().unwrap().window,
                platform.window_appearance()
            );

            let before_close = observations.borrow().len();
            drop(window);
            assert!(callback_dropped.get());
            unsafe {
                let _: () = objc2::msg_send![&*view, viewDidChangeEffectiveAppearance];
            }
            assert_eq!(observations.borrow().len(), before_close);
            drop(later_window);
            let drained = Arc::new(AtomicBool::new(false));
            dispatch2::DispatchQueue::main().exec_async({
                let drained = drained.clone();
                move || drained.store(true, Ordering::Release)
            });
            wait_for_native(
                || drained.load(Ordering::Acquire),
                "native window cleanup must drain on the main thread",
            );
            drop((view, native_window, later_view, later_native_window));

            let app = restore.app.clone();
            let original = restore.original.clone();
            drop(restore);
            let restored: Option<Retained<NSAppearance>> =
                unsafe { objc2::msg_send![&*app, appearance] };
            assert_eq!(restored, original);
        });
        println!("native application appearance, inheritance, callback, and cleanup probe passed");
    }
}
