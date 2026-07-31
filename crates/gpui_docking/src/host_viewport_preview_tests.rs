//! Concern-owned viewport preview regression tests.

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
        DockViewportResolvedDropRoute, DockViewportRouteStatus, DockViewportRouteTarget,
        DockViewportRuntime, DockViewportRuntimeHandle, DockViewportShouldCloseStatus,
        DockViewportTargetContext, DockViewportTearOffOpenOutcome, DockViewportTearOffOutcomeKind,
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
    fn viewport_runtime_revalidates_preview_resolved_target_after_scene_changes(
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
        let policy = cx.read_entity(&controller, |controller, _| {
            controller.workspace().policy().clone()
        });

        let target_window = handle(21);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        let mut runtime = DockViewportRuntime::from_adapter(
            controller.clone(),
            adapter,
            DockViewportClosePolicy::RetainLayout,
        );

        let target_scene =
            DockViewportHostSceneSeed::new(target_space.clone(), target_window, target_tabs);
        let host_position = target_scene.host_position();
        let release_position = target_scene.screen_position();
        target_scene.publish_runtime(&mut runtime);
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
            release_position,
            None,
            crate::DockViewportPlatformSignals::from_target_context(
                DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
            )
            .with_event_receiver_window(target_window)
            .with_global_window_bounds(true),
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(session.clone()));
        let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        assert!(
            matches!(resolution.route(), DockViewportDropRoute::KnownViewport { target, .. }
                if target.window_id() == target_window.window_id()),
            "preview route should target the registered viewport"
        );
        assert!(
            resolution.routed_preview_target_snapshot().is_some(),
            "fresh route should carry a preview target"
        );
        assert!(
            resolution.delivery().is_some(),
            "fresh route should mint delivery from current route facts"
        );
        let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        let commit_plan =
            DockDropDelivery::from_resolution(resolution).expect("fresh route should mint a plan");

        target_scene.begin_empty_runtime_frame(&mut runtime);
        let target_after_scene_change =
            runtime.resolve_host_scene_target(&target_space, host_position, &policy);
        assert!(
            target_after_scene_change.is_none(),
            "new frame intentionally has no facts; re-resolving would fail"
        );

        let result =
            cx.update(|app| runtime.deliver_drop_commit_delivery_with_outcome(commit_plan, app));
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
    fn viewport_runtime_revalidates_routed_preview_release_against_current_policy(
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

        let target_window = handle(24);
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
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: target_tabs,
                target_tabs,
                bounds: floating_bounds(0.0, 0.0, 360.0, 220.0),
                is_central: true,
            }),
        ));

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);
        let request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source_space,
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            screen_position_for_host_position(window_bounds, host_position),
            None,
            crate::DockViewportPlatformSignals::from_target_context(
                DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
            )
            .with_event_receiver_window(target_window)
            .with_global_window_bounds(true),
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(session));

        let preview_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        assert!(
            matches!(
                preview_resolution.route(),
                DockViewportDropRoute::KnownViewport { target, .. }
                    if target.window_id() == target_window.window_id()
            ),
            "preview setup should resolve the target viewport before policy changes"
        );
        let update = runtime.update_routed_drop_preview(&preview_resolution, &payload);
        assert!(update.changed());

        controller.update(cx, |controller, _| {
            controller
                .policy_mut()
                .set_allow_central_region_dock_over(false);
        });

        let release_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery_for_request(&request, app));
        assert_eq!(
            release_resolution.route(),
            &DockViewportDropRoute::rejected_by_policy(
                DockPolicyError::CentralRegionDockOverDisabled,
            ),
            "routed preview release must not reuse a stale KnownViewport route after policy changes"
        );
        assert!(
            release_resolution.delivery().is_none(),
            "policy-rejected release must not carry a commit delivery"
        );
        assert!(
            release_resolution.preview_target().is_some(),
            "policy-rejected release should retain a preview target for rejected feedback"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_preview_respects_payload_dock_class_policy(cx: &mut TestAppContext) {
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
        workspace.register_panel(
            item("a"),
            DockPanel::new("Panel A", test_view(cx, "A")).with_dock_class("editor"),
        );
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        workspace
            .policy_mut()
            .allow_dock_class_in_space(target_space.clone(), "inspector");
        let controller = cx.new(|_| DockController::new(workspace));

        let target_window = handle(22);
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
        let session = runtime.begin_payload_drag(&payload);
        let request = hovered_window_route_request_for_test(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            point(px(220.0), px(200.0)),
            None,
            target_window,
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(session.clone()));
        let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        assert!(
            matches!(
                resolution.route(),
                DockViewportDropRoute::Rejected(
                    crate::DockViewportDropRouteRejectionReason::Policy(
                        DockPolicyError::DockClassRejected { .. }
                    )
                )
            ),
            "policy-rejected cross-viewport targets should render as rejected routes"
        );
        assert!(
            resolution.delivery().is_none(),
            "policy-rejected cross-viewport targets must not carry a delivery"
        );
        let update = runtime.update_routed_drop_preview(&resolution, &payload);
        assert!(update.changed());
        assert_eq!(update.into_windows(), vec![target_window]);
        let preview = runtime
            .routed_drop_preview_for(&target_space, target_window.window_id())
            .expect("policy-rejected route should render a target-window preview");
        assert!(!preview.preview.scene.decision.is_allowed());
        assert!(preview.preview.scene.payload_tabs.is_none());
        assert!(
            runtime.has_routed_drop_preview_for_drag_session(Some(&session)),
            "rejected routed previews should stay scoped to the active drag session"
        );

        let result = DockDropDelivery::from_resolution(resolution);
        assert_eq!(
            result,
            Err(DockActionApplyError::Policy(
                DockPolicyError::DockClassRejected {
                    space: target_space.clone(),
                    item: item("a"),
                    dock_class: Some(DockClassId::from("editor")),
                }
            ))
        );
        assert!(runtime.finish_payload_drag(&session).changed());
        assert_eq!(
            runtime.routed_drop_preview_for(&target_space, target_window.window_id()),
            None,
            "finishing the drag must clear rejected routed previews even though they are not delivery-capable"
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
    fn viewport_runtime_current_hover_rejects_stale_routed_preview_after_resampling(
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

        let target_window = handle(91);
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
        let target_screen_position = point(px(220.0), px(200.0));
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
        let update = runtime.update_routed_drop_preview(&preview_resolution, &payload);
        assert!(update.changed());
        assert_eq!(update.into_windows(), vec![target_window]);

        cx.set_platform_hovered_window(None);
        let stale_release_signals = crate::DockViewportPlatformSignals::from_target_context(
            DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
        )
        .with_target_context_resampling_from_app();
        let release_request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source_space,
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_screen_position,
            None,
            stale_release_signals,
            DockPayloadDropReleaseOrigin::SourceOnly,
        )
        .with_drag_session(Some(session));
        let release_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&release_request, app));

        assert_eq!(
            release_resolution.route(),
            &DockViewportDropRoute::Unavailable,
            "release routing must still honor current hovered=None after resampling"
        );
        assert!(
            release_resolution.delivery().is_none(),
            "current hovered=None must not mint delivery from stale routed preview state"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_current_hovered_window_facts_override_stale_routed_preview(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let decoy_space = DockSpaceId::from("decoy");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        let decoy_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("c")],
            selected: Some(item("c")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_tabs);
        graph.set_root(decoy_space.clone(), decoy_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
        let controller = cx.new(|_| DockController::new(workspace));

        let target_window = handle(91);
        let decoy_window = handle(92);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        let mut runtime = DockViewportRuntime::from_adapter(
            controller,
            adapter,
            DockViewportClosePolicy::RetainLayout,
        );
        assert!(
            runtime
                .register_opened_viewport(decoy_space.clone(), decoy_window)
                .is_empty()
        );

        let shared_window_bounds =
            WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let host_position = point(px(120.0), px(100.0));
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_window.window_id(),
            DockViewportWindowFacts::from_window_bounds(shared_window_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            host_position,
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_window.window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));
        assert!(runtime.begin_viewport_host_scene(
            decoy_space.clone(),
            decoy_window.window_id(),
            DockViewportWindowFacts::from_window_bounds(shared_window_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            host_position,
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &decoy_space,
            decoy_window.window_id(),
            leaf_host_scene_fact(decoy_tabs, decoy_tabs),
        ));

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);
        let target_screen_position = point(px(220.0), px(200.0));
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
        let update = runtime.update_routed_drop_preview(&preview_resolution, &payload);
        assert!(update.changed());
        assert_eq!(update.into_windows(), vec![target_window]);
        let release_request = hovered_window_route_request_for_test(
            source_space,
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_screen_position,
            None,
            decoy_window,
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(session));
        let release_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&release_request, app));

        assert!(
            matches!(
                release_resolution.route(),
                DockViewportDropRoute::KnownViewport { target, source }
                    if target.space() == &decoy_space
                        && target.window_id() == decoy_window.window_id()
                        && *source
                            == crate::DockViewportRouteSelectionSource::TrustedHoveredWindow
            ),
            "current hovered-window facts should beat the previous routed preview, got {:?}",
            release_resolution.route()
        );
        assert!(
            release_resolution.delivery().is_some(),
            "current hovered-window facts should mint delivery for the current target"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_identical_host_routed_preview_does_not_request_refresh_again(
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

        let target_window = handle(190);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        let mut runtime = DockViewportRuntime::from_adapter(
            controller,
            adapter,
            DockViewportClosePolicy::RetainLayout,
        );

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = cache_known_viewport_preview_for_test(
            &mut runtime,
            source_space.clone(),
            source_tabs,
            &target_space,
            target_window,
            target_tabs,
            cx,
        );
        let host_position = point(px(220.0), px(200.0));
        let request = hovered_window_route_request_for_test(
            source_space,
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            host_position,
            None,
            target_window,
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(session.clone()));
        let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));

        let initial = runtime.update_host_routed_drop_preview(
            &resolution,
            &payload,
            target_space.clone(),
            target_window.window_id(),
            host_position,
        );
        assert!(
            initial.changed(),
            "the first host-routed preview write should publish the host route marker"
        );

        let repeated = runtime.update_host_routed_drop_preview(
            &resolution,
            &payload,
            target_space.clone(),
            target_window.window_id(),
            host_position,
        );

        assert!(!repeated.changed());
        assert!(
            repeated.into_windows().is_empty(),
            "writing the same host-routed preview twice must not keep refreshing the target window"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_hovered_host_known_empty_hover_rejects_stale_routed_preview(
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

        let target_window = handle(91);
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
        let target_screen_position = point(px(220.0), px(200.0));
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
        let update = runtime.update_routed_drop_preview(&preview_resolution, &payload);
        assert!(update.changed());

        let release_request = DockViewportDropRouteRequest::from_target_context(
            source_space,
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_screen_position,
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window_known_empty(),
        )
        .with_drag_session(Some(session));
        let release_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&release_request, app));

        assert_eq!(
            release_resolution.route(),
            &DockViewportDropRoute::Unavailable,
            "trusted hovered=None is authoritative on hovered-host release and must not reuse stale routed preview state"
        );
        assert!(
            release_resolution.delivery().is_none(),
            "trusted hovered=None must not mint delivery from stale routed preview state"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_stale_routed_preview_does_not_route_through_front_viewport_window(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let front_space = DockSpaceId::from("front");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        let front_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("c")],
            selected: Some(item("c")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_tabs);
        graph.set_root(front_space.clone(), front_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
        let controller = cx.new(|_| DockController::new(workspace));

        let target_window = handle(92);
        let front_window = handle(93);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        register_viewport(&mut adapter, front_space.clone(), front_window);
        let mut runtime = DockViewportRuntime::from_adapter(
            controller,
            adapter,
            DockViewportClosePolicy::RetainLayout,
        );

        let shared_window_bounds =
            WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_window.window_id(),
            DockViewportWindowFacts::from_window_bounds(shared_window_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            point(px(120.0), px(100.0)),
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_window.window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));
        assert!(runtime.begin_viewport_host_scene(
            front_space.clone(),
            front_window.window_id(),
            DockViewportWindowFacts::from_window_bounds(shared_window_bounds),
            floating_bounds(320.0, 180.0, 20.0, 20.0),
            point(px(0.0), px(0.0)),
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
                DockViewportTargetContext::new().with_window_stack([front_window, target_window]),
            ),
            DockPayloadDropReleaseOrigin::SourceOnly,
        )
        .with_drag_session(Some(session));
        let release_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&release_request, app));

        assert_eq!(
            release_resolution.route(),
            &DockViewportDropRoute::Unavailable,
            "stale routed preview state must not route through a front viewport window that contains the pointer but has no host target"
        );
        assert!(release_resolution.delivery().is_none());
    }

    #[open_gpui::test]
    fn viewport_runtime_scopes_routed_preview_delivery_to_drag_session(cx: &mut TestAppContext) {
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
        let target_window = handle(77);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        let mut runtime = DockViewportRuntime::from_adapter(
            controller,
            adapter,
            DockViewportClosePolicy::RetainLayout,
        );
        let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
        let target_position = center_drop_position(host_bounds);
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_window.window_id(),
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(floating_bounds(
                100.0, 100.0, 360.0, 220.0,
            ))),
            host_bounds,
            target_position,
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
            "A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);
        let request = hovered_window_route_request_for_test(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            point(px(220.0), px(200.0)),
            None,
            target_window,
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(session.clone()));
        let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        assert!(
            matches!(
                resolution.route(),
                DockViewportDropRoute::KnownViewport { target, .. }
                    if target.window_id() == target_window.window_id()
            ),
            "preview setup should resolve a known target viewport"
        );
        runtime.update_routed_drop_preview(&resolution, &payload);
        assert!(runtime.has_routed_drop_preview_for_drag_session(Some(&session)));
        assert!(!runtime.has_routed_drop_preview_for_drag_session(None));

        let local_resolution = DockViewportResolvedDropRoute::new(
            DockViewportDropRoute::local_for_registration_test(
                runtime
                    .registration_key_for_space_window(&target_space, target_window.window_id())
                    .expect("registered viewport should have an exact key"),
                target_position,
                1,
                crate::DockViewportRouteSelectionSource::TrustedHoveredWindow,
            ),
            None,
        );
        runtime.update_routed_drop_preview(&local_resolution, &payload);
        assert!(!runtime.has_routed_drop_preview_for_drag_session(Some(&session)));

        runtime.update_routed_drop_preview(&resolution, &payload);

        assert!(runtime.finish_payload_drag(&session).changed());
        assert!(!runtime.has_routed_drop_preview_for_drag_session(Some(&session)));

        let next_session = runtime.begin_payload_drag(&payload);
        assert_ne!(next_session.id(), session.id());
        let next_request = hovered_window_route_request_for_test(
            source_space,
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            point(px(220.0), px(200.0)),
            None,
            target_window,
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(next_session.clone()));
        let next_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&next_request, app));
        runtime.update_routed_drop_preview(&next_resolution, &payload);
        assert!(runtime.has_routed_drop_preview_for_drag_session(Some(&next_session)));
        assert!(!runtime.has_routed_drop_preview_for_drag_session(Some(&session)));
        assert!(
            !runtime
                .update_routed_drop_preview(&resolution, &payload)
                .changed(),
            "a delayed resolution from the previous drag session must not replace the current preview"
        );
        assert!(runtime.has_routed_drop_preview_for_drag_session(Some(&next_session)));
    }

    #[open_gpui::test]
    fn stale_routed_preview_store_rejects_registration_and_scene_generations(
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

        let target_window = handle(78);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        let mut runtime = DockViewportRuntime::from_adapter(
            controller,
            adapter,
            DockViewportClosePolicy::RetainLayout,
        );
        let target_scene =
            DockViewportHostSceneSeed::new(target_space.clone(), target_window, target_tabs);
        target_scene.publish_runtime(&mut runtime);
        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);
        let request = hovered_window_route_request_for_test(
            source_space,
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_scene.screen_position(),
            None,
            target_window,
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(session));

        let stale_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        let stale_registration = stale_resolution
            .route()
            .route_proof()
            .expect("known viewport route should carry a registration proof")
            .registration_key()
            .clone();

        let replacement =
            runtime.replace_adapter_registration_for_test(target_space.clone(), target_window);
        assert_ne!(replacement, stale_registration);
        target_scene.publish_runtime(&mut runtime);
        let current_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        assert_eq!(
            current_resolution
                .route()
                .route_proof()
                .map(|proof| proof.registration_key()),
            Some(&replacement)
        );
        assert!(
            runtime
                .update_routed_drop_preview(&current_resolution, &payload)
                .changed()
        );
        assert!(
            runtime
                .routed_drop_preview_for(&target_space, target_window.window_id())
                .is_some()
        );

        let stale_update = runtime.update_routed_drop_preview(&stale_resolution, &payload);
        assert!(
            !stale_update.changed(),
            "a delayed G1 resolution must be rejected before preview replacement"
        );
        assert!(
            runtime
                .routed_drop_preview_for(&target_space, target_window.window_id())
                .is_some(),
            "rejecting G1 must preserve the already stored G2 preview"
        );

        let stale_scene_frame = current_resolution
            .routed_preview_target_snapshot()
            .expect("current preview resolution should retain its scene frame")
            .frame()
            .clone();
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_window.window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));
        let next_scene_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        let next_scene_frame = next_scene_resolution
            .routed_preview_target_snapshot()
            .expect("the advanced scene should resolve a current preview target")
            .frame();
        assert_ne!(
            next_scene_frame, &stale_scene_frame,
            "same-registration scene updates must advance the exact frame proof"
        );
        assert!(
            runtime
                .update_routed_drop_preview(&next_scene_resolution, &payload)
                .changed()
        );
        assert!(
            !runtime
                .update_routed_drop_preview(&current_resolution, &payload)
                .changed(),
            "a delayed scene frame must not replace a preview from the current frame"
        );
        assert!(
            runtime
                .routed_drop_preview_for(&target_space, target_window.window_id())
                .is_some(),
            "rejecting a stale scene frame must preserve the current preview"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_begin_payload_drag_clears_previous_routed_preview(cx: &mut TestAppContext) {
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
        let target_window = handle(78);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        let mut runtime = DockViewportRuntime::from_adapter(
            controller,
            adapter,
            DockViewportClosePolicy::RetainLayout,
        );

        let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
        let target_position = center_drop_position(host_bounds);
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_window.window_id(),
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(floating_bounds(
                100.0, 100.0, 360.0, 220.0,
            ))),
            host_bounds,
            target_position,
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_window.window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));

        let first_payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "A".to_string(),
        );
        let first_session = runtime.begin_payload_drag(&first_payload);
        let request = hovered_window_route_request_for_test(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            point(px(220.0), px(200.0)),
            None,
            target_window,
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(first_session.clone()));
        let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        runtime.update_routed_drop_preview(&resolution, &first_payload);

        assert!(
            runtime
                .routed_drop_preview_for(&target_space, target_window.window_id())
                .is_some()
        );
        assert!(runtime.has_routed_drop_preview_for_drag_session(Some(&first_session)));

        let second_payload =
            DockDragPayload::new_item(source_space, source_tabs, item("c"), "C".to_string());
        let second_session = runtime.begin_payload_drag(&second_payload);
        assert_ne!(second_session.id(), first_session.id());
        assert_eq!(
            runtime.routed_drop_preview_for(&target_space, target_window.window_id()),
            None
        );
        assert!(!runtime.has_routed_drop_preview_for_drag_session(Some(&first_session)));
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
        visual_affordance_scene::{DockVisualAffordanceKind, DockVisualAffordanceScene},
    };
    use open_gpui::{
        AppContext as _, Focusable, Modifiers, MouseButton, TestAppContext, VisualTestContext,
        WindowBounds, WindowOptions, point, px, size,
    };
    use slotmap::Key;

    use crate::host_viewport_runtime_test_support::*;

    #[open_gpui::test]
    fn host_render_route_preview_uses_route_debug_selector(cx: &mut TestAppContext) {
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

        let target_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let target_opened = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
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
        let target_host = target_window
            .root(cx)
            .expect("target viewport should expose DockHost root");

        let source_bounds = WindowBounds::Windowed(floating_bounds(520.0, 100.0, 360.0, 220.0));
        let source_opened = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
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
        let source_host = source_window
            .root(cx)
            .expect("source viewport should expose DockHost root");
        cx.run_until_parked();

        let mut source_visual = VisualTestContext::from_window(source_opened.window(), cx);
        let mut target_visual = VisualTestContext::from_window(target_opened.window(), cx);
        let source_tab = selector_for(
            &source_visual,
            &source_host,
            DockDebugRegion::Tab {
                tabs: source_tabs,
                item: item("a"),
            },
        )
        .expect("source tab selector should be emitted");
        let target_tabs_selector = selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::Tabs { node: target_tabs },
        )
        .expect("target tabs selector should be emitted");
        let start = debug_bounds(&mut source_visual, &source_tab).center();
        let threshold = point(start.x + px(24.0), start.y);
        let target_local = debug_bounds(&mut target_visual, &target_tabs_selector).center();
        let target_from_source = point(px(400.0) + target_local.x, target_local.y);
        configure_native_registered_window_hit(
            cx,
            source_opened.window(),
            target_opened.window(),
            target_from_source,
        );

        activate_window_for_pointer_input(&mut source_visual);
        source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
        source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
        assert!(
            cx.update(|app| {
                crate::native_captured_drag::has_active_native_captured_drag_route_for_test(app)
            }),
            "source tab drag should install an exact native-captured route"
        );
        source_visual.simulate_mouse_move(target_from_source, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();

        let source_visual = VisualTestContext::from_window(source_window.into(), cx);

        assert!(
            selector_for(
                &source_visual,
                &source_host,
                DockDebugRegion::DropRoutePreview {
                    kind: DockDropRoutePreviewKind::KnownViewport
                }
            )
            .is_some(),
            "known viewport route should render through the route preview selector"
        );
        assert!(
            selector_for(&source_visual, &source_host, DockDebugRegion::DropPreview).is_none(),
            "known viewport route preview should not be exposed as a local drop preview"
        );
    }

    #[open_gpui::test]
    fn source_hover_over_known_viewport_renders_target_drop_preview(cx: &mut TestAppContext) {
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
        workspace.policy_mut().set_allow_platform_viewports(true);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller);

        let target_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let target_opened = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
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
                runtime.open_viewport_unchecked_policy(
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
        cx.run_until_parked();

        let mut source_visual = VisualTestContext::from_window(source_opened.window(), cx);
        let mut target_visual = VisualTestContext::from_window(target_opened.window(), cx);
        let source_tabs_selector = selector_for(
            &source_visual,
            &source_host,
            DockDebugRegion::Tabs { node: source_tabs },
        )
        .expect("source tabs selector should be emitted");
        let target_tabs_selector = selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::Tabs { node: target_tabs },
        )
        .expect("target tabs selector should be emitted");
        let source_tabs_bounds = debug_bounds(&mut source_visual, &source_tabs_selector);
        let start = point(
            source_tabs_bounds.origin.x + source_tabs_bounds.size.width - px(8.0),
            source_tabs_bounds.origin.y + px(12.0),
        );
        let threshold = point(start.x + px(24.0), start.y);
        let target_local = debug_bounds(&mut target_visual, &target_tabs_selector).center();
        let target_from_source = point(px(400.0) + target_local.x, target_local.y);
        configure_native_registered_window_hit(
            cx,
            source_opened.window(),
            target_opened.window(),
            target_from_source,
        );

        activate_window_for_pointer_input(&mut source_visual);
        source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
        source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
        assert!(
            cx.update(|app| {
                crate::native_captured_drag::has_active_native_captured_drag_route_for_test(app)
            }),
            "source tab-stack drag should install an exact native-captured route"
        );
        source_visual.simulate_mouse_move(target_from_source, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();

        let routed_preview = runtime
            .routed_drop_preview_for(&target_space, target_opened.window().window_id())
            .unwrap_or_else(|| {
                panic!(
                    "target runtime should retain routed preview scene; status: {:?}",
                    runtime.runtime_status()
                )
            });
        assert_eq!(
            routed_preview
                .preview
                .scene
                .payload_tabs
                .as_ref()
                .map(|payload_tabs| payload_tabs.tabs.len()),
            Some(2),
            "routed preview scene should preserve all stack payload tabs before target render"
        );
        let routed_affordance =
            DockVisualAffordanceScene::from_preview(&routed_preview.preview.scene);
        assert_eq!(
            routed_affordance
                .payload_tabs()
                .map(|layer| (layer.payload_index, layer.payload_title.as_deref()))
                .collect::<Vec<_>>(),
            vec![(Some(0), Some("Panel A")), (Some(1), Some("Panel C"))],
            "target routed preview should expose visible payload tab layers"
        );
        assert_eq!(
            routed_affordance
                .payload_ghosts()
                .map(|layer| (layer.payload_index, layer.payload_title.as_deref()))
                .collect::<Vec<_>>(),
            vec![(Some(0), Some("Panel A")), (Some(1), Some("Panel C"))],
            "target routed preview should expose payload ghost layers for visual affordance transitions"
        );
        assert!(
            routed_affordance
                .layers
                .iter()
                .any(|layer| layer.kind == DockVisualAffordanceKind::TabInsertionSlot),
            "target routed preview should keep a tab insertion layer separate from payload ghosts"
        );

        let mut target_visual = VisualTestContext::from_window(target_opened.window(), cx);
        let target_preview =
            selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview)
                .expect("target viewport should render the routed drop preview");
        let target_preview_bounds = debug_bounds(&mut target_visual, &target_preview);
        let target_preview_body = selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::DropPreviewBody,
        )
        .expect("target viewport should render a preview body below the payload tab preview");
        let target_preview_body_bounds = debug_bounds(&mut target_visual, &target_preview_body);
        let target_preview_tab = selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::DropPayloadTabPreview { index: 0 },
        )
        .expect(
            "target viewport should render the first payload tab label inside the routed preview",
        );
        let target_preview_second_tab = selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::DropPayloadTabPreview { index: 1 },
        )
        .expect(
            "target viewport should preserve the second payload tab label inside the routed preview",
        );
        let target_preview_tab_bounds = debug_bounds(&mut target_visual, &target_preview_tab);
        let target_preview_second_tab_bounds =
            debug_bounds(&mut target_visual, &target_preview_second_tab);
        assert!(
            target_preview_bounds.size.width > px(0.0)
                && target_preview_bounds.size.height > px(0.0),
            "target routed drop preview should have visible bounds"
        );
        assert!(
            target_preview_second_tab_bounds.origin.x >= target_preview_tab_bounds.right(),
            "target routed payload tab previews should keep stack order: first={target_preview_tab_bounds:?} second={target_preview_second_tab_bounds:?}"
        );
        assert_close(
            f32::from(target_preview_body_bounds.origin.y),
            f32::from(target_preview_tab_bounds.origin.y + target_preview_tab_bounds.size.height),
        );
        assert!(
            target_preview_body_bounds.origin.y
                >= target_preview_tab_bounds.origin.y + target_preview_tab_bounds.size.height,
            "target routed preview body should start below the payload tab preview"
        );
        assert!(
            selector_for(
                &target_visual,
                &target_host,
                DockDebugRegion::DropRoutePreview {
                    kind: DockDropRoutePreviewKind::KnownViewport
                }
            )
            .is_none(),
            "target viewport should not render the source-only route marker"
        );
        assert!(
            selector_for(
                &VisualTestContext::from_window(source_opened.window(), cx),
                &source_host,
                DockDebugRegion::DropPayloadTabPreview { index: 0 },
            )
            .is_none(),
            "source viewport should not render target payload tab previews"
        );
    }

    #[open_gpui::test]
    fn routed_preview_replacement_clears_old_target_overlay_without_stale_payload(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let first_target_space = DockSpaceId::from("target-a");
        let second_target_space = DockSpaceId::from("target-b");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a"), item("d")],
            selected: Some(item("a")),
        });
        let first_target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        let second_target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("c")],
            selected: Some(item("c")),
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(first_target_space.clone(), first_target_tabs);
        graph.set_root(second_target_space.clone(), second_target_tabs);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
        workspace.register_panel_view(item("d"), "Panel D", test_view(cx, "D"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller);

        let first_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let first_opened = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
                    first_target_space.clone(),
                    WindowOptions {
                        window_bounds: Some(first_bounds),
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("first target viewport should open");
        let second_bounds = WindowBounds::Windowed(floating_bounds(520.0, 100.0, 360.0, 220.0));
        let second_opened = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
                    second_target_space.clone(),
                    WindowOptions {
                        window_bounds: Some(second_bounds),
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("second target viewport should open");
        let first_window = first_opened
            .window()
            .downcast::<crate::DockHost>()
            .expect("first target should render DockHost");
        let second_window = second_opened
            .window()
            .downcast::<crate::DockHost>()
            .expect("second target should render DockHost");
        let first_host = first_window
            .root(cx)
            .expect("first target should expose DockHost root");
        let second_host = second_window
            .root(cx)
            .expect("second target should expose DockHost root");
        cx.run_until_parked();

        let payload =
            DockDragPayload::new_tabs(source_space.clone(), source_tabs, "2 tabs".to_string())
                .with_preview_tabs(["Panel A".to_string(), "Panel D".to_string()]);
        let session = runtime.begin_payload_drag(&payload);
        let first_resolution = cache_known_viewport_preview_with_payload(
            cx,
            &runtime,
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Tabs,
            screen_position_for_host_position(first_bounds, target_center_host_position()),
            first_opened.window(),
            Some(session.clone()),
            &payload,
        );
        assert!(
            matches!(
                first_resolution.route(),
                DockViewportDropRoute::KnownViewport { target, .. }
                    if target.window_id() == first_opened.window().window_id()
            ),
            "first hover should target the first viewport"
        );
        assert!(
            runtime
                .routed_drop_preview_for(&first_target_space, first_opened.window().window_id())
                .is_some(),
            "first target should receive the initial routed preview"
        );
        assert!(
            runtime
                .routed_drop_preview_for(&second_target_space, second_opened.window().window_id())
                .is_none(),
            "second target should start without a routed preview"
        );
        first_window
            .update(cx, |_host, window, cx| {
                window.refresh();
                cx.notify();
            })
            .expect("first target should refresh after initial preview");
        cx.run_until_parked();
        let first_visual = VisualTestContext::from_window(first_opened.window(), cx);
        assert!(
            selector_for(
                &first_visual,
                &first_host,
                DockDebugRegion::DropPayloadTabPreview { index: 0 },
            )
            .is_some(),
            "first target should render payload feedback before the hover changes"
        );

        let second_resolution = cache_known_viewport_preview_with_payload(
            cx,
            &runtime,
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Tabs,
            screen_position_for_host_position(second_bounds, target_center_host_position()),
            second_opened.window(),
            Some(session.clone()),
            &payload,
        );
        assert!(
            matches!(
                second_resolution.route(),
                DockViewportDropRoute::KnownViewport { target, .. }
                    if target.window_id() == second_opened.window().window_id()
            ),
            "second hover should retarget the routed preview"
        );
        assert!(
            runtime
                .routed_drop_preview_for(&first_target_space, first_opened.window().window_id())
                .is_none(),
            "replacing the route should clear the old target preview state"
        );
        assert!(
            runtime
                .routed_drop_preview_for(&second_target_space, second_opened.window().window_id())
                .is_some(),
            "new target should retain the routed preview state"
        );

        first_window
            .update(cx, |_host, window, cx| {
                window.refresh();
                cx.notify();
            })
            .expect("first target should refresh after replacement");
        second_window
            .update(cx, |_host, window, cx| {
                window.refresh();
                cx.notify();
            })
            .expect("second target should refresh after replacement");
        cx.run_until_parked();
        let first_visual = VisualTestContext::from_window(first_opened.window(), cx);
        let second_visual = VisualTestContext::from_window(second_opened.window(), cx);
        assert!(
            selector_for(&first_visual, &first_host, DockDebugRegion::DropPreview).is_none(),
            "old target must not retain a stale drop preview element"
        );
        assert!(
            selector_for(
                &first_visual,
                &first_host,
                DockDebugRegion::DropPayloadTabPreview { index: 0 },
            )
            .is_none(),
            "old target must not retain stale payload tab feedback"
        );
        assert!(
            selector_for(&second_visual, &second_host, DockDebugRegion::DropPreview).is_some(),
            "new target should render the replacement drop preview"
        );
        assert!(
            selector_for(
                &second_visual,
                &second_host,
                DockDebugRegion::DropPayloadTabPreview { index: 0 },
            )
            .is_some(),
            "new target should render replacement payload feedback"
        );
    }

    #[open_gpui::test]
    fn escape_clears_routed_marker_target_overlay_and_active_drag(cx: &mut TestAppContext) {
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
        workspace.policy_mut().set_allow_platform_viewports(true);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller);

        let target_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let target_opened = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
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
                runtime.open_viewport_unchecked_policy(
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
        cx.run_until_parked();

        let mut source_visual = VisualTestContext::from_window(source_opened.window(), cx);
        let mut target_visual = VisualTestContext::from_window(target_opened.window(), cx);
        let source_tab = selector_for(
            &source_visual,
            &source_host,
            DockDebugRegion::Tab {
                tabs: source_tabs,
                item: item("a"),
            },
        )
        .expect("source tab selector should be emitted");
        let target_tabs_selector = selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::Tabs { node: target_tabs },
        )
        .expect("target tabs selector should be emitted");
        let start = debug_bounds(&mut source_visual, &source_tab).center();
        let threshold = point(start.x + px(24.0), start.y);
        let target_local = debug_bounds(&mut target_visual, &target_tabs_selector).center();
        let target_from_source = point(px(400.0) + target_local.x, target_local.y);
        configure_native_registered_window_hit(
            cx,
            source_opened.window(),
            target_opened.window(),
            target_from_source,
        );
        activate_window_for_pointer_input(&mut source_visual);
        source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
        source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
        assert!(
            cx.read(|app| app.has_active_drag()),
            "real tab drag should create a GPUI active drag"
        );
        assert!(
            cx.update(|app| {
                crate::native_captured_drag::has_active_native_captured_drag_route_for_test(app)
            }),
            "source tab drag should install an exact native-captured route"
        );
        let payload = cx
            .read(|app| app.active_drag_value::<DockDragPayload>().cloned())
            .expect("active drag should carry the docking payload");
        let session = runtime
            .active_payload_drag_session(&payload)
            .expect("rendered tab drag should create a runtime drag session");
        source_visual.simulate_mouse_move(target_from_source, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        source_window
            .update(cx, |_host, window, cx| {
                window.refresh();
                cx.notify();
            })
            .expect("source host should update route preview before escape");
        target_window
            .update(cx, |_host, window, cx| {
                window.refresh();
                cx.notify();
            })
            .expect("target host should update preview before escape");
        cx.run_until_parked();

        let source_visual = VisualTestContext::from_window(source_opened.window(), cx);
        let target_visual = VisualTestContext::from_window(target_opened.window(), cx);
        assert!(
            selector_for(
                &source_visual,
                &source_host,
                DockDebugRegion::DropRoutePreview {
                    kind: DockDropRoutePreviewKind::KnownViewport
                },
            )
            .is_some(),
            "source route marker should exist before escape"
        );
        assert!(
            selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview).is_some(),
            "target visual affordance should exist before escape"
        );
        assert!(cx.read(|app| app.has_active_drag()));
        assert!(runtime.has_routed_drop_preview_for_drag_session(Some(&session)));

        cx.simulate_keystrokes(source_opened.window(), "escape");
        source_window
            .update(cx, |_host, window, cx| {
                window.refresh();
                cx.notify();
            })
            .expect("source host should refresh after escape");
        target_window
            .update(cx, |_host, window, cx| {
                window.refresh();
                cx.notify();
            })
            .expect("target host should refresh after escape");
        cx.run_until_parked();

        let source_visual = VisualTestContext::from_window(source_opened.window(), cx);
        let target_visual = VisualTestContext::from_window(target_opened.window(), cx);
        assert!(
            selector_for(
                &source_visual,
                &source_host,
                DockDebugRegion::DropRoutePreview {
                    kind: DockDropRoutePreviewKind::KnownViewport
                },
            )
            .is_none(),
            "escape should clear the source route marker"
        );
        assert!(
            selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview).is_none(),
            "escape should clear the target routed preview"
        );
        assert!(!cx.read(|app| app.has_active_drag()));
        assert!(!runtime.has_routed_drop_preview_for_drag_session(Some(&session)));
    }

    #[open_gpui::test]
    fn local_preview_render_keeps_hidden_routed_preview_deliverable(cx: &mut TestAppContext) {
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
                runtime.open_viewport_unchecked_policy(
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

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);
        let target_screen_position =
            screen_position_for_host_position(target_bounds, target_center_host_position());
        let preview_request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_screen_position,
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(target_opened.window()),
        )
        .with_drag_session(Some(session.clone()));
        let preview_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
        assert!(
            matches!(
                preview_resolution.route(),
                DockViewportDropRoute::KnownViewport { .. }
            ),
            "preview setup should resolve a known viewport route, got {:?}",
            preview_resolution.route()
        );
        assert!(
            preview_resolution.delivery().is_some(),
            "fresh routed preview should mint delivery from current route facts"
        );

        target_window
            .update(cx, |host, window, cx| {
                let position = target_center_host_position();
                host.interaction_mut()
                    .begin_drop_scene(DockHostDropScene::new(position), &DockPolicy::default());
                assert!(host.interaction_mut().push_drop_scene_fact(
                    position,
                    Vec::new(),
                    leaf_host_scene_fact(target_tabs, target_tabs),
                    &DockPolicy::default(),
                ));
                assert!(
                    host.interaction().drop_preview().is_some(),
                    "test setup should create a local target preview before render"
                );
                window.refresh();
                cx.notify();
            })
            .expect("target host should publish a local drop preview");
        cx.update(|app| {
            runtime.update_routed_drop_preview(&preview_resolution, &payload, app);
        });
        cx.run_until_parked();
        target_window
            .update(cx, |host, _, _| {
                assert!(
                    host.interaction().drop_preview().is_some(),
                    "local target preview should remain available after render"
                );
            })
            .expect("target host should remain live after render");

        let target_visual = VisualTestContext::from_window(target_opened.window(), cx);
        assert!(
            selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview).is_some(),
            "target viewport should render the local drop preview"
        );

        let release_resolution = cx
            .update(|app| runtime.resolve_payload_drop_delivery_for_request(&preview_request, app));
        assert!(
            matches!(
                release_resolution.route(),
                DockViewportDropRoute::KnownViewport { .. }
            ),
            "hidden routed preview must not become release authority, got {:?}",
            release_resolution.route()
        );
        let delivery = DockDropDelivery::from_resolution(release_resolution)
            .expect("current routed target should remain deliverable after local preview render");
        let workspace_target = delivery
            .workspace_target()
            .expect("known viewport delivery should carry a workspace target");
        assert_eq!(
            workspace_target.target_space(),
            &target_space,
            "delivery should still point at the current routed viewport target"
        );
        assert_eq!(
            workspace_target.target_window_id(),
            Some(target_opened.window().window_id()),
            "delivery should keep the target viewport window identity"
        );
    }

    #[open_gpui::test]
    fn source_only_release_with_known_empty_hover_does_not_commit_from_stale_routed_preview(
        cx: &mut TestAppContext,
    ) {
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
                runtime.open_viewport_unchecked_policy(
                    target_space.clone(),
                    WindowOptions {
                        window_bounds: Some(target_bounds),
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("target viewport should open");
        let _source_opened = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
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
        let release_screen_position = point(
            target_bounds.get_bounds().origin.x + target_center_host_position().x,
            target_bounds.get_bounds().origin.y + target_center_host_position().y,
        );

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);
        let preview_resolution = cache_known_viewport_preview_with_payload(
            cx,
            &runtime,
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_screen_position,
            target_opened.window(),
            Some(session.clone()),
            &payload,
        );
        assert!(
            matches!(
                preview_resolution.route(),
                DockViewportDropRoute::KnownViewport { target, source }
                    if target.window_id() == target_opened.window().window_id()
                        && *source
                            == crate::DockViewportRouteSelectionSource::TrustedHoveredWindow
            ),
            "preview route should be selected by the current trusted hovered viewport, got {:?}",
            preview_resolution.route()
        );
        assert!(
            preview_resolution
                .routed_preview_target_snapshot()
                .is_some(),
            "preview route must carry a target snapshot for the runtime to remember last routed viewport identity; got route {:?}",
            preview_resolution.route(),
        );
        assert_eq!(
            preview_resolution
                .delivery()
                .and_then(|delivery| delivery.drag_session_id()),
            Some(session.id()),
            "fresh preview should mint delivery bound to the active drag session"
        );
        assert!(
            runtime
                .routed_drop_preview_for(&target_space, target_opened.window().window_id())
                .is_some(),
            "routed preview publication should produce a preview for the target window"
        );
        assert_eq!(
            runtime
                .last_routed_viewport_identity_for_drag_session(Some(&session))
                .map(|identity| identity.window_id()),
            Some(target_opened.window().window_id()),
            "routed preview publication should remember the last routed viewport identity for this drag session"
        );
        let hovered_none_release_request = DockViewportDropRouteRequest::from_platform_signals(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_screen_position,
            None,
            crate::DockViewportPlatformSignals::from_target_context(
                DockViewportTargetContext::new().with_trusted_hovered_window_known_empty(),
            ),
        )
        .with_drag_session(Some(session.clone()));
        let hovered_none_resolution = cx.update(|app| {
            runtime.resolve_payload_drop_delivery(&hovered_none_release_request, app)
        });
        assert_eq!(
            hovered_none_resolution.route(),
            &DockViewportDropRoute::Unavailable,
            "trusted hovered=None is authoritative and must not reuse stale routed preview state"
        );
        assert!(
            hovered_none_resolution.delivery().is_none(),
            "trusted hovered=None must not mint delivery from stale routed preview state"
        );
        let release_request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_screen_position,
            None,
            crate::DockViewportPlatformSignals::from_target_context(
                DockViewportTargetContext::new().with_trusted_hovered_window_known_empty(),
            ),
            DockPayloadDropReleaseOrigin::SourceOnly,
        )
        .with_drag_session(Some(session.clone()));
        let raw_release_route =
            cx.update(|app| runtime.resolve_payload_drop_route_for_test(&release_request, app));
        assert_eq!(
            raw_release_route,
            DockViewportDropRoute::Unavailable,
            "raw route should trust hovered=None instead of reusing cached routed preview state"
        );
        let release_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&release_request, app));
        assert_eq!(
            release_resolution.route(),
            &DockViewportDropRoute::Unavailable,
            "runtime route should trust hovered=None instead of reusing cached routed preview state"
        );

        let commit_result =
            cx.update(|app| runtime.commit_payload_drop_from_screen(&release_request, app));
        assert_eq!(
            commit_result,
            Err(DockActionApplyError::DropTargetUnavailable),
            "trusted hovered=None should prevent cross-viewport commit from stale routed preview state"
        );
        cx.run_until_parked();
        let status = runtime.runtime_status();
        assert!(
            matches!(
                status.last_route.as_ref().map(|record| &record.target),
                Some(crate::DockViewportRouteTarget::Unavailable)
            ),
            "host release should record an unavailable route, got {:?}",
            status.last_route
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
    fn runtime_opened_viewports_do_not_reuse_previewed_target_when_source_only_release_leaves_viewport(
        cx: &mut TestAppContext,
    ) {
        let target_space = DockSpaceId::from("main");
        let source_space = DockSpaceId::from("detached");
        let mut graph = DockGraph::new();
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        graph.set_root(target_space.clone(), target_tabs);
        graph.set_root(source_space.clone(), source_tabs);

        let mut workspace = DockWorkspace::new(target_space.clone(), graph);
        workspace.policy_mut().set_allow_platform_viewports(true);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller.clone());

        let target_opened = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
                    target_space.clone(),
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect("target viewport should open");
        let source_opened = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
                    source_space.clone(),
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect("source viewport should open");
        let _target_window = target_opened
            .window()
            .downcast::<crate::DockHost>()
            .expect("target viewport should render DockHost");
        let source_window = source_opened
            .window()
            .downcast::<crate::DockHost>()
            .expect("source viewport should render DockHost");
        cx.run_until_parked();

        let target_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
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
            "A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);
        let resolution = cache_known_viewport_preview_with_payload(
            cx,
            &runtime,
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            point(px(220.0), px(200.0)),
            target_opened.window(),
            Some(session.clone()),
            &payload,
        );
        assert!(
            matches!(
                resolution.route(),
                DockViewportDropRoute::KnownViewport { target, .. }
                    if target.window_id() == target_opened.window().window_id()
            ),
            "preview route should target the main viewport"
        );
        assert!(
            runtime
                .routed_drop_preview_for(&target_space, target_opened.window().window_id())
                .is_some(),
            "shared runtime should store the routed preview"
        );
        assert!(
            runtime.has_routed_drop_preview_for_drag_session(Some(&session)),
            "rendered routed preview should expose reusable delivery before release"
        );
        assert_eq!(
            runtime
                .active_payload_drag_session(&payload)
                .expect("drag session should still be active before release")
                .id(),
            session.id(),
            "the active session should still match the routed preview session"
        );
        let release = DockPayloadDropRelease::source_only_with_session(
            payload.clone(),
            source_space.clone(),
            point(px(900.0), px(900.0)),
            Some(session.clone()),
        );
        source_window
            .update(cx, |host, window, cx| {
                host.drop_payload_release_from_render(release, window, cx)
            })
            .expect("source host should handle the source-only release");
        cx.run_until_parked();

        cx.read_entity(&controller, |controller, _| {
            let DockNode::Tabs { items, selected } = controller
                .graph()
                .node(target_tabs)
                .expect("target tabs should still exist")
            else {
                panic!("target should remain tabs");
            };
            assert_eq!(items, &vec![item("b")]);
            assert_eq!(selected.as_ref(), items.get(0));
            assert_eq!(
                controller.graph().collect_items_in_space(&target_space),
                vec![item("b")]
            );
        });
    }
}
