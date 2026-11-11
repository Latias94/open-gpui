//! Independent consumers of native window controls at official Zed target
//! 635026fabc38440fce5e1767d125026a17b12012. Baseline closures are uninvoked;
//! bench-support executes an ordinary embedded App without test-support or a renderer.

use open_gpui::{
    AnyWindowHandle, App, Bounds, Corners, Entity, PendingInputStatus, Pixels, PlatformWindow,
    Window, WindowKind, WindowOptions, point,
    popup::{
        PopupAnchor, PopupConstraintAdjustment, PopupGravity, PopupNotSupportedError, PopupOptions,
    },
    px, size,
};

fn popup_options(parent: AnyWindowHandle, grab: bool) -> PopupOptions {
    PopupOptions {
        parent,
        anchor_rect: Bounds::new(point(px(12.0), px(16.0)), size(px(80.0), px(24.0))),
        anchor: PopupAnchor::BottomLeft,
        gravity: PopupGravity::BottomRight,
        constraint_adjustment: PopupConstraintAdjustment::SLIDE_X
            | PopupConstraintAdjustment::SLIDE_Y
            | PopupConstraintAdjustment::FLIP_X
            | PopupConstraintAdjustment::FLIP_Y
            | PopupConstraintAdjustment::RESIZE_X
            | PopupConstraintAdjustment::RESIZE_Y,
        offset: point(px(2.0), px(4.0)),
        grab,
    }
}

#[test]
fn downstream_native_window_controls_and_complete_popup_schema_are_type_only() {
    let _: fn(&Window, Option<&[Bounds<Pixels>]>) = Window::set_input_region;
    let _: fn(&Window, Pixels) = Window::set_exclusive_zone;
    let _: fn(&Window) = Window::toggle_simple_fullscreen;
    let _: fn(&Window) -> bool = Window::is_simple_fullscreen;
    let _: fn(&mut Window, Corners<Pixels>) = Window::set_client_corner_radii;
    let _: fn(&Window) -> bool = Window::is_resizable;
    let _: fn(&Window) -> bool = Window::is_minimizable;
    let _: fn(&Window) -> bool = Window::has_active_prompt;
    let _: fn(&Window) -> Option<PendingInputStatus<'_>> = Window::pending_input;
    let _: fn(&mut Window, &Entity<()>, bool, &mut App) -> bool =
        Window::set_pending_input_timeout_paused;
    let _window_type_only = |window: &Window, region: Vec<Bounds<Pixels>>| -> bool {
        window.set_input_region(Some(&region));
        drop(region);
        window.set_input_region(Some(&[]));
        window.set_input_region(None);
        window.set_exclusive_zone(px(24.0));
        window.toggle_simple_fullscreen();
        window.is_simple_fullscreen()
    };
    let _backend_type_only = |window: &dyn PlatformWindow, region: &[Bounds<Pixels>]| -> bool {
        window.set_client_corner_radii(Corners::all(px(6.0)));
        window.set_input_region(Some(region));
        window.set_input_region(Some(&[]));
        window.set_input_region(None);
        window.set_exclusive_zone(px(-1.0));
        window.toggle_simple_fullscreen();
        window.is_simple_fullscreen()
    };
    let _popup_type_only = |parent: AnyWindowHandle| WindowOptions {
        kind: WindowKind::AnchoredPopup(popup_options(parent, true)),
        ..Default::default()
    };
    let _error_type_only =
        |error: PopupNotSupportedError| -> Box<dyn std::error::Error + Send + Sync> {
            Box::new(error)
        };
    #[cfg(all(target_os = "linux", feature = "wayland"))]
    {
        let _: fn(&Window, open_gpui::layer_shell::Anchor) = Window::set_exclusive_edge;
        let _edge_type_only = |window: &dyn PlatformWindow,
                               edge: open_gpui::layer_shell::Anchor| {
            window.set_exclusive_edge(edge);
        };
    }
}

