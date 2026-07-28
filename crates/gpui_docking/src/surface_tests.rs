use crate::{
    DockController, DockItemId, DockLayout, DockPanelOpenPlacementSource, DockPanelPlacement,
    DockPanelPlacementTarget, DockSpaceId, DockSurface, DockSurfaceChange,
    DockSurfaceChangeCategory, DockSurfacePanelError, DockSurfacePanelLocationKind,
    DockSurfacePanelOutcome, DockSurfaceSnapshot, DockSurfaceViewportCloseStatus,
    DockSurfaceViewportOpenOutcome, DockSurfaceViewportOpenStatus,
    DockSurfaceViewportReadinessStatus, DockSurfaceViewportShouldCloseStatus,
    DockSurfaceViewportSpec, DockSurfaceViewportUnavailable, DockSurfaceViewportUnsupportedFlag,
    DockSurfaceViewports, DockViewportClosePolicy, DockViewportPlacement,
    DockViewportPlacementLayout, DockViewportRestoreReadiness, DockViewportWindowBounds,
    DockViewportWindowState,
    model::{DockLayoutNode, DockLayoutSpace},
};
use open_gpui::{
    App, AppContext as _, Bounds, DisplayId, IntoElement, Render, Window,
    WindowBackgroundAppearance, WindowBounds, WindowMutationDomain, WindowMutationSupport,
    WindowOptions, div, px, size,
};
use std::{cell::RefCell, rc::Rc};

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
            surface.viewport_close_policy(cx),
            DockViewportClosePolicy::Prevent
        );

        cx.read_entity(&surface.controller(cx), |controller, _| {
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
        assert_eq!(surface.controller(cx), controller);
    });
}

