use crate::{
    DockNodeId, DockPolicy, DockSpaceId, DockViewportAdapter, DockViewportCoordinateSpaceRecord,
    DockViewportDropPayload, DockViewportDropRoute, DockViewportDropRouteRequest,
    DockViewportLifecycleRecord, DockViewportPlatformSignals, DockViewportRouteSelectionRecord,
    DockViewportRouteSelectionSource, DockViewportRuntimeStatus, DockViewportTargetContext,
    DockViewportTargetHit, DockViewportWindowFacts,
    viewport_test_support::{bounds, handle, item, register_viewport, space},
};
use open_gpui::{AnyWindowHandle, WindowBounds, point, px};
use slotmap::Key;

fn signals_with_receiver(
    target_context: DockViewportTargetContext,
    receiver: AnyWindowHandle,
) -> DockViewportPlatformSignals {
    DockViewportPlatformSignals::from_target_context(target_context)
        .with_event_receiver_window(receiver)
}

#[test]
fn local_only_receiver_match_records_trusted_hovered_route_and_coordinate_status() {
    let source = space("source");
    let target = space("target");
    let source_window = handle(1);
    let target_window = handle(2);
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, source.clone(), source_window);
    register_viewport(&mut adapter, target.clone(), target_window);
    adapter.update_snapshot(
        &source,
        DockViewportWindowFacts::local_only_window_bounds_for_test(WindowBounds::Windowed(bounds(
            0.0, 0.0, 320.0, 240.0,
        ))),
        bounds(0.0, 0.0, 320.0, 240.0),
    );
    adapter.update_snapshot(
        &target,
        DockViewportWindowFacts::local_only_window_bounds_for_test(WindowBounds::Windowed(bounds(
            0.0, 0.0, 320.0, 240.0,
        ))),
        bounds(10.0, 20.0, 300.0, 200.0),
    );
    let request = DockViewportDropRouteRequest::from_platform_signals(
        source,
        DockNodeId::null(),
        DockViewportDropPayload::Item(item("a")),
        point(px(30.0), px(50.0)),
        None,
        signals_with_receiver(
            DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
            target_window,
        )
        .with_global_window_bounds(false),
    );

    let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

    assert_eq!(
        route,
        DockViewportDropRoute::KnownViewport {
            target: DockViewportTargetHit::with_facts_generation(
                target.clone(),
                target_window,
                point(px(20.0), px(30.0)),
                1,
            ),
            source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
        }
    );
    let mut status = DockViewportRuntimeStatus::default();
    status.record_route(&request, &route, None);
    let route_record = status
        .last_route
        .as_ref()
        .expect("route should be recorded");
    assert_eq!(
        route_record.selection_source,
        Some(DockViewportRouteSelectionRecord::TrustedHoveredWindow),
        "local-only cross-window routing must stay attributed to trusted hovered-window facts"
    );

    let target_snapshot = adapter
        .snapshot(&target)
        .expect("target viewport should be registered");
    let lifecycle = DockViewportLifecycleRecord::from_snapshot(target, target_snapshot);
    assert_eq!(
        lifecycle
            .coordinate_status
            .map(|status| status.coordinate_space),
        Some(DockViewportCoordinateSpaceRecord::WindowLocal),
        "diagnostics should show that this successful route did not use global rectangle bounds"
    );
}

#[test]
fn local_only_receiver_mismatch_rejects_cross_viewport_route() {
    let source = space("source");
    let target = space("target");
    let source_window = handle(1);
    let target_window = handle(2);
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, source.clone(), source_window);
    register_viewport(&mut adapter, target, target_window);
    adapter.update_snapshot(
        &source,
        DockViewportWindowFacts::local_only_window_bounds_for_test(WindowBounds::Windowed(bounds(
            0.0, 0.0, 320.0, 240.0,
        ))),
        bounds(0.0, 0.0, 320.0, 240.0),
    );
    adapter.update_snapshot(
        &space("target"),
        DockViewportWindowFacts::local_only_window_bounds_for_test(WindowBounds::Windowed(bounds(
            0.0, 0.0, 320.0, 240.0,
        ))),
        bounds(10.0, 20.0, 300.0, 200.0),
    );
    let request = DockViewportDropRouteRequest::from_platform_signals(
        source,
        DockNodeId::null(),
        DockViewportDropPayload::Item(item("a")),
        point(px(30.0), px(50.0)),
        None,
        signals_with_receiver(
            DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
            source_window,
        )
        .with_global_window_bounds(false),
    );

    assert_eq!(
        adapter.resolve_payload_drop_route(&request, &DockPolicy::default()),
        DockViewportDropRoute::Unavailable,
        "trusted hovered-window ids are not enough when event coordinates belong to another window"
    );
}

#[test]
fn global_screen_rectangle_route_records_front_to_back_fallback_source() {
    let source = DockSpaceId::from("source");
    let target = DockSpaceId::from("target");
    let source_window = handle(1);
    let target_window = handle(2);
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, source.clone(), source_window);
    register_viewport(&mut adapter, target.clone(), target_window);
    adapter.update_snapshot(
        &source,
        DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
            0.0, 0.0, 320.0, 240.0,
        ))),
        bounds(0.0, 0.0, 320.0, 240.0),
    );
    adapter.update_snapshot(
        &target,
        DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
            400.0, 0.0, 320.0, 240.0,
        ))),
        bounds(10.0, 20.0, 300.0, 200.0),
    );
    let request = DockViewportDropRouteRequest::from_platform_signals(
        source,
        DockNodeId::null(),
        DockViewportDropPayload::Item(item("a")),
        point(px(430.0), px(50.0)),
        None,
        DockViewportPlatformSignals::from_target_context(
            DockViewportTargetContext::new().with_window_stack([target_window, source_window]),
        ),
    );

    let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

    assert_eq!(
        route,
        DockViewportDropRoute::KnownViewport {
            target: DockViewportTargetHit::with_facts_generation(
                target,
                target_window,
                point(px(20.0), px(30.0)),
                1,
            ),
            source: DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
        }
    );
    let mut status = DockViewportRuntimeStatus::default();
    status.record_route(&request, &route, None);
    assert_eq!(
        status
            .last_route
            .as_ref()
            .and_then(|route| route.selection_source),
        Some(DockViewportRouteSelectionRecord::FrontToBackWindowStackFallback)
    );
}

// Mechanical migration: route viewport runtime suites.
mod runtime_suite {
    #![allow(dead_code, unused_imports)]

    use crate::{
        DockActionApplyError, DockActionOutcome, DockClassId, DockController, DockDropDelivery,
        DockFloatingContainer, DockGraph, DockHost, DockItemId, DockNode, DockPanel,
        DockPolicyError, DockSpaceId, DockViewportAdapter, DockViewportClosePolicy,
        DockViewportCloseStatus, DockViewportDropPayload, DockViewportDropRoute,
        DockViewportDropRouteOutcome, DockViewportDropRouteRequest, DockViewportFocusCommand,
        DockViewportFocusRequest, DockViewportInputStatus, DockViewportOpenStatus,
        DockViewportPlatformSyncAction, DockViewportPlatformSyncRequest,
        DockViewportPlatformSyncSkippedReason, DockViewportResolvedDropRoute,
        DockViewportRouteStatus, DockViewportRouteTarget, DockViewportRuntime,
        DockViewportRuntimeHandle, DockViewportShouldCloseStatus, DockViewportTargetContext,
        DockViewportTearOffOpenOutcome, DockViewportTearOffOutcomeKind,
        DockViewportTearOffPlacementSource, DockViewportTearOffRequest,
        DockViewportWindowActivation, DockViewportWindowFacts, DockWorkspace, SplitAxis,
        drag::{DockDragPayload, DockDragTearOffGeometry},
        drop_runtime::DockHostDropSceneFact,
        drop_target::DockLeafDropTarget,
        host_test_support::*,
        interaction::DockPayloadDropReleaseOrigin,
        viewport_activation::{
            DockViewportActivationApplyOutcome, DockViewportActivationBackendFocusApply,
            DockViewportActivationBackendFocusObservation,
            DockViewportActivationBackendFocusRecordEffect,
            DockViewportActivationPendingBackendFocusEffect, apply_viewport_activation_transaction,
        },
        viewport_registry::{
            DockViewportInputMask, DockViewportRouteUnavailableReason, DockViewportStaleReason,
        },
        viewport_tear_off::{DockViewportTearOffBeginOutcome, DockViewportTearOffCancelReason},
        viewport_test_support::{handle, register_viewport},
    };
    use open_gpui::{
        AnyWindowHandle, AppContext as _, Focusable, SharedString, TestAppContext, TitlebarOptions,
        VisualTestContext, WindowBounds, WindowHandle, WindowId, WindowOptions, point, px, size,
    };

