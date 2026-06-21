use crate::{
    DockActionApplyError, DockEdgeDockSizing, DockPolicyError, DockSpaceId, DockViewportAdapter,
    DockViewportDropPayload, DockViewportDropRoute, DockViewportDropRouteRequest,
    DockViewportResolvedDropTargetSnapshot, DockWorkspace, DropZone,
    drop_target::{DockDropResolution, DockResolvedDropTarget, validate_resolved_drop_target},
    viewport_drop_scene::{DockViewportHostSceneFrame, DockViewportHostSceneRegistry},
    workspace_move_validation::{DockPayloadDockClasses, dock_target_validator},
    workspace_transaction::DockWorkspaceResolvedDropTarget,
};
use open_gpui::{Pixels, Size, WindowId};

/// Current workspace target facts for a viewport route.
pub(crate) enum DockViewportWorkspaceRouteTarget {
    Resolved(DockViewportResolvedDropTargetSnapshot),
    Missing,
    Unavailable,
    Rejected {
        target: DockViewportResolvedDropTargetSnapshot,
        reason: DockPolicyError,
    },
    NotWorkspaceRoute,
}

/// Resolves the workspace target authority for a viewport route.
pub(crate) fn resolve_workspace_target_for_route(
    adapter: &DockViewportAdapter,
    host_scenes: &DockViewportHostSceneRegistry,
    route: &DockViewportDropRoute,
    request: &DockViewportDropRouteRequest,
    workspace: &DockWorkspace,
    payload_classes: &DockPayloadDockClasses,
) -> DockViewportWorkspaceRouteTarget {
    let policy = workspace.policy();
    match route {
        DockViewportDropRoute::Local {
            host_position,
            window_id,
            facts_generation,
            ..
        } => {
            if !current_route_window_facts_match(
                adapter,
                request.source_space(),
                *window_id,
                *facts_generation,
            ) {
                return DockViewportWorkspaceRouteTarget::Unavailable;
            }
            let target_validator =
                dock_target_validator(request.source_space(), payload_classes, policy);
            let graph = workspace.graph().clone();
            let target_space = request.source_space().clone();
            let edge_plan_resolver =
                move |target_node: crate::DockNodeId,
                      zone: DropZone,
                      sizing: DockEdgeDockSizing| {
                    graph.edge_dock_plan_with_sizing(&target_space, target_node, zone, sizing)
                };
            let payload_size = request_payload_size(request);
            let excluded_nodes = request
                .payload()
                .excluded_nodes_for_drop_scene(workspace.graph(), request.source_node());
            let resolved = host_scenes
                .resolve_frame_for_window(
                    request.source_space(),
                    Some(*window_id),
                    *host_position,
                    payload_size,
                    excluded_nodes,
                    policy,
                    Some(&target_validator),
                    Some(&edge_plan_resolver),
                )
                .map(|(frame, resolution)| {
                    resolved_target_snapshot(
                        request.source_space().clone(),
                        Some(*window_id),
                        frame,
                        Some(*facts_generation),
                        *host_position,
                        payload_size,
                        resolution,
                    )
                });
            match resolved {
                Some(DockResolvedViewportTarget::Valid(target)) => {
                    DockViewportWorkspaceRouteTarget::Resolved(target)
                }
                Some(DockResolvedViewportTarget::Rejected { target, reason }) => {
                    DockViewportWorkspaceRouteTarget::Rejected { target, reason }
                }
                None => DockViewportWorkspaceRouteTarget::Missing,
            }
        }
        DockViewportDropRoute::KnownViewport { target, .. } => {
            let facts_match = current_route_window_facts_match(
                adapter,
                target.space(),
                target.window_id(),
                target.facts_generation(),
            );
            if !facts_match {
                return DockViewportWorkspaceRouteTarget::Unavailable;
            }
            let target_validator = dock_target_validator(target.space(), payload_classes, policy);
            let graph = workspace.graph().clone();
            let target_space = target.space().clone();
            let edge_plan_resolver =
                move |target_node: crate::DockNodeId,
                      zone: DropZone,
                      sizing: DockEdgeDockSizing| {
                    graph.edge_dock_plan_with_sizing(&target_space, target_node, zone, sizing)
                };
            let payload_size = request_payload_size(request);
            let excluded_nodes = request
                .payload()
                .excluded_nodes_for_drop_scene(workspace.graph(), request.source_node());
            let resolved_frame = host_scenes.resolve_frame_for_window(
                target.space(),
                Some(target.window_id()),
                target.host_position(),
                payload_size,
                excluded_nodes,
                policy,
                Some(&target_validator),
                Some(&edge_plan_resolver),
            );
            let Some((frame, resolution)) = resolved_frame else {
                return DockViewportWorkspaceRouteTarget::Unavailable;
            };
            match resolved_target_snapshot(
                target.space().clone(),
                Some(target.window_id()),
                frame,
                Some(target.facts_generation()),
                target.host_position(),
                payload_size,
                resolution,
            ) {
                DockResolvedViewportTarget::Valid(target) => {
                    DockViewportWorkspaceRouteTarget::Resolved(target)
                }
                DockResolvedViewportTarget::Rejected { target, reason } => {
                    DockViewportWorkspaceRouteTarget::Rejected { target, reason }
                }
            }
        }
        DockViewportDropRoute::TearOff
        | DockViewportDropRoute::Unavailable
        | DockViewportDropRoute::Rejected(_) => DockViewportWorkspaceRouteTarget::NotWorkspaceRoute,
    }
}

