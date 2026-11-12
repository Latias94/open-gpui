//! Exercises real AppKit layout after the normal Core App renderer-shutdown path.
//!
//! Run in a fresh macOS GUI process with:
//! `cargo run -p open-gpui-macos --example native_layer_shutdown --locked`.
//! The native view and window remain retained while their GPUI binding retires. This
//! exposes AppKit requests for a backing layer during the native retirement tail.
//! Add `-- --ancestor-backed` to keep the content view layer backed during retirement.
//! Add `-- --exercise-window` to also require live rendering and a submitted resize.

#[cfg(target_os = "macos")]
fn main() {
    macos::run();
}

#[cfg(not(target_os = "macos"))]
fn main() -> std::process::ExitCode {
    eprintln!("native_layer_shutdown requires a macOS GUI session and Metal device");
    std::process::ExitCode::FAILURE
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::{
        MainThreadMarker,
        rc::Retained,
        runtime::{AnyObject, Bool},
    };
    use objc2_app_kit::{NSView, NSWindow};
    use open_gpui::{
        AppContext, Application, Bounds, Context, IntoElement, QuitMode, Render, Window,
        WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
    };
    use open_gpui_macos::MacPlatform;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::{
        cell::Cell,
        io::Write as _,
        rc::Rc,
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };

    struct ProbeContent;

    impl Render for ProbeContent {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .bg(rgb(0xf5f6f8))
                .child("Native layer shutdown probe")
        }
    }

    pub(super) fn run() {
        MainThreadMarker::new().expect("the native layer probe requires the process main thread");
        let ancestor_backed = std::env::args().any(|argument| argument == "--ancestor-backed");
        let exercise_window = std::env::args().any(|argument| argument == "--exercise-window");
        let panic_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            panic_hook(info);
            std::process::exit(1);
        }));
        let progress = Arc::new(Mutex::new("launching AppKit".to_owned()));
        std::thread::spawn({
            let progress = progress.clone();
            move || {
                std::thread::sleep(Duration::from_secs(10));
                eprintln!(
                    "NATIVE LAYER SHUTDOWN FAILED: the process remained alive after 10 seconds; {}",
                    progress.lock().expect("probe progress"),
                );
                std::process::exit(1);
            }
        });
        objc2::rc::autoreleasepool(|_| {
            let platform = Rc::new(MacPlatform::new(false));
            Application::with_platform(platform)
                .with_quit_mode(QuitMode::Explicit)
                .run(move |cx| {
                    let complete = Rc::new(Cell::new(false));
                    cx.on_app_quit({
                        let complete = complete.clone();
                        move |_| {
                            if !complete.get() {
                                eprintln!("NATIVE LAYER SHUTDOWN FAILED: application quit before retirement-tail verification completed");
                                std::process::exit(1);
                            }
                            std::future::ready(())
                        }
                    })
                    .detach();
                    let bounds = Bounds::centered(None, size(px(320.0), px(200.0)), cx);
                    let retained = Rc::new(std::cell::RefCell::new(None));
                    let window = cx
                        .open_window(
                            WindowOptions {
                                window_bounds: Some(WindowBounds::Windowed(bounds)),
                                ..Default::default()
                            },
                            {
                                let retained = retained.clone();
                                move |window, cx| {
                                    let raw = HasWindowHandle::window_handle(window)
                                        .expect("a real native window handle");
                                    let RawWindowHandle::AppKit(raw) = raw.as_raw() else {
                                        panic!("macOS must expose an AppKit view");
                                    };
                                    // SAFETY: The live platform window owns this native view.
                                    let view = unsafe {
                                        Retained::retain(raw.ns_view.cast::<NSView>().as_ptr())
                                    }
                                    .expect("the live native view");
                                    let native_window: Retained<NSWindow> =
                                        view.window().expect("the live native window");
                                    *retained.borrow_mut() = Some((view, native_window));
                                    cx.new(|_| ProbeContent)
                                }
                            },
                        )
                        .expect("a real AppKit window and Metal renderer");
                    let id = window.window_id();
                    let terminal = Rc::new(Cell::new(false));
                    cx.on_window_native_terminal({
                        let terminal = terminal.clone();
                        move |_, closed| {
                            if closed == id {
                                terminal.set(true);
                            }
                        }
                    })
                    .detach();
                    cx.activate(true);
                    window
                        .update(cx, |_, window, _| {
                            let _ = window.activate_window();
                            window.refresh();
                            *progress.lock().expect("probe progress") = format!(
                                "window opened; {:?}",
                                window.presentation_facts()
                            );
                        })
                        .expect("the newly opened window can refresh");
                    cx.spawn(async move |cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis(100))
                            .await;
                        let (view, native_window) = retained
                            .borrow_mut()
                            .take()
                            .expect("the probe retained both native objects");
                        assert!(view.wantsLayer(), "the initial GPUI view is layer backed");
                        let layer: *mut AnyObject = unsafe { objc2::msg_send![&*view, layer] };
                        assert!(!layer.is_null(), "the live renderer owns a layer");
                        if exercise_window {
                        let deadline = Instant::now() + Duration::from_secs(3);
                        let before = loop {
                            let facts = window
                                .update(cx, |_, window, _| window.presentation_facts())
                                .expect("the native window remains live");
                            *progress.lock().expect("probe progress") =
                                format!("waiting for initial Submitted; {facts:?}");
                            assert!(Instant::now() < deadline, "initial rendering timed out; {facts:?}");
                            if facts.native_visible {
                                if let Some(generation) = facts.present_submitted_generation {
                                    break generation;
                                }
                            }
                            cx.background_executor()
                                .timer(Duration::from_millis(10))
                                .await;
                        };
                        let mut frame = native_window.frame();
                        frame.origin.x += 12.0;
                        frame.origin.y += 8.0;
                        frame.size.width += 32.0;
                        frame.size.height += 24.0;
                        native_window.setFrame_display(frame, true);
                        window.update(cx, |_, window, _| window.refresh())
                            .expect("the resized window can refresh");
                        let deadline = Instant::now() + Duration::from_secs(3);
                        loop {
                            let facts = window
                                .update(cx, |_, window, _| window.presentation_facts())
                                .expect("the moved and resized native window remains live");
                            *progress.lock().expect("probe progress") =
                                format!("waiting for resized Submitted after {before}; {facts:?}");
                            assert!(Instant::now() < deadline, "resize rendering timed out after {before}; {facts:?}");
                            if facts
                                .present_submitted_generation
                                .is_some_and(|generation| generation > before)
                            {
                                break;
                            }
                            cx.background_executor()
                                .timer(Duration::from_millis(10))
                                .await;
                        }
                        println!("NATIVE LAYER SHUTDOWN: move/resize produced a newer real Submitted generation");
                        }
                        if ancestor_backed {
                            native_window
                                .contentView()
                                .expect("the native content view")
                                .setWantsLayer(true);
                        }
                        let facts = window.update(cx, |_, window, _| window.presentation_facts())
                            .expect("the window remains live before shutdown");
                        *progress.lock().expect("probe progress") =
                            format!("removing real window; ancestor_backed={ancestor_backed}; {facts:?}");
                        println!("NATIVE LAYER SHUTDOWN: removing the real window through Core App");
                        std::io::stdout().flush().expect("flush probe phase");
                        window
                            .update(cx, |_, window, cx| window.remove_window(cx))
                            .expect("the live window can be removed");
                        *progress.lock().expect("probe progress") = "waiting for native terminal after remove_window".to_owned();
                        while !terminal.get() {
                            cx.background_executor()
                                .timer(Duration::from_millis(10))
                                .await;
                        }
                        *progress.lock().expect("probe progress") = "forcing retirement-tail AppKit layer/layout".to_owned();
                        let layer: *mut AnyObject = unsafe { objc2::msg_send![&*view, layer] };
                        println!(
                            "NATIVE LAYER SHUTDOWN: native terminal observed; wantsLayer={}, layer={}; forcing retirement-tail layout",
                            view.wantsLayer(),
                            !layer.is_null(),
                        );
                        std::io::stdout().flush().expect("flush native observations");
                        assert!(!view.wantsLayer(), "retirement disables explicit layer backing");
                        if !layer.is_null() {
                            let metal: Bool = unsafe {
                                objc2::msg_send![layer, isKindOfClass: objc2::class!(CAMetalLayer)]
                            };
                            assert!(!metal.as_bool(), "the retired view must not retain its Metal layer");
                        }
                        let backing: Retained<AnyObject> =
                            unsafe { objc2::msg_send![&*view, makeBackingLayer] };
                        let metal: Bool = unsafe {
                            objc2::msg_send![&*backing, isKindOfClass: objc2::class!(CAMetalLayer)]
                        };
                        assert!(!metal.as_bool(), "a retired renderer cannot recreate a Metal surface");
                        view.setNeedsLayout(true);
                        view.layoutSubtreeIfNeeded();
                        view.setNeedsDisplay(true);
                        view.displayIfNeeded();
                        cx.background_executor()
                            .timer(Duration::from_millis(30))
                            .await;
                        let layer: *mut AnyObject = unsafe { objc2::msg_send![&*view, layer] };
                        if !layer.is_null() {
                            let metal: Bool = unsafe {
                                objc2::msg_send![layer, isKindOfClass: objc2::class!(CAMetalLayer)]
                            };
                            assert!(!metal.as_bool(), "retirement-tail layout must not restore a Metal layer");
                        }
                        complete.set(true);
                        println!("NATIVE LAYER SHUTDOWN OK: real AppKit retirement-tail layout completed; requesting native quit");
                        std::io::stdout().flush().expect("flush successful observations");
                        drop(native_window);
                        drop(view);
                        *progress.lock().expect("probe progress") = "native layout passed; waiting for actual process exit".to_owned();
                        cx.update(|cx| cx.quit());
                    })
                    .detach();
                });
        });
        panic!("AppKit returned without terminating the probe process");
    }
}
