use crate::{
    CanvasConnectionEndpointRole, CanvasEdge, CanvasNode, CanvasRecordId, CanvasShape,
    CanvasTransaction, CanvasValue, DocumentCommand,
};
use indexmap::IndexMap;
use open_gpui::{Bounds, Pixels, Point, Size, px};
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
pub struct CanvasNodeBoundsHitTest<'a> {
    pub node: &'a CanvasNode,
    pub bounds: Bounds<Pixels>,
    pub query_bounds: Bounds<Pixels>,
    pub margin: Pixels,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasShapeHitTest<'a> {
    pub shape: &'a CanvasShape,
    pub point: Point<Pixels>,
    pub bounds: Bounds<Pixels>,
    pub margin: Pixels,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasShapeBoundsHitTest<'a> {
    pub shape: &'a CanvasShape,
    pub bounds: Bounds<Pixels>,
    pub query_bounds: Bounds<Pixels>,
    pub margin: Pixels,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CanvasKindPaint {
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub stroke_width: Option<Pixels>,
    pub corner_radius: Option<Pixels>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasKindLabel {
    pub text: String,
    pub inset: Pixels,
    pub color: Option<String>,
    pub visible: bool,
}

impl CanvasKindLabel {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            inset: Pixels::ZERO,
            color: None,
            visible: true,
        }
    }

    pub fn with_inset(mut self, inset: Pixels) -> Self {
        self.inset = inset;
        self
    }

    pub fn with_color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn hidden(mut self) -> Self {
        self.visible = false;
        self
    }
}

pub trait CanvasNodeSchemaPolicy: Send + Sync {
    fn default_data(&self) -> CanvasValue {
        CanvasValue::new()
    }

    fn migrate_node(&self, _node: &mut CanvasNode) -> Result<(), CanvasSchemaError> {
        Ok(())
    }

    fn validate_node(&self, _node: &CanvasNode) -> Result<(), CanvasSchemaError> {
        Ok(())
    }
}

pub trait CanvasNodeGeometryPolicy: Send + Sync {
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
}

pub trait CanvasNodeInteractionPolicy: Send + Sync {
    fn node_contains_point(&self, _hit: CanvasNodeHitTest<'_>) -> Option<bool> {
        None
    }

    fn node_intersects_bounds(&self, _hit: CanvasNodeBoundsHitTest<'_>) -> Option<bool> {
        None
    }

    fn node_accepts_connection_endpoint(
        &self,
        _node: &CanvasNode,
        _role: CanvasConnectionEndpointRole,
    ) -> bool {
        false
    }
}

pub trait CanvasNodeRenderPolicy: Send + Sync {
    fn node_paint(&self, _node: &CanvasNode) -> Option<CanvasKindPaint> {
        None
    }

    fn node_label(&self, _node: &CanvasNode) -> Option<CanvasKindLabel> {
        None
    }
}

pub trait CanvasNodeTransformPolicy: Send + Sync {
    fn resize_node_bounds(
        &self,
        proposal: CanvasNodeResizeProposal<'_>,
    ) -> Result<Bounds<Pixels>, CanvasSchemaError> {
        Ok(proposal.bounds)
    }
}

pub trait CanvasEdgeSchemaPolicy: Send + Sync {
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

pub trait CanvasEdgeRenderPolicy: Send + Sync {
    fn edge_paint(&self, _edge: &CanvasEdge) -> Option<CanvasKindPaint> {
        None
    }
}

pub trait CanvasShapeSchemaPolicy: Send + Sync {
    fn default_data(&self) -> CanvasValue {
        CanvasValue::new()
    }

    fn migrate_shape(&self, _shape: &mut CanvasShape) -> Result<(), CanvasSchemaError> {
        Ok(())
    }

    fn validate_shape(&self, _shape: &CanvasShape) -> Result<(), CanvasSchemaError> {
        Ok(())
    }
}

pub trait CanvasShapeGeometryPolicy: Send + Sync {
    fn shape_bounds(&self, _shape: &CanvasShape) -> Option<Bounds<Pixels>> {
        None
    }
}

pub trait CanvasShapeInteractionPolicy: Send + Sync {
    fn shape_contains_point(&self, _hit: CanvasShapeHitTest<'_>) -> Option<bool> {
        None
    }

    fn shape_intersects_bounds(&self, _hit: CanvasShapeBoundsHitTest<'_>) -> Option<bool> {
        None
    }
}

pub trait CanvasShapeRenderPolicy: Send + Sync {
    fn shape_paint(&self, _shape: &CanvasShape) -> Option<CanvasKindPaint> {
        None
    }

    fn shape_label(&self, _shape: &CanvasShape) -> Option<CanvasKindLabel> {
        None
    }
}

pub trait CanvasShapeTransformPolicy: Send + Sync {
    fn resize_shape_bounds(
        &self,
        proposal: CanvasShapeResizeProposal<'_>,
    ) -> Result<Bounds<Pixels>, CanvasSchemaError> {
        Ok(proposal.bounds)
    }
}

#[derive(Clone, Default)]
pub struct CanvasNodeKind {
    schema: Option<Arc<dyn CanvasNodeSchemaPolicy>>,
    geometry: Option<Arc<dyn CanvasNodeGeometryPolicy>>,
    interaction: Option<Arc<dyn CanvasNodeInteractionPolicy>>,
    render: Option<Arc<dyn CanvasNodeRenderPolicy>>,
    transform: Option<Arc<dyn CanvasNodeTransformPolicy>>,
}

impl CanvasNodeKind {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_schema_policy(mut self, policy: impl CanvasNodeSchemaPolicy + 'static) -> Self {
        self.schema = Some(Arc::new(policy));
        self
    }