#[open_gpui::test]
fn surface_builder_returns_layout_validation_errors(cx: &mut open_gpui::TestAppContext) {
    cx.update(|_| {
        let invalid_layout = DockLayout::from_raw_parts(
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

#[open_gpui::test]
fn surface_facade_embeds_host_and_reports_semantic_snapshots(cx: &mut open_gpui::TestAppContext) {
    cx.update(|cx| {
        let floating_bounds = Bounds::new(
            open_gpui::point(px(20.0), px(30.0)),
            size(px(240.0), px(180.0)),
        );
        let surface = DockSurface::builder("main")
            .panel_placements([
                DockPanelPlacement::center("editor").selected(),
                DockPanelPlacement::right_rail("inspector"),
                DockPanelPlacement::bottom_rail("terminal").fraction(0.30),
            ])
            .panel_factory("editor", "Editor", test_panel)
            .panel_factory("inspector", "Inspector", test_panel)
            .panel_factory("terminal", "Terminal", test_panel)
            .allow_floating(true)
            .build(cx)
            .expect("surface layout should validate");

        let _embedded_host = surface.host_view(cx);
        assert_eq!(surface.dock_spaces(cx), vec![DockSpaceId::from("main")]);
        assert_eq!(
            surface.selected_panel_in_space("main", cx),
            Some("editor".into())
        );

        surface
            .float_panel_in_window("terminal", floating_bounds, cx)
            .expect("terminal should float");

        let mut items = surface.items_in_space("main", cx);
        items.sort();
        assert_eq!(
            items,
            vec!["editor".into(), "inspector".into(), "terminal".into()]
        );

        let terminal_location = surface
            .panel_location("terminal", cx)
            .expect("terminal should have a semantic location");
        assert_eq!(terminal_location.space(), &DockSpaceId::from("main"));
        assert_eq!(
            terminal_location.kind(),
            DockSurfacePanelLocationKind::Floating
        );
        assert_eq!(terminal_location.tab_index(), 0);

        let inspector_location = surface
            .panel_location("inspector", cx)
            .expect("inspector should have a semantic location");
        assert_eq!(
            inspector_location.kind(),
            DockSurfacePanelLocationKind::Docked
        );

        let floating = surface.floating_panels_in_space("main", cx);
        assert_eq!(floating.len(), 1);
        assert_eq!(floating[0].space(), &DockSpaceId::from("main"));
        assert_eq!(floating[0].items(), &[DockItemId::from("terminal")]);
        assert_eq!(floating[0].bounds(), floating_bounds);

        let registered = surface.registered_panels(cx);
        assert_eq!(registered.len(), 3);
        let editor = registered
            .iter()
            .find(|panel| panel.item() == &DockItemId::from("editor"))
            .expect("editor descriptor should be registered");
        assert_eq!(editor.descriptor().title(), "Editor");
        assert!(editor.has_view_lifecycle());
        assert_eq!(
            editor.location().map(|location| location.kind()),
            Some(DockSurfacePanelLocationKind::Docked)
        );

        let exported = surface.export_layout(cx);
        assert_eq!(exported.space_count(), 1);
        assert_eq!(
            exported.space_ids().cloned().collect::<Vec<_>>(),
            vec![DockSpaceId::from("main")]
        );
        assert!(!exported.is_empty());
    });
}

#[open_gpui::test]
fn surface_panel_semantic_commands_select_and_update_floating_by_item(
    cx: &mut open_gpui::TestAppContext,
) {
    cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([
                DockPanelPlacement::center("editor"),
                DockPanelPlacement::right_rail("inspector").selected(),
                DockPanelPlacement::stacked_with("terminal", "inspector"),
            ])
            .panel_factory("editor", "Editor", test_panel)
            .panel_factory("inspector", "Inspector", test_panel)
            .panel_factory("terminal", "Terminal", test_panel)
            .allow_floating(true)
            .build(cx)
            .expect("surface layout should validate");

        let selected = surface
            .select_panel("terminal", cx)
            .expect("registered panel should be selectable by item");
        assert!(matches!(
            selected,
            DockSurfacePanelOutcome::Selected(DockSurfaceChange::Changed)
        ));
        assert!(
            surface
                .selected_panels_in_space("main", cx)
                .contains(&DockItemId::from("terminal"))
        );

        let terminal_bounds = Bounds::new(
            open_gpui::point(px(24.0), px(32.0)),
            size(px(280.0), px(160.0)),
        );
        let editor_bounds = Bounds::new(
            open_gpui::point(px(40.0), px(48.0)),
            size(px(320.0), px(220.0)),
        );
        surface
            .float_panel_in_window("terminal", terminal_bounds, cx)
            .expect("terminal should float");
        surface
            .float_panel_in_window("editor", editor_bounds, cx)
            .expect("editor should float");

        let updated_terminal_bounds = Bounds::new(
            open_gpui::point(px(64.0), px(72.0)),
            size(px(360.0), px(200.0)),
        );
        let bounds_set = surface
            .set_floating_panel_bounds("terminal", updated_terminal_bounds, cx)
            .expect("floating panel bounds should update by item");
        assert!(matches!(
            bounds_set,
            DockSurfacePanelOutcome::FloatingBoundsSet(DockSurfaceChange::Changed)
        ));

        let raised = surface
            .raise_floating_panel("terminal", cx)
            .expect("floating panel should raise by item");
        assert!(matches!(
            raised,
            DockSurfacePanelOutcome::FloatingRaised(DockSurfaceChange::Changed)
        ));

        let floating = surface.floating_panels_in_space("main", cx);
        assert_eq!(floating.len(), 2);
        assert_eq!(floating[0].items(), &[DockItemId::from("editor")]);
        assert_eq!(floating[1].items(), &[DockItemId::from("terminal")]);
        assert_eq!(floating[1].bounds(), updated_terminal_bounds);

        assert_eq!(
            surface.set_floating_panel_bounds("inspector", updated_terminal_bounds, cx),
            Err(DockSurfacePanelError::PanelNotFloating {
                item: "inspector".into()
            })
        );
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
    DockLayout::from_raw_parts(
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
        assert!(!surface.is_viewport_open(surface.primary_space(), cx));
    });
}

#[open_gpui::test]
fn surface_viewport_readiness_reports_policy_disabled(cx: &mut open_gpui::TestAppContext) {
    cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel_factory("editor", "Editor", test_panel)
            .build(cx)
            .expect("surface layout should validate");
        let spec = DockSurfaceViewportSpec::new("main", viewport_options());

        let readiness = surface.viewports().check_open_readiness(&spec, cx);

        assert!(!readiness.is_openable());
        assert!(readiness.status().is_policy_disabled());
        assert_eq!(readiness.space(), surface.primary_space());
        assert!(matches!(
            readiness.status(),
            DockSurfaceViewportReadinessStatus::PolicyDisabled(_)
        ));
        assert!(matches!(
            readiness.unavailable_reason(),
            Some(DockSurfaceViewportUnavailable::PolicyDisabled(_))
        ));
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
        assert!(!surface.is_viewport_open(surface.primary_space(), cx));
    });
}

#[open_gpui::test]
fn surface_viewport_readiness_reports_backend_unsupported(cx: &mut open_gpui::TestAppContext) {
    cx.set_platform_viewport_windows(false);

    cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel_factory("editor", "Editor", test_panel)
            .allow_platform_viewports(true)
            .build(cx)
            .expect("surface layout should validate");
        let spec = DockSurfaceViewportSpec::new("main", viewport_options());

        let readiness = surface.check_viewport_open_readiness(&spec, cx);

        assert!(!readiness.is_openable());
        assert!(readiness.status().is_backend_unsupported());
        assert!(
            !readiness
                .platform()
                .capabilities()
                .platform_viewport_windows
        );
        assert!(matches!(
            readiness.unavailable_reason(),
            Some(DockSurfaceViewportUnavailable::BackendUnsupported)
        ));
    });
}

