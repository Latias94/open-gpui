use crate::{
    DockActionApplyError, DockEdgeDockSizing, DockGraph, DockPolicy, DockPolicyError, DockSpaceId,
    DockViewportAdapter, DockViewportDropPayload, DockViewportDropRoute,
    DockViewportDropRouteRequest, DockViewportRouteProof, DockWorkspace, DropZone,
    drop_target::{DockDropResolution, DockResolvedDropTarget, validate_resolved_drop_target},
    viewport_drop_scene::{
        DockViewportFrameResolution, DockViewportHostSceneFrame, DockViewportHostSceneRegistry,
    },
    workspace_drop_target::DockWorkspaceResolvedDropTarget,
    workspace_move_validation::{DockPayloadDockClasses, dock_target_validator},
};
use open_gpui::{Pixels, Point, Size};

use super::model::{DockViewportResolvedDropTargetSnapshot, DockViewportWorkspaceRouteTarget};

/// Immutable controller facts needed to resolve one viewport route.
///
/// The runtime samples these facts before it mutably borrows its own state, so route resolution
/// cannot hold a runtime borrow across an entity read.
#[derive(Debug, Clone)]
pub(crate) struct DockViewportWorkspaceRouteFacts {
    policy: DockPolicy,
    graph: DockGraph,
    payload_classes: DockPayloadDockClasses,
}

impl DockViewportWorkspaceRouteFacts {
    pub(crate) fn capture(
        workspace: &DockWorkspace,
        request: &DockViewportDropRouteRequest,
    ) -> Self {
        Self {
            policy: workspace.policy().clone(),
            graph: workspace.graph().clone(),
            payload_classes: workspace.payload_dock_classes_for_viewport_payload(
                request.payload(),
                request.source_node(),
            ),
        }
    }

    pub(crate) fn capture_for_payload(
        workspace: &DockWorkspace,
        payload: &DockViewportDropPayload,
        source_node: crate::DockNodeId,
    ) -> Self {
        Self {
            policy: workspace.policy().clone(),
            graph: workspace.graph().clone(),
            payload_classes: workspace
                .payload_dock_classes_for_viewport_payload(payload, source_node),
        }
    }

    pub(crate) fn policy(&self) -> &DockPolicy {
        &self.policy
    }
}

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

#[derive(Clone, Copy)]
struct DockExistingViewportRouteTarget<'a> {
    route_proof: &'a DockViewportRouteProof,
    host_position: Point<Pixels>,
    missing_host_target: DockMissingHostTargetBehavior,
    requires_current_route_facts: bool,
    requires_exact_scene_frame: bool,
    expected_scene_frame: Option<&'a DockViewportHostSceneFrame>,
}

/// Resolves the workspace target selected by a viewport route.
#[cfg(test)]
pub(crate) fn resolve_workspace_target_for_route(
    adapter: &DockViewportAdapter,
    host_scenes: &DockViewportHostSceneRegistry,
    route: &DockViewportDropRoute,
    request: &DockViewportDropRouteRequest,
    workspace: &DockWorkspace,
    payload_classes: &DockPayloadDockClasses,
) -> DockViewportWorkspaceRouteTarget {
    let facts = DockViewportWorkspaceRouteFacts {
        policy: workspace.policy().clone(),
        graph: workspace.graph().clone(),
        payload_classes: payload_classes.clone(),
    };
    resolve_workspace_target_for_route_with_facts(adapter, host_scenes, route, request, &facts)
}

