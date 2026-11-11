//! Ordinary dependency consumption of the platform-window factory boundary.
//! Signature closures are deliberately unexecuted. The optional headless case executes
//! a factory refusal; neither case qualifies native attachment, a host loop or GPU submission.

use open_gpui::{
    AnyWindowHandle, App, AppContext as _, Empty, Entity, PlatformDisplaySnapshot, PlatformWindow,
    Window, WindowHandle, WindowOpenError, WindowOpenFailureStage, WindowOptions, WindowParams,
};

#[test]
fn downstream_platform_window_factory_signature_and_snapshot_values_are_public() {
    type Factory = fn(
        AnyWindowHandle,
        WindowParams,
        PlatformDisplaySnapshot,
    ) -> anyhow::Result<Box<dyn PlatformWindow>>;
    type BuildRoot = fn(&mut Window, &mut App) -> Entity<Empty>;

    let _: fn(
        &mut App,
        WindowOptions,
        Factory,
        BuildRoot,
    ) -> Result<WindowHandle<Empty>, WindowOpenError> =
        App::open_window_with_platform_window::<Empty>;
    let _: fn(&WindowOpenError) -> WindowOpenFailureStage = WindowOpenError::stage;

    let _attach_type_only = |app: &mut App,
                             options: WindowOptions,
                             platform_window: Box<dyn PlatformWindow>|
     -> Result<WindowHandle<Empty>, WindowOpenError> {
        app.open_window_with_platform_window(
            options,
            move |handle, params, displays| {
                let _: AnyWindowHandle = handle;
                let _: WindowParams = params;
                let _: PlatformDisplaySnapshot = displays;
                Ok(platform_window)
            },
            |_, app| app.new(|_| Empty),
        )
    };

    let displays = PlatformDisplaySnapshot::try_new(Some(7), Vec::new(), None)
        .expect("an empty display publication must be constructible without a platform");
    assert_eq!(displays.generation(), Some(7));
    assert!(displays.displays().is_empty());
    assert!(displays.primary_display().is_none());
    assert!(PlatformDisplaySnapshot::try_new(Some(0), Vec::new(), None).is_none());
}

#[cfg(feature = "bench-support")]
#[test]
fn downstream_ordinary_headless_app_executes_factory_refusal_and_retry() {
    use open_gpui::{Application, NoopTextSystem, QuitMode, bench_platform};
    use std::{cell::Cell, io, sync::Arc};

    let app = Application::with_platform(bench_platform(None, Arc::new(NoopTextSystem::new())))
        .with_quit_mode(QuitMode::Explicit)
        .run_embedded(|_| {});
    let reserved = Cell::new(None);
    let calls = Cell::new(0);
    let root_called = Cell::new(false);
    let error = app
        .update(|app| {
            assert!(!app.is_test());
            assert!(!app.background_executor().is_test());
            app.open_window_with_platform_window(
                WindowOptions {
                    show: false,
                    focus_on_appearing: false,
                    ..Default::default()
                },
                |handle, params, displays| {
                    calls.set(calls.get() + 1);
                    reserved.set(Some(handle));
                    assert!(!params.show);
                    assert!(!params.focus_on_appearing);
                    assert!(displays.primary_display().is_some());
                    Err(
                        io::Error::new(io::ErrorKind::Unsupported, "no host window attached")
                            .into(),
                    )
                },
                |_, app| {
                    root_called.set(true);
                    app.new(|_| Empty)
                },
            )
        })
        .expect_err("a hostless factory must return its actual refusal");
    assert_eq!(calls.get(), 1);
    assert!(!root_called.get());
    assert_eq!(error.stage(), WindowOpenFailureStage::NativeCreateOrMap);
    let source = std::error::Error::source(&error)
        .and_then(|source| source.downcast_ref::<io::Error>())
        .expect("the factory's typed source must survive the public error boundary");
    assert_eq!(source.kind(), io::ErrorKind::Unsupported);

    let retry = app
        .update(|app| {
            assert!(app.windows().is_empty());
            assert!(app.window_profile(reserved.get().unwrap()).is_none());
            app.open_window_detailed(
                WindowOptions {
                    show: false,
                    focus_on_appearing: false,
                    ..Default::default()
                },
                |_, app| app.new(|_| Empty),
            )
        })
        .expect("factory refusal must preserve the default window-open path");
    assert_ne!(retry.window_id(), reserved.get().unwrap().window_id());
    app.update(|app| app.update_window(retry.into(), |_, window, app| window.remove_window(app)))
        .expect("the ordinary retry must close normally");
    app.update(|app| {
        assert!(app.windows().is_empty());
        app.shutdown();
    });
}
