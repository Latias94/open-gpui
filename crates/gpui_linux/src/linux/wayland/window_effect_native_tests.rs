use super::*;
use open_gpui::{
    AppContext as _, Application, AsyncApp, Context, GraphicalEnvironment, IntoElement, QuitMode,
    Render, Window, WindowOptions, WindowingModes, WindowingRequest,
};
use std::{
    env,
    process::{Command, Stdio},
    time::Duration,
};

const CHILD: &str = "GPUI_WAYLAND_BACKGROUND_EFFECT_PROBE";
const MARKER: &str = "NATIVE_WAYLAND_BACKGROUND_EFFECTS_COMPLETE";

struct Blank;

impl Render for Blank {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        open_gpui::Empty
    }
}

#[test]
#[ignore = "creates native Wayland surfaces; requires GPUI_LINUX_TEST_WAYLAND_DISPLAY and WGPU"]
fn creation_refresh_and_late_teardown_use_the_same_native_owner() {
    if env::var_os(CHILD).is_none() {
        let test = format!(
            "{}::creation_refresh_and_late_teardown_use_the_same_native_owner",
            module_path!().split_once("::").unwrap().1
        );
        #[allow(
            clippy::disallowed_methods,
            reason = "the native effect probe runs in an isolated child with a bounded lifetime"
        )]
        let mut child = Command::new(env::current_exe().unwrap())
            .args(["--exact", &test, "--ignored", "--nocapture"])
            .env(CHILD, "1")
            .env_remove("WAYLAND_SOCKET")
            .env_remove("XDG_ACTIVATION_TOKEN")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start the native Wayland background effect probe");
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            if Instant::now() >= deadline {
                child.kill().ok();
                let output = child.wait_with_output().unwrap();
                panic!(
                    "native effect probe timed out\nstdout:\n{}\nstderr:\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "native effect probe failed: {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains(MARKER));
        for line in stdout.lines().filter(|line| line.starts_with("NATIVE_")) {
            println!("{line}");
        }
        return;
    }

    let detected = GraphicalEnvironment::detect();
    let display = env::var_os("GPUI_LINUX_TEST_WAYLAND_DISPLAY")
        .or(detected.wayland_display)
        .expect("supply GPUI_LINUX_TEST_WAYLAND_DISPLAY or WAYLAND_DISPLAY");
    let environment = GraphicalEnvironment {
        wayland_display: Some(display),
        xdg_runtime_dir: detected.xdg_runtime_dir,
        ..GraphicalEnvironment::default()
    };
    let platform = crate::linux::linux_platform_with_startup(WindowingModes::all(), None, None);
    let completed = Rc::new(Cell::new(false));
    let outcome = Rc::new(RefCell::new(None));
    Application::with_platform(platform)
        .with_quit_mode(QuitMode::Explicit)
        .with_windowing(WindowingRequest::Headless)
        .run({
            let completed = completed.clone();
            let outcome = outcome.clone();
            move |cx| {
                cx.spawn(async move |cx| {
                    *outcome.borrow_mut() = Some(roundtrip(cx, environment).await);
                    completed.set(true);
                    cx.update(|cx| cx.quit());
                })
                .detach();
            }
        });
    assert!(
        completed.get(),
        "the native effect lifecycle did not finish"
    );
    outcome
        .borrow_mut()
        .take()
        .unwrap()
        .expect("native Wayland background effect lifecycle");
    println!("{MARKER}");
}