#[cfg(feature = "bench-support")]
#[test]
fn downstream_ordinary_app_executes_noop_controls_and_refuses_native_popups() {
    use open_gpui::{
        App, AppContext as _, Application, Empty, NoopTextSystem, QuitMode, bench_platform,
    };
    use std::{cell::Cell, sync::Arc};

    let app = Application::with_platform(bench_platform(None, Arc::new(NoopTextSystem)))
        .with_quit_mode(QuitMode::Explicit)
        .run_embedded(|_| {});
    let parent = app
        .update(|app| {
            assert!(!app.is_test());
            assert!(!app.background_executor().is_test());
            app.open_window(
                WindowOptions {
                    show: false,
                    focus_on_appearing: false,
                    is_resizable: false,
                    is_minimizable: true,
                    ..Default::default()
                },
                |_, app| app.new(|_| Empty),
            )
        })
        .expect("the ordinary embedded App must open an actual headless parent");
    let original = app
        .update(|app| {
            app.update_window(parent.into(), |_, window, _| {
                let original = (window.bounds(), window.is_fullscreen());
                assert!(!window.is_simple_fullscreen());
                assert!(!window.is_resizable());
                assert!(window.is_minimizable());
                assert!(!window.has_active_prompt());
                assert!(window.pending_input().is_none());
                let mut region = vec![
                    Bounds::new(point(px(-2.5), px(3.0)), size(px(40.0), px(30.0))),
                    Bounds::new(point(px(50.0), px(60.0)), size(px(20.0), px(15.0))),
                ];
                window.set_input_region(Some(&region));
                region.clear();
                drop(region);
                window.set_input_region(Some(&[]));
                window.set_input_region(Some(&[Bounds::new(Default::default(), original.0.size)]));
                window.set_input_region(None);
                for zone in [px(-1.0), px(0.0), px(32.0)] {
                    window.set_exclusive_zone(zone);
                }
                #[cfg(all(target_os = "linux", feature = "wayland"))]
                window.set_exclusive_edge(open_gpui::layer_shell::Anchor::TOP);
                window.toggle_simple_fullscreen();
                window.toggle_simple_fullscreen();
                window.set_client_corner_radii(Corners {
                    top_left: px(4.0),
                    top_right: px(8.0),
                    bottom_right: px(12.0),
                    bottom_left: px(16.0),
                });
                window.set_client_corner_radii(Corners::default());
                assert!(!window.is_simple_fullscreen());
                original
            })
        })
        .expect("ordinary public control calls must execute without test-support");

    for grab in [false, true] {
        let root_built = Cell::new(false);
        let result = app.update(|app| {
            assert_eq!(app.windows(), [parent.into()]);
            app.open_window(
                WindowOptions {
                    kind: WindowKind::AnchoredPopup(popup_options(parent.into(), grab)),
                    show: false,
                    focus_on_appearing: false,
                    ..Default::default()
                },
                |_, app| {
                    root_built.set(true);
                    app.new(|_| Empty)
                },
            )
        });
        assert!(
            result
                .expect_err("an actual live parent must reach the typed unsupported-popup error")
                .is::<PopupNotSupportedError>()
        );
        assert!(!root_built.get());
        app.update(|app| assert_eq!(app.windows(), [parent.into()]));
    }

    app.update(|app| {
        app.update_window(parent.into(), |_, window, app| {
            assert!(!window.is_simple_fullscreen());
            assert_eq!((window.bounds(), window.is_fullscreen()), original);
            window.remove_window(app);
        })
    })
    .expect("no-op controls and popup errors must preserve ordinary window closure");
    app.update(|app| assert!(app.windows().is_empty()));
    let replacement = app
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
        .expect("popup refusal must preserve subsequent ordinary window creation");
    assert_ne!(parent.window_id(), replacement.window_id());
    app.update(|app| {
        app.update_window(replacement.into(), |_, window, app| {
            window.remove_window(app)
        })
    })
    .expect("the replacement must close normally");
    app.update(|app| assert!(app.windows().is_empty()));
    app.update(App::shutdown);
}
