use super::*;
use super::{
    frame::label_line_clamp,
    style::{
        CanvasResolvedEdgePaintStyle, CanvasResolvedPaintStyle, edge_paint_style, node_paint_style,
        parse_color, shape_paint_style, style_color,
    },
};
use crate::{
    CanvasConnectionEndpointRole, CanvasDocument, CanvasEdge, CanvasEdgeKind,
    CanvasEdgeRenderPolicy, CanvasEdgeRouteKind, CanvasEditor, CanvasEndpoint, CanvasEvent,
    CanvasHandle, CanvasKey, CanvasKeyModifiers, CanvasKindLabel, CanvasKindPaint,
    CanvasKindRegistry, CanvasNode, CanvasNodeGeometryPolicy, CanvasNodeKind,
    CanvasNodeRenderPolicy, CanvasRecordId, CanvasSelection, CanvasSelectionMode, CanvasShape,
    CanvasShapeKind, CanvasShapeRenderPolicy, CanvasSnapAxis, CanvasSnapGuide, CanvasStyle,
    CanvasTransaction, CanvasTransformTarget, CanvasViewport, DocumentCommand, EdgeId, HandleRole,
    HitTarget, PointerButton,
    routing::{CanvasEdgeRouter, CanvasRoutePath, CanvasRouteRequest},
    session::ToolState,
    test_support::{connected_pair_fixture, document_fixture},
    tool::CanvasToolEffect,
};
use open_gpui::{
    Bounds, Hsla, KeyDownEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, ScrollDelta, ScrollWheelEvent, point, px, rgb, size,
};

mod feedback;
mod input;
mod paint_model;
mod scene;
mod style;

fn scene_layer_index(
    layers: &[CanvasSceneLayerItem],
    target: HitTarget,
    phase: CanvasSceneLayerPhase,
) -> usize {
    layers
        .iter()
        .position(|item| item.target == target && item.phase == phase)
        .expect("scene layer item")
}

fn straight_preview(
    source: open_gpui::Point<Pixels>,
    target: open_gpui::Point<Pixels>,
    target_state: CanvasPaintConnectionTargetState,
    feedback_center: open_gpui::Point<Pixels>,
) -> CanvasPaintConnectionPreview {
    CanvasPaintConnectionPreview {
        source_view_position: source,
        target_view_position: target,
        edge_geometry: CanvasPaintEdgeGeometry {
            view_path: CanvasRoutePath::polyline([source, target]),
            visual_state: preview_visual_state(target_state),
        },
        route_kind: CanvasEdgeRouteKind::new(CanvasEdgeRouteKind::STRAIGHT),
        visual_state: preview_visual_state(target_state),
        target_feedback: CanvasPaintConnectionTargetFeedback {
            role: CanvasConnectionEndpointRole::Target,
            state: target_state,
            view_bounds: Bounds::centered_at(feedback_center, size(px(18.0), px(18.0))),
        },
    }
}

fn preview_visual_state(
    target_state: CanvasPaintConnectionTargetState,
) -> CanvasPaintWireVisualState {
    match target_state {
        CanvasPaintConnectionTargetState::Free => CanvasPaintWireVisualState::PreviewFree,
        CanvasPaintConnectionTargetState::Valid => CanvasPaintWireVisualState::PreviewValidTarget,
        CanvasPaintConnectionTargetState::Invalid => {
            CanvasPaintWireVisualState::PreviewInvalidTarget
        }
    }
}

fn large_grid_document(columns: usize, rows: usize) -> CanvasDocument {
    let mut fixture = document_fixture();

    for row in 0..rows {
        for column in 0..columns {
            fixture.add_node(CanvasNode::new(
                format!("node-{row}-{column}"),
                point(px(column as f32 * 160.0), px(row as f32 * 120.0)),
                size(px(96.0), px(56.0)),
            ));
        }
    }

    fixture.build()
}

fn connected_edge_document() -> CanvasDocument {
    connected_pair_fixture().build()
}

fn geometry_registry() -> CanvasKindRegistry {
    let mut registry = CanvasKindRegistry::open();
    registry.register_node_kind(
        "wide",
        CanvasNodeKind::new().with_geometry_policy(WideNodeKind),
    );
    registry
}

fn paint_registry() -> CanvasKindRegistry {
    let mut registry = CanvasKindRegistry::open();
    registry.register_node_kind(
        "painted-node",
        CanvasNodeKind::new().with_render_policy(PaintedNodeKind),
    );
    registry.register_edge_kind(
        "painted-edge",
        CanvasEdgeKind::new().with_render_policy(PaintedEdgeKind),
    );
    registry.register_shape_kind(
        "painted-shape",
        CanvasShapeKind::new().with_render_policy(PaintedShapeKind),
    );
    registry
}

struct WideNodeKind;

impl CanvasNodeGeometryPolicy for WideNodeKind {
    fn node_bounds(&self, node: &CanvasNode) -> Option<Bounds<open_gpui::Pixels>> {
        Some(node.bounds().dilate(px(5.0)))
    }

    fn handle_position(
        &self,
        node: &CanvasNode,
        handle_id: &crate::HandleId,
    ) -> Option<open_gpui::Point<open_gpui::Pixels>> {
        match handle_id.as_str() {
            "out" => Some(point(
                node.position.x + node.size.width + px(20.0),
                node.position.y + px(5.0),
            )),
            "in" => Some(point(node.position.x - px(20.0), node.position.y + px(5.0))),
            _ => None,
        }
    }
}

struct PaintedNodeKind;

impl CanvasNodeRenderPolicy for PaintedNodeKind {
    fn node_paint(&self, _node: &CanvasNode) -> Option<CanvasKindPaint> {
        Some(CanvasKindPaint {
            fill: Some("#fff8c5".to_string()),
            stroke: Some("#bf8700".to_string()),
            stroke_width: Some(px(2.0)),
            corner_radius: Some(px(10.0)),
        })
    }

    fn node_label(&self, _node: &CanvasNode) -> Option<CanvasKindLabel> {
        Some(
            CanvasKindLabel::new("Node label")
                .with_inset(px(8.0))
                .with_color("#24292f"),
        )
    }
}

struct PaintedEdgeKind;

impl CanvasEdgeRenderPolicy for PaintedEdgeKind {
    fn edge_paint(&self, _edge: &CanvasEdge) -> Option<CanvasKindPaint> {
        Some(CanvasKindPaint {
            fill: None,
            stroke: Some("#d1242f".to_string()),
            stroke_width: Some(px(5.0)),
            corner_radius: None,
        })
    }
}

struct PaintedShapeKind;

impl CanvasShapeRenderPolicy for PaintedShapeKind {
    fn shape_paint(&self, _shape: &CanvasShape) -> Option<CanvasKindPaint> {
        Some(CanvasKindPaint {
            fill: Some("#ddf4ff".to_string()),
            stroke: Some("#0969da".to_string()),
            stroke_width: Some(px(3.0)),
            corner_radius: Some(px(4.0)),
        })
    }

    fn shape_label(&self, _shape: &CanvasShape) -> Option<CanvasKindLabel> {
        Some(
            CanvasKindLabel::new("Shape label")
                .with_inset(px(4.0))
                .with_color("#0969da"),
        )
    }
}

struct VerticalDetourRouter;

impl CanvasEdgeRouter for VerticalDetourRouter {
    fn route_edge(&self, request: CanvasRouteRequest<'_>) -> CanvasRoutePath {
        CanvasRoutePath::polyline([
            request.source,
            point(request.source.x, px(80.0)),
            request.target,
        ])
    }
}
