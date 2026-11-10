//! Ordinary consumers of Zed's 8e4e5a39eed932d943cb7a3e7078885b75d1ea54 API
//! at fixed target 635026fabc38440fce5e1767d125026a17b12012.
//! Windows exercises the actual headless native Platform without starting its loop.
//! The optional App/Window control uses the returning benchmark platform, not native
//! windows or GPUI test-support. Neither path qualifies macOS native titlebars.

use open_gpui::{App, Platform, WindowAppearance};

#[cfg(any(target_os = "windows", feature = "bench-support"))]
const REQUESTS: [Option<WindowAppearance>; 13] = [
    None,
    Some(WindowAppearance::Dark),
    Some(WindowAppearance::Dark),
    Some(WindowAppearance::VibrantLight),
    Some(WindowAppearance::VibrantLight),
    Some(WindowAppearance::VibrantDark),
    Some(WindowAppearance::VibrantDark),
    Some(WindowAppearance::Light),
    Some(WindowAppearance::Light),
    None,
    None,
    Some(WindowAppearance::Dark),
    None,
];

#[test]
fn downstream_shared_appearance_api_is_available_without_optional_features() {
    let _: fn(&App, Option<WindowAppearance>) = App::set_window_appearance;
    let _: fn(&App) -> WindowAppearance = App::window_appearance;
    let _: fn(&dyn Platform, Option<WindowAppearance>) = <dyn Platform>::set_window_appearance;
    let _: fn(&dyn Platform) -> WindowAppearance = <dyn Platform>::window_appearance;
}

#[cfg(target_os = "windows")]
#[test]
fn downstream_native_windows_appearance_requests_preserve_the_system_getter() {
    use open_gpui::{OperatingSystem, PlatformTarget};

    let platform = open_gpui_platform::current_platform(true);
    assert_eq!(platform.platform_info().os, OperatingSystem::Windows);
    assert_eq!(platform.platform_info().target, PlatformTarget::Native);
    assert!(!platform.background_executor().is_test());
    let initial = platform.window_appearance();

    for request in REQUESTS {
        platform.set_window_appearance(request);
        assert_eq!(platform.window_appearance(), initial, "{request:?}");
    }

    drop(platform);
}

#[cfg(feature = "bench-support")]
#[test]
fn downstream_ordinary_app_appearance_requests_preserve_the_headless_window() {
    use open_gpui::{
        AppContext as _, Application, Empty, NoopTextSystem, QuitMode, WindowOptions,
        bench_platform,
    };
    use std::{rc::Rc, sync::Arc};

    let platform = bench_platform(None, Arc::new(NoopTextSystem));
    let app = Application::with_platform(Rc::clone(&platform))
        .with_quit_mode(QuitMode::Explicit)
        .run_embedded(|_| {});
    let initial = app.update(|app| {
        assert!(!app.is_test());
        assert!(!app.background_executor().is_test());
        assert_eq!(app.window_appearance(), platform.window_appearance());
        app.window_appearance()
    });
    let window = app
        .update(|app| {
            app.open_window(
                WindowOptions {
                    show: false,
                    focus_on_appearing: false,
                    ..Default::default()
                },
                |_, app| app.new(|_| Empty),
            )
        })
        .expect("the ordinary benchmark platform must open a headless Window");

    for request in REQUESTS {
        app.update(|app| {
            let app: &App = app;
            app.set_window_appearance(request);
            assert_eq!(app.window_appearance(), initial, "{request:?}");
        });
        assert_eq!(platform.window_appearance(), initial, "{request:?}");
        app.update(|app| {
            app.update_window(window.into(), |_, window, _| {
                assert_eq!(window.appearance(), initial, "{request:?}");
            })
        })
        .expect("appearance requests must leave the headless Window usable");
    }

    app.update(|app| app.update_window(window.into(), |_, window, app| window.remove_window(app)))
        .expect("the ordinary headless Window must close");
    app.update(|app| {
        assert!(app.windows().is_empty());
        app.shutdown();
    });
}
