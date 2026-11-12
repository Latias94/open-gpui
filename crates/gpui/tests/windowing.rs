//! Public consumers of Zed f37989fbdf840b308892af66516c2b9c3ef3853a at fixed
//! target 635026fabc38440fce5e1767d125026a17b12012, including f495dd display refresh.
//! Default-platform failures exercise real returned Tasks without simulating a live switch.

use open_gpui::{
    ActivationPolicy, App, Application, GraphicalEnvironment, Platform, Result, Task,
    WindowingRequest,
};
use std::process::Command;

#[test]
fn public_windowing_api_accepts_owned_requests_without_optional_features() {
    let _: fn(Application, WindowingRequest) -> Application = Application::with_windowing;
    let _: fn(&App, WindowingRequest) -> Task<Result<()>> = App::request_windowing;
    let _: fn(&App) -> Option<GraphicalEnvironment> = App::graphical_environment;
    let _: fn(&mut App, ActivationPolicy) = App::set_activation_policy;
    let _: fn(&dyn Platform, WindowingRequest) = <dyn Platform>::set_initial_windowing;
    let _: fn(&dyn Platform, WindowingRequest) -> Task<Result<()>> =
        <dyn Platform>::request_windowing;
    let _: fn(&dyn Platform) -> Option<GraphicalEnvironment> =
        <dyn Platform>::graphical_environment;
    let _: fn(&dyn Platform, ActivationPolicy) = <dyn Platform>::set_activation_policy;
    let _: fn(&GraphicalEnvironment, &mut Command) = GraphicalEnvironment::apply_to;
    assert_eq!(ActivationPolicy::default(), ActivationPolicy::Regular);

    let request = WindowingRequest::Windowed(GraphicalEnvironment::detect());
    assert!(matches!(request.clone(), WindowingRequest::Windowed(_)));
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
#[test]
fn public_display_environment_detection_reads_the_process_without_taking_its_token() {
    let environment = GraphicalEnvironment::detect();
    assert_eq!(
        environment.wayland_display,
        std::env::var_os("WAYLAND_DISPLAY")
    );
    assert_eq!(environment.x11_display, std::env::var_os("DISPLAY"));
    assert_eq!(
        environment.xdg_runtime_dir,
        std::env::var_os("XDG_RUNTIME_DIR")
    );
    assert!(environment.activation_token.is_none());

    let modes = open_gpui::WindowingModes::all();
    assert!(modes.contains(open_gpui::WindowingModes::HEADLESS));
    assert!(modes.contains(open_gpui::WindowingModes::X11));
    assert!(modes.contains(open_gpui::WindowingModes::WAYLAND));
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
#[test]
fn public_display_environment_overrides_and_removals_belong_only_to_the_child_command() {
    use std::{collections::BTreeMap, ffi::OsString};

    let environment = GraphicalEnvironment {
        wayland_display: Some("wayland-public-消费者".into()),
        x11_display: None,
        xdg_runtime_dir: Some("/tmp/open-gpui-public-session".into()),
        current_desktop: None,
        activation_token: Some("connection-only-token".into()),
    };
    let mut command = Command::new("open-gpui-unstarted-windowing-control");
    command
        .env("WAYLAND_DISPLAY", "old-wayland")
        .env("DISPLAY", ":old")
        .env("XDG_RUNTIME_DIR", "/old-runtime")
        .env("XDG_ACTIVATION_TOKEN", "child-owned-token")
        .env("OPEN_GPUI_UNRELATED", "preserved");

    for _ in 0..2 {
        environment.apply_to(&mut command);
        let overrides = command
            .get_envs()
            .map(|(name, value)| (name.to_owned(), value.map(ToOwned::to_owned)))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            overrides.get(&OsString::from("WAYLAND_DISPLAY")),
            Some(&environment.wayland_display)
        );
        assert_eq!(overrides.get(&OsString::from("DISPLAY")), Some(&None));
        assert_eq!(
            overrides.get(&OsString::from("XDG_RUNTIME_DIR")),
            Some(&environment.xdg_runtime_dir)
        );
        assert_eq!(
            overrides.get(&OsString::from("XDG_ACTIVATION_TOKEN")),
            Some(&Some(OsString::from("child-owned-token")))
        );
        assert_eq!(
            overrides.get(&OsString::from("OPEN_GPUI_UNRELATED")),
            Some(&Some(OsString::from("preserved")))
        );
    }

    GraphicalEnvironment::default().apply_to(&mut command);
    let cleared = command
        .get_envs()
        .map(|(name, value)| (name.to_owned(), value.map(ToOwned::to_owned)))
        .collect::<BTreeMap<_, _>>();
    for name in ["WAYLAND_DISPLAY", "DISPLAY", "XDG_RUNTIME_DIR"] {
        assert_eq!(cleared.get(&OsString::from(name)), Some(&None));
    }
    assert_eq!(
        cleared.get(&OsString::from("XDG_ACTIVATION_TOKEN")),
        Some(&Some(OsString::from("child-owned-token")))
    );
    GraphicalEnvironment {
        wayland_display: Some(OsString::new()),
        ..Default::default()
    }
    .apply_to(&mut command);
    assert!(command.get_envs().any(|(name, value)| {
        name == "WAYLAND_DISPLAY" && value == Some(std::ffi::OsStr::new(""))
    }));
}

#[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
#[test]
fn public_graphical_environment_leaves_child_command_overrides_unchanged() {
    let environment = GraphicalEnvironment::detect();
    #[cfg(target_os = "windows")]
    assert!(
        environment.session_id.is_some(),
        "Windows must detect its actual session"
    );

    let mut command = Command::new("open-gpui-unstarted-windowing-control");
    command
        .env("DISPLAY", "child-owned-display")
        .env("XDG_ACTIVATION_TOKEN", "child-owned-token")
        .env_remove("WAYLAND_DISPLAY");
    let expected = command
        .get_envs()
        .map(|(name, value)| (name.to_owned(), value.map(ToOwned::to_owned)))
        .collect::<Vec<_>>();
    for _ in 0..2 {
        environment.apply_to(&mut command);
        assert_eq!(
            command
                .get_envs()
                .map(|(name, value)| (name.to_owned(), value.map(ToOwned::to_owned)))
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[cfg(feature = "test-support")]
mod runtime {
    use super::*;
    use open_gpui::{
        AppContext as _, Context, IntoElement, ParentElement as _, QuitMode, Render,
        TestAppContext, Window, div,
    };
    use std::{
        future::Future,
        pin::pin,
        task::{Context as TaskContext, Poll, Waker},
    };

    fn failure(task: Task<Result<()>>) -> String {
        let mut task = pin!(task);
        match task
            .as_mut()
            .poll(&mut TaskContext::from_waker(Waker::noop()))
        {
            Poll::Ready(result) => result
                .expect_err("the default platform must refuse switching")
                .to_string(),
            Poll::Pending => panic!("the default refusal must be a ready Task"),
        }
    }

    #[test]
    fn public_unsupported_tasks_complete_while_the_same_app_and_entity_remain_usable() {
        let cx = TestAppContext::single();
        let state = cx.update(|app| {
            app.set_quit_mode(QuitMode::Explicit);
            app.new(|_| 0usize)
        });
        let identity = state.entity_id();

        for revision in 1..=3 {
            for request in [
                WindowingRequest::Headless,
                WindowingRequest::Windowed(GraphicalEnvironment::detect()),
            ] {
                let task: Task<Result<()>> = cx.read(|app: &App| app.request_windowing(request));
                assert_eq!(
                    failure(task),
                    "this platform cannot switch between headless and windowed modes"
                );
                assert!(cx.read(App::graphical_environment).is_none());
            }
            cx.update(|app| {
                app.set_activation_policy(ActivationPolicy::Accessory);
                app.set_activation_policy(ActivationPolicy::Regular);
                state.update(app, |value, _| *value = revision);
            });
            assert_eq!(cx.read(|app| *state.read(app)), revision);
            assert_eq!(state.entity_id(), identity);
        }
        cx.quit();
    }

    struct RetainedRoot(usize);

    impl Render for RetainedRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(format!("retained revision {}", self.0))
        }
    }

    #[test]
    fn public_open_windows_refuse_switching_with_a_retained_root_and_recover_after_close() {
        let mut cx = TestAppContext::single();
        let root = cx.update(|app| {
            app.set_quit_mode(QuitMode::Explicit);
            app.new(|_| RetainedRoot(0))
        });
        let identity = root.entity_id();

        for revision in 1..=3 {
            let handle = cx
                .update(|app| app.open_window(Default::default(), |_, _| root.clone()))
                .expect("the same retained root must open after earlier refusals");
            for request in [
                WindowingRequest::Headless,
                WindowingRequest::Windowed(GraphicalEnvironment::detect()),
            ] {
                assert_eq!(
                    failure(cx.read(|app: &App| app.request_windowing(request))),
                    "cannot switch windowing modes while windows are open"
                );
                assert_eq!(cx.read(App::windows), [handle.into()]);
                handle
                    .update(&mut cx, |view, _, _| view.0 = revision)
                    .expect("a refused switch must leave the window usable");
                assert_eq!(cx.read(|app| root.read(app).0), revision);
            }
            handle
                .update(&mut cx, |_, window, app| window.remove_window(app))
                .expect("the retained root must not prevent window closure");
            cx.run_until_parked();
            assert!(cx.read(App::windows).is_empty());
            assert_eq!(root.entity_id(), identity);
            assert_eq!(cx.read(|app| root.read(app).0), revision);
            assert_eq!(
                failure(cx.read(|app: &App| app.request_windowing(WindowingRequest::Headless))),
                "this platform cannot switch between headless and windowed modes"
            );
        }
        cx.quit();
    }
}