    use crate::host_viewport_runtime_test_support::*;

    #[open_gpui::test]
    fn viewport_runtime_requires_backend_route_selection_for_drop(cx: &mut TestAppContext) {
        let source_space = DockSpaceId::from("source");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        let root = graph.insert_node(DockNode::Split {
            axis: crate::SplitAxis::Horizontal,
            children: vec![source_tabs, target_tabs],
            fractions: vec![0.5, 0.5],
        });
        graph.set_root(source_space.clone(), root);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller);
        let window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
        let host_position = center_drop_position(host_bounds);
        let opened = cx
            .update(|app| {
                runtime.open_viewport(
                    source_space.clone(),
                    WindowOptions {
                        window_bounds: Some(window_bounds),
                        focus: false,
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("source viewport should open through runtime");
        let source_window = opened.window();

        assert!(runtime.begin_viewport_host_scene(
            source_space.clone(),
            source_window.window_id(),
            DockViewportWindowFacts::from_window_bounds(window_bounds),
            host_bounds,
            host_position,
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &source_space,
            source_window.window_id(),
            leaf_host_scene_fact(root, target_tabs),
        ));

        let request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            screen_position_for_host_position(window_bounds, host_position),
            None,
            DockViewportTargetContext::new(),
        );
        let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));

        assert_eq!(
            resolution.route(),
            &DockViewportDropRoute::Unavailable,
            "fresh live-window facts must not route without a backend route selection signal"
        );
        assert!(
            resolution.routed_preview_target_snapshot().is_none(),
            "route-selection-free result must not carry a routed preview target"
        );
        assert!(
            resolution.delivery().is_none(),
            "fallback route must not mint delivery without current route facts"
        );

        let trusted_request = DockViewportDropRouteRequest::from_target_context(
            source_space,
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            screen_position_for_host_position(window_bounds, host_position),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(source_window),
        );
        let trusted_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&trusted_request, app));
        assert!(
            matches!(
                trusted_resolution.route(),
                DockViewportDropRoute::Local {
                    host_position: route_host_position,
                    window_id,
                    source: crate::DockViewportRouteSelectionSource::TrustedHoveredWindow,
                    ..
                } if *route_host_position == host_position && *window_id == source_window.window_id()
            ),
            "trusted hovered live-window facts should route with trusted-hovered selection source"
        );
        assert!(
            trusted_resolution
                .routed_preview_target_snapshot()
                .is_some(),
            "trusted route should carry a preview target"
        );
        assert!(
            trusted_resolution.delivery().is_some(),
            "trusted route should mint delivery from current route facts"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_rejects_cached_local_delivery_after_window_facts_go_stale(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        let root = graph.insert_node(DockNode::Split {
            axis: crate::SplitAxis::Horizontal,
            children: vec![source_tabs, target_tabs],
            fractions: vec![0.5, 0.5],
        });
        graph.set_root(source_space.clone(), root);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        let controller = cx.new(|_| DockController::new(workspace));

        let source_window = handle(32);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source_space.clone(), source_window);
        let mut runtime = DockViewportRuntime::from_adapter(
            controller.clone(),
            adapter,
            DockViewportClosePolicy::RetainLayout,
        );

        let window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
        let host_position = center_drop_position(host_bounds);
        assert!(runtime.begin_viewport_host_scene(
            source_space.clone(),
            source_window.window_id(),
            DockViewportWindowFacts::from_window_bounds(window_bounds),
            host_bounds,
            host_position,
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &source_space,
            source_window.window_id(),
            leaf_host_scene_fact(root, target_tabs),
        ));
        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);

        let request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            screen_position_for_host_position(window_bounds, host_position),
            None,
            crate::DockViewportPlatformSignals::from_target_context(
                DockViewportTargetContext::new().with_trusted_hovered_window(source_window),
            )
            .with_event_receiver_window(source_window)
            .with_global_window_bounds(true),
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(session.clone()));
        let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        assert!(
            matches!(resolution.route(), DockViewportDropRoute::Local { .. }),
            "fresh source viewport facts should resolve a local route, got {:?}",
            resolution.route()
        );
        assert!(
            resolution.routed_preview_target_snapshot().is_some(),
            "fresh local route should carry a preview target"
        );
        assert!(
            resolution.delivery().is_some(),
            "fresh local route should mint delivery from current route facts"
        );
        let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        let delivery = resolution.expect_delivery().clone();
        let commit_plan =
            DockDropDelivery::from_resolution(resolution).expect("fresh route should mint a plan");

        assert!(
            runtime
                .mark_viewport_window_snapshot_stale(source_window.window_id())
                .changed()
        );

        let validation = cx.update(|app| runtime.validate_payload_drop_delivery(&delivery, app));
        assert_eq!(validation, Err(DockActionApplyError::DropTargetUnavailable));
        let result =
            cx.update(|app| runtime.deliver_drop_commit_delivery_with_outcome(commit_plan, app));
        assert_eq!(result, Err(DockActionApplyError::DropTargetUnavailable));
        cx.read_entity(&controller, |controller, _| {
            assert_eq!(
                controller.graph().collect_items_in_space(&source_space),
                vec![item("a"), item("b")]
            );
            let DockNode::Tabs { items, selected } = controller
                .graph()
                .node(source_tabs)
                .expect("source tabs should still exist")
            else {
                panic!("source should remain tabs");
            };
            assert_eq!(items, &vec![item("a")]);
            assert_eq!(selected.as_ref(), items.first());
            let DockNode::Tabs { items, selected } = controller
                .graph()
                .node(target_tabs)
                .expect("target tabs should still exist")
            else {
                panic!("target should remain tabs");
            };
            assert_eq!(items, &vec![item("b")]);
            assert_eq!(selected.as_ref(), items.first());
        });
    }

    #[open_gpui::test]
    fn viewport_runtime_source_only_release_ignores_creation_z_order_fallback(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller);

        let target_window_bounds =
            WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let target_opened = cx
            .update(|app| {
                runtime.open_viewport(
                    target_space.clone(),
                    WindowOptions {
                        window_bounds: Some(target_window_bounds),
                        focus: false,
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("target viewport should open without taking focus");

        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(target_window_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            point(px(120.0), px(100.0)),
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_opened.window().window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));

        let platform_signals = cx.update(|app| {
            crate::DockViewportPlatformSignals::from_app_without_hovered_window_signal(app)
        });
        let request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source_space,
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            point(px(220.0), px(200.0)),
            None,
            platform_signals,
            DockPayloadDropReleaseOrigin::SourceOnly,
        );
        let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));

        assert_eq!(
            resolution.route(),
            &DockViewportDropRoute::Unavailable,
            "source-only release must not use viewport creation z-order as a cross-viewport route selection"
        );
        assert!(resolution.delivery().is_none());
    }

    #[open_gpui::test]
    fn viewport_runtime_source_only_release_uses_current_backend_fallback_not_last_routed_viewport(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller.clone());

        let source_window_bounds =
            WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let target_window_bounds =
            WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let source_opened = cx
            .update(|app| {
                runtime.open_viewport(
                    source_space.clone(),
                    WindowOptions {
                        window_bounds: Some(source_window_bounds),
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("source viewport should open");
        let target_opened = cx
            .update(|app| {
                runtime.open_viewport(
                    target_space.clone(),
                    WindowOptions {
                        window_bounds: Some(target_window_bounds),
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("target viewport should open");
        source_opened
            .window()
            .update(cx, |_, window, _| window.activate_window())
            .expect("source viewport should be activatable");
        cx.run_until_parked();

        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(target_window_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            point(px(120.0), px(100.0)),
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_opened.window().window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));
        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let preview_request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            point(px(220.0), px(200.0)),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(target_opened.window()),
        );
        let resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
        cx.update(|app| {
            runtime.update_routed_drop_preview(&resolution, &payload, app);
        });
        assert!(
            runtime
                .routed_drop_preview_for(&target_space, target_opened.window().window_id())
                .is_some(),
            "preview should store routed delivery for the hovered target"
        );

        let release_position = point(px(220.0), px(200.0));
        let request_without_hovered_or_stack = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_position,
            None,
            DockViewportTargetContext::new(),
        );
        let geometry_only_route = cx.update(|app| {
            runtime
                .resolve_payload_drop_delivery(&request_without_hovered_or_stack, app)
                .route()
                .clone()
        });
        let request_with_stack = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_position,
            None,
            DockViewportTargetContext::new()
                .with_window_stack([source_opened.window(), target_opened.window()]),
        );
        let stack_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&request_with_stack, app));
        assert_eq!(
            geometry_only_route,
            DockViewportDropRoute::Unavailable,
            "empty target context must not use geometry or reuse preview state as route selection"
        );

        assert_eq!(
            stack_resolution.route(),
            &DockViewportDropRoute::Local {
                host_position: point(px(120.0), px(100.0)),
                window_id: source_opened.window().window_id(),
                facts_generation: 1,
                source: crate::DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
            },
            "window-stack fallback must use the current stack instead of reusing the previewed target"
        );
        assert!(
            stack_resolution.routed_preview_target_snapshot().is_some(),
            "current stack fallback should still resolve a preview target"
        );
        assert!(
            stack_resolution.delivery().is_some(),
            "current stack fallback should mint delivery from current route facts"
        );

        let request_with_hovered = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_position,
            None,
            DockViewportTargetContext::new()
                .with_trusted_hovered_window(target_opened.window())
                .with_window_stack([source_opened.window(), target_opened.window()]),
        );
        let hovered_route = cx.update(|app| {
            runtime
                .resolve_payload_drop_delivery(&request_with_hovered, app)
                .route()
                .clone()
        });
        assert!(
            matches!(
                hovered_route,
                DockViewportDropRoute::KnownViewport { target, .. }
                    if target.window_id() == target_opened.window().window_id()
            ),
            "a current hovered signal should still select the target viewport"
        );

        cx.update(|app| {
            assert!(runtime.clear_routed_drop_preview(app));
        });
        assert_eq!(
            runtime.routed_drop_preview_for(&target_space, target_opened.window().window_id()),
            None
        );

        let source_only_request = DockViewportDropRouteRequest::from_platform_signals(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_position,
            None,
            crate::DockViewportPlatformSignals::from_target_context(
                DockViewportTargetContext::new()
                    .with_window_stack([source_opened.window(), target_opened.window()]),
            ),
        );
        let source_only_route = cx.update(|app| {
            runtime
                .resolve_payload_drop_delivery(&source_only_request, app)
                .route()
                .clone()
        });
        assert_eq!(
            source_only_route,
            stack_resolution.route().clone(),
            "source-only release must use the current window-stack fallback route, not last-routed preview state"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_source_only_release_commits_current_trusted_hovered_window(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller.clone());

        let source_window_bounds =
            WindowBounds::Windowed(floating_bounds(520.0, 100.0, 360.0, 220.0));
        let target_window_bounds =
            WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let source_opened = cx
            .update(|app| {
                runtime.open_viewport(
                    source_space.clone(),
                    WindowOptions {
                        window_bounds: Some(source_window_bounds),
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("source viewport should open");
        let target_opened = cx
            .update(|app| {
                runtime.open_viewport(
                    target_space.clone(),
                    WindowOptions {
                        window_bounds: Some(target_window_bounds),
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("target viewport should open");

        let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
        let host_position = point(px(120.0), px(100.0));
        let release_position =
            screen_position_for_host_position(target_window_bounds, host_position);
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(target_window_bounds),
            host_bounds,
            host_position,
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_opened.window().window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = cx.update(|app| runtime.begin_payload_drag_with_app(&payload, app));
        let preview_request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_position,
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(target_opened.window()),
        )
        .with_drag_session(Some(session.clone()));
        let preview_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
        cx.update(|app| {
            runtime.update_routed_drop_preview(&preview_resolution, &payload, app);
        });

        cx.set_platform_window_stack(Some(vec![source_opened.window(), target_opened.window()]));
        cx.set_platform_hovered_window(Some(target_opened.window()));
        let release_signals = cx.update(|app| {
            crate::DockViewportPlatformSignals::from_app(app).with_frozen_target_context()
        });
        cx.set_platform_hovered_window(None);
        let release_request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_position,
            None,
            release_signals,
            DockPayloadDropReleaseOrigin::SourceOnly,
        )
        .with_drag_session(Some(session));
        let outcome = cx
            .update(|app| runtime.commit_payload_drop_from_screen(&release_request, app))
            .expect("source-only release with current trusted hovered-window facts should commit");

        let DockViewportDropRouteOutcome::Action(action) = outcome else {
            panic!("source-only release should commit as an action");
        };
        assert_eq!(action.action(), DockActionOutcome::Changed);
        cx.read_entity(&controller, |controller, _| {
            assert_eq!(controller.graph().collect_items_in_space(&source_space), []);
            assert_eq!(
                controller.graph().collect_items_in_space(&target_space),
                vec![item("a"), item("b")]
            );
        });
    }

    #[open_gpui::test]
    fn viewport_runtime_hovered_host_release_uses_last_hovered_viewport_when_hover_backend_unavailable(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        let controller = cx.new(|_| DockController::new(workspace));
        let source_window = handle(1);
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source_space.clone(), source_window);
        register_viewport(&mut adapter, target_space.clone(), target_window);
        let mut runtime = DockViewportRuntime::from_adapter(
            controller,
            adapter,
            DockViewportClosePolicy::RetainLayout,
        );

        let source_window_bounds = WindowBounds::Windowed(floating_bounds(0.0, 0.0, 360.0, 220.0));
        let target_window_bounds =
            WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));

        let target_host_position = point(px(120.0), px(100.0));
        let target_screen_position = point(
            target_window_bounds.get_bounds().origin.x + target_host_position.x,
            target_window_bounds.get_bounds().origin.y + target_host_position.y,
        );
        assert!(runtime.begin_viewport_host_scene(
            source_space.clone(),
            source_window.window_id(),
            DockViewportWindowFacts::from_window_bounds(source_window_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            point(px(0.0), px(0.0)),
        ));
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_window.window_id(),
            DockViewportWindowFacts::from_window_bounds(target_window_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            target_host_position,
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_window.window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);
        let preview_request = hovered_window_route_request_for_test(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_screen_position,
            None,
            target_window,
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(session.clone()));
        let preview_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
        runtime.update_routed_drop_preview(&preview_resolution, &payload);

        let release_request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source_space,
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_screen_position,
            None,
            crate::DockViewportPlatformSignals::from_target_context(
                DockViewportTargetContext::new(),
            )
            .with_event_receiver_window(target_window),
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(session));
        let release_resolution = cx
            .update(|app| runtime.resolve_payload_drop_delivery_for_request(&release_request, app));

        assert!(
            matches!(
                release_resolution.route(),
                DockViewportDropRoute::KnownViewport { target, source }
                    if target.window_id() == target_window.window_id()
                        && target.host_position() == target_host_position
                        && *source
                            == crate::DockViewportRouteSelectionSource::DragLastHoveredViewportFallback
            ),
            "when hovered-window signal is unavailable, active drag should reuse the last hovered viewport as mouse reference; got {:?}",
            release_resolution.route()
        );
        assert!(
            release_resolution.delivery().is_some(),
            "last-hovered viewport fallback should mint delivery from current route facts"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_hovered_host_release_ignores_last_hovered_viewport_from_stale_drag_session(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        let controller = cx.new(|_| DockController::new(workspace));
        let source_window = handle(1);
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source_space.clone(), source_window);
        register_viewport(&mut adapter, target_space.clone(), target_window);
        let mut runtime = DockViewportRuntime::from_adapter(
            controller,
            adapter,
            DockViewportClosePolicy::RetainLayout,
        );

        let source_window_bounds = WindowBounds::Windowed(floating_bounds(0.0, 0.0, 360.0, 220.0));
        let target_window_bounds =
            WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let target_host_position = point(px(120.0), px(100.0));
        let target_screen_position = point(
            target_window_bounds.get_bounds().origin.x + target_host_position.x,
            target_window_bounds.get_bounds().origin.y + target_host_position.y,
        );
        assert!(runtime.begin_viewport_host_scene(
            source_space.clone(),
            source_window.window_id(),
            DockViewportWindowFacts::from_window_bounds(source_window_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            point(px(0.0), px(0.0)),
        ));
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_window.window_id(),
            DockViewportWindowFacts::from_window_bounds(target_window_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            target_host_position,
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_window.window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let stale_session = runtime.begin_payload_drag(&payload);
        let preview_request = hovered_window_route_request_for_test(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_screen_position,
            None,
            target_window,
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(stale_session.clone()));
        let preview_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
        runtime.update_routed_drop_preview(&preview_resolution, &payload);
        assert!(
            runtime.finish_payload_drag(&stale_session).changed(),
            "ending the drag should clear last-hovered viewport fallback"
        );

        let release_request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source_space,
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_screen_position,
            None,
            crate::DockViewportPlatformSignals::from_target_context(
                DockViewportTargetContext::new(),
            )
            .with_event_receiver_window(target_window),
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(stale_session));
        let release_resolution = cx
            .update(|app| runtime.resolve_payload_drop_delivery_for_request(&release_request, app));

        assert_eq!(
            release_resolution.route(),
            &DockViewportDropRoute::Unavailable,
            "stale drag sessions must not reuse last-hovered viewport fallback source"
        );
        assert!(
            release_resolution.delivery().is_none(),
            "stale last-hovered fallback must not mint delivery"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_source_only_release_does_not_use_last_hovered_viewport_as_route_authority(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        let controller = cx.new(|_| DockController::new(workspace));
        let source_window = handle(1);
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source_space.clone(), source_window);
        register_viewport(&mut adapter, target_space.clone(), target_window);
        let mut runtime = DockViewportRuntime::from_adapter(
            controller,
            adapter,
            DockViewportClosePolicy::RetainLayout,
        );

        let target_window_bounds =
            WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let target_host_position = point(px(120.0), px(100.0));
        let target_screen_position = point(
            target_window_bounds.get_bounds().origin.x + target_host_position.x,
            target_window_bounds.get_bounds().origin.y + target_host_position.y,
        );
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_window.window_id(),
            DockViewportWindowFacts::from_window_bounds(target_window_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            target_host_position,
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_window.window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);
        let preview_request = hovered_window_route_request_for_test(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_screen_position,
            None,
            target_window,
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(session.clone()));
        let preview_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
        runtime.update_routed_drop_preview(&preview_resolution, &payload);

        let release_request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source_space,
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_screen_position,
            None,
            crate::DockViewportPlatformSignals::from_target_context(
                DockViewportTargetContext::new(),
            ),
            DockPayloadDropReleaseOrigin::SourceOnly,
        )
        .with_drag_session(Some(session));
        let release_resolution = cx
            .update(|app| runtime.resolve_payload_drop_delivery_for_request(&release_request, app));

        assert_eq!(
            release_resolution.route(),
            &DockViewportDropRoute::Unavailable,
            "source-only releases must not treat last hovered viewport as fresh hovered-window signal"
        );
        assert!(
            release_resolution.delivery().is_none(),
            "source-only last-hovered fallback must not mint cross-viewport delivery"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_source_only_release_retargets_current_position(cx: &mut TestAppContext) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller.clone());

        let source_opened = cx
            .update(|app| {
                runtime.open_viewport(
                    source_space.clone(),
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(floating_bounds(
                            0.0, 0.0, 360.0, 220.0,
                        ))),
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("source viewport should open");
        let target_opened = cx
            .update(|app| {
                runtime.open_viewport(
                    target_space.clone(),
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(floating_bounds(
                            100.0, 100.0, 360.0, 220.0,
                        ))),
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("target viewport should open");

        let target_window_bounds = target_opened
            .window()
            .update(cx, |_, window, _| window.window_bounds())
            .expect("target window should be live");
        let target_window_bounds = WindowBounds::Windowed(target_window_bounds.get_bounds());
        let target_leaf_bounds = floating_bounds(0.0, 0.0, 180.0, 120.0);
        let preview_host_position = center_drop_position(target_leaf_bounds);
        let preview_screen_position = point(
            target_window_bounds.get_bounds().origin.x + preview_host_position.x,
            target_window_bounds.get_bounds().origin.y + preview_host_position.y,
        );
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(target_window_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            preview_host_position,
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_opened.window().window_id(),
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: target_tabs,
                target_tabs,
                bounds: target_leaf_bounds,
                is_central: false,
            }),
        ));

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);
        let preview_request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            preview_screen_position,
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(target_opened.window()),
        )
        .with_drag_session(Some(session.clone()));
        let resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
        cx.update(|app| {
            runtime.update_routed_drop_preview(&resolution, &payload, app);
        });
        assert!(
            runtime.has_routed_drop_preview_for_drag_session(Some(&session)),
            "preview should cache a routed delivery before release"
        );
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(target_window_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            preview_host_position,
        ));

        let release_request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            screen_position_for_host_position(
                target_window_bounds,
                center_drop_position(floating_bounds(180.0, 120.0, 180.0, 100.0)),
            ),
            None,
            crate::DockViewportPlatformSignals::from_target_context(
                DockViewportTargetContext::new()
                    .with_window_stack([target_opened.window(), source_opened.window()]),
            ),
            DockPayloadDropReleaseOrigin::SourceOnly,
        )
        .with_drag_session(Some(session.clone()));
        let release_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&release_request, app));
        assert_eq!(
            release_resolution.route(),
            &DockViewportDropRoute::Unavailable,
            "release should be retargeted to the current point instead of reusing cached host_position"
        );
        let result = DockDropDelivery::from_resolution(release_resolution);
        assert_eq!(result, Err(DockActionApplyError::DropTargetUnavailable));
        cx.read_entity(&controller, |controller, _| {
            assert_eq!(
                controller.graph().collect_items_in_space(&source_space),
                vec![item("a")]
            );
            assert_eq!(
                controller.graph().collect_items_in_space(&target_space),
                vec![item("b")]
            );
        });
    }

    #[open_gpui::test]
    fn viewport_runtime_source_only_release_requires_current_route_facts(cx: &mut TestAppContext) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        let controller = cx.new(|_| DockController::new(workspace));

        let target_window = handle(90);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        let mut runtime = DockViewportRuntime::from_adapter(
            controller,
            adapter,
            DockViewportClosePolicy::RetainLayout,
        );

        let target_window_bounds =
            WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_window.window_id(),
            DockViewportWindowFacts::from_window_bounds(target_window_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            point(px(120.0), px(100.0)),
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_window.window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);
        let preview_request = hovered_window_route_request_for_test(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            point(px(220.0), px(200.0)),
            None,
            target_window,
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(session.clone()));
        let preview_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
        let update = runtime.update_routed_drop_preview(&preview_resolution, &payload);
        assert!(update.changed());

        let release_request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source_space,
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            point(px(220.0), px(200.0)),
            None,
            crate::DockViewportPlatformSignals::from_target_context(
                DockViewportTargetContext::new(),
            ),
            DockPayloadDropReleaseOrigin::SourceOnly,
        )
        .with_drag_session(Some(session));
        let release_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&release_request, app));

        assert_eq!(
            release_resolution.route(),
            &DockViewportDropRoute::Unavailable,
            "routed preview state must not authorize source-only release without current route facts"
        );
        assert!(release_resolution.delivery().is_none());
    }

    #[open_gpui::test]
    fn viewport_runtime_rejects_known_viewport_delivery_without_drag_session(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        let controller = cx.new(|_| DockController::new(workspace));

        let target_window = handle(20);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        let mut runtime = DockViewportRuntime::from_adapter(
            controller.clone(),
            adapter,
            DockViewportClosePolicy::RetainLayout,
        );

        let window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
        let host_position = center_drop_position(host_bounds);
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_window.window_id(),
            DockViewportWindowFacts::from_window_bounds(window_bounds),
            host_bounds,
            host_position,
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_window.window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));

        let request = hovered_window_route_request_for_test(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            screen_position_for_host_position(window_bounds, host_position),
            None,
            target_window,
            DockPayloadDropReleaseOrigin::HoveredHost,
        );
        let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        assert!(
            resolution.routed_preview_target_snapshot().is_some(),
            "fresh known viewport route should carry a preview target"
        );
        assert_eq!(
            DockDropDelivery::from_resolution(resolution.clone())
                .expect("fresh known viewport route should mint delivery from current route facts")
                .workspace_target()
                .map(|target| target.target_window_id()),
            Some(Some(target_window.window_id())),
            "fresh known viewport route should mint delivery from current route facts"
        );
        cx.read_entity(&controller, |controller, _| {
            assert_eq!(
                controller.graph().collect_items_in_space(&source_space),
                vec![item("a")]
            );
            assert_eq!(
                controller.graph().collect_items_in_space(&target_space),
                vec![item("b")]
            );
        });
    }

    #[open_gpui::test]
    fn viewport_runtime_rejects_known_viewport_delivery_from_stale_drag_session(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        let controller = cx.new(|_| DockController::new(workspace));

        let target_window = handle(21);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        let mut runtime = DockViewportRuntime::from_adapter(
            controller.clone(),
            adapter,
            DockViewportClosePolicy::RetainLayout,
        );

        let window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
        let host_position = center_drop_position(host_bounds);
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_window.window_id(),
            DockViewportWindowFacts::from_window_bounds(window_bounds),
            host_bounds,
            host_position,
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_window.window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let stale_session = runtime.begin_payload_drag(&payload);
        let request = hovered_window_route_request_for_test(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            screen_position_for_host_position(window_bounds, host_position),
            None,
            target_window,
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(stale_session.clone()));
        let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        let stale_plan =
            DockDropDelivery::from_resolution(resolution).expect("fresh route should mint a plan");

        let _replacement = runtime.begin_payload_drag(&payload);
        let result =
            cx.update(|app| runtime.deliver_drop_commit_delivery_with_outcome(stale_plan, app));
        assert!(matches!(
            result,
            Err(DockActionApplyError::DropDragSessionStale { session })
                if session == stale_session.id()
        ));
        cx.read_entity(&controller, |controller, _| {
            assert_eq!(
                controller.graph().collect_items_in_space(&source_space),
                vec![item("a")]
            );
            assert_eq!(
                controller.graph().collect_items_in_space(&target_space),
                vec![item("b")]
            );
        });
    }
}

