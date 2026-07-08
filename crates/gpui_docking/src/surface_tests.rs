use crate::{
    DockController, DockLayout, DockPanelOpenPlacementSource, DockPanelPlacement,
    DockPanelPlacementTarget, DockSpaceId, DockSurface, DockSurfaceChange, DockSurfacePanelError,
    DockSurfacePanelOutcome, DockSurfaceViewportCloseStatus, DockSurfaceViewportOpenOutcome,
    DockSurfaceViewportOpenStatus, DockSurfaceViewportShouldCloseStatus, DockSurfaceViewportSpec,
    DockSurfaceViewportUnavailable, DockViewportClosePolicy, DockViewportPlacement,
    DockViewportPlacementLayout, DockViewportRestoreReadiness, DockViewportWindowBounds,
    DockViewportWindowState,
    model::{DockLayoutNode, DockLayoutSpace},
};
use open_gpui::{
    App, AppContext as _, Bounds, DisplayId, IntoElement, Render, Window, WindowBounds,
    WindowOptions, div, px, size,
};

struct TestPanel;

impl Render for TestPanel {
    fn render(
        &mut self,
        _window: &mut Window,
        _cx: &mut open_gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
    }
}

fn test_panel(cx: &mut App) -> open_gpui::AnyView {
    cx.new(|_| TestPanel).into()
}

#[open_gpui::test]
fn surface_builder_preserves_primary_space_and_runtime_policy(cx: &mut open_gpui::TestAppContext) {
    cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([
                DockPanelPlacement::left_rail("explorer").fraction(0.25),
                DockPanelPlacement::center("editor").selected(),
                DockPanelPlacement::bottom_rail("terminal").fraction(0.30),
            ])
            .panel_factory("explorer", "Explorer", test_panel)
            .panel_factory("editor", "Editor", test_panel)
            .panel_factory("terminal", "Terminal", test_panel)
            .allow_floating(true)
            .close_policy(DockViewportClosePolicy::Prevent)
            .build(cx)
            .expect("surface layout should validate");

        assert_eq!(surface.primary_space(), &DockSpaceId::from("main"));
        assert_eq!(
            surface.viewport_close_policy(),
            DockViewportClosePolicy::Prevent
        );

        cx.read_entity(&surface.controller(), |controller, _| {
            assert!(controller.policy().allows_floating());
            assert!(controller.panels().contains(&"editor".into()));
        });
    });
}

#[open_gpui::test]
fn surface_can_wrap_existing_controller(cx: &mut open_gpui::TestAppContext) {
    cx.update(|cx| {
        let controller = DockController::builder("wrapped")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel_factory("editor", "Editor", test_panel)
            .build();
        let controller = cx.new(|_| controller);
        let surface = DockSurface::from_controller(controller.clone(), cx);

        assert_eq!(surface.primary_space(), &DockSpaceId::from("wrapped"));
        assert_eq!(surface.controller(), controller);
    });
}

#[open_gpui::test]
fn surface_builder_returns_layout_validation_errors(cx: &mut open_gpui::TestAppContext) {
    cx.update(|_| {
        let invalid_layout = DockLayout::new(
            vec![DockLayoutSpace {
                id: "main".into(),
                root: Some(1),
                floatings: Vec::new(),
                central: None,
            }],
            Vec::new(),
        );
        let result = DockSurface::builder("main").try_layout(&invalid_layout);

        assert!(result.is_err());
    });
}

#[open_gpui::test]
fn surface_primary_window_options_sets_windowed_bounds(cx: &mut open_gpui::TestAppContext) {
    cx.update(|cx| {
        let bounds = Bounds::centered(None, size(px(640.0), px(480.0)), cx);
        let options = DockSurface::primary_window_options(bounds);

        assert!(options.window_bounds.is_some());
    });
}

fn viewport_options() -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            open_gpui::point(px(0.0), px(0.0)),
            size(px(320.0), px(240.0)),
        ))),
        ..Default::default()
    }
}

fn two_space_layout() -> DockLayout {
    DockLayout::new(
        vec![
            DockLayoutSpace {
                id: "main".into(),
                root: Some(1),
                floatings: Vec::new(),
                central: None,
            },
            DockLayoutSpace {
                id: "detached".into(),
                root: Some(2),
                floatings: Vec::new(),
                central: None,
            },
        ],
        vec![
            DockLayoutNode::Tabs {
                id: 1,
                items: vec!["main-panel".into()],
                selected: Some("main-panel".into()),
            },
            DockLayoutNode::Tabs {
                id: 2,
                items: vec!["detached-a".into(), "detached-b".into()],
                selected: Some("detached-b".into()),
            },
        ],
    )
}

