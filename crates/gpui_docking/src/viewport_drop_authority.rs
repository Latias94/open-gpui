use crate::{
    DockActionApplyError, DockPolicy, DockPolicyError, DockSpaceId, DockViewportAdapter,
    DockViewportDropPayload, DockViewportDropRoute, DockViewportDropRouteRequest,
    DockViewportResolvedDropTargetSnapshot, DockWorkspace,
    drop_target::{DockDropResolution, DockResolvedDropTarget, validate_resolved_drop_target},
    viewport_drop_scene::{DockViewportHostSceneFrame, DockViewportHostSceneRegistry},
    workspace_move_validation::{DockPayloadDockClasses, dock_target_validator},
    workspace_transaction::DockWorkspaceResolvedDropTarget,
};
use open_gpui::WindowId;

/// Current workspace target facts for a viewport route.
pub(crate) enum DockViewportWorkspaceRouteTarget {
    Resolved(DockViewportResolvedDropTargetSnapshot),
    Missing,
    Unavailable,
    Rejected(DockPolicyError),
    NotWorkspaceRoute,
}

/// Resolves the workspace target authority for a viewport route.
pub(crate) fn resolve_workspace_target_for_route(
    adapter: &DockViewportAdapter,
    host_scenes: &DockViewportHostSceneRegistry,
    route: &DockViewportDropRoute,
    request: &DockViewportDropRouteRequest,
    policy: &DockPolicy,
    payload_classes: &DockPayloadDockClasses,
) -> DockViewportWorkspaceRouteTarget {
    match route {
        DockViewportDropRoute::Local { host_position } => {
            let Some((window_id, facts_generation)) =
                current_route_window_facts(adapter, request.source_space())
            else {
                return DockViewportWorkspaceRouteTarget::Unavailable;
            };
            let target_validator =
                dock_target_validator(request.source_space(), payload_classes, policy);
            let resolved = host_scenes
                .resolve_frame_for_window(
                    request.source_space(),
                    Some(window_id),
                    *host_position,
                    policy,
                    Some(&target_validator),
                )
                .map(|(frame, resolution)| {
                    resolved_target_snapshot(
                        request.source_space().clone(),
                        Some(window_id),
                        frame,
                        Some(facts_generation),
                        resolution,
                    )
                });
            match resolved {
                Some(Ok(target)) => DockViewportWorkspaceRouteTarget::Resolved(target),
                Some(Err(_)) | None => DockViewportWorkspaceRouteTarget::Missing,
            }
        }
        DockViewportDropRoute::KnownViewport { target } => {
            let target_validator = dock_target_validator(target.space(), payload_classes, policy);
            let Some((frame, resolution)) = host_scenes.resolve_frame_for_window(
                target.space(),
                Some(target.window_id()),
                target.host_position(),
                policy,
                Some(&target_validator),
            ) else {
                return DockViewportWorkspaceRouteTarget::Unavailable;
            };
            match resolved_target_snapshot(
                target.space().clone(),
                Some(target.window_id()),
                frame,
                Some(target.facts_generation()),
                resolution,
            ) {
                Ok(target) => DockViewportWorkspaceRouteTarget::Resolved(target),
                Err(error) => DockViewportWorkspaceRouteTarget::Rejected(error),
            }
        }
        DockViewportDropRoute::TearOff
        | DockViewportDropRoute::Unavailable
        | DockViewportDropRoute::Rejected(_) => DockViewportWorkspaceRouteTarget::NotWorkspaceRoute,
    }
}

fn current_route_window_facts(
    adapter: &DockViewportAdapter,
    space: &DockSpaceId,
) -> Option<(WindowId, u64)> {
    let window_id = adapter.window_for_space(space)?.window_id();
    let facts_generation = adapter.snapshot_facts_generation(space, window_id)?;
    Some((window_id, facts_generation))
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
    if target.frame().is_current_in(host_scenes)
        && target_facts_generation_is_current(adapter, &target)
    {
        let target_space = target.target_space().clone();
        validate_resolved_target_snapshot(
            workspace,
            &target_space,
            target.into_target(),
            payload,
            source_node,
        )
    } else {
        Err(DockActionApplyError::DropTargetUnavailable)
    }
}

fn resolved_target_snapshot(
    target_space: DockSpaceId,
    target_window_id: Option<WindowId>,
    frame: DockViewportHostSceneFrame,
    facts_generation: Option<u64>,
    resolution: DockDropResolution,
) -> Result<DockViewportResolvedDropTargetSnapshot, DockPolicyError> {
    match resolution {
        DockDropResolution::Valid(target) => Ok(DockViewportResolvedDropTargetSnapshot::new(
            target_space,
            target_window_id,
            frame,
            facts_generation,
            target,
        )),
        DockDropResolution::Rejected(rejection) => Err(rejection.reason),
    }
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
        DockGraph, DockNodeId, DockViewportTargetContext, DockViewportWindowFacts,
        drop_runtime::DockHostDropSceneFact,
        drop_target::DockEmptySpaceDropTarget,
        viewport_drop_scene::DockViewportHostSceneSnapshot,
        viewport_test_support::{bounds, handle, item, space},
    };
    use open_gpui::{WindowBounds, point, px};
    use slotmap::Key;

    #[test]
    fn local_route_requires_current_window_host_scene_identity() {
        let source_space = space("source");
        let old_window = handle(1);
        let new_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        adapter.register_viewport(source_space.clone(), new_window);
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
                bounds(100.0, 100.0, 320.0, 240.0),
                bounds(0.0, 0.0, 320.0, 240.0),
                point(px(24.0), px(24.0)),
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
            DockViewportTargetContext::new().with_hovered_window(new_window),
        );

        let target = resolve_workspace_target_for_route(
            &adapter,
            &host_scenes,
            &DockViewportDropRoute::Local {
                host_position: point(px(24.0), px(24.0)),
            },
            &request,
            workspace.policy(),
            &payload_classes,
        );

        assert!(
            matches!(target, DockViewportWorkspaceRouteTarget::Missing),
            "local route must not wrap a stale host scene from another window as the current window"
        );
    }
}