pub(crate) fn request_payload_size(request: &DockViewportDropRouteRequest) -> Option<Size<Pixels>> {
    let geometry = request.tear_off_geometry()?;
    geometry
        .preferred_size()
        .or_else(|| Some(geometry.source_bounds().size))
}

fn current_route_window_facts(
    adapter: &DockViewportAdapter,
    space: &DockSpaceId,
) -> Option<(WindowId, u64)> {
    let window_id = adapter.window_for_space(space)?.window_id();
    let facts_generation = adapter.snapshot_facts_generation(space, window_id)?;
    Some((window_id, facts_generation))
}

fn current_route_window_facts_match(
    adapter: &DockViewportAdapter,
    space: &DockSpaceId,
    window_id: WindowId,
    facts_generation: u64,
) -> bool {
    current_route_window_facts(adapter, space) == Some((window_id, facts_generation))
}

/// Resolves a delivery target against current viewport and workspace facts.
pub(crate) fn resolve_delivery_workspace_target(
    adapter: &DockViewportAdapter,
    host_scenes: &DockViewportHostSceneRegistry,
    workspace: &DockWorkspace,
    source_node: crate::DockNodeId,
    payload: &DockViewportDropPayload,
    target: DockViewportResolvedDropTargetSnapshot,
) -> Result<DockWorkspaceResolvedDropTarget, DockActionApplyError> {
    validate_delivery_workspace_target_inner(
        adapter,
        host_scenes,
        workspace,
        source_node,
        payload,
        target,
    )
}

/// Verifies that a resolved delivery still points at current route facts and policy.
#[cfg(test)]
pub(crate) fn validate_delivery_workspace_target(
    adapter: &DockViewportAdapter,
    host_scenes: &DockViewportHostSceneRegistry,
    workspace: &DockWorkspace,
    source_node: crate::DockNodeId,
    payload: &DockViewportDropPayload,
    target: &DockViewportResolvedDropTargetSnapshot,
) -> Result<(), DockActionApplyError> {
    validate_delivery_workspace_target_inner(
        adapter,
        host_scenes,
        workspace,
        source_node,
        payload,
        target.clone(),
    )
    .map(|_| ())
}

fn validate_delivery_workspace_target_inner(
    adapter: &DockViewportAdapter,
    host_scenes: &DockViewportHostSceneRegistry,
    workspace: &DockWorkspace,
    source_node: crate::DockNodeId,
    payload: &DockViewportDropPayload,
    target: DockViewportResolvedDropTargetSnapshot,
) -> Result<DockWorkspaceResolvedDropTarget, DockActionApplyError> {
    let facts_current = target_facts_generation_is_current(adapter, &target);
    if !facts_current {
        return Err(DockActionApplyError::DropTargetUnavailable);
    }
    if !current_resolved_target_key_matches_snapshot(
        host_scenes,
        workspace,
        source_node,
        payload,
        &target,
    ) {
        return Err(DockActionApplyError::DropTargetUnavailable);
    }
    let target_space = target.target_space().clone();
    validate_resolved_target_snapshot(
        workspace,
        &target_space,
        target.into_target(),
        payload,
        source_node,
    )
}