mod handle_suite {
    #![allow(dead_code, unused_imports)]

    use crate::{
        DockAction, DockActionApplyError, DockController, DockDropDelivery, DockGraph,
        DockGraphDropTarget, DockItemId, DockNode, DockNodeId, DockPanel, DockPolicy, DockSpaceId,
        DockViewportClosePolicy, DockViewportCloseStatus, DockViewportDropOutcomeKind,
        DockViewportDropPayload, DockViewportDropRoute, DockViewportDropRouteOutcome,
        DockViewportDropRouteRequest, DockViewportFocusCommand, DockViewportFocusRequest,
        DockViewportInputStatus, DockViewportOpenStatus, DockViewportPlatformSignals,
        DockViewportRouteStatus, DockViewportRuntimeHandle, DockViewportShouldCloseStatus,
        DockViewportStaleStatusReason, DockViewportTargetContext, DockViewportTearOffBeginOutcome,
        DockViewportTearOffCancelReason, DockViewportTearOffOpenOutcome,
        DockViewportTearOffRequest, DockViewportWindowFacts, DockWorkspace, DropZone, SplitAxis,
        debug::DockDebugRegion,
        drag::DockDragPayload,
        drop_preview::DockDropRoutePreviewKind,
        drop_runtime::{DockHostDropScene, DockHostDropSceneFact},
        drop_target::{DockDropResolveSource, DockLeafDropTarget, DockResolvedDropTargetKind},
        host_test_support::*,
        interaction::{
            DockPayloadDropRelease, DockPayloadDropReleaseOrigin, DockRuntimeDragSession,
        },
        viewport_activation::apply_viewport_activation_transaction,
        viewport_registry::{DockViewportRouteUnavailableReason, DockViewportStaleReason},
    };
    use open_gpui::{
        AppContext as _, Focusable, Modifiers, MouseButton, TestAppContext, VisualTestContext,
        WindowBounds, WindowOptions, point, px, size,
    };
    use slotmap::Key;