#[open_gpui::test]
fn surface_viewport_readiness_reports_unsupported_flags_without_opening(
    cx: &mut open_gpui::TestAppContext,
) {
    cx.set_platform_pointer_input_mutation_supported(false);

    cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel_factory("editor", "Editor", test_panel)
            .allow_platform_viewports(true)
            .build(cx)
            .expect("surface layout should validate");
        let before_windows = cx.windows().len();
        let mut options = viewport_options();
        options.accepts_pointer_input = false;
        options.window_background = WindowBackgroundAppearance::Transparent;
        let spec = DockSurfaceViewportSpec::new("main", options);

        let readiness = surface.viewports().readiness(&spec, cx);

        assert!(!readiness.ready());
        assert!(readiness.is_flag_unsupported());
        assert_eq!(
            readiness.window_capabilities().mutations.pointer_input,
            WindowMutationSupport::Unsupported
        );
        assert_eq!(
            readiness.window_capabilities().mutations.alpha,
            WindowMutationSupport::CreationOnly
        );
        assert_eq!(
            readiness.unsupported_flags(),
            &[DockSurfaceViewportUnsupportedFlag::NoInputWindow]
        );

        let outcome = surface.open_viewport_spec(spec, cx);

        let unavailable = outcome
            .unavailable()
            .expect("unsupported viewport flags should prevent opening");
        assert!(unavailable.is_flag_unsupported());
        assert_eq!(
            unavailable.unsupported_flags(),
            &[DockSurfaceViewportUnsupportedFlag::NoInputWindow]
        );
        assert_eq!(cx.windows().len(), before_windows);
        assert!(surface.registered_viewport_spaces(cx).is_empty());
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
        assert!(surface.is_viewport_open(surface.primary_space(), cx));

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
fn surface_revisions_only_observed_platform_placement_not_queued_dispatch(
    cx: &mut open_gpui::TestAppContext,
) {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let observed = changes.clone();
    let (surface, opened, _subscription) = cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel_factory("editor", "Editor", test_panel)
            .allow_platform_viewports(true)
            .build(cx)
            .expect("surface layout should validate");
        let subscription = surface.subscribe_changes(cx, move |event, _| {
            observed.borrow_mut().push(event.clone());
        });
        let opened = match surface.open_viewport("main", viewport_options(), cx) {
            DockSurfaceViewportOpenOutcome::Opened(opened) => opened,
            other => panic!("expected viewport to open, got {other:?}"),
        };
        (surface, opened, subscription)
    });
    cx.run_until_parked();
    changes.borrow_mut().clear();

    let revision_before_dispatch = cx.read(|cx| surface.revision(cx));
    let requested_bounds = Bounds::new(
        open_gpui::point(px(24.0), px(32.0)),
        size(px(480.0), px(260.0)),
    );
    let reused = cx.update(|cx| {
        match surface.open_viewport(
            "main",
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(requested_bounds)),
                ..viewport_options()
            },
            cx,
        ) {
            DockSurfaceViewportOpenOutcome::Opened(reused) => reused,
            other => panic!("expected viewport to reuse, got {other:?}"),
        }
    });

    assert_eq!(reused.status(), DockSurfaceViewportOpenStatus::Reused);
    assert_eq!(reused.window(), opened.window());
    assert_eq!(
        cx.read(|cx| surface.revision(cx)),
        revision_before_dispatch,
        "queued mutation intent must not create a persistence revision"
    );
    assert!(
        changes.borrow().is_empty(),
        "queued mutation intent must not publish a surface change event"
    );

    let adjusted_bounds = Bounds::new(
        open_gpui::point(px(30.0), px(40.0)),
        size(px(460.0), px(250.0)),
    );
    let mut adjusted_facts = reused
        .window()
        .update(cx, |_, window, _| window.platform_facts().clone())
        .expect("reused viewport should remain live");
    adjusted_facts.bounds = adjusted_bounds;
    adjusted_facts.content_size = adjusted_bounds.size;
    adjusted_facts.window_bounds = WindowBounds::Windowed(adjusted_bounds);
    adjusted_facts.inner_window_bounds = WindowBounds::Windowed(adjusted_bounds);

    assert!(cx.simulate_window_mutation_observation(
        reused.window(),
        WindowMutationDomain::Placement,
        adjusted_facts,
    ));
    assert_eq!(
        cx.read(|cx| surface.revision(cx)),
        revision_before_dispatch + 1,
        "one observed terminal placement must create one durable surface revision"
    );
    let changes = changes.borrow();
    assert_eq!(changes.len(), 1);
    assert_eq!(
        changes[0].categories(),
        &[DockSurfaceChangeCategory::ObservedViewportPlacement]
    );
}