#[open_gpui::test]
fn surface_viewport_spec_applies_saved_placement(cx: &mut open_gpui::TestAppContext) {
    cx.update(|_| {
        let saved_bounds = Bounds::new(
            open_gpui::point(px(120.0), px(160.0)),
            size(px(720.0), px(420.0)),
        );
        let placement = DockViewportPlacementLayout::new(vec![DockViewportPlacement {
            space: "main".into(),
            display_id: Some(42),
            window_bounds: Some(DockViewportWindowBounds {
                state: DockViewportWindowState::Maximized,
                bounds: crate::DockLayoutRect::from_bounds(saved_bounds),
            }),
            host_bounds: None,
        }]);

        let spec = DockSurfaceViewportSpec::new("main", viewport_options())
            .with_saved_placement(&placement)
            .expect("valid saved placement should apply to facade viewport spec");

        assert_eq!(spec.space(), &DockSpaceId::from("main"));
        assert_eq!(spec.window_options().display_id, Some(DisplayId::from(42)));
        assert_eq!(
            spec.window_options().window_bounds,
            Some(WindowBounds::Maximized(saved_bounds))
        );
    });
}

#[open_gpui::test]
fn surface_open_viewport_reports_policy_disabled(cx: &mut open_gpui::TestAppContext) {
    cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel_factory("editor", "Editor", test_panel)
            .build(cx)
            .expect("surface layout should validate");
        let before_windows = cx.windows().len();

        let outcome = surface.open_viewport("main", viewport_options(), cx);

        assert!(matches!(
            outcome,
            DockSurfaceViewportOpenOutcome::Unavailable(
                DockSurfaceViewportUnavailable::PolicyDisabled(_)
            )
        ));
        assert_eq!(cx.windows().len(), before_windows);
        assert!(!surface.is_viewport_open(surface.primary_space()));
    });
}

#[open_gpui::test]
fn surface_open_viewports_reports_policy_disabled_for_each_batch_spec(
    cx: &mut open_gpui::TestAppContext,
) {
    cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel_factory("editor", "Editor", test_panel)
            .build(cx)
            .expect("surface layout should validate");
        let before_windows = cx.windows().len();

        let report = surface.open_viewports(
            [
                DockSurfaceViewportSpec::new("main", viewport_options()),
                DockSurfaceViewportSpec::new("secondary", viewport_options()),
            ],
            cx,
        );

        assert_eq!(report.len(), 2);
        assert_eq!(report.opened_count(), 0);
        assert_eq!(report.unavailable_count(), 2);
        for outcome in report.outcomes() {
            assert!(matches!(
                outcome,
                DockSurfaceViewportOpenOutcome::Unavailable(
                    DockSurfaceViewportUnavailable::PolicyDisabled(_)
                )
            ));
        }
        assert_eq!(cx.windows().len(), before_windows);
    });
}

#[open_gpui::test]
fn surface_open_viewport_reports_backend_unsupported_without_registration(
    cx: &mut open_gpui::TestAppContext,
) {
    cx.set_platform_viewport_windows(false);

    cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel_factory("editor", "Editor", test_panel)
            .allow_platform_viewports(true)
            .build(cx)
            .expect("surface layout should validate");
        let before_windows = cx.windows().len();

        let outcome = surface.open_viewport("main", viewport_options(), cx);

        assert!(matches!(
            outcome,
            DockSurfaceViewportOpenOutcome::Unavailable(
                DockSurfaceViewportUnavailable::BackendUnsupported
            )
        ));
        assert_eq!(cx.windows().len(), before_windows);
        assert!(!surface.is_viewport_open(surface.primary_space()));
    });
}