    use crate::host_viewport_runtime_test_support::*;

    #[open_gpui::test]
    fn resolve_payload_drop_delivery_outcome_reports_backend_route_selection_state_changes(
        cx: &mut TestAppContext,
    ) {
        let (runtime, target_window, request) = backend_route_resolution_fixture(cx);

        let _initial =
            cx.update(|app| runtime.resolve_payload_drop_delivery_outcome(&request, app));
        let settled = cx.update(|app| runtime.resolve_payload_drop_delivery_outcome(&request, app));
        assert!(
            !settled.changed(),
            "settled route selection should not report churn without new backend evidence"
        );

        target_window
            .update(cx, |_, window, _| window.activate_window())
            .expect("target viewport should activate while backend focus is unavailable");
        cx.run_until_parked();
        cx.set_platform_focused_window_available(true);

        let focused = cx.update(|app| runtime.resolve_payload_drop_delivery_outcome(&request, app));
        assert!(
            focused.changed(),
            "backend focus sampled during route selection should be reported as runtime state change"
        );
        let focused_again =
            cx.update(|app| runtime.resolve_payload_drop_delivery_outcome(&request, app));
        assert!(
            !focused_again.changed(),
            "re-sampling the same backend focus should not churn route selection state"
        );
    }

    #[open_gpui::test]
    fn resolve_payload_drop_delivery_for_request_outcome_reports_backend_route_selection_state_changes(
        cx: &mut TestAppContext,
    ) {
        let (runtime, target_window, request) = backend_route_resolution_fixture(cx);

        let _initial = cx
            .update(|app| runtime.resolve_payload_drop_delivery_for_request_outcome(&request, app));
        let settled = cx
            .update(|app| runtime.resolve_payload_drop_delivery_for_request_outcome(&request, app));
        assert!(
            !settled.changed(),
            "settled release route selection should not report churn without new backend evidence"
        );
        assert!(
            settled.resolution().delivery().is_none(),
            "release resolution without current route facts must not expose delivery"
        );

        target_window
            .update(cx, |_, window, _| window.activate_window())
            .expect("target viewport should activate while backend focus is unavailable");
        cx.run_until_parked();
        cx.set_platform_focused_window_available(true);

        let focused = cx
            .update(|app| runtime.resolve_payload_drop_delivery_for_request_outcome(&request, app));
        assert!(
            focused.changed(),
            "backend focus sampled during release route selection should be reported as runtime state change"
        );
        assert!(
            focused.resolution().delivery().is_none(),
            "backend focus resampling alone must not grant delivery"
        );
        let focused_again = cx
            .update(|app| runtime.resolve_payload_drop_delivery_for_request_outcome(&request, app));
        assert!(
            !focused_again.changed(),
            "re-sampling the same backend focus should not churn release route selection state"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_handle_resolves_drop_route_with_current_policy(cx: &mut TestAppContext) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.policy_mut().set_allow_platform_viewports(true);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller);

        let opened = cx
            .update(|app| {
                runtime.open_viewport(
                    target_space.clone(),
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect("target viewport should open through runtime handle");
        let target_window_bounds = opened
            .window()
            .update(cx, |_, window, _| window.window_bounds())
            .expect("target window should be live");
        let target_window_bounds = WindowBounds::Windowed(target_window_bounds.get_bounds());
        let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(target_window_bounds),
            host_bounds,
            point(px(0.0), px(0.0))
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            opened.window().window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));
        let host_position = target_center_host_position();
        let target_point = screen_position_for_host_position(target_window_bounds, host_position);

        let route = cx.update(|app| {
            let request = DockViewportDropRouteRequest::from_platform_signals(
                source_space.clone(),
                source_tabs,
                DockViewportDropPayload::Item(item("a")),
                target_point,
                Some(target_window_bounds),
                DockViewportPlatformSignals::from_app(app)
                    .with_trusted_hovered_window(opened.window()),
            );
            runtime
                .resolve_payload_drop_delivery(&request, app)
                .route()
                .clone()
        });

        let expected_generation = runtime
            .borrow()
            .adapter()
            .snapshot_facts_generation(&target_space, opened.window().window_id())
            .expect("target viewport snapshot should expose the current facts generation");
        assert_eq!(
            route,
            DockViewportDropRoute::KnownViewport {
                target: crate::DockViewportTargetHit::with_facts_generation(
                    target_space.clone(),
                    opened.window(),
                    host_position,
                    expected_generation,
                ),
                source: crate::DockViewportRouteSelectionSource::TrustedHoveredWindow,
            }
        );
        let status = runtime.runtime_status();
        let target = &status
            .last_route
            .as_ref()
            .expect("runtime status should expose the last resolved route")
            .target;
        assert_eq!(target.space(), Some(&target_space));
        assert_eq!(target.window_id(), Some(opened.window().window_id()));
        assert_eq!(target.host_position(), Some(host_position));
    }

    #[open_gpui::test]
    fn viewport_runtime_handle_drop_route_uses_workspace_platform_policy(cx: &mut TestAppContext) {
        let source_space = DockSpaceId::from("source");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        graph.set_root(source_space.clone(), source_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller.clone());
        let release_position = point(px(900.0), px(900.0));

        let rejected_request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_position,
            None,
            DockViewportTargetContext::new(),
        );
        let rejected = cx.update(|app| {
            runtime
                .resolve_payload_drop_delivery(&rejected_request, app)
                .route()
                .clone()
        });
        assert!(
            matches!(
                rejected,
                DockViewportDropRoute::Rejected(crate::DockPolicyError::PlatformViewportsDisabled)
            ),
            "default workspace policy should reject outside-all-viewports route"
        );
        let status = runtime.runtime_status();
        let target = &status
            .last_route
            .as_ref()
            .expect("runtime status should record the rejected route")
            .target;
        assert_eq!(
            target.rejection_reason(),
            Some(crate::DockPolicyError::PlatformViewportsDisabled)
        );

        cx.update_entity(&controller, |controller, _| {
            controller.policy_mut().set_allow_platform_viewports(true);
        });
        let tear_off_request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_position,
            None,
            DockViewportTargetContext::new(),
        );
        let tear_off = cx.update(|app| {
            runtime
                .resolve_payload_drop_delivery(&tear_off_request, app)
                .route()
                .clone()
        });
        assert!(
            matches!(tear_off, DockViewportDropRoute::TearOff),
            "allowed workspace policy should resolve an outside release as tear-off"
        );
        let status = runtime.runtime_status();
        let target = &status
            .last_route
            .as_ref()
            .expect("runtime status should record the tear-off route")
            .target;
        assert_eq!(target.release_position(), Some(release_position));
    }

    #[open_gpui::test]
    fn viewport_runtime_handle_delivers_known_viewport_drop_directly(cx: &mut TestAppContext) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller.clone());

        let opened = cx
            .update(|app| {
                runtime.open_viewport(
                    target_space.clone(),
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect("target viewport should open");
        let target_window_bounds = opened
            .window()
            .update(cx, |_, window, _| window.window_bounds())
            .expect("target window should be live");
        let target_window_bounds = WindowBounds::Windowed(target_window_bounds.get_bounds());
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(target_window_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            target_center_host_position(),
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            opened.window().window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);
        let release_position = runtime
            .last_host_scene_screen_position(&target_space)
            .expect("target scene should expose a screen position");
        let request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_position,
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(opened.window()),
        )
        .with_drag_session(Some(session.clone()));
        let plan = fresh_delivery_for_request(cx, &runtime, &request);
        let result = cx.update(|app| runtime.deliver_drop_commit_delivery(plan, app));

        let DockViewportDropRouteOutcome::Action(action) = result.expect("drop should commit")
        else {
            panic!("known viewport drop should produce a normal action outcome");
        };
        assert_eq!(action.action(), crate::DockActionOutcome::Changed);
        cx.read_entity(&controller, |controller, _| {
            assert_eq!(
                controller.graph().collect_items_in_space(&source_space),
                Vec::<DockItemId>::new(),
            );
            assert_eq!(
                controller.graph().collect_items_in_space(&target_space),
                vec![item("b"), item("a")],
                "direct delivery should process the dock request"
            );
        });
    }

    #[open_gpui::test]
    fn viewport_runtime_handle_delivers_commit_deliveries_directly(cx: &mut TestAppContext) {
        let source_space = DockSpaceId::from("source");
        let first_target_space = DockSpaceId::from("first-target");
        let second_target_space = DockSpaceId::from("second-target");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let first_target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("first")],
            selected: Some(item("first")),
        });
        let second_target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("second")],
            selected: Some(item("second")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(first_target_space.clone(), first_target_tabs);
        graph.set_root(second_target_space.clone(), second_target_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("first"), "First", test_view(cx, "First"));
        workspace.register_panel_view(item("second"), "Second", test_view(cx, "Second"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller.clone());

        let first_target = cx
            .update(|app| {
                runtime.open_viewport(
                    first_target_space.clone(),
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect("first target viewport should open");
        let second_target = cx
            .update(|app| {
                runtime.open_viewport(
                    second_target_space.clone(),
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect("second target viewport should open");

        for (space, window, target_tabs) in [
            (
                &first_target_space,
                first_target.window(),
                first_target_tabs,
            ),
            (
                &second_target_space,
                second_target.window(),
                second_target_tabs,
            ),
        ] {
            let window_bounds = window
                .update(cx, |_, window, _| window.window_bounds())
                .expect("target window should be live");
            assert!(runtime.begin_viewport_host_scene(
                space.clone(),
                window.window_id(),
                DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(
                    window_bounds.get_bounds()
                )),
                floating_bounds(0.0, 0.0, 360.0, 220.0),
                target_center_host_position(),
            ));
            assert!(runtime.push_viewport_host_scene_fact(
                space,
                window.window_id(),
                leaf_host_scene_fact(target_tabs, target_tabs),
            ));
        }

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);

        let first_request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            runtime
                .last_host_scene_screen_position(&first_target_space)
                .expect("first target scene should expose a screen position"),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(first_target.window()),
        )
        .with_drag_session(Some(session.clone()));
        let first_plan = fresh_delivery_for_request(cx, &runtime, &first_request);
        let second_request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            runtime
                .last_host_scene_screen_position(&second_target_space)
                .expect("second target scene should expose a screen position"),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(second_target.window()),
        )
        .with_drag_session(Some(session.clone()));
        let second_plan = fresh_delivery_for_request(cx, &runtime, &second_request);

        let second_result = cx.update(|app| runtime.deliver_drop_commit_delivery(second_plan, app));
        let first_result = cx.update(|app| runtime.deliver_drop_commit_delivery(first_plan, app));

        let DockViewportDropRouteOutcome::Action(second_action) =
            second_result.expect("current direct delivery should commit")
        else {
            panic!("current direct delivery should produce a normal action outcome");
        };
        assert_eq!(second_action.action(), crate::DockActionOutcome::Changed);
        assert!(
            matches!(
                first_result,
                Err(DockActionApplyError::DropTargetUnavailable)
            ),
            "a direct delivery from an older host-scene frame should be rejected"
        );
        cx.read_entity(&controller, |controller, _| {
            assert_eq!(
                controller.graph().collect_items_in_space(&source_space),
                Vec::<DockItemId>::new()
            );
            assert_eq!(
                controller
                    .graph()
                    .collect_items_in_space(&first_target_space),
                vec![item("first")]
            );
            assert_eq!(
                controller
                    .graph()
                    .collect_items_in_space(&second_target_space),
                vec![item("second"), item("a")]
            );
        });
    }

    #[open_gpui::test]
    fn hovered_host_release_does_not_consume_cached_delivery_for_another_window(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a"), item("c")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller.clone());

        let target_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let target_opened = cx
            .update(|app| {
                runtime.open_viewport(
                    target_space.clone(),
                    WindowOptions {
                        window_bounds: Some(target_bounds),
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("target viewport should open");
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(target_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            target_center_host_position(),
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_opened.window().window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));

        let source_bounds = WindowBounds::Windowed(floating_bounds(520.0, 100.0, 360.0, 220.0));
        let source_opened = cx
            .update(|app| {
                runtime.open_viewport(
                    source_space.clone(),
                    WindowOptions {
                        window_bounds: Some(source_bounds),
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("source viewport should open");
        let source_window = source_opened
            .window()
            .downcast::<crate::DockHost>()
            .expect("source viewport should render DockHost");
        assert!(runtime.begin_viewport_host_scene(
            source_space.clone(),
            source_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(source_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            target_center_host_position(),
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &source_space,
            source_opened.window().window_id(),
            leaf_host_scene_fact(source_tabs, source_tabs),
        ));

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);
        let target_screen_position =
            screen_position_for_host_position(target_bounds, target_center_host_position());
        let resolution = cache_known_viewport_preview(
            cx,
            &runtime,
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_screen_position,
            target_opened.window(),
            Some(session.clone()),
            "Panel A",
        );

        cache_host_route_preview(
            cx,
            &runtime,
            &resolution,
            "Panel A",
            source_space.clone(),
            source_opened.window().window_id(),
            target_center_host_position(),
        );
        source_window
            .update(cx, |host, window, cx| {
                host.drop_payload_release_from_render(
                    DockPayloadDropRelease::hovered_host_with_session(
                        payload.clone(),
                        source_space.clone(),
                        target_center_host_position(),
                        Some(session.clone()),
                    ),
                    window,
                    cx,
                )
            })
            .expect("source host should handle hovered release");
        cx.run_until_parked();

        cx.read_entity(&controller, |controller, _| {
            let DockNode::Tabs { items, selected } = controller
                .graph()
                .node(source_tabs)
                .expect("source tabs should still exist")
            else {
                panic!("source should remain tabs");
            };
            assert_eq!(items, &vec![item("a"), item("c")]);
            assert_eq!(selected.as_ref(), items.first());

            let DockNode::Tabs { items, selected } = controller
                .graph()
                .node(target_tabs)
                .expect("target tabs should still exist")
            else {
                panic!("target should remain tabs");
            };
            assert_eq!(items, &vec![item("b")]);
            assert_eq!(selected.as_ref(), items.get(0));
        });
    }

    #[open_gpui::test]
    fn hovered_host_release_rejects_cached_delivery_when_release_point_misses_target(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller.clone());

        let source_bounds = WindowBounds::Windowed(floating_bounds(520.0, 100.0, 360.0, 220.0));
        let source_opened = cx
            .update(|app| {
                runtime.open_viewport(
                    source_space.clone(),
                    WindowOptions {
                        window_bounds: Some(source_bounds),
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("source viewport should open");
        let target_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let target_opened = cx
            .update(|app| {
                runtime.open_viewport(
                    target_space.clone(),
                    WindowOptions {
                        window_bounds: Some(target_bounds),
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("target viewport should open");
        let target_window = target_opened
            .window()
            .downcast::<crate::DockHost>()
            .expect("target viewport should render DockHost");
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(target_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            target_center_host_position(),
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_opened.window().window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);
        let target_screen_position =
            screen_position_for_host_position(target_bounds, target_center_host_position());
        let resolution = cache_known_viewport_preview(
            cx,
            &runtime,
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_screen_position,
            target_opened.window(),
            Some(session.clone()),
            "Panel A",
        );
        cache_host_route_preview(
            cx,
            &runtime,
            &resolution,
            "Panel A",
            target_space.clone(),
            target_opened.window().window_id(),
            target_center_host_position(),
        );
        target_window
            .update(cx, |_host, window, cx| {
                window.refresh();
                cx.notify();
            })
            .expect("target host should cache the routed delivery");

        let missed_target_position = point(px(720.0), px(420.0));
        target_window
            .update(cx, |host, window, cx| {
                host.drop_payload_release_from_render(
                    DockPayloadDropRelease::hovered_host_with_session(
                        payload.clone(),
                        target_space.clone(),
                        missed_target_position,
                        Some(session.clone()),
                    ),
                    window,
                    cx,
                )
            })
            .expect("target host should handle release outside the cached target");
        cx.run_until_parked();

        cx.read_entity(&controller, |controller, _| {
            assert_eq!(
                controller.graph().collect_items_in_space(&source_space),
                vec![item("a")],
                "release outside current target should not commit the stale cached delivery"
            );
            assert_eq!(
                controller.graph().collect_items_in_space(&target_space),
                vec![item("b")]
            );
        });
        let status = runtime.runtime_status();
        assert!(
            !matches!(
                status.last_route.as_ref().map(|record| &record.target),
                Some(crate::DockViewportRouteTarget::KnownViewport { window_id, .. })
                    if *window_id == target_opened.window().window_id()
            ),
            "release should be rerouted from the current pointer facts instead of the cached delivery, got {:?}",
            status.last_route
        );
        assert!(
            runtime.active_payload_drag_session(&payload).is_none(),
            "release should still finish the drag session after rejecting stale cached delivery"
        );

        source_opened
            .window()
            .update(cx, |_, _, _| ())
            .expect("source window should remain live");
    }

    #[open_gpui::test]
    fn hovered_host_release_commits_fresh_route_without_cached_delivery(cx: &mut TestAppContext) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller.clone());

        let target_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let target_opened = cx
            .update(|app| {
                runtime.open_viewport(
                    target_space.clone(),
                    WindowOptions {
                        window_bounds: Some(target_bounds),
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("target viewport should open");
        let target_window = target_opened
            .window()
            .downcast::<crate::DockHost>()
            .expect("target viewport should render DockHost");
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(target_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            target_center_host_position(),
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_opened.window().window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);
        cx.set_platform_hovered_window(Some(target_opened.window()));
        target_window
            .update(cx, |host, window, cx| {
                host.begin_host_drop_scene_from_render(
                    &payload,
                    floating_bounds(0.0, 0.0, 360.0, 220.0),
                    target_center_host_position(),
                    window,
                    cx,
                );
                host.update_local_drop_scene_fact_from_render(
                    &payload,
                    crate::drop_scene_fact::leaf(
                        target_tabs,
                        target_tabs,
                        floating_bounds(0.0, 0.0, 360.0, 220.0),
                        false,
                    ),
                    target_center_host_position(),
                    window,
                    cx,
                );
                host.drop_payload_release_from_render(
                    DockPayloadDropRelease::hovered_host_with_session(
                        payload.clone(),
                        target_space.clone(),
                        target_center_host_position(),
                        Some(session.clone()),
                    ),
                    window,
                    cx,
                )
            })
            .expect("target host should handle uncached hovered release");
        cx.run_until_parked();

        cx.read_entity(&controller, |controller, _| {
            assert_eq!(
                controller.graph().collect_items_in_space(&source_space),
                Vec::<DockItemId>::new(),
                "fresh hovered-host local commit should remove the item from the source viewport"
            );
            assert_eq!(
                controller.graph().collect_items_in_space(&target_space),
                vec![item("b"), item("a")]
            );
        });
        assert!(
            runtime.active_payload_drag_session(&payload).is_none(),
            "committed uncached hovered-host release should still finish the drag session"
        );
    }

    #[open_gpui::test]
    fn hovered_host_release_uses_accepted_local_target_instead_of_stale_cached_delivery(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller.clone());

        let target_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let target_opened = cx
            .update(|app| {
                runtime.open_viewport(
                    target_space.clone(),
                    WindowOptions {
                        window_bounds: Some(target_bounds),
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("target viewport should open");
        let target_window = target_opened
            .window()
            .downcast::<crate::DockHost>()
            .expect("target viewport should render DockHost");
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(target_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            target_center_host_position(),
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_opened.window().window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);
        let target_screen_position =
            screen_position_for_host_position(target_bounds, target_center_host_position());
        cache_known_viewport_preview(
            cx,
            &runtime,
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_screen_position,
            target_opened.window(),
            Some(session.clone()),
            "Panel A",
        );

        cx.set_platform_hovered_window(Some(target_opened.window()));
        target_window
            .update(cx, |host, window, cx| {
                host.begin_host_drop_scene_from_render(
                    &payload,
                    floating_bounds(0.0, 0.0, 360.0, 220.0),
                    target_center_host_position(),
                    window,
                    cx,
                );
                host.update_local_drop_scene_fact_from_render(
                    &payload,
                    crate::drop_scene_fact::leaf(
                        target_tabs,
                        target_tabs,
                        floating_bounds(0.0, 0.0, 360.0, 220.0),
                        false,
                    ),
                    target_center_host_position(),
                    window,
                    cx,
                );
                host.drop_payload_release_from_render(
                    DockPayloadDropRelease::hovered_host_with_session(
                        payload.clone(),
                        target_space.clone(),
                        target_center_host_position(),
                        Some(session.clone()),
                    ),
                    window,
                    cx,
                )
            })
            .expect("target host should handle hovered release after a cached preview");
        cx.run_until_parked();

        cx.read_entity(&controller, |controller, _| {
            assert_eq!(
                controller.graph().collect_items_in_space(&source_space),
                Vec::<DockItemId>::new()
            );
            let DockNode::Tabs { items, selected } = controller
                .graph()
                .node(target_tabs)
                .expect("target tabs should still exist")
            else {
                panic!("target should remain tabs");
            };
            assert_eq!(items, &vec![item("b"), item("a")]);
            assert_eq!(selected.as_ref(), items.get(1));
        });
        let status = runtime.runtime_status();
        assert_eq!(
            status.last_drop_outcome.as_ref().map(|record| record.kind),
            None,
            "accepted same-window delivery should commit locally instead of consuming the routed runtime preview"
        );
        assert!(
            runtime.active_payload_drag_session(&payload).is_none(),
            "hovered-host release should finish the drag session even after a cached preview"
        );
    }

    #[open_gpui::test]
    fn hovered_host_release_rejects_when_release_point_misses_current_target(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller.clone());

        let target_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let target_opened = cx
            .update(|app| {
                runtime.open_viewport(
                    target_space.clone(),
                    WindowOptions {
                        window_bounds: Some(target_bounds),
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("target viewport should open");
        let target_window = target_opened
            .window()
            .downcast::<crate::DockHost>()
            .expect("target viewport should render DockHost");

        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(target_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            target_center_host_position(),
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_opened.window().window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);
        let target_screen_position =
            screen_position_for_host_position(target_bounds, target_center_host_position());
        let resolution = cache_known_viewport_preview(
            cx,
            &runtime,
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_screen_position,
            target_opened.window(),
            Some(session.clone()),
            "Panel A",
        );

        cache_host_route_preview(
            cx,
            &runtime,
            &resolution,
            "Panel A",
            target_space.clone(),
            target_opened.window().window_id(),
            target_center_host_position(),
        );
        target_window
            .update(cx, |host, window, cx| {
                let missed = point(px(720.0), px(420.0));
                host.drop_payload_release_from_render(
                    DockPayloadDropRelease::hovered_host_with_session(
                        payload.clone(),
                        target_space.clone(),
                        missed,
                        Some(session.clone()),
                    ),
                    window,
                    cx,
                )
            })
            .expect("target host should reject miss outside the current target");
        cx.run_until_parked();

        cx.read_entity(&controller, |controller, _| {
            assert_eq!(
                controller.graph().collect_items_in_space(&source_space),
                vec![item("a")]
            );
            assert_eq!(
                controller.graph().collect_items_in_space(&target_space),
                vec![item("b")]
            );
        });
        assert!(
            runtime.active_payload_drag_session(&payload).is_none(),
            "rejected hovered-host release should still finish the drag session"
        );
    }

    #[open_gpui::test]
    fn source_release_prefers_local_target_over_cached_route_delivery(cx: &mut TestAppContext) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a"), item("b")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("c")],
            selected: Some(item("c")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller.clone());

        let target_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let target_opened = cx
            .update(|app| {
                runtime.open_viewport(
                    target_space.clone(),
                    WindowOptions {
                        window_bounds: Some(target_bounds),
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("target viewport should open");
        let source_bounds = WindowBounds::Windowed(floating_bounds(520.0, 100.0, 360.0, 220.0));
        let source_opened = cx
            .update(|app| {
                runtime.open_viewport(
                    source_space.clone(),
                    WindowOptions {
                        window_bounds: Some(source_bounds),
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("source viewport should open");
        let source_window = source_opened
            .window()
            .downcast::<crate::DockHost>()
            .expect("source viewport should render DockHost");
        let target_window = target_opened
            .window()
            .downcast::<crate::DockHost>()
            .expect("target viewport should render DockHost");
        let source_host = source_window
            .root(cx)
            .expect("source viewport should expose DockHost root");
        let target_host = target_window
            .root(cx)
            .expect("target viewport should expose DockHost root");

        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(target_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            target_center_host_position(),
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_opened.window().window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));

        let target_screen_position = point(
            target_bounds.get_bounds().origin.x + px(120.0),
            target_bounds.get_bounds().origin.y + px(100.0),
        );
        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let resolution = cache_known_viewport_preview(
            cx,
            &runtime,
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_screen_position,
            target_opened.window(),
            None,
            "Panel A",
        );

        cache_host_route_preview(
            cx,
            &runtime,
            &resolution,
            "Panel A",
            source_space.clone(),
            source_opened.window().window_id(),
            target_center_host_position(),
        );
        source_window
            .update(cx, |_host, window, cx| {
                window.refresh();
                cx.notify();
            })
            .expect("source host should update route preview");
        cx.run_until_parked();

        let target_visual = VisualTestContext::from_window(target_opened.window(), cx);
        assert!(
            selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview).is_some(),
            "target viewport should draw the cached routed preview before release"
        );
        let mut source_visual = VisualTestContext::from_window(source_opened.window(), cx);
        assert!(
            selector_for(
                &source_visual,
                &source_host,
                DockDebugRegion::DropRoutePreview {
                    kind: DockDropRoutePreviewKind::KnownViewport
                }
            )
            .is_some(),
            "source viewport should cache the routed preview before release"
        );
        let source_tab_selector = selector_for(
            &source_visual,
            &source_host,
            DockDebugRegion::Tab {
                tabs: source_tabs,
                item: item("a"),
            },
        )
        .expect("source tab selector should be emitted");
        let source_tab_center = debug_bounds(&mut source_visual, &source_tab_selector).center();

        source_window
            .update(cx, |host, window, cx| {
                host.drop_payload_release_from_render(
                    DockPayloadDropRelease::hovered_host(
                        payload.clone(),
                        source_space.clone(),
                        source_tab_center,
                    ),
                    window,
                    cx,
                )
            })
            .expect("source host should commit the rendered release");
        cx.run_until_parked();

        let final_source_visual = VisualTestContext::from_window(source_opened.window(), cx);
        assert!(
            selector_for(
                &final_source_visual,
                &source_host,
                DockDebugRegion::DropPreview
            )
            .is_none(),
            "release should clear the cached routed preview"
        );
        cx.read_entity(&controller, |controller, _| {
            let DockNode::Tabs { items, selected } = controller
                .graph()
                .node(source_tabs)
                .expect("source tabs should still exist")
            else {
                panic!("source should remain tabs");
            };
            assert_eq!(items, &vec![item("a"), item("b")]);
            assert_eq!(selected.as_ref(), items.get(0));

            let DockNode::Tabs { items, selected } = controller
                .graph()
                .node(target_tabs)
                .expect("target tabs should still exist")
            else {
                panic!("target should remain tabs");
            };
            assert_eq!(items, &vec![item("c")]);
            assert_eq!(selected.as_ref(), items.get(0));
        });
    }

    #[open_gpui::test]
    fn source_only_release_does_not_consume_cached_route_delivery(cx: &mut TestAppContext) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a"), item("b")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("c")],
            selected: Some(item("c")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller.clone());

        let target_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let target_opened = cx
            .update(|app| {
                runtime.open_viewport(
                    target_space.clone(),
                    WindowOptions {
                        window_bounds: Some(target_bounds),
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("target viewport should open");
        let source_opened = cx
            .update(|app| {
                runtime.open_viewport(
                    source_space.clone(),
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(floating_bounds(
                            520.0, 100.0, 360.0, 220.0,
                        ))),
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("source viewport should open");
        let source_window = source_opened
            .window()
            .downcast::<crate::DockHost>()
            .expect("source viewport should render DockHost");
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(target_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            target_center_host_position(),
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_opened.window().window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);
        let target_screen_position = point(
            target_bounds.get_bounds().origin.x + px(120.0),
            target_bounds.get_bounds().origin.y + px(100.0),
        );
        let preview_request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_screen_position,
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(target_opened.window()),
        )
        .with_drag_session(Some(session.clone()));
        let resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
        assert!(
            matches!(
                resolution.route(),
                DockViewportDropRoute::KnownViewport { target, .. }
                    if target.window_id() == target_opened.window().window_id()
            ),
            "preview setup should resolve the target viewport"
        );
        cache_host_route_preview(
            cx,
            &runtime,
            &resolution,
            "Panel A",
            source_space.clone(),
            source_opened.window().window_id(),
            target_center_host_position(),
        );
        source_window
            .update(cx, |_host, window, cx| {
                window.refresh();
                cx.notify();
            })
            .expect("source host should cache the routed delivery");

        source_window
            .update(cx, |host, window, cx| {
                host.drop_payload_release_from_render(
                    DockPayloadDropRelease::source_only_with_session(
                        payload.clone(),
                        source_space.clone(),
                        point(px(900.0), px(900.0)),
                        Some(session.clone()),
                    ),
                    window,
                    cx,
                )
            })
            .expect("source host should handle source-only release");
        cx.run_until_parked();

        cx.read_entity(&controller, |controller, _| {
            let DockNode::Tabs { items, selected } = controller
                .graph()
                .node(source_tabs)
                .expect("source tabs should still exist")
            else {
                panic!("source should remain tabs");
            };
            assert_eq!(items, &vec![item("a"), item("b")]);
            assert_eq!(selected.as_ref(), items.get(0));

            let DockNode::Tabs { items, selected } = controller
                .graph()
                .node(target_tabs)
                .expect("target tabs should still exist")
            else {
                panic!("target should remain tabs");
            };
            assert_eq!(items, &vec![item("c")]);
            assert_eq!(selected.as_ref(), items.get(0));
        });
    }
}
