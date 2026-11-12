//! Native UIKit and Metal integration probe, packaged with build_simulator.py.

#[cfg(any(target_os = "ios", test))]
#[path = "native_windowing/oracles.rs"]
mod oracles;

#[cfg(target_os = "ios")]
#[path = "native_windowing/interaction.rs"]
mod interaction;

#[cfg(target_os = "ios")]
fn main() {
    ios::run();
}

#[cfg(not(target_os = "ios"))]
fn main() -> std::process::ExitCode {
    eprintln!("native_windowing requires an iOS device or simulator");
    std::process::ExitCode::FAILURE
}

#[cfg(target_os = "ios")]
mod ios {
    use crate::oracles::{CASES, check_case, resize_was_submitted};
    use objc2::{MainThreadMarker, msg_send, rc::Retained};
    use objc2_core_foundation::CGSize;
    use objc2_quartz_core::CAMetalLayer;
    use objc2_ui_kit::{UIApplication, UISceneActivationState, UIView};
    use open_gpui::{
        AppContext, Application, Bounds, Context, Empty, IntoElement, PathBuilder, Platform,
        QuitMode, Render, Window, WindowActivationStatus, WindowActivationTerminal, WindowOptions,
        canvas, div, fill, point, prelude::*, px, rgb, size,
    };
    use open_gpui_ios::IosPlatform;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::{
        cell::{Cell, RefCell},
        io::Write as _,
        rc::Rc,
        time::{Duration, Instant},
    };

    struct Probe;

    pub(super) fn native_view(window: &Window) -> Retained<UIView> {
        let raw = HasWindowHandle::window_handle(window).expect("a native UIKit view");
        let RawWindowHandle::UiKit(raw) = raw.as_raw() else {
            panic!("expected a UIKit handle")
        };
        unsafe { Retained::retain(raw.ui_view.cast::<UIView>().as_ptr()) }
            .expect("the live GPUI view has a native owner")
    }