#[open_gpui::test]
fn surface_viewports_facade_opens_detached_panel_space(cx: &mut open_gpui::TestAppContext) {
    cx.update(|cx| {
        let child_space = DockSpaceId::from("preview-window");
        let surface = DockSurface::builder("main")
            .panel_placements([
                DockPanelPlacement::center("editor"),
                DockPanelPlacement::right_rail("inspector"),
                DockPanelPlacement::center("preview"),
            ])
            .panel_factory("editor", "Editor", test_panel)
            .panel_factory("inspector", "Inspector", test_panel)
            .panel_factory("preview", "Preview", test_panel)
            .allow_platform_viewports(true)
            .build(cx)
            .expect("surface layout should validate");
        let viewports: DockSurfaceViewports = surface.viewports();

        let detached = surface
            .detach_panel_to_space("main", "preview", child_space.clone(), cx)
            .expect("registered preview panel should detach into a child dock space");
        assert_eq!(detached, DockSurfaceChange::Changed);
        assert!(
            !surface
                .items_in_space("main", cx)
                .contains(&DockItemId::from("preview"))
        );
        assert_eq!(
            surface.items_in_space(child_space.clone(), cx),
            vec![DockItemId::from("preview")]
        );

        let opened = match viewports.open(child_space.clone(), viewport_options(), cx) {
            DockSurfaceViewportOpenOutcome::Opened(opened) => opened,
            other => panic!("expected child dock space viewport to open, got {other:?}"),
        };
        assert_eq!(opened.status(), DockSurfaceViewportOpenStatus::Opened);
        assert_eq!(opened.space(), &child_space);
        assert!(viewports.is_open(&child_space, cx));
        assert_eq!(viewports.registered_spaces(cx), vec![child_space.clone()]);

        let placement = viewports.export_placement(cx);
        assert_eq!(
            viewports
                .check_restore(&placement, cx)
                .expect("session placement should validate"),
            DockViewportRestoreReadiness {
                matched: 1,
                missing: 0,
            }
        );
    });
}

