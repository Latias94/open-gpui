use std::sync::{Arc, Mutex};

use super::*;
use crate::{
    CanvasEdge, CanvasKindRegistry, CanvasNode, CanvasNodeGeometryPolicy, CanvasNodeHitTest,
    CanvasNodeInteractionPolicy, CanvasNodeKind, CanvasNodeResizeProposal, CanvasNodeSchemaPolicy,
    CanvasNodeTransformPolicy, CanvasRecordId, CanvasRecordKind, CanvasResizeHandle,
    CanvasSchemaError, CanvasShape, CanvasTransformTarget, CanvasValue, HandleId, HitOptions,
    canvas_transform_handles,
    record_scope::CanvasRecordScopeOptions,
    routing::{CanvasRoutePath, CanvasRouteRequest},
    test_support::{connected_pair_fixture, document_fixture},
};
use open_gpui::{point, px, size};
use serde_json::{Value, json};

mod connection;
mod custom_tool;
mod group_clipboard;
mod history;
mod selection;
mod transform;
mod z_order;

#[derive(Default)]
struct StampTool {
    calls: usize,
    last_tool_id: Option<CanvasToolId>,
    last_hit: Option<HitTarget>,
}

impl CanvasToolReducer for StampTool {
    fn handle_event(
        &mut self,
        context: CanvasToolContext<'_>,
        event: CanvasEvent,
    ) -> Result<Vec<CanvasToolIntent>, DocumentError> {
        self.calls += 1;
        self.last_tool_id = context.active_custom_tool_id().cloned();

        let CanvasEvent::PointerDown {
            position,
            button: PointerButton::Primary,
            ..
        } = event
        else {
            return Ok(Vec::new());
        };

        self.last_hit = context
            .hit_test_view(position, HitOptions::default())
            .next()
            .map(|record| record.target.clone());

        let node_id = NodeId::new(format!("stamp-{}", context.document().node_count()));
        let mut selection = CanvasSelection::default();
        selection.nodes.insert(node_id.clone());

        Ok(vec![
            CanvasToolIntent::ApplyTransaction(CanvasTransaction::single(
                DocumentCommand::InsertNode(CanvasNode::new(
                    node_id.clone(),
                    context.document_position(position),
                    size(px(20.0), px(20.0)),
                )),
            )),
            CanvasToolIntent::SetSelection(selection),
            CanvasToolIntent::CommitTransaction,
        ])
    }
}

struct RequiredTitleNodeKind;

impl CanvasNodeSchemaPolicy for RequiredTitleNodeKind {
    fn default_data(&self) -> CanvasValue {
        CanvasValue::from_iter([("title".to_string(), json!("Untitled"))])
    }

    fn migrate_node(&self, node: &mut CanvasNode) -> Result<(), CanvasSchemaError> {
        if let Some(value) = node.data.remove("label") {
            node.data.insert("title".to_string(), value);
        }
        Ok(())
    }

    fn validate_node(&self, node: &CanvasNode) -> Result<(), CanvasSchemaError> {
        match node.data.get("title") {
            Some(Value::String(title)) if !title.trim().is_empty() => Ok(()),
            Some(_) => Err(CanvasSchemaError::invalid_data(
                CanvasRecordKind::Node,
                node.id.clone(),
                &node.kind,
                "title must be a non-empty string",
            )),
            None => Err(CanvasSchemaError::missing_required_data(
                CanvasRecordKind::Node,
                node.id.clone(),
                &node.kind,
                "title",
            )),
        }
    }
}

struct WideBoundsNodeKind;

impl CanvasNodeGeometryPolicy for WideBoundsNodeKind {
    fn node_bounds(&self, node: &CanvasNode) -> Option<Bounds<Pixels>> {
        Some(Bounds::new(
            node.position,
            size(node.size.width + px(30.0), node.size.height),
        ))
    }
}

struct MinimumResizeNodeKind;

impl CanvasNodeTransformPolicy for MinimumResizeNodeKind {
    fn resize_node_bounds(
        &self,
        proposal: CanvasNodeResizeProposal<'_>,
    ) -> Result<Bounds<Pixels>, CanvasSchemaError> {
        Ok(Bounds::new(
            proposal.bounds.origin,
            size(
                proposal.bounds.size.width.max(px(64.0)),
                proposal.bounds.size.height.max(px(48.0)),
            ),
        ))
    }
}

struct RejectResizeNodeKind;

impl CanvasNodeTransformPolicy for RejectResizeNodeKind {
    fn resize_node_bounds(
        &self,
        proposal: CanvasNodeResizeProposal<'_>,
    ) -> Result<Bounds<Pixels>, CanvasSchemaError> {
        Err(CanvasSchemaError::invalid_data(
            CanvasRecordKind::Node,
            proposal.node.id.clone(),
            &proposal.node.kind,
            "resize is disabled",
        ))
    }
}

struct RightHalfNodeKind;

impl CanvasNodeInteractionPolicy for RightHalfNodeKind {
    fn node_contains_point(&self, hit: CanvasNodeHitTest<'_>) -> Option<bool> {
        Some(hit.point.x >= hit.bounds.center().x)
    }
}

struct WholeNodeEndpointKind;

impl CanvasNodeInteractionPolicy for WholeNodeEndpointKind {
    fn node_accepts_connection_endpoint(
        &self,
        _node: &CanvasNode,
        _role: CanvasConnectionEndpointRole,
    ) -> bool {
        true
    }
}

fn required_title_node_kind() -> CanvasNodeKind {
    CanvasNodeKind::new().with_schema_policy(RequiredTitleNodeKind)
}

fn wide_bounds_node_kind() -> CanvasNodeKind {
    CanvasNodeKind::new().with_geometry_policy(WideBoundsNodeKind)
}

fn minimum_resize_node_kind() -> CanvasNodeKind {
    CanvasNodeKind::new().with_transform_policy(MinimumResizeNodeKind)
}

fn reject_resize_node_kind() -> CanvasNodeKind {
    CanvasNodeKind::new().with_transform_policy(RejectResizeNodeKind)
}

fn right_half_node_kind() -> CanvasNodeKind {
    CanvasNodeKind::new().with_interaction_policy(RightHalfNodeKind)
}

fn whole_node_endpoint_kind() -> CanvasNodeKind {
    CanvasNodeKind::new().with_interaction_policy(WholeNodeEndpointKind)
}

fn connected_edge_document() -> CanvasDocument {
    connected_pair_fixture().build()
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
