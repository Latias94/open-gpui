use crate::{
    CanvasEdge, CanvasNode, CanvasRecordId, CanvasShape, CanvasTransaction, CanvasValue,
    DocumentCommand,
};
use indexmap::IndexMap;
use open_gpui::{Bounds, Pixels, Point};
use std::{fmt, sync::Arc};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasRecordKind {
    Node,
    Edge,
    Shape,
}

impl CanvasRecordKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Edge => "edge",
            Self::Shape => "shape",
        }
    }
}

impl fmt::Display for CanvasRecordKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CanvasSchemaError {
    #[error(
        "{record_kind} `{record_id}` with kind `{kind}` is missing required data field `{field}`"
    )]
    MissingRequiredData {
        record_kind: CanvasRecordKind,
        record_id: CanvasRecordId,
        kind: String,
        field: String,
    },
    #[error("{record_kind} `{record_id}` with kind `{kind}` has invalid data: {message}")]
    InvalidData {
        record_kind: CanvasRecordKind,
        record_id: CanvasRecordId,
        kind: String,
        message: String,
    },
}

impl CanvasSchemaError {
    pub fn missing_required_data(
        record_kind: CanvasRecordKind,
        record_id: impl Into<CanvasRecordId>,
        kind: impl Into<String>,
        field: impl Into<String>,
    ) -> Self {
        Self::MissingRequiredData {
            record_kind,
            record_id: record_id.into(),
            kind: kind.into(),
            field: field.into(),
        }
    }