#[open_gpui::test]
fn surface_snapshot_roundtrips_layout_and_viewport_placement(cx: &mut open_gpui::TestAppContext) {
    cx.update(|cx| {
        let child_space = DockSpaceId::from("preview-window");
        let surface = DockSurface::builder("main")
            .panel_placements([
                DockPanelPlacement::center("editor"),
                DockPanelPlacement::right_rail("inspector"),
                DockPanelPlacement::center("preview"),
            ])
            .panel_factory("editor", "Editor", test_panel)
            .panel_factory("inspector", "Inspector", test_panel)
            .panel_factory("preview", "Preview", test_panel)
            .allow_platform_viewports(true)
            .build(cx)
            .expect("surface layout should validate");
        let viewports = surface.viewports();
        surface
            .detach_panel_to_space("main", "preview", child_space.clone(), cx)
            .expect("preview should detach into a child dock space");
        let opened = viewports.open(child_space.clone(), viewport_options(), cx);
        assert!(opened.opened());

        let snapshot = surface.export_snapshot(cx);
        assert_eq!(snapshot.layout().space_count(), 2);
        assert_eq!(snapshot.viewport_placement().viewports.len(), 1);
        assert_eq!(
            snapshot.viewport_placement().viewports[0].space,
            child_space
        );

        let restored = DockSurface::builder("main")
            .try_snapshot(&snapshot)
            .expect("snapshot layout should validate")
            .panel_factory("editor", "Editor", test_panel)
            .panel_factory("inspector", "Inspector", test_panel)
            .panel_factory("preview", "Preview", test_panel)
            .allow_platform_viewports(true)
            .build(cx)
            .expect("restored surface should validate");
        let restored_layout = restored.export_layout(cx);
        assert_eq!(&restored_layout, snapshot.layout());

        let restore_report =
            restored
                .viewports()
                .restore_snapshot(&snapshot, |_| viewport_options(), cx);
        assert_eq!(restore_report.len(), 1);
        assert!(restore_report.all_opened());
        assert_eq!(
            restored.registered_viewport_spaces(cx),
            vec![DockSpaceId::from("preview-window")]
        );

        let (layout, viewport_placement) = snapshot.into_parts();
        let rebuilt_snapshot = DockSurfaceSnapshot::new(layout, viewport_placement);
        assert_eq!(rebuilt_snapshot.viewport_placement().viewports.len(), 1);
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

        let placement = surface.export_viewport_placement(cx);

        assert_eq!(placement.viewports.len(), 1);
        assert_eq!(placement.viewports[0].space, DockSpaceId::from("main"));
        assert_eq!(
            surface
                .check_viewport_placement_restore(&placement, cx)
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
                .check_viewport_placement_restore(&missing, cx)
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
fn surface_opens_viewports_from_saved_placement_with_keyed_report(
    cx: &mut open_gpui::TestAppContext,
) {
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
            .build(cx)
            .expect("surface layout should validate");
        let placement = DockViewportPlacementLayout::new(vec![
            DockViewportPlacement {
                space: main.clone(),
                display_id: None,
                window_bounds: None,
                host_bounds: None,
            },
            DockViewportPlacement {
                space: detached.clone(),
                display_id: Some(7),
                window_bounds: None,
                host_bounds: None,
            },
        ]);

        let report =
            surface.open_viewports_from_saved_placement(&placement, |_| viewport_options(), cx);

        assert_eq!(report.len(), 2);
        assert!(!report.is_empty());
        assert!(report.all_opened());
        assert_eq!(report.opened_count(), 2);
        assert_eq!(report.unavailable_count(), 0);
        assert!(matches!(
            report.outcome_for_space(&detached),
            Some(DockSurfaceViewportOpenOutcome::Opened(opened))
                if opened.space() == &detached
        ));
        assert_eq!(
            surface.registered_viewport_spaces(cx),
            vec![detached, main.clone()]
        );
    });
}

#[open_gpui::test]
fn surface_saved_placement_restore_reports_invalid_placement_without_opening(
    cx: &mut open_gpui::TestAppContext,
) {
    cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel_factory("editor", "Editor", test_panel)
            .allow_platform_viewports(true)
            .build(cx)
            .expect("surface layout should validate");
        let mut placement = DockViewportPlacementLayout::new(vec![DockViewportPlacement {
            space: "main".into(),
            display_id: None,
            window_bounds: None,
            host_bounds: None,
        }]);
        placement.placement_version = 0;
        let before_windows = cx.windows().len();
        let mut fallback_calls = 0;

        let report = surface.open_viewports_from_saved_placement(
            &placement,
            |_| {
                fallback_calls += 1;
                viewport_options()
            },
            cx,
        );

        assert_eq!(report.len(), 1);
        assert_eq!(report.opened_count(), 0);
        assert_eq!(report.unavailable_count(), 1);
        assert!(matches!(
            report.outcome_for_space(&DockSpaceId::from("main")),
            Some(DockSurfaceViewportOpenOutcome::Unavailable(
                DockSurfaceViewportUnavailable::InvalidPlacement { .. }
            ))
        ));
        let unavailable = report
            .outcome_for_space(&DockSpaceId::from("main"))
            .and_then(DockSurfaceViewportOpenOutcome::unavailable)
            .expect("invalid placement should be reported as unavailable");
        assert!(unavailable.is_invalid_placement());
        assert!(matches!(
            unavailable.placement_validation_error(),
            Some(
                crate::DockViewportPlacementValidationError::UnsupportedVersion {
                    expected: 1,
                    found: 0,
                }
            )
        ));
        assert_eq!(fallback_calls, 0);
        assert_eq!(cx.windows().len(), before_windows);
        assert!(surface.registered_viewport_spaces(cx).is_empty());
    });
}