fn current_resolved_target_key_matches_snapshot(
    host_scenes: &DockViewportHostSceneRegistry,
    workspace: &DockWorkspace,
    source_node: crate::DockNodeId,
    payload: &DockViewportDropPayload,
    target: &DockViewportResolvedDropTargetSnapshot,
) -> bool {
    let policy = workspace.policy();
    let payload_classes = workspace.payload_dock_classes_for_viewport_payload(payload, source_node);
    let target_validator = dock_target_validator(target.target_space(), &payload_classes, policy);
    let graph = workspace.graph().clone();
    let target_space = target.target_space().clone();
    let edge_plan_resolver =
        move |target_node: crate::DockNodeId, zone: DropZone, sizing: DockEdgeDockSizing| {
            graph.edge_dock_plan_with_sizing(&target_space, target_node, zone, sizing)
        };
    let excluded_nodes = payload.excluded_nodes_for_drop_scene(workspace.graph(), source_node);
    let Some((_, resolution)) = host_scenes.resolve_frame_for_window(
        target.target_space(),
        target.target_window_id(),
        target.host_position(),
        target.payload_size(),
        excluded_nodes,
        policy,
        Some(&target_validator),
        Some(&edge_plan_resolver),
    ) else {
        return false;
    };
    match resolution {
        DockDropResolution::Valid(current) => current.target_key() == *target.target_key(),
        DockDropResolution::Rejected(rejection) => {
            rejection.target.target_key() == *target.target_key()
        }
    }
}

fn resolved_target_snapshot(
    target_space: DockSpaceId,
    target_window_id: Option<WindowId>,
    frame: DockViewportHostSceneFrame,
    facts_generation: Option<u64>,
    host_position: open_gpui::Point<Pixels>,
    payload_size: Option<Size<Pixels>>,
    resolution: DockDropResolution,
) -> DockResolvedViewportTarget {
    match resolution {
        DockDropResolution::Valid(target) => {
            DockResolvedViewportTarget::Valid(DockViewportResolvedDropTargetSnapshot::new(
                target_space,
                target_window_id,
                frame,
                facts_generation,
                host_position,
                payload_size,
                target,
            ))
        }
        DockDropResolution::Rejected(rejection) => DockResolvedViewportTarget::Rejected {
            target: DockViewportResolvedDropTargetSnapshot::new(
                target_space,
                target_window_id,
                frame,
                facts_generation,
                host_position,
                payload_size,
                rejection.target,
            ),
            reason: rejection.reason,
        },
    }
}

enum DockResolvedViewportTarget {
    Valid(DockViewportResolvedDropTargetSnapshot),
    Rejected {
        target: DockViewportResolvedDropTargetSnapshot,
        reason: DockPolicyError,
    },
}

fn validate_resolved_target_snapshot(
    workspace: &DockWorkspace,
    target_space: &DockSpaceId,
    target: DockResolvedDropTarget,
    payload: &DockViewportDropPayload,
    source_node: crate::DockNodeId,
) -> Result<DockWorkspaceResolvedDropTarget, DockActionApplyError> {
    let policy = workspace.policy().clone();
    let payload_classes = workspace.payload_dock_classes_for_viewport_payload(payload, source_node);
    let target_validator = dock_target_validator(target_space, &payload_classes, &policy);
    match validate_resolved_drop_target(target, &policy, Some(&target_validator)) {
        DockDropResolution::Valid(target) => Ok(DockWorkspaceResolvedDropTarget::new(
            target_space.clone(),
            target,
        )),
        DockDropResolution::Rejected(rejection) => {
            Err(DockActionApplyError::Policy(rejection.reason))
        }
    }
}

