use crate::{
    DockActionApplyError, DockEdgeDockSizing, DockPolicyError, DockSpaceId, DockViewportAdapter,
    DockViewportDropPayload, DockViewportDropRoute, DockViewportDropRouteRequest,
    DockViewportRouteSelectionSource, DockWorkspace, DropZone,
    drop_target::{DockDropResolution, DockResolvedDropTarget, validate_resolved_drop_target},
    viewport_drop_scene::{
        DockViewportFrameResolution, DockViewportHostSceneFrame, DockViewportHostSceneRegistry,
    },
    workspace_drop_target::DockWorkspaceResolvedDropTarget,
    workspace_move_validation::{DockPayloadDockClasses, dock_target_validator},
};
use open_gpui::{Pixels, Point, Size, WindowId};

use super::model::{DockViewportResolvedDropTargetSnapshot, DockViewportWorkspaceRouteTarget};

#[derive(Debug, Clone, Copy)]
enum DockMissingHostTargetBehavior {
    PreserveRoute,
    MarkRouteUnavailable,
}

impl DockMissingHostTargetBehavior {
    fn into_route_target(self) -> DockViewportWorkspaceRouteTarget {
        match self {
            Self::PreserveRoute => DockViewportWorkspaceRouteTarget::NoCurrentHostTarget,
            Self::MarkRouteUnavailable => DockViewportWorkspaceRouteTarget::RouteUnavailable,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum DockViewportRouteFactsSource {
    CurrentRouteFacts,
    EventReceiverLocalScene,
}

impl DockViewportRouteFactsSource {
    fn for_local_route_selection_source(source: DockViewportRouteSelectionSource) -> Self {
        match source {
            DockViewportRouteSelectionSource::EventReceiverLocalScene => {
                Self::EventReceiverLocalScene
            }
            DockViewportRouteSelectionSource::TrustedHoveredWindow
            | DockViewportRouteSelectionSource::FrontToBackWindowStackFallback
            | DockViewportRouteSelectionSource::FocusStampWindowStackFallback
            | DockViewportRouteSelectionSource::DragLastHoveredViewportFallback => {
                Self::CurrentRouteFacts
            }
        }
    }

    fn requires_current_route_facts(self) -> bool {
        matches!(self, Self::CurrentRouteFacts)
    }

    fn facts_generation_for_snapshot(self, facts_generation: u64) -> Option<u64> {
        self.requires_current_route_facts()
            .then_some(facts_generation)
    }
}

#[derive(Clone, Copy)]
struct DockExistingViewportRouteTarget<'a> {
    space: &'a DockSpaceId,
    window_id: WindowId,
    host_position: Point<Pixels>,
    facts_generation: u64,
    missing_host_target: DockMissingHostTargetBehavior,
    route_facts_source: DockViewportRouteFactsSource,
}

/// Resolves the workspace target selected by a viewport route.
pub(crate) fn resolve_workspace_target_for_route(
    adapter: &DockViewportAdapter,
    host_scenes: &DockViewportHostSceneRegistry,
    route: &DockViewportDropRoute,
    request: &DockViewportDropRouteRequest,
    workspace: &DockWorkspace,
    payload_classes: &DockPayloadDockClasses,
) -> DockViewportWorkspaceRouteTarget {
    match route {
        DockViewportDropRoute::Local {
            host_position,
            window_id,
            facts_generation,
            source,
            ..
        } => resolve_existing_viewport_workspace_target(
            adapter,
            host_scenes,
            request,
            workspace,
            payload_classes,
            DockExistingViewportRouteTarget {
                space: request.source_space(),
                window_id: *window_id,
                host_position: *host_position,
                facts_generation: *facts_generation,
                missing_host_target: DockMissingHostTargetBehavior::PreserveRoute,
                route_facts_source: DockViewportRouteFactsSource::for_local_route_selection_source(
                    *source,
                ),
            },
        ),
        DockViewportDropRoute::KnownViewport { target, .. } => {
            resolve_existing_viewport_workspace_target(
                adapter,
                host_scenes,
                request,
                workspace,
                payload_classes,
                DockExistingViewportRouteTarget {
                    space: target.space(),
                    window_id: target.window_id(),
                    host_position: target.host_position(),
                    facts_generation: target.facts_generation(),
                    missing_host_target: DockMissingHostTargetBehavior::MarkRouteUnavailable,
                    route_facts_source: DockViewportRouteFactsSource::CurrentRouteFacts,
                },
            )
        }
        DockViewportDropRoute::TearOff
        | DockViewportDropRoute::Unavailable
        | DockViewportDropRoute::Rejected(_) => DockViewportWorkspaceRouteTarget::NotWorkspaceRoute,
    }
}

fn resolve_existing_viewport_workspace_target(
    adapter: &DockViewportAdapter,
    host_scenes: &DockViewportHostSceneRegistry,
    request: &DockViewportDropRouteRequest,
    workspace: &DockWorkspace,
    payload_classes: &DockPayloadDockClasses,
    target: DockExistingViewportRouteTarget<'_>,
) -> DockViewportWorkspaceRouteTarget {
    let current_facts_generation =
        adapter.snapshot_facts_generation(target.space, target.window_id);
    if target.route_facts_source.requires_current_route_facts()
        && current_facts_generation != Some(target.facts_generation)
    {
        return DockViewportWorkspaceRouteTarget::RouteUnavailable;
    }

    let policy = workspace.policy();
    let target_validator = dock_target_validator(target.space, payload_classes, policy);
    let graph = workspace.graph().clone();
    let target_space = target.space.clone();
    let edge_plan_resolver =
        move |target_node: crate::DockNodeId, zone: DropZone, sizing: DockEdgeDockSizing| {
            graph.edge_dock_plan_with_sizing(&target_space, target_node, zone, sizing)
        };
    let payload_size = request_payload_size(request);
    let excluded_nodes = request
        .payload()
        .excluded_nodes_for_drop_scene(workspace.graph(), request.source_node());
    let Some(resolved_frame) = host_scenes.resolve_frame_for_window(
        target.space,
        Some(target.window_id),
        target.host_position,
        payload_size,
        excluded_nodes,
        policy,
        Some(&target_validator),
        Some(&edge_plan_resolver),
    ) else {
        return target.missing_host_target.into_route_target();
    };

    let facts_generation = target
        .route_facts_source
        .facts_generation_for_snapshot(target.facts_generation);
    match resolved_frame.resolution {
        DockViewportFrameResolution::Drop(resolution) => match resolved_target_snapshot(
            target.space.clone(),
            Some(target.window_id),
            resolved_frame.frame,
            resolved_frame.drop_guide_style,
            facts_generation,
            target.host_position,
            payload_size,
            resolution,
        ) {
            DockResolvedViewportTarget::Valid(target) => {
                DockViewportWorkspaceRouteTarget::Resolved(target)
            }
            DockResolvedViewportTarget::Rejected { target, reason } => {
                DockViewportWorkspaceRouteTarget::Rejected { target, reason }
            }
        },
        DockViewportFrameResolution::GuideOnly(guide_target) => {
            DockViewportWorkspaceRouteTarget::PreviewOnly(
                DockViewportResolvedDropTargetSnapshot::new_preview_only(
                    target.space.clone(),
                    Some(target.window_id),
                    resolved_frame.frame,
                    resolved_frame.drop_guide_style,
                    facts_generation,
                    target.host_position,
                    payload_size,
                    guide_target,
                ),
            )
        }
    }
}

fn request_payload_size(request: &DockViewportDropRouteRequest) -> Option<Size<Pixels>> {
    let geometry = request.tear_off_geometry()?;
    geometry
        .preferred_size()
        .or_else(|| Some(geometry.source_bounds().size))
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

pub(super) fn validate_delivery_workspace_target_inner(
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
    let Some(resolved_frame) = host_scenes.resolve_frame_for_window(
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
    if &resolved_frame.frame != target.frame() {
        return false;
    }
    match resolved_frame.resolution {
        DockViewportFrameResolution::Drop(DockDropResolution::Valid(current)) => {
            current.target_key() == *target.target_key()
        }
        DockViewportFrameResolution::Drop(DockDropResolution::Rejected(rejection)) => {
            rejection.target.target_key() == *target.target_key()
        }
        DockViewportFrameResolution::GuideOnly(_) => false,
    }
}

fn resolved_target_snapshot(
    target_space: DockSpaceId,
    target_window_id: Option<WindowId>,
    frame: DockViewportHostSceneFrame,
    drop_guide_style: crate::DockDropGuideStyle,
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
                drop_guide_style,
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
                drop_guide_style,
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
