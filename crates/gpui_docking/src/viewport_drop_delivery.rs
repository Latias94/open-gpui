mod current_facts;
mod model;

#[cfg(test)]
pub(crate) use current_facts::resolve_workspace_target_for_route;
#[cfg(test)]
pub(crate) use current_facts::validate_delivery_workspace_target;
#[cfg(test)]
use current_facts::validate_delivery_workspace_target_inner;
pub(crate) use current_facts::{
    DockViewportWorkspaceRouteFacts, resolve_delivery_workspace_target_with_facts,
    resolve_workspace_target_for_route_with_facts_and_reorder_hold,
};
pub(crate) use model::{
    DockDropDelivery, DockDropWorkspaceCommit, DockViewportResolvedDropRoute,
    DockViewportResolvedDropTargetSnapshot, DockViewportTabReorderHold,
};
#[cfg(test)]
pub(crate) use model::{DockDropDeliveryKind, DockViewportWorkspaceRouteTarget};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockActionApplyError, DockClassId, DockGraph, DockItemId, DockNode, DockNodeId, DockPanel,
        DockPolicyError, DockSpaceId, DockViewportAdapter, DockViewportDropPayload,
        DockViewportDropReleasePoint, DockViewportDropRoute, DockViewportDropRouteRequest,
        DockViewportPlatformSignals, DockViewportPointerCoordinateSpace,
        DockViewportRouteSelectionSource, DockViewportTargetContext, DockViewportTargetHit,
        DockViewportTearOffRequest, DockViewportWindowFacts, DockWorkspace,
        drag::{DockDragPayload, DockDragTearOffGeometry},
        drop_runtime::DockHostDropSceneFact,
        drop_target::{
            DockDropResolveSource, DockEmptySpaceDropTarget, DockLeafDropTarget,
            DockResolvedDropTarget, DockResolvedDropTargetKind,
        },
        geometry::{self, DockDropBoxKind, DockDropBoxSet},
        host_test_support::center_drop_position,
        interaction::{DockPayloadDropReleaseOrigin, DockRuntimeDragSession},
        viewport_drop_scene::{
            DockViewportHostSceneDraft, DockViewportHostSceneRegistry,
            DockViewportHostSceneSnapshot,
        },
        viewport_registry::DockViewportWindowBoundsFrame,
        viewport_test_support::{bounds, handle, item, register_viewport, space},
    };
    use open_gpui::{Bounds, WindowBounds, WindowId, point, px, size};
    use slotmap::Key;

    fn current_registration_host_scene(
        adapter: &DockViewportAdapter,
        space: DockSpaceId,
        window_id: WindowId,
        current_bounds: DockViewportWindowBoundsFrame,
        host_bounds: Bounds<open_gpui::Pixels>,
        host_position: open_gpui::Point<open_gpui::Pixels>,
        drop_guide_metrics: crate::DockDropGuideMetrics,
    ) -> DockViewportHostSceneSnapshot {
        let registration_key = adapter
            .registration_key(&space)
            .filter(|key| key.window_id() == window_id)
            .expect("test host scene must bind to the current viewport registration");
        DockViewportHostSceneDraft::new(
            space,
            window_id,
            current_bounds,
            host_bounds,
            host_position,
            drop_guide_metrics,
        )
        .bind(registration_key)
        .expect("matching current registration should bind test host scene")
    }

    #[test]
    fn current_facts_delivery_mints_for_current_route_selection_sources() {
        let source_window = handle(1);
        let target = space("target");
        let target_window = handle(2);
        let host_position = point(px(12.0), px(34.0));
        let target_hit = crate::DockViewportTargetHit::with_facts_generation(
            target.clone(),
            target_window,
            host_position,
            7,
        );

        let request = DockViewportDropRouteRequest::from_target_context(
            space("source"),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(900.0), px(900.0)),
            None,
            DockViewportTargetContext::new(),
        );
        let resolved_target =
            resolved_drop_target_snapshot(target.clone(), target_window.window_id(), 7);

        let local_delivery = DockDropDelivery::from_route_request_with_resolved_target(
            &request,
            DockViewportDropRoute::local_for_test(
                space("source"),
                source_window.window_id(),
                host_position,
                7,
                crate::DockViewportRouteSelectionSource::TrustedHoveredWindow,
            ),
            Some(resolved_target.clone()),
        );
        assert!(local_delivery.is_some());

        let known_delivery = DockDropDelivery::from_route_request_with_resolved_target(
            &request,
            DockViewportDropRoute::KnownViewport {
                target: target_hit.clone(),
                source: crate::DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
            },
            Some(resolved_target.clone()),
        );
        assert!(known_delivery.is_some());

        let tear_off_delivery =
            DockDropDelivery::from_route_request(&request, DockViewportDropRoute::TearOff);
        assert!(tear_off_delivery.is_some());

        assert_eq!(
            DockDropDelivery::from_route_request(
                &request,
                DockViewportDropRoute::rejected_by_policy(
                    DockPolicyError::PlatformViewportsDisabled,
                )
            ),
            None
        );
        assert_eq!(
            DockDropDelivery::from_route_request(&request, DockViewportDropRoute::Unavailable),
            None
        );
    }

    #[test]
    fn local_drop_delivery_without_resolved_snapshot_is_absent() {
        let source = space("source");
        let source_tabs = DockNodeId::null();
        let item = item("a");
        let drag_payload =
            DockDragPayload::new_item(source.clone(), source_tabs, item.clone(), "A".to_string());
        let drag_session = DockRuntimeDragSession::new(13, &drag_payload);
        let request = DockViewportDropRouteRequest::from_target_context(
            source.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item.clone()),
            point(px(900.0), px(900.0)),
            None,
            DockViewportTargetContext::new(),
        )
        .with_drag_session(Some(drag_session.clone()));
        let local_position = point(px(5.0), px(7.0));

        let delivery = DockDropDelivery::from_route_request(
            &request,
            DockViewportDropRoute::local_for_test(
                source,
                handle(1).window_id(),
                local_position,
                1,
                DockViewportRouteSelectionSource::TrustedHoveredWindow,
            ),
        );
        assert_eq!(delivery, None);
    }

    #[test]
    fn fallback_local_route_without_resolved_snapshot_is_absent() {
        let source = space("source");
        let source_tabs = DockNodeId::null();
        let item = item("a");
        let request = DockViewportDropRouteRequest::from_target_context(
            source,
            source_tabs,
            DockViewportDropPayload::Item(item),
            point(px(120.0), px(140.0)),
            None,
            DockViewportTargetContext::new(),
        );

        let delivery = DockDropDelivery::from_route_request_with_resolved_target(
            &request,
            DockViewportDropRoute::local_for_test(
                request.source_space().clone(),
                handle(1).window_id(),
                point(px(20.0), px(40.0)),
                1,
                DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
            ),
            None,
        );
        assert_eq!(
            delivery, None,
            "workspace delivery still requires the resolved drop target snapshot"
        );
    }

    #[test]
    fn known_viewport_drop_delivery_without_resolved_snapshot_is_absent() {
        let source = space("source");
        let source_tabs = DockNodeId::null();
        let item = item("a");
        let drag_payload =
            DockDragPayload::new_item(source.clone(), source_tabs, item.clone(), "A".to_string());
        let drag_session = DockRuntimeDragSession::new(13, &drag_payload);
        let request = DockViewportDropRouteRequest::from_target_context(
            source.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item.clone()),
            point(px(900.0), px(900.0)),
            None,
            DockViewportTargetContext::new(),
        )
        .with_drag_session(Some(drag_session.clone()));
        let target = space("target");
        let target_window = handle(9);
        let known_position = point(px(12.0), px(34.0));

        let delivery = DockDropDelivery::from_route_request(
            &request,
            DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::new(target.clone(), target_window, known_position),
                source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
        );
        assert_eq!(delivery, None);
    }

    #[test]
    fn known_viewport_drop_delivery_mints_from_current_target_snapshot() {
        let source = space("source");
        let source_tabs = DockNodeId::null();
        let item = item("a");
        let drag_payload =
            DockDragPayload::new_item(source.clone(), source_tabs, item.clone(), "A".to_string());
        let drag_session = DockRuntimeDragSession::new(13, &drag_payload);
        let request = DockViewportDropRouteRequest::from_target_context(
            source.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item.clone()),
            point(px(900.0), px(900.0)),
            None,
            DockViewportTargetContext::new(),
        )
        .with_drag_session(Some(drag_session.clone()));
        let target = space("target");
        let target_window = handle(9);
        let known_position = point(px(12.0), px(34.0));
        let target_facts_generation = 41;
        let resolved_target = resolved_drop_target_snapshot(
            target.clone(),
            target_window.window_id(),
            target_facts_generation,
        );

        let target_hit = DockViewportTargetHit::with_facts_generation(
            target,
            target_window,
            known_position,
            target_facts_generation,
        );

        let delivery = DockDropDelivery::from_route_request_with_resolved_target(
            &request,
            DockViewportDropRoute::KnownViewport {
                target: target_hit.clone(),
                source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            Some(resolved_target.clone()),
        );
        assert!(delivery.is_some());

        let delivery = DockDropDelivery::from_route_request_with_resolved_target(
            &request,
            DockViewportDropRoute::KnownViewport {
                target: target_hit,
                source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            Some(resolved_target.clone()),
        )
        .expect("current facts should derive a delivery");
        let DockDropDeliveryKind::Workspace(known) = delivery.kind() else {
            panic!("current known viewport route should derive a workspace commit");
        };
        assert_eq!(delivery.drag_session_id(), Some(drag_session.id()));
        assert_eq!(delivery.source_space(), &source);
        assert_eq!(delivery.source_node(), source_tabs);
        assert_eq!(delivery.payload(), &DockViewportDropPayload::Item(item));
        assert_eq!(known, &resolved_target);
    }

    #[test]
    fn source_only_cross_viewport_delivery_mints_from_current_target_snapshot() {
        let source = space("source");
        let source_tabs = DockNodeId::null();
        let item = item("a");
        let drag_payload =
            DockDragPayload::new_item(source.clone(), source_tabs, item.clone(), "A".to_string());
        let drag_session = DockRuntimeDragSession::new(13, &drag_payload);
        let request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item),
            point(px(900.0), px(900.0)),
            None,
            DockViewportPlatformSignals::from_target_context(DockViewportTargetContext::new()),
            DockPayloadDropReleaseOrigin::SourceOnly,
        )
        .with_drag_session(Some(drag_session));
        let target = space("target");
        let target_window = handle(9);
        let known_position = point(px(12.0), px(34.0));
        let target_facts_generation = 41;
        let resolved_target = resolved_drop_target_snapshot(
            target.clone(),
            target_window.window_id(),
            target_facts_generation,
        );
        let target_hit = DockViewportTargetHit::with_facts_generation(
            target,
            target_window,
            known_position,
            target_facts_generation,
        );

        let delivery = DockDropDelivery::from_route_request_with_resolved_target(
            &request,
            DockViewportDropRoute::KnownViewport {
                target: target_hit.clone(),
                source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            Some(resolved_target.clone()),
        );
        assert!(delivery.is_some());

        let delivery = DockDropDelivery::from_route_request_with_resolved_target(
            &request,
            DockViewportDropRoute::KnownViewport {
                target: target_hit,
                source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            Some(resolved_target.clone()),
        )
        .expect("current target snapshot may mint source-only cross-viewport delivery");
        let DockDropDeliveryKind::Workspace(known) = delivery.kind() else {
            panic!("current source-only delivery should derive a workspace commit");
        };
        assert_eq!(known, &resolved_target);
    }

    #[test]
    fn drop_delivery_derives_tear_off_request_from_route_request() {
        let source = space("source");
        let source_tabs = DockNodeId::null();
        let item = item("a");
        let release_position = point(px(900.0), px(900.0));
        let suggested_window_bounds = WindowBounds::Windowed(bounds(880.0, 880.0, 360.0, 240.0));
        let tear_off_geometry = DockDragTearOffGeometry::from_source_bounds(
            bounds(200.0, 120.0, 480.0, 300.0),
            point(px(260.0), px(150.0)),
        );
        let drag_payload =
            DockDragPayload::new_item(source.clone(), source_tabs, item.clone(), "A".to_string());
        let drag_session = DockRuntimeDragSession::new(21, &drag_payload);
        let request = DockViewportDropRouteRequest::from_target_context(
            source.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item.clone()),
            release_position,
            Some(suggested_window_bounds),
            DockViewportTargetContext::new(),
        )
        .with_drag_session(Some(drag_session.clone()))
        .with_tear_off_geometry(Some(tear_off_geometry));
        let route = DockViewportDropRoute::TearOff;

        assert_eq!(
            DockDropDelivery::from_route_request(&request, route)
                .expect("tear-off route should derive a delivery")
                .tear_off_request()
                .as_ref(),
            Some(
                &DockViewportTearOffRequest::new(
                    source,
                    source_tabs,
                    DockViewportDropPayload::Item(item),
                    Some(release_position),
                    Some(suggested_window_bounds),
                )
                .with_drag_session(Some(drag_session))
                .with_tear_off_geometry(Some(tear_off_geometry))
            )
        );
    }

    #[test]
    fn drop_delivery_preserves_global_release_point_for_tear_off_request() {
        let source = space("source");
        let source_tabs = DockNodeId::null();
        let release_position = point(px(430.0), px(350.0));
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_position,
            None,
            DockViewportPlatformSignals::from_target_context(DockViewportTargetContext::new()),
        );

        let tear_off =
            DockDropDelivery::from_route_request(&request, DockViewportDropRoute::TearOff)
                .expect("tear-off route should derive a delivery")
                .tear_off_request()
                .expect("delivery should contain a tear-off request");

        assert_eq!(tear_off.release_position(), Some(release_position));
    }

    #[test]
    fn drop_delivery_omits_local_release_point_for_tear_off_request() {
        let source = space("source");
        let source_tabs = DockNodeId::null();
        let request = DockViewportDropRouteRequest::from_host_release(
            source.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            DockViewportDropReleasePoint::host_local(
                point(px(30.0), px(50.0)),
                bounds(400.0, 300.0, 320.0, 240.0),
            ),
            None,
            DockViewportPlatformSignals::from_target_context(DockViewportTargetContext::new())
                .with_global_window_bounds(false),
            DockPayloadDropReleaseOrigin::HoveredHost,
        );

        let tear_off =
            DockDropDelivery::from_route_request(&request, DockViewportDropRoute::TearOff)
                .expect("tear-off route should derive a delivery")
                .tear_off_request()
                .expect("delivery should contain a tear-off request");

        assert_eq!(request.release_position(), point(px(30.0), px(50.0)));
        assert_eq!(
            request.coordinate_space(),
            DockViewportPointerCoordinateSpace::EventReceiverLocal
        );
        assert_eq!(tear_off.release_position(), None);
    }

    #[test]
    fn drop_delivery_omits_source_local_release_point_for_tear_off_request() {
        let source = space("source");
        let source_tabs = DockNodeId::null();
        let request = DockViewportDropRouteRequest::from_host_release(
            source.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            DockViewportDropReleasePoint::host_local(
                point(px(30.0), px(50.0)),
                bounds(400.0, 300.0, 320.0, 240.0),
            ),
            None,
            DockViewportPlatformSignals::from_target_context(DockViewportTargetContext::new())
                .with_global_window_bounds(false),
            DockPayloadDropReleaseOrigin::SourceOnly,
        );

        let tear_off =
            DockDropDelivery::from_route_request(&request, DockViewportDropRoute::TearOff)
                .expect("tear-off route should derive a delivery")
                .tear_off_request()
                .expect("delivery should contain a tear-off request");

        assert_eq!(request.release_position(), point(px(30.0), px(50.0)));
        assert_eq!(
            request.coordinate_space(),
            DockViewportPointerCoordinateSpace::SourceLocalOnly
        );
        assert_eq!(tear_off.release_position(), None);
    }

    #[test]
    fn local_route_requires_current_window_host_scene_identity() {
        let source_space = space("source");
        let old_window = handle(1);
        let new_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source_space.clone(), new_window);
        adapter.update_snapshot(
            &source_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );

        let mut host_scenes = DockViewportHostSceneRegistry::default();
        let old_frame = host_scenes
            .register(DockViewportHostSceneSnapshot::new(
                source_space.clone(),
                old_window.window_id(),
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(100.0, 100.0, 320.0, 240.0)),
                bounds(0.0, 0.0, 320.0, 240.0),
                point(px(24.0), px(24.0)),
                crate::DockDropGuideMetrics::default(),
            ))
            .frame;
        assert!(
            host_scenes
                .push_frame_fact(
                    &old_frame,
                    DockHostDropSceneFact::EmptySpace(DockEmptySpaceDropTarget {
                        space: source_space.clone(),
                        bounds: bounds(0.0, 0.0, 320.0, 240.0),
                        is_central: false,
                    })
                )
                .is_some()
        );

        let workspace = DockWorkspace::new(source_space.clone(), DockGraph::new());
        let payload = DockViewportDropPayload::Item(item("a"));
        let payload_classes =
            workspace.payload_dock_classes_for_viewport_payload(&payload, DockNodeId::null());
        let request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            DockNodeId::null(),
            payload,
            point(px(124.0), px(124.0)),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(new_window),
        );

        let target = resolve_workspace_target_for_route(
            &adapter,
            &host_scenes,
            &DockViewportDropRoute::local_for_test(
                source_space.clone(),
                old_window.window_id(),
                point(px(24.0), px(24.0)),
                1,
                crate::DockViewportRouteSelectionSource::TrustedHoveredWindow,
            ),
            &request,
            &workspace,
            &payload_classes,
        );

        assert!(
            matches!(target, DockViewportWorkspaceRouteTarget::RouteUnavailable),
            "local route must not replace its frozen source window with the current source mapping"
        );
    }

    #[test]
    fn local_route_rejects_recreated_registration_with_same_identity() {
        let source_space = space("source");
        let window = handle(1);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source_space.clone(), window);
        adapter.update_snapshot(
            &source_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        let stale_proof = crate::DockViewportRouteProof::new(
            adapter
                .registration_key(&source_space)
                .expect("registered viewport should have an exact key"),
            adapter
                .snapshot_facts_generation(&source_space, window.window_id())
                .expect("registered viewport should have route facts"),
        );

        adapter.unregister_space(&source_space);
        register_viewport(&mut adapter, source_space.clone(), window);
        adapter.update_snapshot(
            &source_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        assert_ne!(
            adapter.registration_key(&source_space).as_ref(),
            Some(stale_proof.registration_key())
        );

        let workspace = DockWorkspace::new(source_space.clone(), DockGraph::new());
        let payload = DockViewportDropPayload::Item(item("a"));
        let payload_classes =
            workspace.payload_dock_classes_for_viewport_payload(&payload, DockNodeId::null());
        let request = DockViewportDropRouteRequest::from_target_context(
            source_space,
            DockNodeId::null(),
            payload,
            point(px(124.0), px(124.0)),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(window),
        );
        let target = resolve_workspace_target_for_route(
            &adapter,
            &DockViewportHostSceneRegistry::default(),
            &DockViewportDropRoute::Local {
                host_position: point(px(24.0), px(24.0)),
                route_proof: stale_proof,
                source: crate::DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            &request,
            &workspace,
            &payload_classes,
        );

        assert!(matches!(
            target,
            DockViewportWorkspaceRouteTarget::RouteUnavailable
        ));
    }

    #[test]
    fn local_route_without_current_host_target_preserves_route_state() {
        let source_space = space("source");
        let window = handle(1);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source_space.clone(), window);
        adapter.update_snapshot(
            &source_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        let facts_generation = adapter
            .snapshot_facts_generation(&source_space, window.window_id())
            .expect("source snapshot should have facts");

        let host_scenes = DockViewportHostSceneRegistry::default();
        let workspace = DockWorkspace::new(source_space.clone(), DockGraph::new());
        let payload = DockViewportDropPayload::Item(item("a"));
        let payload_classes =
            workspace.payload_dock_classes_for_viewport_payload(&payload, DockNodeId::null());
        let request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            DockNodeId::null(),
            payload,
            point(px(124.0), px(124.0)),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(window),
        );

        let target = resolve_workspace_target_for_route(
            &adapter,
            &host_scenes,
            &DockViewportDropRoute::local_for_registration_test(
                adapter
                    .registration_key(&source_space)
                    .expect("registered viewport should have an exact key"),
                point(px(24.0), px(24.0)),
                facts_generation,
                crate::DockViewportRouteSelectionSource::TrustedHoveredWindow,
            ),
            &request,
            &workspace,
            &payload_classes,
        );

        assert!(
            matches!(
                target,
                DockViewportWorkspaceRouteTarget::NoCurrentHostTarget
            ),
            "local route should keep its route state even when the host scene has no target"
        );
    }

    #[test]
    fn local_route_excludes_source_floating_from_cached_host_scene() {
        let source_space = space("source");
        let window = handle(4);
        let mut graph = DockGraph::new();
        let root = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("target")],
            selected: Some(DockItemId::from("target")),
        });
        graph.set_root(source_space.clone(), root);
        let floating_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("floating")],
            selected: Some(DockItemId::from("floating")),
        });
        let floating = graph.insert_node(DockNode::Floating {
            child: floating_tabs,
        });

        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source_space.clone(), window);
        adapter.update_snapshot(
            &source_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 360.0, 240.0,
            ))),
            bounds(0.0, 0.0, 360.0, 240.0),
        );
        let facts_generation = adapter
            .snapshot_facts_generation(&source_space, window.window_id())
            .expect("source snapshot should have facts");

        let host_position = center_drop_position(bounds(0.0, 0.0, 360.0, 240.0));
        let mut host_scenes = DockViewportHostSceneRegistry::default();
        let frame = host_scenes
            .register(current_registration_host_scene(
                &adapter,
                source_space.clone(),
                window.window_id(),
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(100.0, 100.0, 360.0, 240.0)),
                bounds(0.0, 0.0, 360.0, 240.0),
                host_position,
                crate::DockDropGuideMetrics::default(),
            ))
            .frame;
        let frame = host_scenes
            .push_frame_fact(
                &frame,
                DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                    root,
                    target_tabs: root,
                    bounds: bounds(0.0, 0.0, 360.0, 240.0),
                    is_central: false,
                }),
            )
            .expect("root target fact should update the current frame");
        let frame = host_scenes
            .push_frame_fact(
                &frame,
                DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                    root: floating,
                    target_tabs: floating_tabs,
                    bounds: bounds(0.0, 0.0, 360.0, 240.0),
                    is_central: false,
                }),
            )
            .expect("source floating child fact should update the current frame");
        assert!(
            host_scenes
                .push_frame_fact(
                    &frame,
                    crate::drop_scene_fact::floating_title_bar(
                        floating,
                        floating_tabs,
                        bounds(0.0, 0.0, 360.0, 240.0),
                        bounds(0.0, 0.0, 360.0, 240.0),
                    )
                )
                .is_some()
        );

        let workspace = DockWorkspace::new(source_space.clone(), graph);
        let payload = DockViewportDropPayload::Floating(floating);
        let payload_classes =
            workspace.payload_dock_classes_for_viewport_payload(&payload, floating);
        let request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            floating,
            payload,
            point(px(280.0), px(220.0)),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(window),
        );

        let target = resolve_workspace_target_for_route(
            &adapter,
            &host_scenes,
            &DockViewportDropRoute::local_for_registration_test(
                adapter
                    .registration_key(&source_space)
                    .expect("registered viewport should have an exact key"),
                host_position,
                facts_generation,
                crate::DockViewportRouteSelectionSource::TrustedHoveredWindow,
            ),
            &request,
            &workspace,
            &payload_classes,
        );

        let DockViewportWorkspaceRouteTarget::Resolved(target) = target else {
            panic!("local route should resolve the underlying root target");
        };
        assert_eq!(
            target.into_target().kind,
            DockResolvedDropTargetKind::LeafCenter {
                root,
                target_tabs: root,
            }
        );
    }

    #[test]
    fn local_route_preserves_policy_rejected_target() {
        let source_space = space("source");
        let window = handle(3);
        let mut graph = DockGraph::new();
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("a")],
            selected: Some(DockItemId::from("a")),
        });
        graph.set_root(source_space.clone(), tabs);

        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source_space.clone(), window);
        adapter.update_snapshot(
            &source_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        let facts_generation = adapter
            .snapshot_facts_generation(&source_space, window.window_id())
            .expect("source snapshot should have facts");

        let mut host_scenes = DockViewportHostSceneRegistry::default();
        let host_position = center_drop_position(bounds(0.0, 0.0, 320.0, 240.0));
        let frame = host_scenes
            .register(current_registration_host_scene(
                &adapter,
                source_space.clone(),
                window.window_id(),
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(100.0, 100.0, 320.0, 240.0)),
                bounds(0.0, 0.0, 320.0, 240.0),
                host_position,
                crate::DockDropGuideMetrics::default(),
            ))
            .frame;
        assert!(
            host_scenes
                .push_frame_fact(
                    &frame,
                    DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                        root: tabs,
                        target_tabs: tabs,
                        bounds: bounds(0.0, 0.0, 320.0, 240.0),
                        is_central: true,
                    })
                )
                .is_some()
        );

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel(
            item("a"),
            DockPanel::lazy("Panel A", |_| unreachable!()).with_dock_class("editor"),
        );
        workspace
            .policy_mut()
            .set_allowed_dock_classes_for_space(source_space.clone(), ["inspector"]);

        let payload = DockViewportDropPayload::Item(DockItemId::from("a"));
        let payload_classes =
            workspace.payload_dock_classes_for_viewport_payload(&payload, DockNodeId::null());
        let request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            DockNodeId::null(),
            payload,
            point(px(220.0), px(200.0)),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(window),
        )
        .with_tear_off_geometry(Some(
            DockDragTearOffGeometry::from_source_bounds(
                Bounds::new(point(px(0.0), px(0.0)), size(px(240.0), px(180.0))),
                point(px(12.0), px(12.0)),
            )
            .with_preferred_size(size(px(240.0), px(180.0))),
        ));

        let target = resolve_workspace_target_for_route(
            &adapter,
            &host_scenes,
            &DockViewportDropRoute::local_for_registration_test(
                adapter
                    .registration_key(&source_space)
                    .expect("registered viewport should have an exact key"),
                host_position,
                facts_generation,
                crate::DockViewportRouteSelectionSource::TrustedHoveredWindow,
            ),
            &request,
            &workspace,
            &payload_classes,
        );

        match target {
            DockViewportWorkspaceRouteTarget::Rejected { target, reason } => {
                assert_eq!(
                    reason,
                    DockPolicyError::DockClassRejected {
                        space: source_space.clone(),
                        item: DockItemId::from("a"),
                        dock_class: Some(DockClassId::from("editor")),
                    }
                );
                assert_eq!(target.target_space(), &source_space);
                assert_eq!(target.target_window_id(), Some(window.window_id()));
            }
            DockViewportWorkspaceRouteTarget::Resolved(_) => {
                panic!("local route should not resolve when policy rejects the payload")
            }
            DockViewportWorkspaceRouteTarget::PreviewOnly(_) => {
                panic!(
                    "local route should preserve rejected target instead of degrading to preview"
                )
            }
            DockViewportWorkspaceRouteTarget::NoCurrentHostTarget => {
                panic!("local route should preserve rejected target instead of losing host target")
            }
            DockViewportWorkspaceRouteTarget::RouteUnavailable => {
                panic!("local route should not be route-unavailable when the current facts match")
            }
            DockViewportWorkspaceRouteTarget::NotWorkspaceRoute => {
                panic!("local route should not be classified as non-workspace")
            }
        }
    }

    #[test]
    fn known_viewport_route_requires_current_target_window_facts() {
        let source_space = space("source");
        let target_space = space("target");
        let target_window = handle(7);
        let mut graph = DockGraph::new();
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("target")],
            selected: Some(DockItemId::from("target")),
        });
        graph.set_root(target_space.clone(), target_tabs);

        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        adapter.update_snapshot(
            &target_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        let old_generation = adapter
            .snapshot_facts_generation(&target_space, target_window.window_id())
            .expect("target snapshot should have route facts");
        assert!(adapter.mark_window_snapshot_stale(target_window.window_id()));

        let mut host_scenes = DockViewportHostSceneRegistry::default();
        let frame = host_scenes
            .register(current_registration_host_scene(
                &adapter,
                target_space.clone(),
                target_window.window_id(),
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(100.0, 100.0, 320.0, 240.0)),
                bounds(0.0, 0.0, 320.0, 240.0),
                point(px(24.0), px(24.0)),
                crate::DockDropGuideMetrics::default(),
            ))
            .frame;
        assert!(
            host_scenes
                .push_frame_fact(
                    &frame,
                    DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                        root: target_tabs,
                        target_tabs,
                        bounds: bounds(0.0, 0.0, 320.0, 240.0),
                        is_central: true,
                    })
                )
                .is_some()
        );

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_descriptor(
            item("a"),
            crate::DockPanelDescriptor::new("Panel A").with_dock_class("editor"),
        );
        let payload = DockViewportDropPayload::Item(item("a"));
        let payload_classes =
            workspace.payload_dock_classes_for_viewport_payload(&payload, DockNodeId::null());
        let request = DockViewportDropRouteRequest::from_target_context(
            source_space,
            DockNodeId::null(),
            payload,
            point(px(124.0), px(124.0)),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
        );

        let target = resolve_workspace_target_for_route(
            &adapter,
            &host_scenes,
            &DockViewportDropRoute::KnownViewport {
                target: crate::DockViewportTargetHit::with_facts_generation(
                    target_space,
                    target_window,
                    point(px(24.0), px(24.0)),
                    old_generation,
                ),
                source: crate::DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            &request,
            &workspace,
            &payload_classes,
        );

        assert!(
            matches!(target, DockViewportWorkspaceRouteTarget::RouteUnavailable),
            "known viewport route must not resolve preview targets from stale window facts"
        );
    }

    #[test]
    fn known_viewport_route_without_current_host_target_is_unavailable() {
        let source_space = space("source");
        let target_space = space("target");
        let target_window = handle(7);
        let mut graph = DockGraph::new();
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("target")],
            selected: Some(DockItemId::from("target")),
        });
        graph.set_root(target_space.clone(), target_tabs);

        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        adapter.update_snapshot(
            &target_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        let facts_generation = adapter
            .snapshot_facts_generation(&target_space, target_window.window_id())
            .expect("target snapshot should have route facts");

        let host_scenes = DockViewportHostSceneRegistry::default();
        let workspace = DockWorkspace::new(source_space.clone(), graph);
        let payload = DockViewportDropPayload::Item(item("a"));
        let payload_classes =
            workspace.payload_dock_classes_for_viewport_payload(&payload, DockNodeId::null());
        let request = DockViewportDropRouteRequest::from_target_context(
            source_space,
            DockNodeId::null(),
            payload,
            point(px(124.0), px(124.0)),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
        );

        let target = resolve_workspace_target_for_route(
            &adapter,
            &host_scenes,
            &DockViewportDropRoute::KnownViewport {
                target: crate::DockViewportTargetHit::with_facts_generation(
                    target_space,
                    target_window,
                    point(px(24.0), px(24.0)),
                    facts_generation,
                ),
                source: crate::DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            &request,
            &workspace,
            &payload_classes,
        );

        assert!(
            matches!(target, DockViewportWorkspaceRouteTarget::RouteUnavailable),
            "known viewport route should become unavailable when its current host scene disappears"
        );
    }

    #[test]
    fn delivery_validation_requires_current_target_key() {
        let source_space = space("source");
        let target_space = space("target");
        let target_window = handle(8);
        let mut graph = DockGraph::new();
        let current_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("current")],
            selected: Some(DockItemId::from("current")),
        });
        let stale_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("stale")],
            selected: Some(DockItemId::from("stale")),
        });
        graph.set_root(target_space.clone(), current_tabs);

        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        adapter.update_snapshot(
            &target_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        let facts_generation = adapter
            .snapshot_facts_generation(&target_space, target_window.window_id())
            .expect("target snapshot should have facts");

        let mut host_scenes = DockViewportHostSceneRegistry::default();
        let host_position = center_drop_position(bounds(0.0, 0.0, 320.0, 240.0));
        let frame = host_scenes
            .register(current_registration_host_scene(
                &adapter,
                target_space.clone(),
                target_window.window_id(),
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(100.0, 100.0, 320.0, 240.0)),
                bounds(0.0, 0.0, 320.0, 240.0),
                host_position,
                crate::DockDropGuideMetrics::default(),
            ))
            .frame;
        let frame = host_scenes
            .push_frame_fact(
                &frame,
                DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                    root: current_tabs,
                    target_tabs: current_tabs,
                    bounds: bounds(0.0, 0.0, 320.0, 240.0),
                    is_central: true,
                }),
            )
            .expect("current target fact should produce a current frame");

        let workspace = DockWorkspace::new(source_space.clone(), graph);
        let payload = DockViewportDropPayload::Item(item("current"));
        let stale_snapshot = DockViewportResolvedDropTargetSnapshot::new(
            frame,
            crate::DockDropGuideMetrics::default(),
            facts_generation,
            true,
            host_position,
            None,
            DockResolvedDropTarget {
                kind: DockResolvedDropTargetKind::LeafCenter {
                    root: stale_tabs,
                    target_tabs: stale_tabs,
                },
                source: DockDropResolveSource::LeafBody,
                target_bounds: Some(bounds(0.0, 0.0, 320.0, 240.0)),
                inner_target_bounds: Some(bounds(0.0, 0.0, 320.0, 240.0)),
                availability: crate::drop_target::DockResolvedDropTargetAvailability::all(),
                drop_box: None,
                hit_bounds: Some(bounds(0.0, 0.0, 320.0, 240.0)),
                preview_bounds: Some(bounds(0.0, 0.0, 320.0, 240.0)),
                tab_insertion_bounds: None,
                edge_sizing: None,
                edge_plan: None,
                is_central_region: true,
            },
        );

        let result = validate_delivery_workspace_target(
            &adapter,
            &host_scenes,
            &workspace,
            DockNodeId::null(),
            &payload,
            &stale_snapshot,
        );

        assert_eq!(result, Err(DockActionApplyError::DropTargetUnavailable));
    }

    #[test]
    fn delivery_validation_requires_current_host_scene_frame() {
        let source_space = space("source");
        let target_space = space("target");
        let target_window = handle(9);
        let mut graph = DockGraph::new();
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("target")],
            selected: Some(DockItemId::from("target")),
        });
        graph.set_root(target_space.clone(), target_tabs);

        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        adapter.update_snapshot(
            &target_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        let facts_generation = adapter
            .snapshot_facts_generation(&target_space, target_window.window_id())
            .expect("target snapshot should have facts");

        let mut host_scenes = DockViewportHostSceneRegistry::default();
        let host_position = center_drop_position(bounds(0.0, 0.0, 320.0, 240.0));
        let stale_frame = host_scenes
            .register(current_registration_host_scene(
                &adapter,
                target_space.clone(),
                target_window.window_id(),
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(100.0, 100.0, 320.0, 240.0)),
                bounds(0.0, 0.0, 320.0, 240.0),
                host_position,
                crate::DockDropGuideMetrics::default(),
            ))
            .frame;
        let current_frame = host_scenes
            .push_frame_fact(
                &stale_frame,
                DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                    root: target_tabs,
                    target_tabs,
                    bounds: bounds(0.0, 0.0, 320.0, 240.0),
                    is_central: true,
                }),
            )
            .expect("current target fact should produce a current frame");
        assert_ne!(stale_frame, current_frame);

        let workspace = DockWorkspace::new(source_space.clone(), graph);
        let payload = DockViewportDropPayload::Item(item("target"));
        let stale_snapshot = DockViewportResolvedDropTargetSnapshot::new(
            stale_frame,
            crate::DockDropGuideMetrics::default(),
            facts_generation,
            true,
            host_position,
            None,
            DockResolvedDropTarget {
                kind: DockResolvedDropTargetKind::LeafCenter {
                    root: target_tabs,
                    target_tabs,
                },
                source: DockDropResolveSource::LeafBody,
                target_bounds: Some(bounds(0.0, 0.0, 320.0, 240.0)),
                inner_target_bounds: Some(bounds(0.0, 0.0, 320.0, 240.0)),
                availability: crate::drop_target::DockResolvedDropTargetAvailability::all(),
                drop_box: None,
                hit_bounds: Some(bounds(0.0, 0.0, 320.0, 240.0)),
                preview_bounds: Some(bounds(0.0, 0.0, 320.0, 240.0)),
                tab_insertion_bounds: None,
                edge_sizing: None,
                edge_plan: None,
                is_central_region: true,
            },
        );

        let result = validate_delivery_workspace_target(
            &adapter,
            &host_scenes,
            &workspace,
            DockNodeId::null(),
            &payload,
            &stale_snapshot,
        );

        assert_eq!(result, Err(DockActionApplyError::DropTargetUnavailable));
    }

    #[test]
    fn known_viewport_route_resolves_edge_sizing_from_request_payload_geometry() {
        let source_space = space("source");
        let target_space = space("target");
        let target_window = handle(7);
        let mut graph = DockGraph::new();
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("target")],
            selected: Some(DockItemId::from("target")),
        });
        graph.set_root(target_space.clone(), target_tabs);
        let host_position =
            geometry::drop_boxes(bounds(0.0, 0.0, 1000.0, 600.0), DockDropBoxSet::Inner)
                .into_iter()
                .find(|drop_box| {
                    drop_box.kind == DockDropBoxKind::InnerEdge(crate::DropZone::Right)
                })
                .map(|drop_box| drop_box.hit_bounds.center())
                .expect("right edge drop box should exist");
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        adapter.update_snapshot(
            &target_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 1000.0, 600.0,
            ))),
            bounds(0.0, 0.0, 1000.0, 600.0),
        );
        let facts_generation = adapter
            .snapshot_facts_generation(&target_space, target_window.window_id())
            .expect("target snapshot should have facts");

        let mut host_scenes = DockViewportHostSceneRegistry::default();
        let frame = host_scenes
            .register(current_registration_host_scene(
                &adapter,
                target_space.clone(),
                target_window.window_id(),
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(100.0, 100.0, 1000.0, 600.0)),
                bounds(0.0, 0.0, 1000.0, 600.0),
                host_position,
                crate::DockDropGuideMetrics::default(),
            ))
            .frame;
        assert!(
            host_scenes
                .push_frame_fact(
                    &frame,
                    DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                        root: target_tabs,
                        target_tabs,
                        bounds: bounds(0.0, 0.0, 1000.0, 600.0),
                        is_central: false,
                    })
                )
                .is_some()
        );

        let workspace = DockWorkspace::new(source_space.clone(), graph);
        let payload = DockViewportDropPayload::Item(DockItemId::from("a"));
        let payload_classes =
            workspace.payload_dock_classes_for_viewport_payload(&payload, DockNodeId::null());
        let request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            DockNodeId::null(),
            payload,
            point(px(970.0), px(400.0)),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
        )
        .with_tear_off_geometry(Some(
            DockDragTearOffGeometry::from_source_bounds(
                Bounds::new(point(px(0.0), px(0.0)), size(px(240.0), px(180.0))),
                point(px(12.0), px(12.0)),
            )
            .with_preferred_size(size(px(240.0), px(180.0))),
        ));

        let target = resolve_workspace_target_for_route(
            &adapter,
            &host_scenes,
            &DockViewportDropRoute::KnownViewport {
                target: crate::DockViewportTargetHit::with_facts_generation(
                    target_space,
                    target_window,
                    host_position,
                    facts_generation,
                ),
                source: crate::DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            &request,
            &workspace,
            &payload_classes,
        );

        let DockViewportWorkspaceRouteTarget::Resolved(target) = target else {
            panic!("known viewport route should resolve an edge target");
        };
        let target = target.into_target();
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::InnerEdge {
                root: target_tabs,
                target_tabs,
                zone: crate::DropZone::Right,
            }
        );
        assert_eq!(
            target.preview_bounds,
            Some(bounds(760.0, 0.0, 240.0, 600.0))
        );
        assert_eq!(
            target.edge_sizing.map(|sizing| sizing.new_child_share()),
            Some(0.24)
        );
    }

    #[test]
    fn local_event_receiver_scene_route_can_skip_current_route_facts_generation_match() {
        let source_space = space("source");
        let window = handle(7);
        let mut graph = DockGraph::new();
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        graph.set_root(source_space.clone(), tabs);

        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source_space.clone(), window);
        adapter.update_snapshot(
            &source_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        let current_facts_generation = adapter
            .snapshot_facts_generation(&source_space, window.window_id())
            .expect("source snapshot should have route facts");
        let mismatched_facts_generation = current_facts_generation + 1;

        let mut host_scenes = DockViewportHostSceneRegistry::default();
        let host_position = center_drop_position(bounds(0.0, 0.0, 320.0, 240.0));
        let frame = host_scenes
            .register(current_registration_host_scene(
                &adapter,
                source_space.clone(),
                window.window_id(),
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(100.0, 100.0, 320.0, 240.0)),
                bounds(0.0, 0.0, 320.0, 240.0),
                host_position,
                crate::DockDropGuideMetrics::default(),
            ))
            .frame;
        let leaf_fact = DockHostDropSceneFact::Leaf(DockLeafDropTarget {
            root: tabs,
            target_tabs: tabs,
            bounds: bounds(0.0, 0.0, 320.0, 240.0),
            is_central: true,
        });
        let frame = host_scenes
            .push_frame_fact(&frame, leaf_fact.clone())
            .expect("the rendered leaf must advance the current scene frame");

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel(
            item("a"),
            DockPanel::lazy("Panel A", |_| unreachable!()).with_dock_class("inspector"),
        );
        let payload = DockViewportDropPayload::Item(item("a"));
        let payload_classes =
            workspace.payload_dock_classes_for_viewport_payload(&payload, DockNodeId::null());
        let request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            DockNodeId::null(),
            payload,
            point(px(220.0), px(200.0)),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(window),
        )
        .with_event_receiver_local_scene_proof(Some(frame.clone()));

        let target = resolve_workspace_target_for_route(
            &adapter,
            &host_scenes,
            &DockViewportDropRoute::local_for_registration_test(
                adapter
                    .registration_key(&source_space)
                    .expect("registered viewport should have an exact key"),
                host_position,
                mismatched_facts_generation,
                crate::DockViewportRouteSelectionSource::EventReceiverLocalScene,
            ),
            &request,
            &workspace,
            &payload_classes,
        );

        let DockViewportWorkspaceRouteTarget::Resolved(target) = target else {
            panic!("event-receiver-local-scene route should resolve against current host scene");
        };
        assert_eq!(target.facts_generation(), None);

        let current_frame = host_scenes
            .push_frame_fact(&frame, leaf_fact)
            .expect("a later rendered fact must advance the scene generation");
        assert_ne!(frame, current_frame);
        assert!(matches!(
            resolve_workspace_target_for_route(
                &adapter,
                &host_scenes,
                &DockViewportDropRoute::local_for_registration_test(
                    adapter
                        .registration_key(&source_space)
                        .expect("registered viewport should have an exact key"),
                    host_position,
                    mismatched_facts_generation,
                    crate::DockViewportRouteSelectionSource::EventReceiverLocalScene,
                ),
                &request,
                &workspace,
                &payload_classes,
            ),
            DockViewportWorkspaceRouteTarget::RouteUnavailable
        ));
    }

    #[test]
    fn known_viewport_leaf_interior_preserves_preview_without_delivery() {
        let source_space = space("source");
        let target_space = space("target");
        let target_window = handle(10);
        let mut graph = DockGraph::new();
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("target")],
            selected: Some(item("target")),
        });
        graph.set_root(target_space.clone(), target_tabs);

        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        adapter.update_snapshot(
            &target_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                500.0, 220.0, 920.0, 672.0,
            ))),
            bounds(0.0, 0.0, 920.0, 640.0),
        );
        let facts_generation = adapter
            .snapshot_facts_generation(&target_space, target_window.window_id())
            .expect("target snapshot should have route facts");

        let host_position = point(px(754.9751), px(583.56213));
        let mut host_scenes = DockViewportHostSceneRegistry::default();
        let frame = host_scenes
            .register(current_registration_host_scene(
                &adapter,
                target_space.clone(),
                target_window.window_id(),
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(500.0, 220.0, 920.0, 672.0)),
                bounds(0.0, 0.0, 920.0, 640.0),
                host_position,
                crate::DockDropGuideMetrics::default(),
            ))
            .frame;
        assert!(
            host_scenes
                .push_frame_fact(
                    &frame,
                    DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                        root: target_tabs,
                        target_tabs,
                        bounds: bounds(222.0, 436.0, 697.0, 203.0),
                        is_central: false,
                    })
                )
                .is_some()
        );

        let workspace = DockWorkspace::new(source_space.clone(), graph);
        let payload = DockViewportDropPayload::Item(item("source"));
        let payload_classes =
            workspace.payload_dock_classes_for_viewport_payload(&payload, DockNodeId::null());
        let request = DockViewportDropRouteRequest::from_target_context(
            source_space,
            DockNodeId::null(),
            payload,
            point(px(1254.9751), px(803.56213)),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
        );
        let route = DockViewportDropRoute::KnownViewport {
            target: DockViewportTargetHit::with_facts_generation(
                target_space.clone(),
                target_window,
                host_position,
                facts_generation,
            ),
            source: crate::DockViewportRouteSelectionSource::TrustedHoveredWindow,
        };

        let target = resolve_workspace_target_for_route(
            &adapter,
            &host_scenes,
            &route,
            &request,
            &workspace,
            &payload_classes,
        );
        let DockViewportWorkspaceRouteTarget::PreviewOnly(preview_target) = target else {
            panic!("leaf interior without drop box should resolve as preview-only");
        };
        let expected_tabs = target_tabs;
        assert!(preview_target.is_preview_only());
        assert!(matches!(
            preview_target.target().kind,
            DockResolvedDropTargetKind::LeafCenter {
                root,
                target_tabs
            } if root == expected_tabs && target_tabs == expected_tabs
        ));

        let resolution = DockViewportResolvedDropRoute::from_workspace_route_target(
            &request,
            route,
            DockViewportWorkspaceRouteTarget::PreviewOnly(preview_target),
        );

        assert!(resolution.delivery().is_none());
        assert!(
            resolution
                .routed_preview_target_snapshot()
                .is_some_and(DockViewportResolvedDropTargetSnapshot::is_preview_only)
        );
        assert_eq!(
            DockDropDelivery::from_resolution(resolution),
            Err(DockActionApplyError::DropTargetUnavailable)
        );
    }

    fn resolved_drop_target_snapshot(
        target_space: DockSpaceId,
        target_window_id: WindowId,
        facts_generation: u64,
    ) -> DockViewportResolvedDropTargetSnapshot {
        let mut registry = DockViewportHostSceneRegistry::default();
        let frame = registry
            .register(DockViewportHostSceneSnapshot::new(
                target_space.clone(),
                target_window_id,
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(0.0, 0.0, 320.0, 240.0)),
                bounds(0.0, 0.0, 320.0, 240.0),
                point(px(0.0), px(0.0)),
                crate::DockDropGuideMetrics::default(),
            ))
            .frame;
        DockViewportResolvedDropTargetSnapshot::new(
            frame,
            crate::DockDropGuideMetrics::default(),
            facts_generation,
            true,
            point(px(0.0), px(0.0)),
            None,
            DockResolvedDropTarget {
                kind: DockResolvedDropTargetKind::EmptyDockSpace {
                    space: target_space,
                },
                source: DockDropResolveSource::EmptyDockSpace,
                target_bounds: Some(bounds(0.0, 0.0, 320.0, 240.0)),
                inner_target_bounds: None,
                availability: crate::drop_target::DockResolvedDropTargetAvailability::all(),
                drop_box: None,
                hit_bounds: Some(bounds(0.0, 0.0, 320.0, 240.0)),
                preview_bounds: Some(bounds(0.0, 0.0, 320.0, 240.0)),
                tab_insertion_bounds: None,
                edge_sizing: None,
                edge_plan: None,
                is_central_region: false,
            },
        )
    }
}