fn target_facts_generation_is_current(
    adapter: &DockViewportAdapter,
    target: &DockViewportResolvedDropTargetSnapshot,
) -> bool {
    let (Some(window_id), Some(facts_generation)) =
        (target.target_window_id(), target.facts_generation())
    else {
        return true;
    };
    adapter.snapshot_facts_generation(target.target_space(), window_id) == Some(facts_generation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockClassId, DockGraph, DockItemId, DockNode, DockNodeId, DockPanel,
        DockViewportTargetContext, DockViewportWindowFacts,
        drag::DockDragTearOffGeometry,
        drop_runtime::DockHostDropSceneFact,
        drop_target::{
            DockDropResolveSource, DockEmptySpaceDropTarget, DockLeafDropTarget,
            DockResolvedDropTarget, DockResolvedDropTargetKind,
        },
        geometry::{self, DockDropBoxKind, DockDropBoxSet},
        host_test_support::center_drop_position,
        viewport_drop_scene::DockViewportHostSceneSnapshot,
        viewport_registry::DockViewportWindowBoundsFrame,
        viewport_test_support::{bounds, handle, item, register_viewport, space},
    };
    use open_gpui::{Bounds, WindowBounds, point, px, size};
    use slotmap::Key;

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
                crate::DockDropGuideStyle::default(),
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
            source_space,
            DockNodeId::null(),
            payload,
            point(px(124.0), px(124.0)),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(new_window),
        );

        let target = resolve_workspace_target_for_route(
            &adapter,
            &host_scenes,
            &DockViewportDropRoute::Local {
                host_position: point(px(24.0), px(24.0)),
                window_id: old_window.window_id(),
                facts_generation: 1,
                authority: crate::DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
            },
            &request,
            &workspace,
            &payload_classes,
        );

        assert!(
            matches!(target, DockViewportWorkspaceRouteTarget::Unavailable),
            "local route must not replace its frozen source window with the current source mapping"
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
            .register(DockViewportHostSceneSnapshot::new(
                source_space.clone(),
                window.window_id(),
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(100.0, 100.0, 360.0, 240.0)),
                bounds(0.0, 0.0, 360.0, 240.0),
                host_position,
                crate::DockDropGuideStyle::default(),
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
            &DockViewportDropRoute::Local {
                host_position,
                window_id: window.window_id(),
                facts_generation,
                authority: crate::DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
            },
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
            .register(DockViewportHostSceneSnapshot::new(
                source_space.clone(),
                window.window_id(),
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(100.0, 100.0, 320.0, 240.0)),
                bounds(0.0, 0.0, 320.0, 240.0),
                host_position,
                crate::DockDropGuideStyle::default(),
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
            &DockViewportDropRoute::Local {
                host_position,
                window_id: window.window_id(),
                facts_generation,
                authority: crate::DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
            },
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
            DockViewportWorkspaceRouteTarget::Missing => {
                panic!("local route should preserve rejected target instead of missing it")
            }
            DockViewportWorkspaceRouteTarget::Unavailable => {
                panic!("local route should not be unavailable when the current facts match")
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
            .register(DockViewportHostSceneSnapshot::new(
                target_space.clone(),
                target_window.window_id(),
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(100.0, 100.0, 320.0, 240.0)),
                bounds(0.0, 0.0, 320.0, 240.0),
                point(px(24.0), px(24.0)),
                crate::DockDropGuideStyle::default(),
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
                authority: crate::DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
            },
            &request,
            &workspace,
            &payload_classes,
        );

        assert!(
            matches!(target, DockViewportWorkspaceRouteTarget::Unavailable),
            "known viewport route must not resolve preview targets from stale window facts"
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
            .register(DockViewportHostSceneSnapshot::new(
                target_space.clone(),
                target_window.window_id(),
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(100.0, 100.0, 320.0, 240.0)),
                bounds(0.0, 0.0, 320.0, 240.0),
                host_position,
                crate::DockDropGuideStyle::default(),
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
            target_space.clone(),
            Some(target_window.window_id()),
            frame,
            Some(facts_generation),
            host_position,
            None,
            DockResolvedDropTarget {
                kind: DockResolvedDropTargetKind::LeafCenter {
                    root: stale_tabs,
                    target_tabs: stale_tabs,
                },
                source: DockDropResolveSource::LeafBody,
                drop_box: None,
                preview_bounds: Some(bounds(0.0, 0.0, 320.0, 240.0)),
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
            .register(DockViewportHostSceneSnapshot::new(
                target_space.clone(),
                target_window.window_id(),
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(100.0, 100.0, 1000.0, 600.0)),
                bounds(0.0, 0.0, 1000.0, 600.0),
                host_position,
                crate::DockDropGuideStyle::default(),
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
                authority: crate::DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
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
}
