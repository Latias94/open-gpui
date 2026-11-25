use super::*;

#[test]
fn native_window_services_keep_unsupported_restore_and_queued_attention_lifetimes_exact() {
    discard_stale_quit_messages();
    let platform =
        Rc::new(WindowsPlatform::new_windowed_for_test().expect("native window services backend"));
    let pump = Rc::downgrade(&platform.inner);
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let handle = app
        .update_for_test(|app| {
            app.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), app)),
                    show: false,
                    focus_on_appearing: false,
                    ..WindowOptions::default()
                },
                |_, app| app.new(|_| Empty),
            )
        })
        .expect("hidden native services window should open");
    let hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("native services HWND should register")
        .as_raw();
    let native = platform
        .window_from_hwnd(hwnd)
        .expect("native services window should have its exact registered owner");
    assert!(unsafe { IsWindow(Some(hwnd)).as_bool() });
    assert!(!unsafe { IsWindowVisible(hwnd).as_bool() });
    assert_ne!(unsafe { GetForegroundWindow() }, hwnd);
    let native_lifetime = Rc::downgrade(&native);
    let cursor = app.update_for_test(|app| {
        app.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    let bounds = app.update_for_test(|app| {
        handle
            .update(app, |_, window, _| {
                assert_eq!(window.native_window_state(), None);
                let state = vec![0x00, 0xff, 0x80, 0x01];
                window.restore_native_window_state(&state);
                drop(state);
                assert_eq!(window.native_window_state(), None);
                assert!(!window.is_simple_fullscreen());
                window.toggle_simple_fullscreen();
                window.set_input_region(Some(&[Bounds::new(
                    point(px(2.0), px(3.0)),
                    size(px(24.0), px(18.0)),
                )]));
                window.set_input_region(Some(&[]));
                window.set_input_region(None);
                window.set_exclusive_zone(px(12.0));
                window.bounds()
            })
            .expect("unsupported native restore should accept its public request")
    });
    let diagnostics = app.update_for_test(|app| app.native_boundary_diagnostics(cursor));
    let restoration = diagnostics
        .terminal
        .iter()
        .filter(|diagnostic| {
            diagnostic.target == NativeBoundaryTarget::Window(handle.window_id())
                && diagnostic.kind
                    == NativeBoundaryKind::Command(
                        NativePlatformCommandKind::RestoreNativeWindowState,
                    )
        })
        .collect::<Vec<_>>();
    assert_eq!(restoration.len(), 1);
    assert_eq!(
        restoration[0].disposition,
        NativeBoundaryDisposition::Rejected
    );
    assert_eq!(restoration[0].domain_generation, None);
    let fullscreen = diagnostics
        .terminal
        .iter()
        .filter(|diagnostic| {
            diagnostic.target == NativeBoundaryTarget::Window(handle.window_id())
                && diagnostic.kind
                    == NativeBoundaryKind::Command(
                        NativePlatformCommandKind::ToggleSimpleFullscreen,
                    )
        })
        .collect::<Vec<_>>();
    assert_eq!(fullscreen.len(), 1);
    assert_eq!(
        fullscreen[0].disposition,
        NativeBoundaryDisposition::Rejected
    );
    assert_eq!(fullscreen[0].domain_generation, None);
    assert!(is_registered(&platform, hwnd));
    let registrations = platform.raw_window_handles.read().len();
    let popup = app
        .update_for_test(|app| {
            app.open_window(
                WindowOptions {
                    kind: WindowKind::AnchoredPopup(open_gpui::popup::PopupOptions {
                        parent: handle.into(),
                        anchor_rect: Bounds::new(point(px(2.0), px(3.0)), size(px(24.0), px(18.0))),
                        anchor: open_gpui::popup::PopupAnchor::BottomLeft,
                        gravity: open_gpui::popup::PopupGravity::BottomRight,
                        constraint_adjustment: open_gpui::popup::PopupConstraintAdjustment::SLIDE_X,
                        offset: point(px(0.0), px(4.0)),
                        grab: false,
                    }),
                    show: false,
                    focus_on_appearing: false,
                    ..WindowOptions::default()
                },
                |_, app| app.new(|_| Empty),
            )
        })
        .expect_err("unsupported native popup must fail before construction");
    assert!(popup.is::<open_gpui::popup::PopupNotSupportedError>());
    assert_eq!(platform.raw_window_handles.read().len(), registrations);
    assert_eq!(app.update_for_test(|app| app.windows().len()), 1);
    app.update_for_test(|app| {
        handle
            .update(app, |_, window, app| {
                assert_eq!(window.bounds(), bounds);
                assert!(!window.is_simple_fullscreen());
                window.request_attention();
                window.remove_window(app);
            })
            .expect("queued native attention should allow exact window retirement")
    });
    assert!(native.is_native_window_terminal());
    assert!(!unsafe { IsWindow(Some(hwnd)).as_bool() });
    assert!(!is_registered(&platform, hwnd));
    assert_eq!(
        Rc::strong_count(&native),
        1,
        "queued attention must not retain a retired native owner"
    );
    platform.inner.run_foreground_task();
    pump_messages_until_idle(&pump, "retired attention and unsupported restore settle");
    assert!(native.is_native_window_terminal());
    assert!(native_lifetime.upgrade().is_some());
    drop(native);
    assert!(native_lifetime.upgrade().is_none());
    assert!(app.update_for_test(|app| app.windows().is_empty()));
    drop(app);
    drop(platform);
    pump_messages_until_with_delay(
        &pump,
        "native service platform releases after queued attention",
        Duration::from_millis(1),
        || pump.upgrade().is_none(),
    );
}