#[open_gpui::test]
fn surface_open_viewports_reports_backend_unsupported_for_each_batch_spec(
    cx: &mut open_gpui::TestAppContext,
) {
    cx.set_platform_viewport_windows(false);

    cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel_factory("editor", "Editor", test_panel)
            .allow_platform_viewports(true)
            .build(cx)
            .expect("surface layout should validate");
        let before_windows = cx.windows().len();

        let report = surface.open_viewports(
            [
                DockSurfaceViewportSpec::new("main", viewport_options()),
                DockSurfaceViewportSpec::new("secondary", viewport_options()),
            ],
            cx,
        );

        assert_eq!(report.len(), 2);
        assert_eq!(report.opened_count(), 0);
        assert_eq!(report.unavailable_count(), 2);
        for outcome in report.outcomes() {
            assert!(matches!(
                outcome,
                DockSurfaceViewportOpenOutcome::Unavailable(
                    DockSurfaceViewportUnavailable::BackendUnsupported
                )
            ));
        }
        assert_eq!(cx.windows().len(), before_windows);
    });
}

#[open_gpui::test]
fn surface_open_viewport_opens_and_reuses_supported_backend(cx: &mut open_gpui::TestAppContext) {
    cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel_factory("editor", "Editor", test_panel)
            .allow_platform_viewports(true)
            .build(cx)
            .expect("surface layout should validate");

        let opened = surface.open_viewport("main", viewport_options(), cx);
        let opened = match opened {
            DockSurfaceViewportOpenOutcome::Opened(opened) => opened,
            other => panic!("expected viewport to open, got {other:?}"),
        };
        assert_eq!(opened.status(), DockSurfaceViewportOpenStatus::Opened);
        assert_eq!(opened.space(), surface.primary_space());
        assert!(surface.is_viewport_open(surface.primary_space()));

        let reused = surface.open_viewport("main", viewport_options(), cx);
        let reused = match reused {
            DockSurfaceViewportOpenOutcome::Opened(reused) => reused,
            other => panic!("expected viewport to reuse, got {other:?}"),
        };
        assert_eq!(reused.status(), DockSurfaceViewportOpenStatus::Reused);
        assert_eq!(reused.window(), opened.window());
    });
}

#[open_gpui::test]
fn surface_exports_and_checks_viewport_placement_restore(cx: &mut open_gpui::TestAppContext) {
    cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel_factory("editor", "Editor", test_panel)
            .allow_platform_viewports(true)
            .build(cx)
            .expect("surface layout should validate");
        let opened = match surface.open_viewport("main", viewport_options(), cx) {
            DockSurfaceViewportOpenOutcome::Opened(opened) => opened,
            other => panic!("expected viewport to open, got {other:?}"),
        };

        let placement = surface.export_viewport_placement();

        assert_eq!(placement.viewports.len(), 1);
        assert_eq!(placement.viewports[0].space, DockSpaceId::from("main"));
        assert_eq!(
            surface
                .check_viewport_placement_restore(&placement)
                .expect("exported placement should validate against live facade viewport"),
            DockViewportRestoreReadiness {
                matched: 1,
                missing: 0,
            }
        );

        let missing = DockViewportPlacementLayout::new(vec![DockViewportPlacement {
            space: "detached".into(),
            display_id: None,
            window_bounds: None,
            host_bounds: None,
        }]);
        assert_eq!(
            surface
                .check_viewport_placement_restore(&missing)
                .expect("missing saved placement should still validate"),
            DockViewportRestoreReadiness {
                matched: 0,
                missing: 1,
            }
        );
        assert_eq!(opened.space(), surface.primary_space());
    });
}

#[open_gpui::test]
fn surface_open_viewports_reports_ordered_batch_outcomes(cx: &mut open_gpui::TestAppContext) {
    cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel_factory("editor", "Editor", test_panel)
            .allow_platform_viewports(true)
            .build(cx)
            .expect("surface layout should validate");

        let report = surface.open_viewports(
            [
                DockSurfaceViewportSpec::new("main", viewport_options()),
                DockSurfaceViewportSpec::new("main", viewport_options()),
            ],
            cx,
        );

        assert_eq!(report.len(), 2);
        assert!(!report.is_empty());
        assert!(report.all_opened());
        assert_eq!(report.opened_count(), 2);
        assert_eq!(report.unavailable_count(), 0);

        let outcomes = report.into_outcomes();
        let first = match &outcomes[0] {
            DockSurfaceViewportOpenOutcome::Opened(opened) => opened,
            other => panic!("expected first viewport to open, got {other:?}"),
        };
        let second = match &outcomes[1] {
            DockSurfaceViewportOpenOutcome::Opened(opened) => opened,
            other => panic!("expected second viewport to reuse, got {other:?}"),
        };
        assert_eq!(first.status(), DockSurfaceViewportOpenStatus::Opened);
        assert_eq!(second.status(), DockSurfaceViewportOpenStatus::Reused);
        assert_eq!(first.window(), second.window());
    });
}