    pub(super) fn submitted_drawable(view: &UIView) -> (u64, (usize, usize)) {
        let serial: u64 = unsafe { msg_send![view, gpuiSubmissionSerial] };
        let extent: CGSize = unsafe { msg_send![view, gpuiSubmittedDrawableSize] };
        (serial, (extent.width as usize, extent.height as usize))
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .bg(rgb(0xffffff))
                .child(
                    div()
                        .absolute()
                        .left(px(4.25))
                        .top(px(4.25))
                        .w(px(80.5))
                        .h(px(56.5))
                        .rounded_md()
                        .overflow_hidden()
                        .child(div().w(px(100.)).h(px(80.)).bg(rgb(0xff0000))),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(104.))
                        .top(px(4.))
                        .w(px(80.))
                        .h(px(56.))
                        .border_2()
                        .border_color(rgb(0x00ff00))
                        .bg(rgb(0x0000ff)),
                )
                .child(
                    canvas(
                        |_, _, _| {},
                        |bounds, _, window, _| {
                            let mut path = PathBuilder::fill();
                            path.move_to(bounds.origin);
                            path.curve_to(
                                bounds.origin + point(px(72.), px(48.)),
                                bounds.origin + point(px(72.), px(0.)),
                            );
                            path.line_to(bounds.origin + point(px(0.), px(48.)));
                            path.close();
                            window.paint_path(
                                path.build().expect("a valid analytic quadratic path"),
                                rgb(0x00ff00),
                            );
                        },
                    )
                    .absolute()
                    .left(px(4.))
                    .top(px(84.))
                    .w(px(80.))
                    .h(px(56.)),
                )
                .child(
                    canvas(
                        |_, _, _| {},
                        |bounds, _, window, _| {
                            window.paint_layer(bounds, |window| {
                                window.paint_quad(fill(bounds, rgb(0xff0000)));
                                window.paint_quad(fill(
                                    Bounds::new(
                                        bounds.origin + point(px(8.), px(8.)),
                                        size(px(64.), px(40.)),
                                    ),
                                    rgb(0x00ffff),
                                ));
                            });
                        },
                    )
                    .absolute()
                    .left(px(104.))
                    .top(px(84.))
                    .w(px(80.))
                    .h(px(56.)),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(4.))
                        .top(px(164.))
                        .w(px(80.))
                        .h(px(56.))
                        .bg(rgb(0xff0000))
                        .child(div().size_full().bg(rgb(0x0000ff)).opacity(0.5)),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(104.))
                        .top(px(164.))
                        .w(px(80.))
                        .h(px(56.))
                        .bg(rgb(0xffff00))
                        .rounded_md()
                        .shadow_lg(),
                )
        }
    }

    pub(super) fn run() {
        let main_thread =
            MainThreadMarker::new().expect("the probe runs on the process main thread");
        let panic_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            panic_hook(info);
            std::process::exit(1);
        }));
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_secs(30));
            eprintln!("IOS NATIVE WINDOWING FAILED: timed out before native retirement");
            std::process::exit(1);
        });
        let platform = Rc::new(IosPlatform::new());
        Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit).run(move |cx| {
            let retained = Rc::new(RefCell::new(None));
            let window = cx.open_window(WindowOptions::default(), {
                let retained = retained.clone();
                move |window, cx| {
                    let view = native_view(window);
                    assert!(!window.presentation_facts().native_visible, "creation must wait for the original initial-presentation command");
                    *retained.borrow_mut() = Some(view);
                    cx.new(|_| Probe)
                }
            }).expect("open the ordinary public GPUI iOS window");
            let terminal = Rc::new(Cell::new(0));
            cx.on_window_native_terminal({
                let terminal = terminal.clone();
                move |_, id| if id == window.window_id() { terminal.set(terminal.get() + 1); }
            }).detach();
            cx.spawn(async move |cx| {
                let deadline = Instant::now() + Duration::from_secs(20);
                loop {
                    let facts = window.update(cx, |_, window, _| window.presentation_facts()).expect("a live GPUI window");
                    if facts.present_submitted_generation.is_some() {
                        break;
                    }
                    assert!(Instant::now() < deadline, "initial Metal submission timed out: {facts:?}");
                    cx.background_executor().timer(Duration::from_millis(16)).await;
                }
                let view = retained.borrow_mut().take().expect("retain the native view through shutdown");
                let native_window = view.window().expect("UIKit attached the submitted view");
                while !native_window.windowScene().is_some_and(|scene| scene.activationState() == UISceneActivationState::ForegroundActive) {
                    assert!(Instant::now() < deadline, "the initial scene never became active");
                    cx.background_executor().timer(Duration::from_millis(16)).await;
                }
                let late_window = cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Empty))
                    .expect("open a window after its scene has already become active");
                let late_view = late_window.update(cx, |_, window, _| native_view(window))
                    .expect("retain the late-created native window");
                loop {
                    let active = late_window.update(cx, |_, window, _| window.is_window_active())
                        .expect("the late-created window remains live");
                    let active_window = cx.update(|app| app.active_window());
                    if active
                        && active_window.is_some_and(|active| active.window_id() == late_window.window_id())
                        && late_view.window().is_some_and(|window| window.isKeyWindow())
                    {
                        break;
                    }
                    assert!(Instant::now() < deadline, "late-created key window did not become active");
                    cx.background_executor().timer(Duration::from_millis(16)).await;
                }
                let (superseded, activation) = cx.update(|app| {
                    let superseded = late_window.update(app, |_, window, _| window.activate_window())
                        .expect("queue an older public activation");
                    let activation = window.update(app, |_, window, _| window.activate_window())
                        .expect("queue the original window's public activation");
                    assert_eq!(activation.snapshot().status(), WindowActivationStatus::Queued);
                    (superseded, activation)
                });
                assert!(superseded.snapshot().request_generation() > 0);
                assert!(activation.snapshot().request_generation() > superseded.snapshot().request_generation());
                assert_eq!(superseded.snapshot().status().terminal(), Some(WindowActivationTerminal::Superseded));
                assert_eq!(activation.snapshot().target(), window.window_id());
                loop {
                    if let Some(terminal) = activation.snapshot().status().terminal() {
                        assert_eq!(terminal, WindowActivationTerminal::Activated);
                    }
                    let active = window.update(cx, |_, window, _| window.is_window_active())
                        .expect("the original window remains live");
                    let active_window = cx.update(|app| app.active_window());
                    if activation.snapshot().status().terminal() == Some(WindowActivationTerminal::Activated)
                        && active && native_window.isKeyWindow()
                        && active_window.is_some_and(|active| active.window_id() == window.window_id())
                        && platform.focused_window().window().is_some_and(|active| active.window_id() == window.window_id())
                    {
                        break;
                    }
                    assert!(Instant::now() < deadline, "public activation did not reach exact native Activated: {:?}", activation.snapshot());
                    cx.background_executor().timer(Duration::from_millis(16)).await;
                }
                let repeated = window.update(cx, |_, window, _| window.activate_window())
                    .expect("request activation of the already-key native window");
                assert!(repeated.snapshot().request_generation() > activation.snapshot().request_generation());
                loop {
                    if let Some(terminal) = repeated.snapshot().status().terminal() {
                        assert_eq!(terminal, WindowActivationTerminal::Activated);
                        break;
                    }
                    assert!(Instant::now() < deadline, "already-key activation lacked an exact observation");
                    cx.background_executor().timer(Duration::from_millis(16)).await;
                }
                late_window.update(cx, |_, window, cx| window.remove_window(cx))
                    .expect("close the late-created window through Core");
                let (image, scale) = window.update(cx, |_, window, _| {
                    (window.render_to_image().expect("capture the actual accepted scene"), window.scale_factor())
                }).expect("capture the live window");
                for case in CASES {
                    check_case(case, |(horizontal, vertical)| {
                        image.get_pixel((horizontal * scale) as u32, (vertical * scale) as u32).0
                    }).unwrap_or_else(|error| panic!("{error}"));
                }
                let application = UIApplication::sharedApplication(main_thread);
                let original_idle = application.isIdleTimerDisabled();
                let first = platform.prevent_idle_sleep("iOS probe first guard").await.expect("acquire idle-sleep guard");
                let second = platform.prevent_idle_sleep("iOS probe second guard").await.expect("acquire overlapping guard");
                assert!(application.isIdleTimerDisabled());
                drop(first);
                assert!(application.isIdleTimerDisabled());
                std::thread::spawn(move || drop(second)).join().expect("release an idle guard off the main thread");
                cx.background_executor().timer(Duration::from_millis(16)).await;
                assert_eq!(application.isIdleTimerDisabled(), original_idle);

                let mut frame = native_window.frame();
                frame.size.width -= 16.;
                frame.size.height -= 16.;
                let (baseline_generation, scale) = window.update(cx, |_, window, _| {
                    (window.presentation_facts().present_submitted_generation.expect("an initial submitted frame"), window.scale_factor())
                }).expect("capture the generation immediately before native resize");
                let (baseline_serial, _) = submitted_drawable(&view);
                assert!(baseline_serial > 0, "Core Submitted must have a real drawable submission");
                let expected_extent = ((frame.size.width as f32 * scale) as usize, (frame.size.height as f32 * scale) as usize);
                native_window.setFrame(frame);
                native_window.setNeedsLayout();
                native_window.layoutIfNeeded();
                loop {
                    let (facts, bounds) = window.update(cx, |_, window, _| (window.presentation_facts(), window.bounds())).expect("the resized window is live");
                    let (serial, extent) = submitted_drawable(&view);
                    if resize_was_submitted(baseline_generation, baseline_serial, facts.present_submitted_generation, serial, extent, expected_extent)
                        && (f32::from(bounds.size.width) - frame.size.width as f32).abs() < 0.5
                        && (f32::from(bounds.size.height) - frame.size.height as f32).abs() < 0.5
                    {
                        break;
                    }
                    assert!(Instant::now() < deadline, "resized Metal submission timed out: {facts:?}, {bounds:?}, serial={serial}, extent={extent:?}, expected={expected_extent:?}");
                    cx.background_executor().timer(Duration::from_millis(16)).await;
                }
                crate::interaction::run(cx, &platform, deadline).await;
                window.update(cx, |_, window, cx| window.remove_window(cx)).expect("retire through Core's original shutdown owner");
                while terminal.get() == 0 {
                    assert!(Instant::now() < deadline, "native close timed out");
                    cx.background_executor().timer(Duration::from_millis(16)).await;
                }
                assert_eq!(terminal.get(), 1, "publish native terminal exactly once");
                assert!(native_window.isHidden());
                assert!(native_window.rootViewController().is_none());
                assert!(native_window.windowScene().is_none());
                assert!(view.superview().is_none());
                let layer = view.layer();
                let layer = layer.downcast_ref::<CAMetalLayer>().expect("the UIKit backing-layer class remains stable");
                assert!(layer.device().is_none(), "retired UIKit layers must not retain the renderer's Metal device");
                view.setNeedsLayout();
                view.layoutIfNeeded();
                assert!(layer.device().is_none(), "retirement-tail layout cannot revive renderer ownership");
                assert!(layer.nextDrawable().is_none(), "a retired layer cannot acquire another drawable");
                println!("IOS NATIVE WINDOWING OK: public exact-generation activation, idle and checked-out IME cleanup, inert native presentation, six pixel oracles, resized drawable extent, idle guards and renderer-before-native retirement");
                std::io::stdout().flush().expect("flush native probe result");
                std::process::exit(0);
            }).detach();
        });
    }
}
