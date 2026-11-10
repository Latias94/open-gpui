//! Independent consumers of native window services at Zed target
//! 635026fabc38440fce5e1767d125026a17b12012. The baseline checks types in uninvoked
//! closures. The optional ordinary App control uses bench-support without test-support;
//! it qualifies unsupported headless behavior, not native OS restoration or attention.

use open_gpui::{PlatformWindow, Window};

#[test]
fn downstream_native_window_service_calls_are_type_only() {
    let _: fn(&Window) -> Option<Vec<u8>> = Window::native_window_state;
    let _: fn(&Window, &[u8]) = Window::restore_native_window_state;
    let _: fn(&Window) = Window::request_attention;
    let _window_type_only = |window: &Window, state: Vec<u8>| -> Option<Vec<u8>> {
        let snapshot = window.native_window_state();
        window.restore_native_window_state(&state);
        drop(state);
        window.request_attention();
        snapshot
    };
    let _backend_type_only = |window: &dyn PlatformWindow, state: &[u8]| -> Option<Vec<u8>> {
        window.restore_native_window_state(state);
        window.request_attention();
        window.native_window_state()
    };
}

#[cfg(feature = "bench-support")]
#[test]
fn downstream_ordinary_window_executes_unsupported_native_state_and_attention_calls() {
    use open_gpui::{
        App, AppContext as _, Application, Empty, NoopTextSystem, QuitMode, WindowOptions,
        bench_platform,
    };
    use std::sync::Arc;

    let app = Application::with_platform(bench_platform(None, Arc::new(NoopTextSystem)))
        .with_quit_mode(QuitMode::Explicit)
        .run_embedded(|_| {});
    let handle = app
        .update(|app| {
            assert!(!app.is_test());
            assert!(!app.background_executor().is_test());
            app.open_window(
                WindowOptions {
                    show: false,
                    focus_on_appearing: false,
                    ..Default::default()
                },
                |_, app| app.new(|_| Empty),
            )
        })
        .expect("the ordinary headless platform must open a Window");

    let initial_bounds = app
        .update(|app| {
            app.update_window(handle.into(), |_, window, _| {
                assert_eq!(window.native_window_state(), None);
                let initial_bounds = window.bounds();
                let caller_buffer = vec![0x00, 0xff, 0x80, 0x01];
                window.restore_native_window_state(&caller_buffer);
                drop(caller_buffer);
                window.restore_native_window_state(&[]);
                window.request_attention();
                window.request_attention();
                assert_eq!(window.native_window_state(), None);
                initial_bounds
            })
        })
        .expect("ordinary public native-service calls must execute without test-support");

    app.update(|app| {
        app.update_window(handle.into(), |_, window, app| {
            assert_eq!(window.native_window_state(), None);
            assert_eq!(window.bounds(), initial_bounds);
            window.remove_window(app);
        })
    })
    .expect("unsupported restoration must leave the ordinary Window usable and closable");
    app.update(|app| {
        assert!(app.windows().is_empty());
    });
    app.update(App::shutdown);
}