#[open_gpui::test]
fn surface_viewport_close_policy_can_be_changed_without_runtime_import(
    cx: &mut open_gpui::TestAppContext,
) {
    cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel_factory("editor", "Editor", test_panel)
            .allow_platform_viewports(true)
            .build(cx)
            .expect("surface layout should validate");
        let opened = match surface.open_viewport("main", viewport_options(), cx) {
            DockSurfaceViewportOpenOutcome::Opened(opened) => opened,
            other => panic!("expected viewport to open, got {other:?}"),
        };

        surface.set_viewport_close_policy(DockViewportClosePolicy::Prevent);
        let should_close =
            surface.handle_viewport_window_should_close(opened.window().window_id(), cx);

        assert_eq!(
            should_close.status(),
            DockSurfaceViewportShouldCloseStatus::Vetoed
        );
        assert!(!should_close.allows_close());
        assert_eq!(should_close.space(), Some(surface.primary_space()));
        assert_eq!(
            surface.cancel_viewport_window_close(opened.window().window_id(), cx),
            DockSurfaceChange::Unchanged
        );
    });
}

#[open_gpui::test]
fn surface_viewport_merge_back_close_moves_content_to_fallback(cx: &mut open_gpui::TestAppContext) {
    cx.update(|cx| {
        let main = DockSpaceId::from("main");
        let detached = DockSpaceId::from("detached");
        let surface = DockSurface::builder(main.clone())
            .try_layout(&two_space_layout())
            .expect("test layout should validate")
            .panel_factory("main-panel", "Main", test_panel)
            .panel_factory("detached-a", "Detached A", test_panel)
            .panel_factory("detached-b", "Detached B", test_panel)
            .allow_platform_viewports(true)
            .close_policy(DockViewportClosePolicy::MergeBack {
                target_space: main.clone(),
            })
            .build(cx)
            .expect("surface layout should validate");
        let opened = match surface.open_viewport(detached.clone(), viewport_options(), cx) {
            DockSurfaceViewportOpenOutcome::Opened(opened) => opened,
            other => panic!("expected detached viewport to open, got {other:?}"),
        };

        let should_close =
            surface.handle_viewport_window_should_close(opened.window().window_id(), cx);
        assert_eq!(
            should_close.status(),
            DockSurfaceViewportShouldCloseStatus::Allowed
        );
        assert_eq!(should_close.space(), Some(&detached));
        assert!(should_close.allows_close());
        assert_eq!(
            surface.cancel_viewport_window_close(opened.window().window_id(), cx),
            DockSurfaceChange::Changed
        );

        let should_close =
            surface.handle_viewport_window_should_close(opened.window().window_id(), cx);
        assert_eq!(
            should_close.status(),
            DockSurfaceViewportShouldCloseStatus::Allowed
        );

        let closed = surface.handle_viewport_window_closed(opened.window().window_id(), cx);

        assert_eq!(closed.status(), DockSurfaceViewportCloseStatus::MergedBack);
        assert_eq!(closed.space(), Some(&detached));
        assert_eq!(closed.merge_target_space(), Some(&main));
        assert!(!surface.is_viewport_open(&detached));
        assert_eq!(
            surface.registered_viewport_spaces(),
            Vec::<DockSpaceId>::new()
        );

        cx.read_entity(&surface.controller(), |controller, _| {
            assert_eq!(
                controller.graph().collect_items_in_space(&main),
                vec![
                    "main-panel".into(),
                    "detached-a".into(),
                    "detached-b".into()
                ]
            );
            assert!(
                controller
                    .graph()
                    .collect_items_in_space(&detached)
                    .is_empty()
            );
        });
    });
}

