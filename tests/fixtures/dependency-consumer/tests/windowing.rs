//! Independent consumers of Zed f37989fbdf840b308892af66516c2b9c3ef3853a at fixed
//! target 635026fabc38440fce5e1767d125026a17b12012, with f495dd display refresh.
//! Ordinary benchmark-platform Tasks exercise unsupported switching without test-support.
//! Native fixed-headless controls never enter a platform loop on the test runner.

use open_gpui::{
    ActivationPolicy, App, Application, GraphicalEnvironment, Platform, Result, Task,
    WindowingRequest,
};
use std::process::Command;

#[test]
fn downstream_windowing_api_and_facades_are_available_without_optional_features() {
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
    let _: fn(&GraphicalEnvironment) -> Option<String> = GraphicalEnvironment::desktop_environment;
    let _: fn() -> Application = open_gpui_platform::application;
    let _: fn() -> Application = open_gpui_platform::headless;
    assert_eq!(ActivationPolicy::default(), ActivationPolicy::Regular);

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        let _: fn(open_gpui::WindowingModes) -> Application = open_gpui_platform::linux;
        let allowed = open_gpui::WindowingModes::HEADLESS | open_gpui::WindowingModes::X11;
        assert!(allowed.contains(open_gpui::WindowingModes::HEADLESS));
        assert!(!allowed.contains(open_gpui::WindowingModes::WAYLAND));
    }
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
#[test]
fn downstream_display_environment_is_detected_and_applied_only_to_child_commands() {
    use std::{collections::BTreeMap, ffi::OsString};

    let detected = GraphicalEnvironment::detect();
    assert_eq!(
        detected.wayland_display,
        std::env::var_os("WAYLAND_DISPLAY")
    );
    assert_eq!(detected.x11_display, std::env::var_os("DISPLAY"));
    assert_eq!(
        detected.xdg_runtime_dir,
        std::env::var_os("XDG_RUNTIME_DIR")
    );
    assert_eq!(
        detected.current_desktop,
        std::env::var_os("XDG_CURRENT_DESKTOP")
    );
    assert!(detected.activation_token.is_none());

    let environment = GraphicalEnvironment {
        wayland_display: None,
        x11_display: Some(":consumer-会话".into()),
        xdg_runtime_dir: Some("/tmp/open-gpui-consumer-session".into()),
        current_desktop: Some("consumer:GNOME".into()),
        activation_token: Some("attach-only-token".into()),
    };
    assert_eq!(
        environment.desktop_environment().as_deref(),
        Some("consumer:GNOME")
    );
    let mut command = Command::new("open-gpui-unstarted-consumer-control");
    command
        .env("WAYLAND_DISPLAY", "old-wayland")
        .env("DISPLAY", ":old")
        .env("XDG_RUNTIME_DIR", "/old-runtime")
        .env("XDG_CURRENT_DESKTOP", "child:KDE")
        .env("XDG_ACTIVATION_TOKEN", "child-owned-token")
        .env("OPEN_GPUI_UNRELATED", "preserved");

    for _ in 0..2 {
        environment.apply_to(&mut command);
        assert_eq!(
            command
                .get_envs()
                .map(|(name, value)| (name.to_owned(), value.map(ToOwned::to_owned)))
                .collect::<BTreeMap<_, _>>(),
            BTreeMap::from([
                (OsString::from("WAYLAND_DISPLAY"), None),
                (OsString::from("DISPLAY"), environment.x11_display.clone()),
                (
                    OsString::from("XDG_RUNTIME_DIR"),
                    environment.xdg_runtime_dir.clone(),
                ),
                (
                    OsString::from("XDG_ACTIVATION_TOKEN"),
                    Some("child-owned-token".into()),
                ),
                (
                    OsString::from("XDG_CURRENT_DESKTOP"),
                    Some("child:KDE".into()),
                ),
                (
                    OsString::from("OPEN_GPUI_UNRELATED"),
                    Some("preserved".into())
                ),
            ])
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
    assert_eq!(
        cleared.get(&OsString::from("XDG_CURRENT_DESKTOP")),
        Some(&Some(OsString::from("child:KDE")))
    );
}

#[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
#[test]
fn downstream_graphical_environment_preserves_explicit_child_environment() {
    let environment = GraphicalEnvironment::detect();
    assert_eq!(environment.desktop_environment(), None);
    #[cfg(target_os = "windows")]
    let environment = {
        assert!(
            environment.session_id.is_some(),
            "the actual Windows session must be detected"
        );
        GraphicalEnvironment {
            session_id: Some(u32::MAX),
        }
    };
    let mut command = Command::new("open-gpui-unstarted-consumer-control");
    command
        .env("DISPLAY", "child-owned-display")
        .env("XDG_ACTIVATION_TOKEN", "child-owned-token")
        .env("XDG_CURRENT_DESKTOP", "child:KDE")
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

#[cfg(any(target_os = "windows", feature = "bench-support"))]
fn failure(task: Task<Result<()>>) -> String {
    use std::{
        future::Future,
        pin::pin,
        task::{Context as TaskContext, Poll, Waker},
    };

    let mut task = pin!(task);
    match task
        .as_mut()
        .poll(&mut TaskContext::from_waker(Waker::noop()))
    {
        Poll::Ready(result) => result
            .expect_err("an unsupported request must return an error")
            .to_string(),
        Poll::Pending => panic!("an unsupported request must return a ready Task"),
    }
}

#[cfg(target_os = "windows")]
#[test]
fn downstream_native_fixed_headless_platform_cannot_become_switchable() {
    use open_gpui::{OperatingSystem, PlatformTarget};
    use std::sync::Arc;

    let platform = open_gpui_platform::current_platform(true);
    assert_eq!(platform.platform_info().os, OperatingSystem::Windows);
    assert_eq!(platform.platform_info().target, PlatformTarget::Native);
    assert!(!platform.background_executor().is_test());
    let text_system = platform.text_system();

    for _ in 0..3 {
        for initial in [
            WindowingRequest::Windowed(GraphicalEnvironment::detect()),
            WindowingRequest::Headless,
        ] {
            platform.set_initial_windowing(initial);
            for policy in [ActivationPolicy::Accessory, ActivationPolicy::Regular] {
                platform.set_activation_policy(policy);
            }
            for request in [
                WindowingRequest::Headless,
                WindowingRequest::Windowed(GraphicalEnvironment::detect()),
            ] {
                assert_eq!(
                    failure(platform.request_windowing(request)),
                    "a platform created headless can't switch windowing modes"
                );
                assert!(platform.graphical_environment().is_none());
                assert!(Arc::ptr_eq(&text_system, &platform.text_system()));
            }
        }
    }
}

#[cfg(feature = "bench-support")]
mod runtime {
    use super::*;
    use open_gpui::{
        AppContext as _, Context, IntoElement, NoopTextSystem, ParentElement as _, QuitMode,
        Render, Window, WindowOptions, bench_platform, div,
    };
    use std::{rc::Rc, sync::Arc};

    struct RetainedRoot(usize);

    impl Render for RetainedRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(format!("consumer revision {}", self.0))
        }
    }

    #[test]
    fn downstream_ordinary_app_consumes_failed_tasks_and_reopens_the_same_retained_root() {
        let platform = bench_platform(None, Arc::new(NoopTextSystem));
        let app = Application::with_platform(Rc::clone(&platform))
            .with_windowing(WindowingRequest::Windowed(GraphicalEnvironment::detect()))
            .with_windowing(WindowingRequest::Headless)
            .with_quit_mode(QuitMode::Explicit)
            .run_embedded(|app| {
                assert!(!app.is_test());
                assert!(!app.background_executor().is_test());
                assert!(app.graphical_environment().is_none());
                assert!(app.windows().is_empty());
            });
        let root = app.update(|app| app.new(|_| RetainedRoot(0)));
        let identity = root.entity_id();
        let text_system = app.update(|app| Arc::clone(app.text_system()));
        let foreground_executor = platform.foreground_executor();
        let dispatcher = foreground_executor
            .dispatcher()
            .as_threaded()
            .expect("the ordinary benchmark platform must use its threaded dispatcher");

        for revision in 1..=3 {
            for request in [
                WindowingRequest::Headless,
                WindowingRequest::Windowed(GraphicalEnvironment::detect()),
            ] {
                assert_eq!(
                    failure(platform.request_windowing(request.clone())),
                    "this platform cannot switch between headless and windowed modes"
                );
                let task: Task<Result<()>> = app.update(|app| {
                    let app: &App = app;
                    app.request_windowing(request)
                });
                assert_eq!(
                    failure(task),
                    "this platform cannot switch between headless and windowed modes"
                );
            }

            app.update(|app| {
                app.set_activation_policy(ActivationPolicy::Accessory);
                app.set_activation_policy(ActivationPolicy::Regular);
                root.update(app, |view, _| view.0 = revision);
                assert!(app.graphical_environment().is_none());
                assert!(Arc::ptr_eq(&text_system, app.text_system()));
            });
            let handle = app
                .update(|app| {
                    app.open_window(
                        WindowOptions {
                            show: false,
                            focus_on_appearing: false,
                            ..Default::default()
                        },
                        |_, _| root.clone(),
                    )
                })
                .expect("a failed request must leave the ordinary App able to open a window");

            for request in [
                WindowingRequest::Headless,
                WindowingRequest::Windowed(GraphicalEnvironment::detect()),
            ] {
                assert_eq!(
                    failure(app.update(|app| app.request_windowing(request))),
                    "cannot switch windowing modes while windows are open"
                );
                app.update(|app| {
                    assert_eq!(app.windows(), [handle.into()]);
                    handle.update(app, |view, _, _| assert_eq!(view.0, revision))
                })
                .expect("the open window must remain usable after a refused switch");
            }

            app.update(|app| handle.update(app, |_, window, app| window.remove_window(app)))
                .expect("ordinary window closure must succeed with its root retained");
            dispatcher.run_until_idle();
            app.update(|app| {
                assert!(app.windows().is_empty());
                assert_eq!(root.entity_id(), identity);
                assert_eq!(root.read(app).0, revision);
                assert!(app.graphical_environment().is_none());
            });
            assert_eq!(
                failure(app.update(|app| app.request_windowing(WindowingRequest::Headless))),
                "this platform cannot switch between headless and windowed modes"
            );
        }
        app.update(App::shutdown);
        dispatcher.run_until_idle();
    }
}