/// Resolves a workspace target using controller facts sampled before a runtime mutation.
pub(crate) fn resolve_workspace_target_for_route_with_facts(
    adapter: &DockViewportAdapter,
    host_scenes: &DockViewportHostSceneRegistry,
    route: &DockViewportDropRoute,
    request: &DockViewportDropRouteRequest,
    facts: &DockViewportWorkspaceRouteFacts,
) -> DockViewportWorkspaceRouteTarget {
    match route {
        DockViewportDropRoute::Local {
            host_position,
            route_proof,
            source,
            ..
        } => {
            if route_proof.space() != request.source_space() {
                return DockViewportWorkspaceRouteTarget::RouteUnavailable;
            }
            resolve_existing_viewport_workspace_target(
                adapter,
                host_scenes,
                request,
                facts,
                DockExistingViewportRouteTarget {
                    route_proof,
                    host_position: *host_position,
                    missing_host_target: DockMissingHostTargetBehavior::PreserveRoute,
                    requires_current_route_facts: source.requires_current_route_facts(),
                    requires_exact_scene_frame: *source
                        == crate::DockViewportRouteSelectionSource::EventReceiverLocalScene,
                    expected_scene_frame: request.event_receiver_local_scene_proof(),
                },
            )
        }
        DockViewportDropRoute::KnownViewport { target, .. } => {
            resolve_existing_viewport_workspace_target(
                adapter,
                host_scenes,
                request,
                facts,
                DockExistingViewportRouteTarget {
                    route_proof: target.route_proof(),
                    host_position: target.host_position(),
                    missing_host_target: DockMissingHostTargetBehavior::MarkRouteUnavailable,
                    requires_current_route_facts: true,
                    requires_exact_scene_frame: false,
                    expected_scene_frame: None,
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
    facts: &DockViewportWorkspaceRouteFacts,
    target: DockExistingViewportRouteTarget<'_>,
) -> DockViewportWorkspaceRouteTarget {
    if !adapter.is_current_registration(target.route_proof.registration_key()) {
        return DockViewportWorkspaceRouteTarget::RouteUnavailable;
    }
    let target_space = target.route_proof.space();
    let target_window_id = target.route_proof.window_id();
    let current_facts_generation =
        adapter.snapshot_facts_generation(target_space, target_window_id);
    if target.requires_current_route_facts
        && current_facts_generation != Some(target.route_proof.facts_generation())
    {
        return DockViewportWorkspaceRouteTarget::RouteUnavailable;
    }
    if target.requires_exact_scene_frame && target.expected_scene_frame.is_none() {
        return DockViewportWorkspaceRouteTarget::RouteUnavailable;
    }

    let policy = &facts.policy;
    let target_validator = dock_target_validator(target_space, &facts.payload_classes, policy);
    let graph = facts.graph.clone();
    let target_space_for_edge_plan = target_space.clone();
    let edge_plan_resolver =
        move |target_node: crate::DockNodeId, zone: DropZone, sizing: DockEdgeDockSizing| {
            graph.edge_dock_plan_with_sizing(&target_space_for_edge_plan, target_node, zone, sizing)
        };
    let payload_size = request_payload_size(request);
    let excluded_nodes = request
        .payload()
        .excluded_nodes_for_drop_scene(&facts.graph, request.source_node());
    let Some(resolved_frame) = host_scenes.resolve_frame_for_window(
        target_space,
        Some(target_window_id),
        target.host_position,
        payload_size,
        excluded_nodes,
        policy,
        Some(&target_validator),
        Some(&edge_plan_resolver),
    ) else {
        return target.missing_host_target.into_route_target();
    };
    if target.requires_exact_scene_frame
        && target.expected_scene_frame != Some(&resolved_frame.frame)
    {
        return DockViewportWorkspaceRouteTarget::RouteUnavailable;
    }
    if resolved_frame.frame.registration_key() != target.route_proof.registration_key() {
        return DockViewportWorkspaceRouteTarget::RouteUnavailable;
    }

    let facts_generation = target.route_proof.facts_generation();
    let requires_current_route_facts = target.requires_current_route_facts;
    match resolved_frame.resolution {
        DockViewportFrameResolution::Drop(resolution) => match resolved_target_snapshot(
            resolved_frame.frame,
            resolved_frame.drop_guide_metrics,
            facts_generation,
            requires_current_route_facts,
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
                    resolved_frame.frame,
                    resolved_frame.drop_guide_metrics,
                    facts_generation,
                    requires_current_route_facts,
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

pub(crate) fn resolve_delivery_workspace_target_with_facts(
    adapter: &DockViewportAdapter,
    host_scenes: &DockViewportHostSceneRegistry,
    source_node: crate::DockNodeId,
    payload: &DockViewportDropPayload,
    target: DockViewportResolvedDropTargetSnapshot,
    facts: &DockViewportWorkspaceRouteFacts,
) -> Result<DockWorkspaceResolvedDropTarget, DockActionApplyError> {
    validate_delivery_workspace_target_inner(
        adapter,
        host_scenes,
        source_node,
        payload,
        target,
        facts,
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
    let facts =
        DockViewportWorkspaceRouteFacts::capture_for_payload(workspace, payload, source_node);
    validate_delivery_workspace_target_inner(
        adapter,
        host_scenes,
        source_node,
        payload,
        target.clone(),
        &facts,
    )
    .map(|_| ())
}

pub(super) fn validate_delivery_workspace_target_inner(
    adapter: &DockViewportAdapter,
    host_scenes: &DockViewportHostSceneRegistry,
    source_node: crate::DockNodeId,
    payload: &DockViewportDropPayload,
    target: DockViewportResolvedDropTargetSnapshot,
    facts: &DockViewportWorkspaceRouteFacts,
) -> Result<DockWorkspaceResolvedDropTarget, DockActionApplyError> {
    let facts_current = target_facts_generation_is_current(adapter, &target);
    if !facts_current {
        return Err(DockActionApplyError::DropTargetUnavailable);
    }
    if !current_resolved_target_key_matches_snapshot(
        host_scenes,
        source_node,
        payload,
        &target,
        facts,
    ) {
        return Err(DockActionApplyError::DropTargetUnavailable);
    }
    let target_space = target.target_space().clone();
    validate_resolved_target_snapshot(&target_space, target.into_target(), facts)
}

fn current_resolved_target_key_matches_snapshot(
    host_scenes: &DockViewportHostSceneRegistry,
    source_node: crate::DockNodeId,
    payload: &DockViewportDropPayload,
    target: &DockViewportResolvedDropTargetSnapshot,
    facts: &DockViewportWorkspaceRouteFacts,
) -> bool {
    let policy = &facts.policy;
    let target_validator =
        dock_target_validator(target.target_space(), &facts.payload_classes, policy);
    let graph = facts.graph.clone();
    let target_space = target.target_space().clone();
    let edge_plan_resolver =
        move |target_node: crate::DockNodeId, zone: DropZone, sizing: DockEdgeDockSizing| {
            graph.edge_dock_plan_with_sizing(&target_space, target_node, zone, sizing)
        };
    let excluded_nodes = payload.excluded_nodes_for_drop_scene(&facts.graph, source_node);
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
    frame: DockViewportHostSceneFrame,
    drop_guide_metrics: crate::DockDropGuideMetrics,
    facts_generation: u64,
    requires_current_route_facts: bool,
    host_position: open_gpui::Point<Pixels>,
    payload_size: Option<Size<Pixels>>,
    resolution: DockDropResolution,
) -> DockResolvedViewportTarget {
    match resolution {
        DockDropResolution::Valid(target) => {
            DockResolvedViewportTarget::Valid(DockViewportResolvedDropTargetSnapshot::new(
                frame,
                drop_guide_metrics,
                facts_generation,
                requires_current_route_facts,
                host_position,
                payload_size,
                target,
            ))
        }
        DockDropResolution::Rejected(rejection) => DockResolvedViewportTarget::Rejected {
            target: DockViewportResolvedDropTargetSnapshot::new(
                frame,
                drop_guide_metrics,
                facts_generation,
                requires_current_route_facts,
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
    target_space: &DockSpaceId,
    target: DockResolvedDropTarget,
    facts: &DockViewportWorkspaceRouteFacts,
) -> Result<DockWorkspaceResolvedDropTarget, DockActionApplyError> {
    let target_validator =
        dock_target_validator(target_space, &facts.payload_classes, &facts.policy);
    match validate_resolved_drop_target(target, &facts.policy, Some(&target_validator)) {
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
    if !adapter.is_current_registration(target.route_proof().registration_key()) {
        return false;
    }
    let (Some(window_id), Some(facts_generation)) =
        (target.target_window_id(), target.facts_generation())
    else {
        return true;
    };
    adapter.snapshot_facts_generation(target.target_space(), window_id) == Some(facts_generation)
}