#[open_gpui::test]
fn surface_panel_commands_open_close_and_reopen_by_item(cx: &mut open_gpui::TestAppContext) {
    cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([
                DockPanelPlacement::center("editor"),
                DockPanelPlacement::right_rail("inspector"),
                DockPanelPlacement::stacked_with("terminal", "inspector"),
            ])
            .panel_factory("editor", "Editor", test_panel)
            .panel_factory("inspector", "Inspector", test_panel)
            .panel_factory("terminal", "Terminal", test_panel)
            .build(cx)
            .expect("surface layout should validate");

        let closed = surface
            .close_panel("terminal", cx)
            .expect("registered panel should close");
        let DockSurfacePanelOutcome::Closed(closed) = closed else {
            panic!("expected close outcome");
        };
        assert!(closed.changed());
        assert_eq!(
            closed.placement().map(DockPanelPlacement::target),
            Some(&DockPanelPlacementTarget::stacked_with("inspector").insert_index(1))
        );

        cx.read_entity(&surface.controller(), |controller, _| {
            assert!(
                controller
                    .graph()
                    .find_item_in_space(surface.primary_space(), &"terminal".into())
                    .is_none()
            );
        });

        let opened = surface
            .open_panel("terminal", cx)
            .expect("registered panel should reopen");
        let DockSurfacePanelOutcome::Opened(opened) = opened else {
            panic!("expected open outcome");
        };
        assert!(opened.changed());
        assert_eq!(
            opened.placement_source(),
            DockPanelOpenPlacementSource::LastKnown
        );
    });
}

#[open_gpui::test]
fn surface_panel_commands_open_at_explicit_placement(cx: &mut open_gpui::TestAppContext) {
    cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([
                DockPanelPlacement::center("editor"),
                DockPanelPlacement::right_rail("inspector"),
            ])
            .panel_factory("editor", "Editor", test_panel)
            .panel_factory("inspector", "Inspector", test_panel)
            .panel_factory("terminal", "Terminal", test_panel)
            .build(cx)
            .expect("surface layout should validate");

        let opened = surface
            .open_panel_at(
                DockPanelPlacement::stacked_with("terminal", "inspector"),
                cx,
            )
            .expect("registered panel should open at explicit placement");
        let DockSurfacePanelOutcome::Opened(opened) = opened else {
            panic!("expected open outcome");
        };
        assert_eq!(
            opened.placement_source(),
            DockPanelOpenPlacementSource::Explicit
        );

        cx.read_entity(&surface.controller(), |controller, _| {
            let (terminal_tabs, terminal_index) = controller
                .graph()
                .find_item_in_space(surface.primary_space(), &"terminal".into())
                .expect("terminal should be present");
            let (inspector_tabs, inspector_index) = controller
                .graph()
                .find_item_in_space(surface.primary_space(), &"inspector".into())
                .expect("inspector should remain present");
            assert_eq!(terminal_tabs, inspector_tabs);
            assert_eq!((inspector_index, terminal_index), (0, 1));
        });
    });
}

#[open_gpui::test]
fn surface_panel_commands_float_and_dock_back_panel(cx: &mut open_gpui::TestAppContext) {
    cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([
                DockPanelPlacement::center("editor"),
                DockPanelPlacement::bottom_rail("terminal").fraction(0.30),
            ])
            .panel_factory("editor", "Editor", test_panel)
            .panel_factory("terminal", "Terminal", test_panel)
            .allow_floating(true)
            .build(cx)
            .expect("surface layout should validate");
        let bounds = Bounds::new(
            open_gpui::point(px(24.0), px(32.0)),
            size(px(280.0), px(160.0)),
        );

        let floated = surface
            .float_panel_in_window("terminal", bounds, cx)
            .expect("registered panel should float");
        assert!(matches!(floated, DockSurfacePanelOutcome::Floated(_)));

        cx.read_entity(&surface.controller(), |controller, _| {
            assert_eq!(
                controller
                    .graph()
                    .floating_containers(surface.primary_space())
                    .len(),
                1
            );
        });

        let docked = surface
            .dock_panel_at(DockPanelPlacement::stacked_with("terminal", "editor"), cx)
            .expect("floating panel should dock back through product placement");
        let DockSurfacePanelOutcome::Docked(docked) = docked else {
            panic!("expected dock-back outcome");
        };
        assert!(docked.changed());

        cx.read_entity(&surface.controller(), |controller, _| {
            assert!(
                controller
                    .graph()
                    .floating_containers(surface.primary_space())
                    .is_empty()
            );
            assert!(
                controller
                    .graph()
                    .find_item_in_space(surface.primary_space(), &"terminal".into())
                    .is_some()
            );
        });
    });
}

#[open_gpui::test]
fn surface_panel_commands_report_unregistered_items(cx: &mut open_gpui::TestAppContext) {
    cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel_factory("editor", "Editor", test_panel)
            .build(cx)
            .expect("surface layout should validate");

        assert_eq!(
            surface.open_panel("missing", cx),
            Err(DockSurfacePanelError::PanelNotRegistered {
                item: "missing".into()
            })
        );
    });
}