    pub fn with_geometry_policy(mut self, policy: impl CanvasNodeGeometryPolicy + 'static) -> Self {
        self.geometry = Some(Arc::new(policy));
        self
    }

    pub fn with_interaction_policy(
        mut self,
        policy: impl CanvasNodeInteractionPolicy + 'static,
    ) -> Self {
        self.interaction = Some(Arc::new(policy));
        self
    }

    pub fn with_render_policy(mut self, policy: impl CanvasNodeRenderPolicy + 'static) -> Self {
        self.render = Some(Arc::new(policy));
        self
    }

    pub fn with_transform_policy(
        mut self,
        policy: impl CanvasNodeTransformPolicy + 'static,
    ) -> Self {
        self.transform = Some(Arc::new(policy));
        self
    }

    fn schema_policy(&self) -> Option<&dyn CanvasNodeSchemaPolicy> {
        self.schema.as_deref()
    }

    fn geometry_policy(&self) -> Option<&dyn CanvasNodeGeometryPolicy> {
        self.geometry.as_deref()
    }

    fn interaction_policy(&self) -> Option<&dyn CanvasNodeInteractionPolicy> {
        self.interaction.as_deref()
    }

    fn render_policy(&self) -> Option<&dyn CanvasNodeRenderPolicy> {
        self.render.as_deref()
    }

    fn transform_policy(&self) -> Option<&dyn CanvasNodeTransformPolicy> {
        self.transform.as_deref()
    }
}

impl fmt::Debug for CanvasNodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CanvasNodeKind")
            .field("has_schema", &self.schema.is_some())
            .field("has_geometry", &self.geometry.is_some())
            .field("has_interaction", &self.interaction.is_some())
            .field("has_render", &self.render.is_some())
            .field("has_transform", &self.transform.is_some())
            .finish()
    }
}

#[derive(Clone, Default)]
pub struct CanvasEdgeKind {
    schema: Option<Arc<dyn CanvasEdgeSchemaPolicy>>,
    render: Option<Arc<dyn CanvasEdgeRenderPolicy>>,
}

impl CanvasEdgeKind {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_schema_policy(mut self, policy: impl CanvasEdgeSchemaPolicy + 'static) -> Self {
        self.schema = Some(Arc::new(policy));
        self
    }

    pub fn with_render_policy(mut self, policy: impl CanvasEdgeRenderPolicy + 'static) -> Self {
        self.render = Some(Arc::new(policy));
        self
    }

    fn schema_policy(&self) -> Option<&dyn CanvasEdgeSchemaPolicy> {
        self.schema.as_deref()
    }

    fn render_policy(&self) -> Option<&dyn CanvasEdgeRenderPolicy> {
        self.render.as_deref()
    }
}

impl fmt::Debug for CanvasEdgeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CanvasEdgeKind")
            .field("has_schema", &self.schema.is_some())
            .field("has_render", &self.render.is_some())
            .finish()
    }
}

#[derive(Clone, Default)]
pub struct CanvasShapeKind {
    schema: Option<Arc<dyn CanvasShapeSchemaPolicy>>,
    geometry: Option<Arc<dyn CanvasShapeGeometryPolicy>>,
    interaction: Option<Arc<dyn CanvasShapeInteractionPolicy>>,
    render: Option<Arc<dyn CanvasShapeRenderPolicy>>,
    transform: Option<Arc<dyn CanvasShapeTransformPolicy>>,
}

impl CanvasShapeKind {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_schema_policy(mut self, policy: impl CanvasShapeSchemaPolicy + 'static) -> Self {
        self.schema = Some(Arc::new(policy));
        self
    }

    pub fn with_geometry_policy(
        mut self,
        policy: impl CanvasShapeGeometryPolicy + 'static,
    ) -> Self {
        self.geometry = Some(Arc::new(policy));
        self
    }

    pub fn with_interaction_policy(
        mut self,
        policy: impl CanvasShapeInteractionPolicy + 'static,
    ) -> Self {
        self.interaction = Some(Arc::new(policy));
        self
    }

    pub fn with_render_policy(mut self, policy: impl CanvasShapeRenderPolicy + 'static) -> Self {
        self.render = Some(Arc::new(policy));
        self
    }

    pub fn with_transform_policy(
        mut self,
        policy: impl CanvasShapeTransformPolicy + 'static,
    ) -> Self {
        self.transform = Some(Arc::new(policy));
        self
    }

    fn schema_policy(&self) -> Option<&dyn CanvasShapeSchemaPolicy> {
        self.schema.as_deref()
    }

    fn geometry_policy(&self) -> Option<&dyn CanvasShapeGeometryPolicy> {
        self.geometry.as_deref()
    }