async fn roundtrip(cx: &mut AsyncApp, environment: GraphicalEnvironment) -> anyhow::Result<()> {
    cx.update(|cx| cx.request_windowing(WindowingRequest::Windowed(environment)))
        .await?;
    for (allow_ext, allow_kde, close_in_resize) in [
        (true, true, false),
        (false, true, false),
        (true, false, false),
        (false, false, false),
        (true, true, true),
    ] {
        let created = Rc::new(RefCell::new(None));
        CREATION_OBSERVER.with_borrow_mut(|observer| {
            *observer = Some(Box::new({
                let created = created.clone();
                move |window| {
                    let mut state = window.borrow_mut();
                    assert!(state.background_effects.is_none());
                    assert!(state.kde_blur.is_none());
                    assert_eq!(state.client_corner_radii, Corners::default());
                    assert_eq!(
                        state.background_appearance,
                        WindowBackgroundAppearance::Opaque
                    );
                    if !allow_ext {
                        state.globals.background_effects_manager = None;
                    }
                    if !allow_kde {
                        state.globals.kde_blur_manager = None;
                    }
                    *created.borrow_mut() = Some(window.0.downgrade());
                }
            }));
        });
        let opened = cx.update(|cx| {
            cx.open_window(
                WindowOptions {
                    show: false,
                    window_background: WindowBackgroundAppearance::Blurred,
                    window_decorations: Some(WindowDecorations::Client),
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(25.0), px(35.0)),
                        size(px(160.0), px(100.0)),
                    ))),
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Blank),
            )
        });
        CREATION_OBSERVER.with_borrow_mut(|observer| *observer = None);
        let window = opened?;
        let native = created
            .borrow_mut()
            .take()
            .expect("observe the actual fork window constructor")
            .upgrade()
            .expect("the newly created native window must be live");
        let (ext, kde) = {
            let state = native.state.borrow();
            assert_eq!(
                state.requested_background_appearance,
                WindowBackgroundAppearance::Blurred,
                "a capability downgrade must retain the original request"
            );
            (
                state.globals.background_effects_manager.is_some()
                    && state.globals.background_effects_blur.get(),
                state.globals.kde_blur_manager.is_some(),
            )
        };
        let expected = if ext || kde {
            WindowBackgroundAppearance::Blurred
        } else {
            WindowBackgroundAppearance::Transparent
        };
        cx.update(|cx| {
            window.update(cx, |_, window, _| {
                assert_eq!(window.platform_facts().background_appearance, expected);
                assert!(!window.presentation_facts().native_visible);
            })
        })?;
        if allow_ext && allow_kde && !close_in_resize {
            for (requested, observed) in [
                (
                    WindowBackgroundAppearance::Opaque,
                    WindowBackgroundAppearance::Opaque,
                ),
                (WindowBackgroundAppearance::Blurred, expected),
            ] {
                native.state.borrow_mut().requested_background_appearance = requested;
                native.background_effects_changed();
                cx.update(|_| ());
                cx.update(|cx| {
                    window.update(cx, |_, window, _| {
                        assert_eq!(window.platform_facts().background_appearance, observed);
                        assert_eq!(
                            window.window_capabilities().mutations.alpha,
                            open_gpui::WindowMutationSupport::CreationOnly,
                        );
                        if observed == WindowBackgroundAppearance::Opaque {
                            assert!(matches!(
                                window.set_background_appearance(
                                    WindowBackgroundAppearance::Transparent
                                ),
                                open_gpui::WindowMutationDispatch::Unsupported,
                            ));
                        }
                    })
                })?;
            }
            println!("NATIVE_WAYLAND_BACKGROUND_FACT_OBSERVED alpha=CreationOnly");
        }
        assert_effects(&native, ext, kde);
        let original_effects = {
            let state = native.state.borrow();
            (state.background_effects.clone(), state.kde_blur.clone())
        };
        let radii = Corners {
            top_left: px(4.0),
            top_right: px(8.0),
            bottom_left: px(12.0),
            bottom_right: px(16.0),
        };
        cx.update(|cx| {
            window.update(cx, |_, window, _| {
                window.set_client_inset(px(3.0));
                window.set_client_corner_radii(radii);
            })
        })?;
        let input_region = vec![Bounds::new(
            point(px(8.0), px(9.0)),
            size(px(20.0), px(30.0)),
        )];
        {
            let mut state = native.state.borrow_mut();
            state.input_region = Some(input_region.clone());
            state.apply_input_region();
        }
        native.resize(size(px(180.0), px(120.0)));
        {
            let state = native.state.borrow();
            assert_eq!(state.bounds.size, size(px(180.0), px(120.0)));
            assert_eq!(state.client_corner_radii, radii);
            assert_eq!(state.client_inset, Some(px(3.0)));
            assert_eq!(state.input_region.as_ref(), Some(&input_region));
            assert_eq!(state.background_effects, original_effects.0);
            assert_eq!(state.kde_blur, original_effects.1);
        }
        native.state.borrow_mut().requested_background_appearance =
            WindowBackgroundAppearance::Transparent;
        update_window(native.state.borrow_mut());
        assert!(native.state.borrow().background_effects.is_none());
        assert!(native.state.borrow().kde_blur.is_none());
        assert_destroyed(&original_effects);
        native.state.borrow_mut().requested_background_appearance =
            WindowBackgroundAppearance::Blurred;
        update_window(native.state.borrow_mut());
        assert_effects(&native, ext, kde);
        let final_effects = {
            let state = native.state.borrow();
            (state.background_effects.clone(), state.kde_blur.clone())
        };
        if close_in_resize {
            let previous_resize = native.callbacks.borrow_mut().resize.take();
            let state = Rc::downgrade(&native.state);
            native.callbacks.borrow_mut().resize = Some(Rc::new(move |_, _| {
                let state = state.upgrade().expect("resize retains its exact state");
                state.borrow_mut().requested_background_appearance =
                    WindowBackgroundAppearance::Transparent;
                update_window(state.borrow_mut());
                let mut state = state.borrow_mut();
                state.requested_background_appearance = WindowBackgroundAppearance::Blurred;
                state.quiesce_input();
            }));
            native.resize(size(px(190.0), px(130.0)));
            native.callbacks.borrow_mut().resize = previous_resize;
            assert!(!native.is_live());
            assert!(native.state.borrow().background_effects.is_none());
            assert!(native.state.borrow().kde_blur.is_none());
            assert_destroyed(&final_effects);
        }
        cx.update(|cx| window.update(cx, |_, window, cx| window.remove_window(cx)))?;
        let deadline = Instant::now() + Duration::from_secs(10);
        while native.state.borrow().surface.is_alive() {
            anyhow::ensure!(
                Instant::now() < deadline,
                "the exact native surface did not retire"
            );
            cx.background_executor()
                .timer(Duration::from_millis(10))
                .await;
        }
        assert_destroyed(&final_effects);
        assert!(!native.is_live());
        let old_bounds = native.state.borrow().bounds;
        let old_scale = native.state.borrow().scale;
        native.resize(size(px(200.0), px(140.0)));
        native.rescale(old_scale + 1.0);
        update_window(native.state.borrow_mut());
        {
            let state = native.state.borrow();
            assert_eq!(state.bounds, old_bounds);
            assert_eq!(state.scale, old_scale);
            assert!(state.background_effects.is_none());
            assert!(state.kde_blur.is_none());
            assert_eq!(state.input_region.as_ref(), Some(&input_region));
        }
        println!(
            "NATIVE_WAYLAND_BACKGROUND_EFFECT_OWNER allow_ext={allow_ext} allow_kde={allow_kde} available_ext={ext} available_kde={kde} close_in_resize={close_in_resize} appearance={expected:?}"
        );
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match cx
            .update(|cx| cx.request_windowing(WindowingRequest::Headless))
            .await
        {
            Ok(()) => break,
            Err(error) => {
                anyhow::ensure!(
                    Instant::now() < deadline && format!("{error:#}").contains("retiring"),
                    "native effect owners prevented detach: {error:#}"
                );
                cx.background_executor()
                    .timer(Duration::from_millis(10))
                    .await;
            }
        }
    }
    Ok(())
}

fn assert_effects(native: &WaylandWindowStatePtr, ext: bool, kde: bool) {
    let state = native.state.borrow();
    assert_eq!(state.background_effects.is_some(), ext);
    assert_eq!(state.kde_blur.is_some(), !ext && kde);
    assert!(state.is_transparent());
}

fn assert_destroyed(
    effects: &(
        Option<ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1>,
        Option<org_kde_kwin_blur::OrgKdeKwinBlur>,
    ),
) {
    assert!(effects.0.as_ref().is_none_or(|effect| !effect.is_alive()));
    assert!(effects.1.as_ref().is_none_or(|effect| !effect.is_alive()));
}
