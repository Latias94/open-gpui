use crate::{
    DockActionApplyError, DockDropWorkspaceTarget, DockPolicy, DockPolicyError, DockSpaceId,
    DockViewportAdapter, DockViewportDropPayload, DockViewportDropRoute,
    DockViewportDropRouteRequest, DockViewportResolvedDropTargetSnapshot, DockWorkspace,
    drop_target::{DockDropResolution, DockResolvedDropTarget, validate_resolved_drop_target},
    viewport_drop_scene::{DockViewportHostSceneFrame, DockViewportHostSceneRegistry},
    workspace_move_validation::{DockPayloadDockClasses, dock_target_validator},
};
use open_gpui::{Point, WindowId};

/// Current workspace target facts for a viewport route.
pub(crate) enum DockViewportWorkspaceRouteTarget {
    Valid(Option<DockViewportResolvedDropTargetSnapshot>),
    Unavailable,
    Rejected(DockPolicyError),
}

/// Resolves the workspace target authority for a viewport route.
pub(crate) fn resolve_workspace_target_for_route(
    host_scenes: &DockViewportHostSceneRegistry,
    route: &DockViewportDropRoute,
    request: &DockViewportDropRouteRequest,
    policy: &DockPolicy,
    payload_classes: &DockPayloadDockClasses,
) -> DockViewportWorkspaceRouteTarget {
    match route {
        DockViewportDropRoute::Local { host_position } => {
            let target_validator =
                dock_target_validator(request.source_space(), payload_classes, policy);
            let resolved = host_scenes
                .resolve_frame_for_window(
                    request.source_space(),
                    None,
                    *host_position,
                    policy,
                    Some(&target_validator),
                )
                .map(|(frame, resolution)| {
                    resolved_target_snapshot(
                        request.source_space().clone(),
                        None,
                        frame,
                        None,
                        resolution,
                    )
                });
            DockViewportWorkspaceRouteTarget::Valid(resolved.and_then(Result::ok))
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
                Ok(target) => DockViewportWorkspaceRouteTarget::Valid(Some(target)),
                Err(error) => DockViewportWorkspaceRouteTarget::Rejected(error),
            }
        }
        DockViewportDropRoute::TearOff(_)
        | DockViewportDropRoute::Unavailable
        | DockViewportDropRoute::Rejected(_) => DockViewportWorkspaceRouteTarget::Valid(None),
    }
}

/// Resolves a delivery target against current viewport and workspace facts.
pub(crate) fn resolve_delivery_workspace_target(
    adapter: &DockViewportAdapter,
    host_scenes: &DockViewportHostSceneRegistry,
    workspace: &DockWorkspace,
    source_space: &DockSpaceId,
    source_node: crate::DockNodeId,
    payload: &DockViewportDropPayload,
    target: DockDropWorkspaceTarget,
) -> Result<(DockSpaceId, DockResolvedDropTarget), DockActionApplyError> {
    match target {
        DockDropWorkspaceTarget::Resolved(target)
            if target.frame().is_current_in(host_scenes)
                && target_facts_generation_is_current(adapter, &target) =>
        {
            let target_space = target.target_space().clone();
            validate_resolved_target_snapshot(
                workspace,
                &target_space,
                target.into_target(),
                payload,
                source_node,
            )
        }
        DockDropWorkspaceTarget::Resolved(_) => Err(DockActionApplyError::DropTargetUnavailable),
        DockDropWorkspaceTarget::ResolveLocalAtDelivery { host_position } => {
            resolve_local_route_target(
                host_scenes,
                workspace,
                source_space,
                host_position,
                payload,
                source_node,
            )
        }
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
) -> Result<(DockSpaceId, DockResolvedDropTarget), DockActionApplyError> {
    let policy = workspace.policy().clone();
    let payload_classes = workspace.payload_dock_classes_for_viewport_payload(payload, source_node);
    let target_validator = dock_target_validator(target_space, &payload_classes, &policy);
    match validate_resolved_drop_target(target, &policy, Some(&target_validator)) {
        DockDropResolution::Valid(target) => Ok((target_space.clone(), target)),
        DockDropResolution::Rejected(rejection) => {
            Err(DockActionApplyError::Policy(rejection.reason))
        }
    }
}

fn resolve_local_route_target(
    host_scenes: &DockViewportHostSceneRegistry,
    workspace: &DockWorkspace,
    target_space: &DockSpaceId,
    host_position: Point<open_gpui::Pixels>,
    payload: &DockViewportDropPayload,
    source_node: crate::DockNodeId,
) -> Result<(DockSpaceId, DockResolvedDropTarget), DockActionApplyError> {
    let policy = workspace.policy().clone();
    let payload_classes = workspace.payload_dock_classes_for_viewport_payload(payload, source_node);
    let target_validator = dock_target_validator(target_space, &payload_classes, &policy);
    let Some((_, resolution)) = host_scenes.resolve_frame_for_window(
        target_space,
        None,
        host_position,
        &policy,
        Some(&target_validator),
    ) else {
        return Err(DockActionApplyError::DropTargetUnavailable);
    };
    match resolution {
        DockDropResolution::Valid(target) => Ok((target_space.clone(), target)),
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