    fn interaction_policy(&self) -> Option<&dyn CanvasShapeInteractionPolicy> {
        self.interaction.as_deref()
    }

    fn render_policy(&self) -> Option<&dyn CanvasShapeRenderPolicy> {
        self.render.as_deref()
    }

    fn transform_policy(&self) -> Option<&dyn CanvasShapeTransformPolicy> {
        self.transform.as_deref()
    }
}

impl fmt::Debug for CanvasShapeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CanvasShapeKind")
            .field("has_schema", &self.schema.is_some())
            .field("has_geometry", &self.geometry.is_some())
            .field("has_interaction", &self.interaction.is_some())
            .field("has_render", &self.render.is_some())
            .field("has_transform", &self.transform.is_some())
            .finish()
    }
}

#[derive(Clone, Default)]
pub struct CanvasKindRegistry {
    node_kinds: IndexMap<String, Arc<CanvasNodeKind>>,
    edge_kinds: IndexMap<String, Arc<CanvasEdgeKind>>,
    shape_kinds: IndexMap<String, Arc<CanvasShapeKind>>,
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
    pub const GROUP_SHAPE_KIND: &'static str = "group";

    pub fn open() -> Self {
        let mut registry = Self::default();
        registry.register_shape_kind(Self::GROUP_SHAPE_KIND, built_in_group_shape_kind());
        registry
    }

    pub fn register_node_kind(&mut self, kind: impl Into<String>, node_kind: CanvasNodeKind) {
        self.node_kinds.insert(kind.into(), Arc::new(node_kind));
    }

    pub fn register_edge_kind(&mut self, kind: impl Into<String>, edge_kind: CanvasEdgeKind) {
        self.edge_kinds.insert(kind.into(), Arc::new(edge_kind));
    }

    pub fn register_shape_kind(&mut self, kind: impl Into<String>, shape_kind: CanvasShapeKind) {
        self.shape_kinds.insert(kind.into(), Arc::new(shape_kind));
    }

    pub fn node_kind(&self, kind: &str) -> Option<&CanvasNodeKind> {
        self.node_kinds.get(kind).map(Arc::as_ref)
    }

    pub fn edge_kind(&self, kind: &str) -> Option<&CanvasEdgeKind> {
        self.edge_kinds.get(kind).map(Arc::as_ref)
    }

    pub fn shape_kind(&self, kind: &str) -> Option<&CanvasShapeKind> {
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

        if let Some(schema) = schema.schema_policy() {
            schema.migrate_node(&mut node)?;
            merge_default_data(&mut node.data, schema.default_data());
            schema.validate_node(&node)?;
        }
        Ok(node)
    }

    pub fn normalize_edge(&self, mut edge: CanvasEdge) -> Result<CanvasEdge, CanvasSchemaError> {
        let Some(schema) = self.edge_kind(&edge.kind) else {
            return Ok(edge);
        };

        if let Some(schema) = schema.schema_policy() {
            schema.migrate_edge(&mut edge)?;
            merge_default_data(&mut edge.data, schema.default_data());
            schema.validate_edge(&edge)?;
        }
        Ok(edge)
    }

    pub fn normalize_shape(
        &self,
        mut shape: CanvasShape,
    ) -> Result<CanvasShape, CanvasSchemaError> {
        let Some(schema) = self.shape_kind(&shape.kind) else {
            return Ok(shape);
        };

        if let Some(schema) = schema.schema_policy() {
            schema.migrate_shape(&mut shape)?;
            merge_default_data(&mut shape.data, schema.default_data());
            schema.validate_shape(&shape)?;
        }
        Ok(shape)
    }

    pub fn validate_document(
        &self,
        document: &crate::CanvasDocument,
    ) -> Result<(), CanvasSchemaError> {
        for node in document.nodes() {
            if let Some(schema) = self
                .node_kind(&node.kind)
                .and_then(CanvasNodeKind::schema_policy)
            {
                schema.validate_node(node)?;
            }
        }

        for edge in document.edges() {
            if let Some(schema) = self
                .edge_kind(&edge.kind)
                .and_then(CanvasEdgeKind::schema_policy)
            {
                schema.validate_edge(edge)?;
            }
        }

        for shape in document.shapes() {
            if let Some(schema) = self
                .shape_kind(&shape.kind)
                .and_then(CanvasShapeKind::schema_policy)
            {
                schema.validate_shape(shape)?;
            }
        }

        Ok(())
    }

    pub fn node_bounds(&self, node: &CanvasNode) -> Option<Bounds<Pixels>> {
        self.node_kind(&node.kind)
            .and_then(CanvasNodeKind::geometry_policy)
            .and_then(|geometry| geometry.node_bounds(node))
    }

    pub fn handle_position(
        &self,
        node: &CanvasNode,
        handle_id: &crate::HandleId,
    ) -> Option<Point<Pixels>> {
        self.node_kind(&node.kind)
            .and_then(CanvasNodeKind::geometry_policy)
            .and_then(|geometry| geometry.handle_position(node, handle_id))
    }

    pub fn shape_bounds(&self, shape: &CanvasShape) -> Option<Bounds<Pixels>> {
        self.shape_kind(&shape.kind)
            .and_then(CanvasShapeKind::geometry_policy)
            .and_then(|geometry| geometry.shape_bounds(shape))
    }