    pub fn invalid_data(
        record_kind: CanvasRecordKind,
        record_id: impl Into<CanvasRecordId>,
        kind: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::InvalidData {
            record_kind,
            record_id: record_id.into(),
            kind: kind.into(),
            message: message.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasNodeResizeProposal<'a> {
    pub node: &'a CanvasNode,
    pub bounds: Bounds<Pixels>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasShapeResizeProposal<'a> {
    pub shape: &'a CanvasShape,
    pub bounds: Bounds<Pixels>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasNodeHitTest<'a> {
    pub node: &'a CanvasNode,
    pub point: Point<Pixels>,
    pub bounds: Bounds<Pixels>,
    pub margin: Pixels,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasShapeHitTest<'a> {
    pub shape: &'a CanvasShape,
    pub point: Point<Pixels>,
    pub bounds: Bounds<Pixels>,
    pub margin: Pixels,
}

pub trait CanvasNodeKind: Send + Sync {
    fn default_data(&self) -> CanvasValue {
        CanvasValue::new()
    }

    fn migrate_node(&self, _node: &mut CanvasNode) -> Result<(), CanvasSchemaError> {
        Ok(())
    }

    fn validate_node(&self, _node: &CanvasNode) -> Result<(), CanvasSchemaError> {
        Ok(())
    }

    fn node_bounds(&self, _node: &CanvasNode) -> Option<Bounds<Pixels>> {
        None
    }

    fn handle_position(
        &self,
        _node: &CanvasNode,
        _handle_id: &crate::HandleId,
    ) -> Option<Point<Pixels>> {
        None
    }

    fn node_contains_point(&self, _hit: CanvasNodeHitTest<'_>) -> Option<bool> {
        None
    }

    fn resize_node_bounds(
        &self,
        proposal: CanvasNodeResizeProposal<'_>,
    ) -> Result<Bounds<Pixels>, CanvasSchemaError> {
        Ok(proposal.bounds)
    }
}

pub trait CanvasEdgeKind: Send + Sync {
    fn default_data(&self) -> CanvasValue {
        CanvasValue::new()
    }

    fn migrate_edge(&self, _edge: &mut CanvasEdge) -> Result<(), CanvasSchemaError> {
        Ok(())
    }

    fn validate_edge(&self, _edge: &CanvasEdge) -> Result<(), CanvasSchemaError> {
        Ok(())
    }
}

pub trait CanvasShapeKind: Send + Sync {
    fn default_data(&self) -> CanvasValue {
        CanvasValue::new()
    }

    fn migrate_shape(&self, _shape: &mut CanvasShape) -> Result<(), CanvasSchemaError> {
        Ok(())
    }

    fn validate_shape(&self, _shape: &CanvasShape) -> Result<(), CanvasSchemaError> {
        Ok(())
    }

    fn shape_bounds(&self, _shape: &CanvasShape) -> Option<Bounds<Pixels>> {
        None
    }

    fn shape_contains_point(&self, _hit: CanvasShapeHitTest<'_>) -> Option<bool> {
        None
    }

    fn resize_shape_bounds(
        &self,
        proposal: CanvasShapeResizeProposal<'_>,
    ) -> Result<Bounds<Pixels>, CanvasSchemaError> {
        Ok(proposal.bounds)
    }
}

#[derive(Clone, Default)]
pub struct CanvasKindRegistry {
    node_kinds: IndexMap<String, Arc<dyn CanvasNodeKind>>,
    edge_kinds: IndexMap<String, Arc<dyn CanvasEdgeKind>>,
    shape_kinds: IndexMap<String, Arc<dyn CanvasShapeKind>>,
}

impl fmt::Debug for CanvasKindRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CanvasKindRegistry")
            .field(
                "node_kinds",
                &self.node_kinds.keys().cloned().collect::<Vec<_>>(),
            )
            .field(
                "edge_kinds",
                &self.edge_kinds.keys().cloned().collect::<Vec<_>>(),
            )
            .field(
                "shape_kinds",
                &self.shape_kinds.keys().cloned().collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl CanvasKindRegistry {
    pub fn open() -> Self {
        Self::default()
    }

    pub fn register_node_kind(
        &mut self,
        kind: impl Into<String>,
        schema: impl CanvasNodeKind + 'static,
    ) {
        self.node_kinds.insert(kind.into(), Arc::new(schema));
    }

    pub fn register_edge_kind(
        &mut self,
        kind: impl Into<String>,
        schema: impl CanvasEdgeKind + 'static,
    ) {
        self.edge_kinds.insert(kind.into(), Arc::new(schema));
    }

    pub fn register_shape_kind(
        &mut self,
        kind: impl Into<String>,
        schema: impl CanvasShapeKind + 'static,
    ) {
        self.shape_kinds.insert(kind.into(), Arc::new(schema));
    }

    pub fn node_kind(&self, kind: &str) -> Option<&dyn CanvasNodeKind> {
        self.node_kinds.get(kind).map(Arc::as_ref)
    }

    pub fn edge_kind(&self, kind: &str) -> Option<&dyn CanvasEdgeKind> {
        self.edge_kinds.get(kind).map(Arc::as_ref)
    }

    pub fn shape_kind(&self, kind: &str) -> Option<&dyn CanvasShapeKind> {
        self.shape_kinds.get(kind).map(Arc::as_ref)
    }

    pub fn normalize_transaction(
        &self,
        transaction: CanvasTransaction,
    ) -> Result<CanvasTransaction, CanvasSchemaError> {
        Ok(CanvasTransaction {
            commands: transaction
                .commands
                .into_iter()
                .map(|command| self.normalize_command(command))
                .collect::<Result<Vec<_>, _>>()?,
            metadata: transaction.metadata,
        })
    }

    pub fn normalize_command(
        &self,
        command: DocumentCommand,
    ) -> Result<DocumentCommand, CanvasSchemaError> {
        Ok(match command {
            DocumentCommand::InsertNode(node) => {
                DocumentCommand::InsertNode(self.normalize_node(node)?)
            }
            DocumentCommand::UpdateNode(node) => {
                DocumentCommand::UpdateNode(self.normalize_node(node)?)
            }
            DocumentCommand::InsertEdge(edge) => {
                DocumentCommand::InsertEdge(self.normalize_edge(edge)?)
            }
            DocumentCommand::UpdateEdge(edge) => {
                DocumentCommand::UpdateEdge(self.normalize_edge(edge)?)
            }
            DocumentCommand::InsertShape(shape) => {
                DocumentCommand::InsertShape(self.normalize_shape(shape)?)
            }
            DocumentCommand::UpdateShape(shape) => {
                DocumentCommand::UpdateShape(self.normalize_shape(shape)?)
            }
            command => command,
        })
    }

    pub fn normalize_node(&self, mut node: CanvasNode) -> Result<CanvasNode, CanvasSchemaError> {
        let Some(schema) = self.node_kind(&node.kind) else {
            return Ok(node);
        };

        schema.migrate_node(&mut node)?;
        merge_default_data(&mut node.data, schema.default_data());
        schema.validate_node(&node)?;
        Ok(node)
    }

    pub fn normalize_edge(&self, mut edge: CanvasEdge) -> Result<CanvasEdge, CanvasSchemaError> {
        let Some(schema) = self.edge_kind(&edge.kind) else {
            return Ok(edge);
        };

        schema.migrate_edge(&mut edge)?;
        merge_default_data(&mut edge.data, schema.default_data());
        schema.validate_edge(&edge)?;
        Ok(edge)
    }

    pub fn normalize_shape(
        &self,
        mut shape: CanvasShape,
    ) -> Result<CanvasShape, CanvasSchemaError> {
        let Some(schema) = self.shape_kind(&shape.kind) else {
            return Ok(shape);
        };

        schema.migrate_shape(&mut shape)?;
        merge_default_data(&mut shape.data, schema.default_data());
        schema.validate_shape(&shape)?;
        Ok(shape)
    }

    pub fn validate_document(
        &self,
        document: &crate::CanvasDocument,
    ) -> Result<(), CanvasSchemaError> {
        for node in document.nodes.values() {
            if let Some(schema) = self.node_kind(&node.kind) {
                schema.validate_node(node)?;
            }
        }

        for edge in document.edges.values() {
            if let Some(schema) = self.edge_kind(&edge.kind) {
                schema.validate_edge(edge)?;
            }
        }

        for shape in document.shapes.values() {
            if let Some(schema) = self.shape_kind(&shape.kind) {
                schema.validate_shape(shape)?;
            }
        }

        Ok(())
    }

    pub fn node_bounds(&self, node: &CanvasNode) -> Option<Bounds<Pixels>> {
        self.node_kind(&node.kind)
            .and_then(|schema| schema.node_bounds(node))
    }

    pub fn handle_position(
        &self,
        node: &CanvasNode,
        handle_id: &crate::HandleId,
    ) -> Option<Point<Pixels>> {
        self.node_kind(&node.kind)
            .and_then(|schema| schema.handle_position(node, handle_id))
    }

    pub fn shape_bounds(&self, shape: &CanvasShape) -> Option<Bounds<Pixels>> {
        self.shape_kind(&shape.kind)
            .and_then(|schema| schema.shape_bounds(shape))
    }

    pub fn node_contains_point(
        &self,
        node: &CanvasNode,
        point: Point<Pixels>,
        bounds: Bounds<Pixels>,
        margin: Pixels,
    ) -> Option<bool> {
        self.node_kind(&node.kind).and_then(|schema| {
            schema.node_contains_point(CanvasNodeHitTest {
                node,
                point,
                bounds,
                margin,
            })
        })
    }

    pub fn shape_contains_point(
        &self,
        shape: &CanvasShape,
        point: Point<Pixels>,
        bounds: Bounds<Pixels>,
        margin: Pixels,
    ) -> Option<bool> {
        self.shape_kind(&shape.kind).and_then(|schema| {
            schema.shape_contains_point(CanvasShapeHitTest {
                shape,
                point,
                bounds,
                margin,
            })
        })
    }

    pub fn resize_node_bounds(
        &self,
        node: &CanvasNode,
        proposed: Bounds<Pixels>,
    ) -> Result<Bounds<Pixels>, CanvasSchemaError> {
        let Some(schema) = self.node_kind(&node.kind) else {
            return Ok(proposed);
        };

        let bounds = schema.resize_node_bounds(CanvasNodeResizeProposal {
            node,
            bounds: proposed,
        })?;
        validate_resize_bounds(CanvasRecordKind::Node, node.id.clone(), &node.kind, bounds)?;
        Ok(bounds)
    }

    pub fn resize_shape_bounds(
        &self,
        shape: &CanvasShape,
        proposed: Bounds<Pixels>,
    ) -> Result<Bounds<Pixels>, CanvasSchemaError> {
        let Some(schema) = self.shape_kind(&shape.kind) else {
            return Ok(proposed);
        };

        let bounds = schema.resize_shape_bounds(CanvasShapeResizeProposal {
            shape,
            bounds: proposed,
        })?;
        validate_resize_bounds(
            CanvasRecordKind::Shape,
            shape.id.clone(),
            &shape.kind,
            bounds,
        )?;
        Ok(bounds)
    }
}

fn merge_default_data(data: &mut CanvasValue, defaults: CanvasValue) {
    for (key, value) in defaults {
        data.entry(key).or_insert(value);
    }
}

fn validate_resize_bounds(
    record_kind: CanvasRecordKind,
    record_id: impl Into<CanvasRecordId>,
    kind: &str,
    bounds: Bounds<Pixels>,
) -> Result<(), CanvasSchemaError> {
    if !bounds.origin.x.as_f32().is_finite()
        || !bounds.origin.y.as_f32().is_finite()
        || !bounds.size.width.as_f32().is_finite()
        || !bounds.size.height.as_f32().is_finite()
        || bounds.size.width <= Pixels::ZERO
        || bounds.size.height <= Pixels::ZERO
    {
        return Err(CanvasSchemaError::invalid_data(
            record_kind,
            record_id,
            kind,
            "resize policy returned invalid bounds",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CanvasDocument, CanvasEndpoint, CanvasRecordKind, DocumentError, EdgeId, NodeId, ShapeId,
    };
    use open_gpui::{Bounds, point, px, size};
    use serde_json::{Value, json};

    struct RequiredTitleNodeKind;

    impl CanvasNodeKind for RequiredTitleNodeKind {
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
                None => Err(CanvasSchemaError::missing_required_data(
                    CanvasRecordKind::Node,
                    node.id.clone(),
                    &node.kind,
                    "title",
                )),
                Some(_) => Err(CanvasSchemaError::invalid_data(
                    CanvasRecordKind::Node,
                    node.id.clone(),
                    &node.kind,
                    "title must be a non-empty string",
                )),
            }
        }

        fn node_bounds(&self, node: &CanvasNode) -> Option<Bounds<Pixels>> {
            Some(node.bounds().dilate(px(10.0)))
        }

        fn handle_position(
            &self,
            node: &CanvasNode,
            handle_id: &crate::HandleId,
        ) -> Option<Point<Pixels>> {
            (handle_id.as_str() == "out").then(|| {
                point(
                    node.position.x + node.size.width + px(20.0),
                    node.position.y,
                )
            })
        }
    }

    struct RequiredRelationEdgeKind;

    impl CanvasEdgeKind for RequiredRelationEdgeKind {
        fn validate_edge(&self, edge: &CanvasEdge) -> Result<(), CanvasSchemaError> {
            if edge.data.contains_key("relation") {
                Ok(())
            } else {
                Err(CanvasSchemaError::missing_required_data(
                    CanvasRecordKind::Edge,
                    edge.id.clone(),
                    &edge.kind,
                    "relation",
                ))
            }
        }
    }

    struct SizedShapeKind;

    impl CanvasShapeKind for SizedShapeKind {
        fn default_data(&self) -> CanvasValue {
            CanvasValue::from_iter([("shapeType".to_string(), json!("box"))])
        }

        fn validate_shape(&self, shape: &CanvasShape) -> Result<(), CanvasSchemaError> {
            if shape.data.contains_key("shapeType") {
                Ok(())
            } else {
                Err(CanvasSchemaError::missing_required_data(
                    CanvasRecordKind::Shape,
                    shape.id.clone(),
                    &shape.kind,
                    "shapeType",
                ))
            }
        }

        fn shape_bounds(&self, shape: &CanvasShape) -> Option<Bounds<Pixels>> {
            Some(shape.bounds.dilate(px(5.0)))
        }
    }

    struct MinimumSizeNodeKind;

    impl CanvasNodeKind for MinimumSizeNodeKind {
        fn resize_node_bounds(
            &self,
            proposal: CanvasNodeResizeProposal<'_>,
        ) -> Result<Bounds<Pixels>, CanvasSchemaError> {
            Ok(Bounds::new(
                proposal.bounds.origin,
                size(
                    proposal.bounds.size.width.max(px(48.0)),
                    proposal.bounds.size.height.max(px(32.0)),
                ),
            ))
        }
    }

    struct RejectingShapeResizeKind;

    impl CanvasShapeKind for RejectingShapeResizeKind {
        fn resize_shape_bounds(
            &self,
            proposal: CanvasShapeResizeProposal<'_>,
        ) -> Result<Bounds<Pixels>, CanvasSchemaError> {
            Err(CanvasSchemaError::invalid_data(
                CanvasRecordKind::Shape,
                proposal.shape.id.clone(),
                &proposal.shape.kind,
                "resize is disabled",
            ))
        }
    }

    struct InvalidShapeResizeKind;

    struct RightHalfNodeKind;

    impl CanvasNodeKind for RightHalfNodeKind {
        fn node_contains_point(&self, hit: CanvasNodeHitTest<'_>) -> Option<bool> {
            Some(hit.point.x >= hit.bounds.center().x)
        }
    }

    impl CanvasShapeKind for InvalidShapeResizeKind {
        fn resize_shape_bounds(
            &self,
            proposal: CanvasShapeResizeProposal<'_>,
        ) -> Result<Bounds<Pixels>, CanvasSchemaError> {
            Ok(Bounds::new(
                proposal.bounds.origin,
                size(px(0.0), proposal.bounds.size.height),
            ))
        }
    }

    #[test]
    fn open_registry_leaves_unknown_kinds_unchanged() {
        let registry = CanvasKindRegistry::open();
        let mut node = CanvasNode::new("n", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        node.kind = "unknown".to_string();
        node.data
            .insert("custom".to_string(), json!({"kept": true}));

        let normalized = registry.normalize_node(node.clone()).unwrap();

        assert_eq!(normalized, node);
    }

    #[test]
    fn registered_node_kind_applies_migration_defaults_and_validation() {
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind("note", RequiredTitleNodeKind);

        let mut node = CanvasNode::new("n", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        node.kind = "note".to_string();
        node.data.insert("label".to_string(), json!("Migrated"));
        let normalized = registry.normalize_node(node).unwrap();

        assert_eq!(normalized.data.get("title"), Some(&json!("Migrated")));

        let mut defaulted = CanvasNode::new(
            "defaulted",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        );
        defaulted.kind = "note".to_string();
        let defaulted = registry.normalize_node(defaulted).unwrap();

        assert_eq!(defaulted.data.get("title"), Some(&json!("Untitled")));

        let mut invalid =
            CanvasNode::new("invalid", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        invalid.kind = "note".to_string();
        invalid.data.insert("title".to_string(), json!(false));

        assert!(matches!(
            registry.normalize_node(invalid),
            Err(CanvasSchemaError::InvalidData {
                record_kind: CanvasRecordKind::Node,
                record_id: CanvasRecordId::Node(id),
                kind,
                ..
            }) if id == NodeId::from("invalid") && kind == "note"
        ));
    }

    #[test]
    fn registered_edge_and_shape_kinds_validate_data() {
        let mut registry = CanvasKindRegistry::open();
        registry.register_edge_kind("relation", RequiredRelationEdgeKind);
        registry.register_shape_kind("box", SizedShapeKind);

        let edge = edge_with_kind("relation");
        assert!(matches!(
            registry.normalize_edge(edge),
            Err(CanvasSchemaError::MissingRequiredData {
                record_kind: CanvasRecordKind::Edge,
                record_id: CanvasRecordId::Edge(id),
                field,
                ..
            }) if id == EdgeId::from("a-b") && field == "relation"
        ));

        let mut shape = CanvasShape::new(
            "shape",
            Bounds::new(point(px(0.0), px(0.0)), size(px(10.0), px(10.0))),
        );
        shape.kind = "box".to_string();
        let shape = registry.normalize_shape(shape).unwrap();

        assert_eq!(shape.data.get("shapeType"), Some(&json!("box")));
        assert_eq!(
            registry.shape_bounds(&shape).unwrap(),
            Bounds::new(point(px(-5.0), px(-5.0)), size(px(20.0), px(20.0)))
        );
    }

    #[test]
    fn registered_resize_policy_can_clamp_or_reject_bounds() {
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind("min", MinimumSizeNodeKind);
        registry.register_shape_kind("locked-size", RejectingShapeResizeKind);

        let mut node =
            CanvasNode::new("node", point(px(10.0), px(20.0)), size(px(100.0), px(80.0)));
        node.kind = "min".to_string();
        let bounds = registry
            .resize_node_bounds(
                &node,
                Bounds::new(point(px(10.0), px(20.0)), size(px(12.0), px(8.0))),
            )
            .unwrap();
        assert_eq!(
            bounds,
            Bounds::new(point(px(10.0), px(20.0)), size(px(48.0), px(32.0)))
        );

        let mut shape = CanvasShape::new(
            "shape",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(80.0))),
        );
        shape.kind = "locked-size".to_string();
        assert!(matches!(
            registry.resize_shape_bounds(&shape, shape.bounds),
            Err(CanvasSchemaError::InvalidData {
                record_kind: CanvasRecordKind::Shape,
                record_id: CanvasRecordId::Shape(id),
                kind,
                message,
            }) if id == ShapeId::from("shape")
                && kind == "locked-size"
                && message == "resize is disabled"
        ));
    }

    #[test]
    fn registered_resize_policy_output_is_validated() {
        let mut registry = CanvasKindRegistry::open();
        registry.register_shape_kind("invalid-size", InvalidShapeResizeKind);

        let mut shape = CanvasShape::new(
            "shape",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(80.0))),
        );
        shape.kind = "invalid-size".to_string();

        assert!(matches!(
            registry.resize_shape_bounds(&shape, shape.bounds),
            Err(CanvasSchemaError::InvalidData {
                record_kind: CanvasRecordKind::Shape,
                record_id: CanvasRecordId::Shape(id),
                kind,
                message,
            }) if id == ShapeId::from("shape")
                && kind == "invalid-size"
                && message == "resize policy returned invalid bounds"
        ));
    }

    #[test]
    fn registered_hit_policy_can_reject_points_inside_bounds() {
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind("right-half", RightHalfNodeKind);

        let mut node = CanvasNode::new("node", point(px(0.0), px(0.0)), size(px(100.0), px(80.0)));
        node.kind = "right-half".to_string();
        let bounds = node.bounds();

        assert_eq!(
            registry.node_contains_point(&node, point(px(25.0), px(20.0)), bounds, Pixels::ZERO),
            Some(false)
        );
        assert_eq!(
            registry.node_contains_point(&node, point(px(75.0), px(20.0)), bounds, Pixels::ZERO),
            Some(true)
        );
    }

    #[test]
    fn document_from_snapshot_runs_registered_kind_normalization() {
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind("note", RequiredTitleNodeKind);

        let mut document = CanvasDocument::default();
        let mut node = CanvasNode::new("n", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        node.kind = "note".to_string();
        node.data.insert("label".to_string(), json!("Snapshot"));
        document.insert_node(node).unwrap();

        let loaded =
            CanvasDocument::from_snapshot_with_kind_registry(document.to_snapshot(), &registry)
                .unwrap();

        assert_eq!(
            loaded.nodes[&NodeId::from("n")].data.get("title"),
            Some(&json!("Snapshot"))
        );
    }

    #[test]
    fn document_mutation_path_rejects_registered_kind_errors_atomically() {
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind("note", RequiredTitleNodeKind);
        let mut document = CanvasDocument::default();
        let mut node = CanvasNode::new("n", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        node.kind = "note".to_string();
        node.data.insert("title".to_string(), json!(false));

        let err = document
            .commit_transaction_with_kind_registry(
                CanvasTransaction::single(DocumentCommand::InsertNode(node)),
                &registry,
            )
            .unwrap_err();

        assert!(matches!(
            err,
            DocumentError::Schema(CanvasSchemaError::InvalidData { .. })
        ));
        assert!(document.nodes.is_empty());
    }

    fn edge_with_kind(kind: &str) -> CanvasEdge {
        let mut edge = CanvasEdge::new(
            "a-b",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        );
        edge.kind = kind.to_string();
        edge
    }

    #[test]
    fn record_ids_format_with_record_kind_prefix() {
        assert_eq!(
            CanvasRecordId::Node(NodeId::from("n")).to_string(),
            "node:n"
        );
        assert_eq!(
            CanvasRecordId::Edge(EdgeId::from("e")).to_string(),
            "edge:e"
        );
        assert_eq!(
            CanvasRecordId::Shape(ShapeId::from("s")).to_string(),
            "shape:s"
        );
    }
}