#[open_gpui::test]
fn surface_restore_readiness_reports_invalid_placement_without_fallback(
    cx: &mut open_gpui::TestAppContext,
) {
    cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel_factory("editor", "Editor", test_panel)
            .allow_platform_viewports(true)
            .build(cx)
            .expect("surface layout should validate");
        let mut placement = DockViewportPlacementLayout::new(vec![DockViewportPlacement {
            space: "main".into(),
            display_id: None,
            window_bounds: None,
            host_bounds: None,
        }]);
        placement.placement_version = 0;
        let before_windows = cx.windows().len();
        let mut fallback_calls = 0;

        let report = surface.viewports().check_restore_readiness(
            &placement,
            |_| {
                fallback_calls += 1;
                viewport_options()
            },
            cx,
        );

        assert_eq!(report.len(), 1);
        assert_eq!(report.openable_count(), 0);
        assert_eq!(report.unavailable_count(), 1);
        let readiness = report
            .entries()
            .iter()
            .find(|readiness| readiness.space() == &DockSpaceId::from("main"))
            .expect("invalid placement should still be keyed by saved space");
        assert!(readiness.status().is_invalid_placement());
        match readiness.status() {
            DockSurfaceViewportReadinessStatus::InvalidPlacement { error } => {
                assert!(matches!(
                    error,
                    crate::DockViewportPlacementValidationError::UnsupportedVersion {
                        expected: 1,
                        found: 0,
                    }
                ));
            }
            other => panic!("expected invalid placement readiness, got {other:?}"),
        }
        assert_eq!(fallback_calls, 0);
        assert_eq!(cx.windows().len(), before_windows);
        assert!(surface.registered_viewport_spaces(cx).is_empty());
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

        surface.set_viewport_close_policy(DockViewportClosePolicy::Prevent, cx);
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
        assert!(!surface.is_viewport_open(&detached, cx));
        assert_eq!(
            surface.registered_viewport_spaces(cx),
            Vec::<DockSpaceId>::new()
        );

        cx.read_entity(&surface.controller(cx), |controller, _| {
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
fn surface_viewport_lifecycle_commits_one_typed_event_per_root_operation(
    cx: &mut open_gpui::TestAppContext,
) {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let observed = changes.clone();
    let (surface, opened, _subscription) = cx.update(|cx| {
        let main = DockSpaceId::from("main");
        let detached = DockSpaceId::from("detached");
        let surface = DockSurface::builder(main.clone())
            .try_layout(&two_space_layout())
            .expect("test layout should validate")
            .panel_factory("main-panel", "Main", test_panel)
            .panel_factory("detached-a", "Detached A", test_panel)
            .allow_platform_viewports(true)
            .close_policy(DockViewportClosePolicy::MergeBack {
                target_space: main.clone(),
            })
            .build(cx)
            .expect("surface layout should validate");
        let subscription = surface.subscribe_changes(cx, move |event, _| {
            observed.borrow_mut().push(event.clone());
        });
        let opened = match surface.open_viewport(detached, viewport_options(), cx) {
            DockSurfaceViewportOpenOutcome::Opened(opened) => opened,
            other => panic!("expected detached viewport to open, got {other:?}"),
        };
        (surface, opened, subscription)
    });

    let _ = cx.update(|cx| {
        let window_id = opened.window().window_id();
        let should_close = surface.handle_viewport_window_should_close(window_id, cx);
        assert!(should_close.allows_close());
        let _ = surface.handle_viewport_window_closed(window_id, cx);
    });
    cx.run_until_parked();

    let changes = changes.borrow();
    assert_eq!(changes.len(), 2);
    assert_eq!(
        changes[0].categories(),
        &[DockSurfaceChangeCategory::ViewportTopology]
    );
    assert_eq!(
        changes[1].categories(),
        &[
            DockSurfaceChangeCategory::Layout,
            DockSurfaceChangeCategory::Selection,
            DockSurfaceChangeCategory::PanelLifecycle,
            DockSurfaceChangeCategory::ViewportTopology,
        ]
    );
    assert_eq!(cx.read(|cx| surface.revision(cx)), 2);
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

        cx.read_entity(&surface.controller(cx), |controller, _| {
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

        cx.read_entity(&surface.controller(cx), |controller, _| {
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

        cx.read_entity(&surface.controller(cx), |controller, _| {
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

        cx.read_entity(&surface.controller(cx), |controller, _| {
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