    pub fn node_contains_point(
        &self,
        node: &CanvasNode,
        point: Point<Pixels>,
        bounds: Bounds<Pixels>,
        margin: Pixels,
    ) -> Option<bool> {
        self.node_kind(&node.kind)
            .and_then(CanvasNodeKind::interaction_policy)
            .and_then(|interaction| {
                interaction.node_contains_point(CanvasNodeHitTest {
                    node,
                    point,
                    bounds,
                    margin,
                })
            })
    }

    pub fn node_intersects_bounds(
        &self,
        node: &CanvasNode,
        bounds: Bounds<Pixels>,
        query_bounds: Bounds<Pixels>,
        margin: Pixels,
    ) -> Option<bool> {
        self.node_kind(&node.kind)
            .and_then(CanvasNodeKind::interaction_policy)
            .and_then(|interaction| {
                interaction.node_intersects_bounds(CanvasNodeBoundsHitTest {
                    node,
                    bounds,
                    query_bounds,
                    margin,
                })
            })
    }

    pub fn node_accepts_connection_endpoint(
        &self,
        node: &CanvasNode,
        role: CanvasConnectionEndpointRole,
    ) -> bool {
        self.node_kind(&node.kind)
            .and_then(CanvasNodeKind::interaction_policy)
            .is_some_and(|interaction| interaction.node_accepts_connection_endpoint(node, role))
    }

    pub fn shape_contains_point(
        &self,
        shape: &CanvasShape,
        point: Point<Pixels>,
        bounds: Bounds<Pixels>,
        margin: Pixels,
    ) -> Option<bool> {
        self.shape_kind(&shape.kind)
            .and_then(CanvasShapeKind::interaction_policy)
            .and_then(|interaction| {
                interaction.shape_contains_point(CanvasShapeHitTest {
                    shape,
                    point,
                    bounds,
                    margin,
                })
            })
    }

    pub fn shape_intersects_bounds(
        &self,
        shape: &CanvasShape,
        bounds: Bounds<Pixels>,
        query_bounds: Bounds<Pixels>,
        margin: Pixels,
    ) -> Option<bool> {
        self.shape_kind(&shape.kind)
            .and_then(CanvasShapeKind::interaction_policy)
            .and_then(|interaction| {
                interaction.shape_intersects_bounds(CanvasShapeBoundsHitTest {
                    shape,
                    bounds,
                    query_bounds,
                    margin,
                })
            })
    }

    pub fn node_paint(&self, node: &CanvasNode) -> Option<CanvasKindPaint> {
        self.node_kind(&node.kind)
            .and_then(CanvasNodeKind::render_policy)
            .and_then(|render| render.node_paint(node))
    }

    pub fn shape_paint(&self, shape: &CanvasShape) -> Option<CanvasKindPaint> {
        self.shape_kind(&shape.kind)
            .and_then(CanvasShapeKind::render_policy)
            .and_then(|render| render.shape_paint(shape))
    }

    pub fn edge_paint(&self, edge: &CanvasEdge) -> Option<CanvasKindPaint> {
        self.edge_kind(&edge.kind)
            .and_then(CanvasEdgeKind::render_policy)
            .and_then(|render| render.edge_paint(edge))
    }

    pub fn node_label(&self, node: &CanvasNode) -> Option<CanvasKindLabel> {
        self.node_kind(&node.kind)
            .and_then(CanvasNodeKind::render_policy)
            .and_then(|render| render.node_label(node))
            .filter(|label| label.visible && !label.text.trim().is_empty())
    }

    pub fn shape_label(&self, shape: &CanvasShape) -> Option<CanvasKindLabel> {
        self.shape_kind(&shape.kind)
            .and_then(CanvasShapeKind::render_policy)
            .and_then(|render| render.shape_label(shape))
            .filter(|label| label.visible && !label.text.trim().is_empty())
    }

