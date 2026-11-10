//! Public consumers of native window controls at official Zed target
//! 635026fabc38440fce5e1767d125026a17b12012. Runtime cases exercise unsupported
//! TestPlatform services and popup refusal, without qualifying native presentation.

use open_gpui::{
    AnyWindowHandle, Bounds, Corners, Pixels, PlatformWindow, Window, WindowKind, WindowOptions,
    point,
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
fn public_native_window_control_signatures_and_popup_schema_are_type_only() {
    let _: fn(&Window, Option<&[Bounds<Pixels>]>) = Window::set_input_region;
    let _: fn(&Window, Pixels) = Window::set_exclusive_zone;
    let _: fn(&Window) = Window::toggle_simple_fullscreen;
    let _: fn(&Window) -> bool = Window::is_simple_fullscreen;
    let _: fn(&mut Window, Corners<Pixels>) = Window::set_client_corner_radii;
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

#[cfg(feature = "test-support")]
#[test]
fn public_unsupported_controls_preserve_a_live_window_and_normal_close() {
    use open_gpui::{AppContext as _, Empty, QuitMode, TestAppContext};

    let mut cx = TestAppContext::single();
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let handle = cx.open_window(size(px(160.0), px(120.0)), |_, _| Empty);
    cx.run_until_parked();
    let original = cx
        .update_window(handle.into(), |_, window, _| {
            let original = (window.bounds(), window.is_fullscreen());
            assert!(!window.is_simple_fullscreen());
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
        .expect("public controls must accept a live unsupported Window");
    cx.run_until_parked();

    cx.update_window(handle.into(), |_, window, app| {
        assert!(!window.is_simple_fullscreen());
        assert_eq!((window.bounds(), window.is_fullscreen()), original);
        window.remove_window(app);
    })
    .expect("unsupported native controls must leave the Window usable and closable");
    cx.run_until_parked();
    assert!(cx.windows().is_empty());
    cx.quit();
}

#[cfg(feature = "test-support")]
#[test]
fn public_popup_refusal_uses_a_live_parent_and_publishes_no_window() {
    use open_gpui::{AppContext as _, Empty, QuitMode, TestAppContext};
    use std::cell::Cell;

    let mut cx = TestAppContext::single();
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let parent = cx.open_window(size(px(160.0), px(120.0)), |_, _| Empty);
    cx.run_until_parked();
    for grab in [false, true] {
        let root_built = Cell::new(false);
        let result = cx.update(|app| {
            app.open_window(
                WindowOptions {
                    kind: WindowKind::AnchoredPopup(popup_options(parent.into(), grab)),
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
                .expect_err("an unsupported native popup must return a typed error")
                .is::<PopupNotSupportedError>()
        );
        assert!(!root_built.get());
        assert_eq!(cx.windows(), [parent.into()]);
    }

    let original_creation_capabilities = cx.read(|app| app.window_capabilities().creation);
    cx.set_platform_window_creation_capabilities(Default::default());
    let passive = cx
        .update(|app| {
            app.open_window(
                WindowOptions {
                    kind: WindowKind::AnchoredPopup(popup_options(parent.into(), false)),
                    focus_on_appearing: false,
                    show: false,
                    ..Default::default()
                },
                |_, app| app.new(|_| Empty),
            )
        })
        .expect_err(
            "passive popup refusal must preserve its backend error before capability checks",
        );
    assert!(passive.is::<PopupNotSupportedError>());
    assert_eq!(cx.windows(), [parent.into()]);

    let detailed = cx
        .update(|app| {
            app.open_window_detailed(
                WindowOptions {
                    kind: WindowKind::AnchoredPopup(popup_options(parent.into(), false)),
                    ..Default::default()
                },
                |_, app| app.new(|_| Empty),
            )
        })
        .expect_err("detailed popup refusal must preserve its synchronous stage");
    assert_eq!(
        detailed.stage(),
        open_gpui::WindowOpenFailureStage::NativeCreateOrMap
    );
    assert!(
        std::error::Error::source(&detailed)
            .and_then(|source| source.downcast_ref::<PopupNotSupportedError>())
            .is_some()
    );
    assert_eq!(cx.windows(), [parent.into()]);

    cx.set_platform_window_creation_capabilities(original_creation_capabilities);
    let replacement = cx.open_window(size(px(80.0), px(60.0)), |_, _| Empty);
    for handle in [replacement, parent] {
        cx.update_window(handle.into(), |_, window, app| window.remove_window(app))
            .expect("popup refusal must preserve ordinary window creation and closure");
    }
    cx.run_until_parked();
    assert!(cx.windows().is_empty());
    cx.quit();
}

#[cfg(feature = "test-support")]
#[test]
fn public_popup_rejects_mixed_and_closed_parent_before_unsupported_backend_refusal() {
    use open_gpui::{AppContext as _, Empty, QuitMode, TestAppContext};
    use std::cell::Cell;

    let mut cx = TestAppContext::single();
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let parent = cx.open_window(size(px(160.0), px(120.0)), |_, _| Empty);
    let root_built = Cell::new(false);
    let mixed = cx.update(|app| {
        let owner = app
            .transient_window_owner(parent.into())
            .expect("the actual parent must have a live transient-owner token");
        app.open_window(
            WindowOptions {
                kind: WindowKind::AnchoredPopup(popup_options(parent.into(), false)),
                transient_for: Some(owner),
                ..Default::default()
            },
            |_, app| {
                root_built.set(true);
                app.new(|_| Empty)
            },
        )
    });
    let error = mixed.expect_err("popup.parent must be the sole parent contract");
    assert!(!error.is::<PopupNotSupportedError>());
    assert!(error.to_string().contains("rather than a transient owner"));
    assert!(!root_built.get());
    assert_eq!(cx.windows(), [parent.into()]);

    cx.update_window(parent.into(), |_, window, app| window.remove_window(app))
        .expect("the original parent must close before stale-handle controls");
    cx.run_until_parked();
    for replacement_open in [false, true] {
        let replacement =
            replacement_open.then(|| cx.open_window(size(px(80.0), px(60.0)), |_, _| Empty));
        if let Some(replacement) = replacement {
            assert_ne!(replacement.window_id(), parent.window_id());
        }
        let result = cx.update(|app| {
            app.open_window(
                WindowOptions {
                    kind: WindowKind::AnchoredPopup(popup_options(parent.into(), false)),
                    ..Default::default()
                },
                |_, app| {
                    root_built.set(true);
                    app.new(|_| Empty)
                },
            )
        });
        let error = result.expect_err("closed parent generations must fail before construction");
        assert!(!error.is::<PopupNotSupportedError>());
        assert!(
            error
                .to_string()
                .contains("closed or its generation is stale")
        );
        assert!(!root_built.get());
        assert_eq!(
            cx.windows(),
            replacement.into_iter().map(Into::into).collect::<Vec<_>>()
        );
        if let Some(replacement) = replacement {
            cx.update_window(replacement.into(), |_, window, app| {
                window.remove_window(app)
            })
            .expect("stale popup refusal must preserve the replacement window");
        }
    }
    cx.run_until_parked();
    assert!(cx.windows().is_empty());
    cx.quit();
}
