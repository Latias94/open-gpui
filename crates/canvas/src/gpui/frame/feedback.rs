use super::*;
use crate::record_scope::CanvasResolvedSelectionScope;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CanvasPaintInteractionFrame {
    pub selection_bounds: Option<Bounds<Pixels>>,
    pub structural_selection_bounds: Option<Bounds<Pixels>>,
    pub connection_preview: Option<CanvasPaintConnectionPreview>,
    pub reconnect_handles: Vec<CanvasPaintReconnectHandle>,
    pub transform_handles: Vec<CanvasPaintTransformHandle>,
    pub snap_guides: Vec<CanvasPaintSnapGuide>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintConnectionPreview {
    pub source_view_position: Point<Pixels>,
    pub target_view_position: Point<Pixels>,
    pub edge_geometry: CanvasPaintEdgeGeometry,
    pub route_kind: CanvasEdgeRouteKind,
    pub visual_state: CanvasPaintWireVisualState,
    pub target_feedback: CanvasPaintConnectionTargetFeedback,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasPaintConnectionTargetState {
    Free,
    Valid,
    Invalid,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintConnectionTargetFeedback {
    pub role: CanvasConnectionEndpointRole,
    pub state: CanvasPaintConnectionTargetState,
    pub view_bounds: Bounds<Pixels>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasPaintReconnectEndpoint {
    Source,
    Target,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintReconnectHandle {
    pub edge_id: EdgeId,
    pub endpoint: CanvasPaintReconnectEndpoint,
    pub shape: CanvasPaintReconnectHandleShape,
    pub view_bounds: Bounds<Pixels>,
    pub hit_bounds: Bounds<Pixels>,
    pub visual_bounds: Bounds<Pixels>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasPaintReconnectHandleShape {
    SourcePlug,
    TargetSocket,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintTransformHandle {
    pub target: CanvasTransformTarget,
    pub handle: crate::CanvasResizeHandle,
    pub view_bounds: Bounds<Pixels>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintSnapGuide {
    pub axis: CanvasSnapAxis,
    pub view_start: Point<Pixels>,
    pub view_end: Point<Pixels>,
}

pub(super) fn interaction_frame(
    model: &CanvasPaintModel,
    options: CanvasPaintOptions,
    selection_scope: Option<&CanvasResolvedSelectionScope>,
) -> CanvasPaintInteractionFrame {
    match model.interaction.state() {
        CanvasPaintInteractionState::Selecting { origin, current } => CanvasPaintInteractionFrame {
            selection_bounds: Some(
                model
                    .viewport
                    .document_bounds_to_view(bounds_from_points(*origin, *current)),
            ),
            structural_selection_bounds: None,
            connection_preview: None,
            reconnect_handles: Vec::new(),
            transform_handles: Vec::new(),
            snap_guides: Vec::new(),
        },
        CanvasPaintInteractionState::Connecting { source, current } => {
            CanvasPaintInteractionFrame {
                selection_bounds: None,
                structural_selection_bounds: structural_selection_bounds(model, selection_scope),
                connection_preview: connection_preview(
                    model,
                    source,
                    *current,
                    options.connection_preview_route.edge_route(),
                ),
                reconnect_handles: Vec::new(),
                transform_handles: Vec::new(),
                snap_guides: Vec::new(),
            }
        }
        CanvasPaintInteractionState::Reconnecting {
            edge_id,
            endpoint,
            fixed,
            current,
        } => CanvasPaintInteractionFrame {
            selection_bounds: None,
            structural_selection_bounds: structural_selection_bounds(model, selection_scope),
            connection_preview: reconnect_preview(
                model,
                edge_id,
                *endpoint,
                fixed,
                *current,
                options.connection_preview_route.edge_route(),
            ),
            reconnect_handles: Vec::new(),
            transform_handles: Vec::new(),
            snap_guides: Vec::new(),
        },
        CanvasPaintInteractionState::Transforming { snap_guides } => CanvasPaintInteractionFrame {
            selection_bounds: None,
            structural_selection_bounds: structural_selection_bounds(model, selection_scope),
            connection_preview: None,
            reconnect_handles: reconnect_handles_for_model(model),
            transform_handles: transform_handles_for_model(model),
            snap_guides: paint_snap_guides(model, snap_guides),
        },
        CanvasPaintInteractionState::Idle => CanvasPaintInteractionFrame {
            selection_bounds: None,
            structural_selection_bounds: structural_selection_bounds(model, selection_scope),
            connection_preview: None,
            reconnect_handles: reconnect_handles_for_model(model),
            transform_handles: transform_handles_for_model(model),
            snap_guides: Vec::new(),
        },
    }
}

fn structural_selection_bounds(
    model: &CanvasPaintModel,
    selection_scope: Option<&CanvasResolvedSelectionScope>,
) -> Option<Bounds<Pixels>> {
    let selection_scope = selection_scope?;
    let facts = CanvasGeometryFacts::with_kind_registry(
        model.document.as_ref(),
        model.kind_registry.as_ref(),
    );
    let structural_bounds =
        facts.node_shape_bounds_for_records(selection_scope.paint_structural_records())?;
    if Some(structural_bounds) == facts.selected_bounds(model.interaction.selection()) {
        return None;
    }

    Some(model.viewport.document_bounds_to_view(structural_bounds))
}

fn paint_snap_guides(
    model: &CanvasPaintModel,
    guides: &[CanvasSnapGuide],
) -> Vec<CanvasPaintSnapGuide> {
    guides
        .iter()
        .map(|guide| CanvasPaintSnapGuide {
            axis: guide.axis,
            view_start: model.viewport.document_to_view(guide.document_start),
            view_end: model.viewport.document_to_view(guide.document_end),
        })
        .collect()
}

fn transform_handles_for_model(model: &CanvasPaintModel) -> Vec<CanvasPaintTransformHandle> {
    canvas_transform_handles(
        model.document.as_ref(),
        model.interaction.selection(),
        model.viewport,
        Some(model.kind_registry.as_ref()),
    )
    .into_iter()
    .map(|handle: CanvasTransformHandle| CanvasPaintTransformHandle {
        target: handle.target,
        handle: handle.handle,
        view_bounds: model
            .viewport
            .document_bounds_to_view(handle.document_bounds),
    })
    .collect()
}

fn reconnect_handles_for_model(model: &CanvasPaintModel) -> Vec<CanvasPaintReconnectHandle> {
    let facts = CanvasGeometryFacts::with_kind_registry(
        model.document.as_ref(),
        model.kind_registry.as_ref(),
    );
    model
        .interaction
        .selection()
        .selected_edges()
        .filter_map(|edge_id| {
            let edge = model.document.edge(edge_id)?;
            let source = facts.endpoint_position(&edge.source).ok()?;
            let target = facts.endpoint_position(&edge.target).ok()?;
            Some([
                reconnect_handle(
                    edge_id.clone(),
                    CanvasPaintReconnectEndpoint::Source,
                    model.viewport.document_to_view(source),
                ),
                reconnect_handle(
                    edge_id.clone(),
                    CanvasPaintReconnectEndpoint::Target,
                    model.viewport.document_to_view(target),
                ),
            ])
        })
        .flatten()
        .collect()
}

fn reconnect_handle(
    edge_id: EdgeId,
    endpoint: CanvasPaintReconnectEndpoint,
    center: Point<Pixels>,
) -> CanvasPaintReconnectHandle {
    let hit_bounds = Bounds::centered_at(
        center,
        size(RECONNECT_HANDLE_VIEW_SIZE, RECONNECT_HANDLE_VIEW_SIZE),
    );
    let visual_size = RECONNECT_HANDLE_VISUAL_SIZE
        .min(hit_bounds.size.width)
        .min(hit_bounds.size.height);
    CanvasPaintReconnectHandle {
        edge_id,
        endpoint,
        shape: reconnect_handle_shape(endpoint),
        view_bounds: hit_bounds,
        hit_bounds,
        visual_bounds: Bounds::centered_at(center, size(visual_size, visual_size)),
    }
}

fn reconnect_handle_shape(
    endpoint: CanvasPaintReconnectEndpoint,
) -> CanvasPaintReconnectHandleShape {
    match endpoint {
        CanvasPaintReconnectEndpoint::Source => CanvasPaintReconnectHandleShape::SourcePlug,
        CanvasPaintReconnectEndpoint::Target => CanvasPaintReconnectHandleShape::TargetSocket,
    }
}

fn connection_preview(
    model: &CanvasPaintModel,
    source: &CanvasEndpoint,
    current: Point<Pixels>,
    route: CanvasEdgeRoute,
) -> Option<CanvasPaintConnectionPreview> {
    let facts = CanvasGeometryFacts::with_kind_registry(
        model.document.as_ref(),
        model.kind_registry.as_ref(),
    );
    let source = facts.endpoint_position(source).ok()?;
    let target = connection_preview_endpoint_target(
        model,
        CanvasConnectionEndpointRole::Target,
        source,
        current,
    );
    Some(connection_preview_frame(
        model,
        source,
        target.document_position,
        route,
        target.feedback,
    ))
}

fn reconnect_preview(
    model: &CanvasPaintModel,
    edge_id: &EdgeId,
    endpoint: CanvasConnectionEndpointRole,
    fixed: &CanvasEndpoint,
    current: Point<Pixels>,
    fallback_route: CanvasEdgeRoute,
) -> Option<CanvasPaintConnectionPreview> {
    let facts = CanvasGeometryFacts::with_kind_registry(
        model.document.as_ref(),
        model.kind_registry.as_ref(),
    );
    let fixed = facts.endpoint_position(fixed).ok()?;
    let moving = connection_preview_endpoint_target(model, endpoint, fixed, current);
    let route = model
        .document
        .edge(edge_id)
        .map(|edge| edge.route.clone())
        .unwrap_or(fallback_route);
    Some(match endpoint {
        CanvasConnectionEndpointRole::Source => connection_preview_frame(
            model,
            moving.document_position,
            fixed,
            route,
            moving.feedback,
        ),
        CanvasConnectionEndpointRole::Target => connection_preview_frame(
            model,
            fixed,
            moving.document_position,
            route,
            moving.feedback,
        ),
    })
}

fn connection_preview_frame(
    model: &CanvasPaintModel,
    source: Point<Pixels>,
    target: Point<Pixels>,
    route: CanvasEdgeRoute,
    target_feedback: CanvasPaintConnectionTargetFeedback,
) -> CanvasPaintConnectionPreview {
    let route_kind = route.kind.clone();
    let visual_state = preview_wire_visual_state(target_feedback.state);
    let mut edge = CanvasEdge::new(
        "__connection_preview",
        CanvasEndpoint::new("__connection_preview_source", None::<&str>),
        CanvasEndpoint::new("__connection_preview_target", None::<&str>),
    );
    edge.route = route;
    let path = CanvasDefaultEdgeRouter.route_edge(CanvasRouteRequest {
        edge: &edge,
        source,
        target,
    });
    CanvasPaintConnectionPreview {
        source_view_position: model.viewport.document_to_view(source),
        target_view_position: model.viewport.document_to_view(target),
        edge_geometry: CanvasPaintEdgeGeometry {
            view_path: route_path_to_view(&path, model.viewport),
            visual_state,
        },
        route_kind,
        visual_state,
        target_feedback,
    }
}

fn preview_wire_visual_state(
    state: CanvasPaintConnectionTargetState,
) -> CanvasPaintWireVisualState {
    match state {
        CanvasPaintConnectionTargetState::Free => CanvasPaintWireVisualState::PreviewFree,
        CanvasPaintConnectionTargetState::Valid => CanvasPaintWireVisualState::PreviewValidTarget,
        CanvasPaintConnectionTargetState::Invalid => {
            CanvasPaintWireVisualState::PreviewInvalidTarget
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct CanvasConnectionPreviewEndpointTarget {
    document_position: Point<Pixels>,
    feedback: CanvasPaintConnectionTargetFeedback,
}

fn connection_preview_endpoint_target(
    model: &CanvasPaintModel,
    role: CanvasConnectionEndpointRole,
    fixed: Point<Pixels>,
    current: Point<Pixels>,
) -> CanvasConnectionPreviewEndpointTarget {
    let facts = CanvasGeometryFacts::with_kind_registry(
        model.document.as_ref(),
        model.kind_registry.as_ref(),
    );
    let records = model
        .runtime
        .precise_hit_test_with_facts(facts, current, connection_hit_options())
        .collect::<Vec<_>>();

    for record in records {
        match &record.target {
            HitTarget::Handle { node_id, handle_id } => {
                let Some(node) = model.document.node(node_id) else {
                    continue;
                };
                let Some(handle) = node.handle(Some(handle_id)) else {
                    continue;
                };
                let endpoint = CanvasEndpoint {
                    node_id: node_id.clone(),
                    handle_id: Some(handle_id.clone()),
                };
                let position = facts.endpoint_position(&endpoint).unwrap_or(current);
                let state = if handle.is_pickable_connection_endpoint(role) && position != fixed {
                    CanvasPaintConnectionTargetState::Valid
                } else {
                    CanvasPaintConnectionTargetState::Invalid
                };
                return CanvasConnectionPreviewEndpointTarget {
                    document_position: if state == CanvasPaintConnectionTargetState::Valid {
                        position
                    } else {
                        current
                    },
                    feedback: connection_target_feedback(model, role, state, position),
                };
            }
            HitTarget::Node(node_id) => {
                let endpoint = CanvasEndpoint {
                    node_id: node_id.clone(),
                    handle_id: None,
                };
                let position = facts.endpoint_position(&endpoint).unwrap_or(current);
                let state = if position != fixed {
                    CanvasPaintConnectionTargetState::Valid
                } else {
                    CanvasPaintConnectionTargetState::Invalid
                };
                return CanvasConnectionPreviewEndpointTarget {
                    document_position: if state == CanvasPaintConnectionTargetState::Valid {
                        position
                    } else {
                        current
                    },
                    feedback: connection_target_feedback(model, role, state, position),
                };
            }
            HitTarget::Edge(_) | HitTarget::Shape(_) => {}
        }
    }

    CanvasConnectionPreviewEndpointTarget {
        document_position: current,
        feedback: connection_target_feedback(
            model,
            role,
            CanvasPaintConnectionTargetState::Free,
            current,
        ),
    }
}

fn connection_target_feedback(
    model: &CanvasPaintModel,
    role: CanvasConnectionEndpointRole,
    state: CanvasPaintConnectionTargetState,
    document_position: Point<Pixels>,
) -> CanvasPaintConnectionTargetFeedback {
    CanvasPaintConnectionTargetFeedback {
        role,
        state,
        view_bounds: Bounds::centered_at(
            model.viewport.document_to_view(document_position),
            size(
                CONNECTION_TARGET_FEEDBACK_VIEW_SIZE,
                CONNECTION_TARGET_FEEDBACK_VIEW_SIZE,
            ),
        ),
    }
}

pub(super) fn target_is_selected(target: &HitTarget, selection: &CanvasSelection) -> bool {
    selection.contains_target(target)
}

pub(super) fn committed_wire_visual_state(
    selected: bool,
    hovered: bool,
) -> CanvasPaintWireVisualState {
    match (selected, hovered) {
        (true, true) => CanvasPaintWireVisualState::SelectedHovered,
        (true, false) => CanvasPaintWireVisualState::Selected,
        (false, true) => CanvasPaintWireVisualState::Hovered,
        (false, false) => CanvasPaintWireVisualState::Committed,
    }
}

pub(super) fn target_is_structurally_selected(
    target: &HitTarget,
    scope: Option<&CanvasResolvedSelectionScope>,
) -> bool {
    let Some(scope) = scope else {
        return false;
    };
    let Some(record_id) = record_id_for_target(target) else {
        return false;
    };

    scope.contains_paint_structural_record(&record_id)
}

fn record_id_for_target(target: &HitTarget) -> Option<CanvasRecordId> {
    match target {
        HitTarget::Node(id) => Some(CanvasRecordId::Node(id.clone())),
        HitTarget::Edge(id) => Some(CanvasRecordId::Edge(id.clone())),
        HitTarget::Shape(id) => Some(CanvasRecordId::Shape(id.clone())),
        HitTarget::Handle { .. } => None,
    }
}
fn bounds_from_points(a: Point<Pixels>, b: Point<Pixels>) -> Bounds<Pixels> {
    Bounds::from_corners(
        Point::new(a.x.min(b.x), a.y.min(b.y)),
        Point::new(a.x.max(b.x), a.y.max(b.y)),
    )
}