    pub fn resize_node_bounds(
        &self,
        node: &CanvasNode,
        proposed: Bounds<Pixels>,
    ) -> Result<Bounds<Pixels>, CanvasSchemaError> {
        let Some(transform) = self
            .node_kind(&node.kind)
            .and_then(CanvasNodeKind::transform_policy)
        else {
            return Ok(proposed);
        };

        let bounds = transform.resize_node_bounds(CanvasNodeResizeProposal {
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
        let Some(transform) = self
            .shape_kind(&shape.kind)
            .and_then(CanvasShapeKind::transform_policy)
        else {
            return Ok(proposed);
        };

        let bounds = transform.resize_shape_bounds(CanvasShapeResizeProposal {
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

fn built_in_group_shape_kind() -> CanvasShapeKind {
    CanvasShapeKind::new().with_interaction_policy(GroupShapeKind)
}

struct GroupShapeKind;

impl CanvasShapeInteractionPolicy for GroupShapeKind {
    fn shape_contains_point(&self, hit: CanvasShapeHitTest<'_>) -> Option<bool> {
        let border = group_border_width(hit.shape, hit.margin);
        let outer = group_outer_bounds(hit.bounds, hit.margin);
        if !outer.contains(&hit.point) {
            return Some(false);
        }

        let Some(inner) = group_inner_bounds(hit.bounds, border) else {
            return Some(true);
        };
        Some(!inner.contains(&hit.point))
    }

    fn shape_intersects_bounds(&self, hit: CanvasShapeBoundsHitTest<'_>) -> Option<bool> {
        let outer = group_outer_bounds(hit.bounds, hit.margin);
        if !outer.intersects(&hit.query_bounds) {
            return Some(false);
        }

        let border = group_border_width(hit.shape, hit.margin);
        match group_inner_bounds(hit.bounds, border) {
            Some(inner) => Some(!bounds_contains_bounds(inner, hit.query_bounds)),
            None => Some(true),
        }
    }
}

fn group_border_width(shape: &CanvasShape, margin: Pixels) -> Pixels {
    shape.style.stroke_width.max(px(4.0)) + margin
}

fn group_outer_bounds(bounds: Bounds<Pixels>, margin: Pixels) -> Bounds<Pixels> {
    bounds.dilate(margin)
}

fn group_inner_bounds(bounds: Bounds<Pixels>, border: Pixels) -> Option<Bounds<Pixels>> {
    let inner_size = Size {
        width: (bounds.size.width - border * 2.0).max(Pixels::ZERO),
        height: (bounds.size.height - border * 2.0).max(Pixels::ZERO),
    };
    if inner_size.width == Pixels::ZERO || inner_size.height == Pixels::ZERO {
        return None;
    }

    Some(Bounds::new(
        Point::new(bounds.origin.x + border, bounds.origin.y + border),
        inner_size,
    ))
}

fn bounds_contains_bounds(outer: Bounds<Pixels>, inner: Bounds<Pixels>) -> bool {
    inner.origin.x >= outer.origin.x
        && inner.origin.y >= outer.origin.y
        && inner.origin.x + inner.size.width <= outer.origin.x + outer.size.width
        && inner.origin.y + inner.size.height <= outer.origin.y + outer.size.height
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
        test_support::document_fixture,
    };
    use open_gpui::{Bounds, point, px, size};
    use serde_json::{Value, json};

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
    }

    impl CanvasNodeGeometryPolicy for RequiredTitleNodeKind {
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

    impl CanvasEdgeSchemaPolicy for RequiredRelationEdgeKind {
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

    impl CanvasShapeSchemaPolicy for SizedShapeKind {
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
    }

    impl CanvasShapeGeometryPolicy for SizedShapeKind {
        fn shape_bounds(&self, shape: &CanvasShape) -> Option<Bounds<Pixels>> {
            Some(shape.bounds.dilate(px(5.0)))
        }
    }

    struct MinimumSizeNodeKind;

    impl CanvasNodeTransformPolicy for MinimumSizeNodeKind {
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

    impl CanvasShapeTransformPolicy for RejectingShapeResizeKind {
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

    impl CanvasNodeInteractionPolicy for RightHalfNodeKind {
        fn node_contains_point(&self, hit: CanvasNodeHitTest<'_>) -> Option<bool> {
            Some(hit.point.x >= hit.bounds.center().x)
        }

        fn node_intersects_bounds(&self, hit: CanvasNodeBoundsHitTest<'_>) -> Option<bool> {
            let active = Bounds::from_corners(
                Point::new(hit.bounds.center().x, hit.bounds.origin.y),
                Point::new(
                    hit.bounds.origin.x + hit.bounds.size.width,
                    hit.bounds.origin.y + hit.bounds.size.height,
                ),
            )
            .dilate(hit.margin);
            Some(active.intersects(&hit.query_bounds))
        }
    }

    struct TopHalfShapeKind;

    impl CanvasShapeInteractionPolicy for TopHalfShapeKind {
        fn shape_contains_point(&self, hit: CanvasShapeHitTest<'_>) -> Option<bool> {
            Some(hit.point.y <= hit.bounds.center().y)
        }

        fn shape_intersects_bounds(&self, hit: CanvasShapeBoundsHitTest<'_>) -> Option<bool> {
            let active = Bounds::from_corners(
                hit.bounds.origin,
                Point::new(
                    hit.bounds.origin.x + hit.bounds.size.width,
                    hit.bounds.center().y,
                ),
            )
            .dilate(hit.margin);
            Some(active.intersects(&hit.query_bounds))
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

    struct HiddenLabelNodeKind;

    impl CanvasNodeRenderPolicy for HiddenLabelNodeKind {
        fn node_label(&self, _node: &CanvasNode) -> Option<CanvasKindLabel> {
            Some(CanvasKindLabel::new("Hidden").hidden())
        }
    }

    struct EmptyLabelShapeKind;

    impl CanvasShapeRenderPolicy for EmptyLabelShapeKind {
        fn shape_label(&self, _shape: &CanvasShape) -> Option<CanvasKindLabel> {
            Some(CanvasKindLabel::new("   "))
        }
    }

    impl CanvasShapeTransformPolicy for InvalidShapeResizeKind {
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

    fn required_title_node_kind() -> CanvasNodeKind {
        CanvasNodeKind::new()
            .with_schema_policy(RequiredTitleNodeKind)
            .with_geometry_policy(RequiredTitleNodeKind)
    }

    fn relation_edge_kind() -> CanvasEdgeKind {
        CanvasEdgeKind::new().with_schema_policy(RequiredRelationEdgeKind)
    }

    fn sized_shape_kind() -> CanvasShapeKind {
        CanvasShapeKind::new()
            .with_schema_policy(SizedShapeKind)
            .with_geometry_policy(SizedShapeKind)
    }

    fn minimum_size_node_kind() -> CanvasNodeKind {
        CanvasNodeKind::new().with_transform_policy(MinimumSizeNodeKind)
    }

    fn rejecting_shape_resize_kind() -> CanvasShapeKind {
        CanvasShapeKind::new().with_transform_policy(RejectingShapeResizeKind)
    }

    fn invalid_shape_resize_kind() -> CanvasShapeKind {
        CanvasShapeKind::new().with_transform_policy(InvalidShapeResizeKind)
    }

    fn right_half_node_kind() -> CanvasNodeKind {
        CanvasNodeKind::new().with_interaction_policy(RightHalfNodeKind)
    }

    fn top_half_shape_kind() -> CanvasShapeKind {
        CanvasShapeKind::new().with_interaction_policy(TopHalfShapeKind)
    }

    fn painted_node_kind() -> CanvasNodeKind {
        CanvasNodeKind::new().with_render_policy(PaintedNodeKind)
    }

    fn painted_edge_kind() -> CanvasEdgeKind {
        CanvasEdgeKind::new().with_render_policy(PaintedEdgeKind)
    }

    fn painted_shape_kind() -> CanvasShapeKind {
        CanvasShapeKind::new().with_render_policy(PaintedShapeKind)
    }

    fn hidden_label_node_kind() -> CanvasNodeKind {
        CanvasNodeKind::new().with_render_policy(HiddenLabelNodeKind)
    }

    fn empty_label_shape_kind() -> CanvasShapeKind {
        CanvasShapeKind::new().with_render_policy(EmptyLabelShapeKind)
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
        registry.register_node_kind("note", required_title_node_kind());

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
    fn node_policy_categories_are_independent() {
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind(
            "schema-only",
            CanvasNodeKind::new().with_schema_policy(RequiredTitleNodeKind),
        );
        registry.register_node_kind(
            "geometry-only",
            CanvasNodeKind::new().with_geometry_policy(RequiredTitleNodeKind),
        );
        registry.register_node_kind("render-only", painted_node_kind());
        registry.register_node_kind("transform-only", minimum_size_node_kind());
        registry.register_node_kind("interaction-only", right_half_node_kind());

        let mut schema_node =
            CanvasNode::new("schema", point(px(0.0), px(0.0)), size(px(100.0), px(80.0)));
        schema_node.kind = "schema-only".to_string();
        let normalized = registry.normalize_node(schema_node.clone()).unwrap();
        assert_eq!(normalized.data.get("title"), Some(&json!("Untitled")));
        assert_eq!(registry.node_bounds(&normalized), None);
        assert_eq!(registry.node_paint(&normalized), None);

        let mut geometry_node = CanvasNode::new(
            "geometry",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(80.0)),
        );
        geometry_node.kind = "geometry-only".to_string();
        assert_eq!(
            registry.normalize_node(geometry_node.clone()).unwrap(),
            geometry_node
        );
        assert_eq!(
            registry.node_bounds(&geometry_node),
            Some(geometry_node.bounds().dilate(px(10.0)))
        );
        assert_eq!(registry.node_paint(&geometry_node), None);

        let mut render_node =
            CanvasNode::new("render", point(px(0.0), px(0.0)), size(px(100.0), px(80.0)));
        render_node.kind = "render-only".to_string();
        assert!(registry.node_paint(&render_node).is_some());
        assert_eq!(registry.node_bounds(&render_node), None);

        let mut transform_node = CanvasNode::new(
            "transform",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(80.0)),
        );
        transform_node.kind = "transform-only".to_string();
        assert_eq!(
            registry
                .resize_node_bounds(
                    &transform_node,
                    Bounds::new(point(px(0.0), px(0.0)), size(px(1.0), px(1.0))),
                )
                .unwrap()
                .size,
            size(px(48.0), px(32.0))
        );
        assert_eq!(registry.node_paint(&transform_node), None);

        let mut interaction_node = CanvasNode::new(
            "interaction",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(80.0)),
        );
        interaction_node.kind = "interaction-only".to_string();
        assert_eq!(
            registry.node_contains_point(
                &interaction_node,
                point(px(25.0), px(20.0)),
                interaction_node.bounds(),
                Pixels::ZERO,
            ),
            Some(false)
        );
        assert_eq!(registry.node_bounds(&interaction_node), None);
    }

    #[test]
    fn registered_edge_and_shape_kinds_validate_data() {
        let mut registry = CanvasKindRegistry::open();
        registry.register_edge_kind("relation", relation_edge_kind());
        registry.register_shape_kind("box", sized_shape_kind());

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
    fn edge_and_shape_policy_categories_are_independent() {
        let mut registry = CanvasKindRegistry::open();
        registry.register_edge_kind(
            "edge-schema",
            CanvasEdgeKind::new().with_schema_policy(RequiredRelationEdgeKind),
        );
        registry.register_edge_kind("edge-render", painted_edge_kind());
        registry.register_shape_kind(
            "shape-schema",
            CanvasShapeKind::new().with_schema_policy(SizedShapeKind),
        );
        registry.register_shape_kind(
            "shape-geometry",
            CanvasShapeKind::new().with_geometry_policy(SizedShapeKind),
        );
        registry.register_shape_kind("shape-render", painted_shape_kind());
        registry.register_shape_kind("shape-transform", rejecting_shape_resize_kind());
        registry.register_shape_kind("shape-interaction", top_half_shape_kind());

        let mut edge = edge_with_kind("edge-schema");
        assert!(matches!(
            registry.normalize_edge(edge.clone()),
            Err(CanvasSchemaError::MissingRequiredData { .. })
        ));
        edge.data.insert("relation".to_string(), json!("uses"));
        assert_eq!(registry.normalize_edge(edge.clone()).unwrap(), edge);
        assert_eq!(registry.edge_paint(&edge), None);

        let render_edge = edge_with_kind("edge-render");
        assert!(registry.edge_paint(&render_edge).is_some());
        assert_eq!(
            registry.normalize_edge(render_edge.clone()).unwrap(),
            render_edge
        );

        let mut schema_shape = CanvasShape::new(
            "schema-shape",
            Bounds::new(point(px(0.0), px(0.0)), size(px(10.0), px(10.0))),
        );
        schema_shape.kind = "shape-schema".to_string();
        let normalized = registry.normalize_shape(schema_shape).unwrap();
        assert_eq!(normalized.data.get("shapeType"), Some(&json!("box")));
        assert_eq!(registry.shape_bounds(&normalized), None);

        let mut geometry_shape = CanvasShape::new(
            "geometry-shape",
            Bounds::new(point(px(0.0), px(0.0)), size(px(10.0), px(10.0))),
        );
        geometry_shape.kind = "shape-geometry".to_string();
        assert_eq!(
            registry.normalize_shape(geometry_shape.clone()).unwrap(),
            geometry_shape
        );
        assert_eq!(
            registry.shape_bounds(&geometry_shape),
            Some(Bounds::new(
                point(px(-5.0), px(-5.0)),
                size(px(20.0), px(20.0))
            ))
        );

        let mut render_shape = CanvasShape::new(
            "render-shape",
            Bounds::new(point(px(0.0), px(0.0)), size(px(10.0), px(10.0))),
        );
        render_shape.kind = "shape-render".to_string();
        assert!(registry.shape_paint(&render_shape).is_some());
        assert_eq!(registry.shape_bounds(&render_shape), None);

        let mut transform_shape = CanvasShape::new(
            "transform-shape",
            Bounds::new(point(px(0.0), px(0.0)), size(px(10.0), px(10.0))),
        );
        transform_shape.kind = "shape-transform".to_string();
        assert!(matches!(
            registry.resize_shape_bounds(&transform_shape, transform_shape.bounds),
            Err(CanvasSchemaError::InvalidData { message, .. }) if message == "resize is disabled"
        ));
        assert_eq!(registry.shape_paint(&transform_shape), None);

        let mut interaction_shape = CanvasShape::new(
            "interaction-shape",
            Bounds::new(point(px(0.0), px(0.0)), size(px(10.0), px(10.0))),
        );
        interaction_shape.kind = "shape-interaction".to_string();
        assert_eq!(
            registry.shape_contains_point(
                &interaction_shape,
                point(px(5.0), px(2.0)),
                interaction_shape.bounds,
                Pixels::ZERO,
            ),
            Some(true)
        );
        assert_eq!(
            registry.shape_contains_point(
                &interaction_shape,
                point(px(5.0), px(8.0)),
                interaction_shape.bounds,
                Pixels::ZERO,
            ),
            Some(false)
        );
        assert_eq!(
            registry.shape_intersects_bounds(
                &interaction_shape,
                interaction_shape.bounds,
                Bounds::new(point(px(2.0), px(1.0)), size(px(4.0), px(2.0))),
                Pixels::ZERO,
            ),
            Some(true)
        );
        assert_eq!(
            registry.shape_intersects_bounds(
                &interaction_shape,
                interaction_shape.bounds,
                Bounds::new(point(px(2.0), px(8.0)), size(px(4.0), px(2.0))),
                Pixels::ZERO,
            ),
            Some(false)
        );
        assert_eq!(registry.shape_paint(&interaction_shape), None);
    }

    #[test]
    fn built_in_group_shape_kind_hits_border_but_not_interior() {
        let registry = CanvasKindRegistry::open();
        let mut group = CanvasShape::new(
            "group",
            Bounds::new(point(px(10.0), px(20.0)), size(px(100.0), px(80.0))),
        );
        group.kind = CanvasKindRegistry::GROUP_SHAPE_KIND.to_string();
        group.style.stroke_width = px(1.0);

        assert_eq!(
            registry.shape_contains_point(
                &group,
                point(px(12.0), px(22.0)),
                group.bounds,
                Pixels::ZERO,
            ),
            Some(true)
        );
        assert_eq!(
            registry.shape_contains_point(
                &group,
                point(px(60.0), px(60.0)),
                group.bounds,
                Pixels::ZERO,
            ),
            Some(false)
        );
        assert_eq!(
            registry.shape_contains_point(&group, point(px(60.0), px(60.0)), group.bounds, px(8.0)),
            Some(false)
        );
        assert_eq!(
            registry.shape_intersects_bounds(
                &group,
                group.bounds,
                Bounds::new(point(px(40.0), px(40.0)), size(px(20.0), px(20.0))),
                Pixels::ZERO,
            ),
            Some(false)
        );
        assert_eq!(
            registry.shape_intersects_bounds(
                &group,
                group.bounds,
                Bounds::new(point(px(8.0), px(40.0)), size(px(10.0), px(20.0))),
                Pixels::ZERO,
            ),
            Some(true)
        );
    }

    #[test]
    fn registered_resize_policy_can_clamp_or_reject_bounds() {
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind("min", minimum_size_node_kind());
        registry.register_shape_kind("locked-size", rejecting_shape_resize_kind());

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
        registry.register_shape_kind("invalid-size", invalid_shape_resize_kind());

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
        registry.register_node_kind("right-half", right_half_node_kind());

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
    fn registered_paint_policy_supplies_renderer_neutral_defaults() {
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind("painted-node", painted_node_kind());
        registry.register_edge_kind("painted-edge", painted_edge_kind());
        registry.register_shape_kind("painted-shape", painted_shape_kind());
        registry.register_node_kind("hidden-label", hidden_label_node_kind());
        registry.register_shape_kind("empty-label", empty_label_shape_kind());

        let mut node = CanvasNode::new("node", point(px(0.0), px(0.0)), size(px(100.0), px(80.0)));
        node.kind = "painted-node".to_string();
        assert_eq!(
            registry.node_paint(&node),
            Some(CanvasKindPaint {
                fill: Some("#fff8c5".to_string()),
                stroke: Some("#bf8700".to_string()),
                stroke_width: Some(px(2.0)),
                corner_radius: Some(px(10.0)),
            })
        );
        assert_eq!(
            registry.node_label(&node),
            Some(
                CanvasKindLabel::new("Node label")
                    .with_inset(px(8.0))
                    .with_color("#24292f")
            )
        );

        let mut edge = CanvasEdge::new(
            "edge",
            CanvasEndpoint::new("source", None::<&str>),
            CanvasEndpoint::new("target", None::<&str>),
        );
        edge.kind = "painted-edge".to_string();
        assert_eq!(
            registry.edge_paint(&edge),
            Some(CanvasKindPaint {
                fill: None,
                stroke: Some("#d1242f".to_string()),
                stroke_width: Some(px(5.0)),
                corner_radius: None,
            })
        );

        let mut shape = CanvasShape::new(
            "shape",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(80.0))),
        );
        shape.kind = "painted-shape".to_string();
        assert_eq!(
            registry.shape_paint(&shape),
            Some(CanvasKindPaint {
                fill: Some("#ddf4ff".to_string()),
                stroke: Some("#0969da".to_string()),
                stroke_width: Some(px(3.0)),
                corner_radius: Some(px(4.0)),
            })
        );
        assert_eq!(
            registry.shape_label(&shape),
            Some(
                CanvasKindLabel::new("Shape label")
                    .with_inset(px(4.0))
                    .with_color("#0969da")
            )
        );

        let mut hidden_label_node =
            CanvasNode::new("hidden", point(px(0.0), px(0.0)), size(px(100.0), px(80.0)));
        hidden_label_node.kind = "hidden-label".to_string();
        assert_eq!(registry.node_label(&hidden_label_node), None);

        let mut empty_label_shape = CanvasShape::new(
            "empty",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(80.0))),
        );
        empty_label_shape.kind = "empty-label".to_string();
        assert_eq!(registry.shape_label(&empty_label_shape), None);

        node.kind = "unknown".to_string();
        assert_eq!(registry.node_paint(&node), None);
        assert_eq!(registry.node_label(&node), None);
        edge.kind = "unknown".to_string();
        assert_eq!(registry.edge_paint(&edge), None);
        shape.kind = "unknown".to_string();
        assert_eq!(registry.shape_label(&shape), None);
    }

    #[test]
    fn document_from_snapshot_runs_registered_kind_normalization() {
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind("note", required_title_node_kind());

        let mut node = CanvasNode::new("n", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        node.kind = "note".to_string();
        node.data.insert("label".to_string(), json!("Snapshot"));
        let document = document_fixture().node(node).build();

        let loaded =
            CanvasDocument::from_snapshot_with_kind_registry(document.to_snapshot(), &registry)
                .unwrap();

        assert_eq!(
            loaded.node(&NodeId::from("n")).unwrap().data.get("title"),
            Some(&json!("Snapshot"))
        );
    }

    #[test]
    fn document_mutation_path_rejects_registered_kind_errors_atomically() {
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind("note", required_title_node_kind());
        let mut node = CanvasNode::new("n", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        node.kind = "note".to_string();
        node.data.insert("title".to_string(), json!(false));
        let mut document = document_fixture().build();

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
        assert_eq!(document.node_count(), 0);
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
