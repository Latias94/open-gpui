use indexmap::{IndexMap, IndexSet};
use open_gpui::{Bounds, Pixels, Point, Size};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fmt;
use thiserror::Error;

use crate::format::{
    CANVAS_DOCUMENT_FORMAT_VERSION, default_document_format_version, migrate_canvas_snapshot,
};
use crate::relations::{CanvasRecordBindingRelation, CanvasRecordRelations};
use crate::schema::{CanvasKindRegistry, CanvasSchemaError};

mod builder;
mod commands;
mod diff;
mod geometry;
mod relations;
mod snapshot;
mod validation;

pub use builder::CanvasDocumentBuilder;

pub type CanvasValue = Map<String, Value>;

macro_rules! canvas_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self::new(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

canvas_id!(NodeId);
canvas_id!(EdgeId);
canvas_id!(ShapeId);
canvas_id!(HandleId);
canvas_id!(BindingId);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CanvasEdgeRouteKind(String);

impl CanvasEdgeRouteKind {
    pub const STRAIGHT: &'static str = "straight";
    pub const POLYLINE: &'static str = "polyline";
    pub const ORTHOGONAL: &'static str = "orthogonal";
    pub const CUBIC_BEZIER: &'static str = "cubic-bezier";

    pub fn new(kind: impl Into<String>) -> Self {
        Self(kind.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for CanvasEdgeRouteKind {
    fn default() -> Self {
        Self::new(Self::STRAIGHT)
    }
}

impl From<&str> for CanvasEdgeRouteKind {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for CanvasEdgeRouteKind {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for CanvasEdgeRouteKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum CanvasRecordId {
    Node(NodeId),
    Edge(EdgeId),
    Shape(ShapeId),
}

impl fmt::Display for CanvasRecordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Node(id) => write!(f, "node:{id}"),
            Self::Edge(id) => write!(f, "edge:{id}"),
            Self::Shape(id) => write!(f, "shape:{id}"),
        }
    }
}

impl From<NodeId> for CanvasRecordId {
    fn from(value: NodeId) -> Self {
        Self::Node(value)
    }
}

impl From<EdgeId> for CanvasRecordId {
    fn from(value: EdgeId) -> Self {
        Self::Edge(value)
    }
}

impl From<ShapeId> for CanvasRecordId {
    fn from(value: ShapeId) -> Self {
        Self::Shape(value)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum HandleRole {
    #[default]
    Any,
    Source,
    Target,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CanvasConnectionEndpointRole {
    Source,
    Target,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasStyle {
    #[serde(default)]
    pub fill: Option<String>,
    #[serde(default)]
    pub stroke: Option<String>,
    #[serde(default)]
    pub stroke_width: Pixels,
}

impl Default for CanvasStyle {
    fn default() -> Self {
        Self {
            fill: None,
            stroke: None,
            stroke_width: Pixels::ZERO,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasHandle {
    pub id: HandleId,
    #[serde(default)]
    pub role: HandleRole,
    pub position: Point<Pixels>,
    #[serde(default = "default_handle_size")]
    pub size: Size<Pixels>,
    #[serde(default = "default_true")]
    pub connectable: bool,
    #[serde(default)]
    pub hidden: bool,
}

impl CanvasHandle {
    pub fn new(id: impl Into<HandleId>, position: Point<Pixels>) -> Self {
        Self {
            id: id.into(),
            role: HandleRole::Any,
            position,
            size: default_handle_size(),
            connectable: true,
            hidden: false,
        }
    }

    pub fn bounds_in_node(&self) -> Bounds<Pixels> {
        Bounds::centered_at(self.position, self.size)
    }

    pub fn bounds_in_document(&self, node: &CanvasNode) -> Bounds<Pixels> {
        let local = self.bounds_in_node();
        Bounds::new(node.position + local.origin, local.size)
    }

    pub fn accepts_connection_role(&self, role: CanvasConnectionEndpointRole) -> bool {
        self.connectable
            && match role {
                CanvasConnectionEndpointRole::Source => self.role != HandleRole::Target,
                CanvasConnectionEndpointRole::Target => self.role != HandleRole::Source,
            }
    }

    pub fn is_pickable_connection_endpoint(&self, role: CanvasConnectionEndpointRole) -> bool {
        !self.hidden && self.accepts_connection_role(role)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasNode {
    pub id: NodeId,
    #[serde(default = "default_kind")]
    pub kind: String,
    pub position: Point<Pixels>,
    pub size: Size<Pixels>,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub handles: Vec<CanvasHandle>,
    #[serde(default)]
    pub data: CanvasValue,
    #[serde(default)]
    pub style: CanvasStyle,
}

impl CanvasNode {
    pub fn new(id: impl Into<NodeId>, position: Point<Pixels>, size: Size<Pixels>) -> Self {
        Self {
            id: id.into(),
            kind: default_kind(),
            position,
            size,
            z_index: 0,
            hidden: false,
            locked: false,
            handles: Vec::new(),
            data: CanvasValue::new(),
            style: CanvasStyle::default(),
        }
    }

    pub fn bounds(&self) -> Bounds<Pixels> {
        Bounds::new(self.position, self.size)
    }

    pub fn handle(&self, id: Option<&HandleId>) -> Option<&CanvasHandle> {
        match id {
            Some(id) => self.handles.iter().find(|handle| &handle.id == id),
            None => self.handles.first(),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct CanvasEndpoint {
    pub node_id: NodeId,
    #[serde(default)]
    pub handle_id: Option<HandleId>,
}

impl CanvasEndpoint {
    pub fn new(node_id: impl Into<NodeId>, handle_id: Option<impl Into<HandleId>>) -> Self {
        Self {
            node_id: node_id.into(),
            handle_id: handle_id.map(Into::into),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasEdgeRoute {
    #[serde(default)]
    pub kind: CanvasEdgeRouteKind,
    #[serde(default)]
    pub waypoints: Vec<Point<Pixels>>,
    #[serde(default)]
    pub control_points: Vec<Point<Pixels>>,
    #[serde(default = "default_edge_interaction_width")]
    pub interaction_width: Pixels,
    #[serde(default)]
    pub options: CanvasValue,
}

impl CanvasEdgeRoute {
    pub fn new(kind: impl Into<CanvasEdgeRouteKind>) -> Self {
        Self {
            kind: kind.into(),
            ..Self::default()
        }
    }

    pub fn straight() -> Self {
        Self::new(CanvasEdgeRouteKind::STRAIGHT)
    }

    pub fn polyline(waypoints: impl IntoIterator<Item = Point<Pixels>>) -> Self {
        Self {
            kind: CanvasEdgeRouteKind::new(CanvasEdgeRouteKind::POLYLINE),
            waypoints: waypoints.into_iter().collect(),
            ..Self::default()
        }
    }

    pub fn orthogonal() -> Self {
        Self::new(CanvasEdgeRouteKind::ORTHOGONAL)
    }
}

impl Default for CanvasEdgeRoute {
    fn default() -> Self {
        Self {
            kind: CanvasEdgeRouteKind::default(),
            waypoints: Vec::new(),
            control_points: Vec::new(),
            interaction_width: default_edge_interaction_width(),
            options: CanvasValue::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasEdge {
    pub id: EdgeId,
    #[serde(default = "default_kind")]
    pub kind: String,
    pub source: CanvasEndpoint,
    pub target: CanvasEndpoint,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub data: CanvasValue,
    #[serde(default)]
    pub style: CanvasStyle,
    #[serde(default)]
    pub route: CanvasEdgeRoute,
}

impl CanvasEdge {
    pub fn new(id: impl Into<EdgeId>, source: CanvasEndpoint, target: CanvasEndpoint) -> Self {
        Self {
            id: id.into(),
            kind: default_kind(),
            source,
            target,
            z_index: 0,
            hidden: false,
            locked: false,
            data: CanvasValue::new(),
            style: CanvasStyle::default(),
            route: CanvasEdgeRoute::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasShape {
    pub id: ShapeId,
    #[serde(default = "default_kind")]
    pub kind: String,
    pub bounds: Bounds<Pixels>,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub data: CanvasValue,
    #[serde(default)]
    pub style: CanvasStyle,
}

impl CanvasShape {
    pub fn new(id: impl Into<ShapeId>, bounds: Bounds<Pixels>) -> Self {
        Self {
            id: id.into(),
            kind: default_kind(),
            bounds,
            z_index: 0,
            hidden: false,
            locked: false,
            data: CanvasValue::new(),
            style: CanvasStyle::default(),
        }
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum DocumentError {
    #[error("unsupported canvas document format version `{found}`, expected `{expected}`")]
    UnsupportedFormatVersion { expected: u32, found: u32 },
    #[error("node `{0}` already exists")]
    DuplicateNode(NodeId),
    #[error("edge `{0}` already exists")]
    DuplicateEdge(EdgeId),
    #[error("shape `{0}` already exists")]
    DuplicateShape(ShapeId),
    #[error("node `{0}` was not found")]
    MissingNode(NodeId),
    #[error("edge `{0}` was not found")]
    MissingEdge(EdgeId),
    #[error("shape `{0}` was not found")]
    MissingShape(ShapeId),
    #[error("handle `{handle_id}` was not found on node `{node_id}`")]
    MissingHandle {
        node_id: NodeId,
        handle_id: HandleId,
    },
    #[error("handle `{handle_id}` already exists on node `{node_id}`")]
    DuplicateHandle {
        node_id: NodeId,
        handle_id: HandleId,
    },
    #[error("handle `{handle_id}` on node `{node_id}` is not connectable")]
    NonConnectableHandle {
        node_id: NodeId,
        handle_id: HandleId,
    },
    #[error("handle `{handle_id}` on node `{node_id}` cannot be used as an edge source")]
    InvalidSourceHandle {
        node_id: NodeId,
        handle_id: HandleId,
    },
    #[error("handle `{handle_id}` on node `{node_id}` cannot be used as an edge target")]
    InvalidTargetHandle {
        node_id: NodeId,
        handle_id: HandleId,
    },
    #[error("edge `{0}` has an empty route kind")]
    EmptyEdgeRouteKind(EdgeId),
    #[error("edge `{0}` has an invalid route interaction width")]
    InvalidEdgeInteractionWidth(EdgeId),
    #[error("edge `{0}` has an invalid route point")]
    InvalidEdgeRoutePoint(EdgeId),
    #[error("canvas relation references missing record `{0}`")]
    MissingRelationRecord(CanvasRecordId),
    #[error("canvas record `{0}` cannot be its own parent")]
    SelfParentRelation(CanvasRecordId),
    #[error("canvas record relation cycle includes `{0}`")]
    CyclicRecordRelation(CanvasRecordId),
    #[error("canvas record `{0}` has more than one parent relation")]
    DuplicateParentRelation(CanvasRecordId),
    #[error("canvas group relation from `{group}` to `{member}` is duplicated")]
    DuplicateGroupRelation {
        group: CanvasRecordId,
        member: CanvasRecordId,
    },
    #[error("canvas binding relation `{0}` is duplicated")]
    DuplicateBindingRelation(BindingId),
    #[error("canvas record `{0}` cannot be bound to itself")]
    SelfBindingRelation(CanvasRecordId),
    #[error(transparent)]
    Schema(#[from] CanvasSchemaError),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DocumentCommand {
    InsertNode(CanvasNode),
    UpdateNode(CanvasNode),
    RemoveNode(NodeId),
    InsertEdge(CanvasEdge),
    UpdateEdge(CanvasEdge),
    RemoveEdge(EdgeId),
    InsertShape(CanvasShape),
    UpdateShape(CanvasShape),
    RemoveShape(ShapeId),
    SetRecordParent {
        child: CanvasRecordId,
        parent: CanvasRecordId,
    },
    ClearRecordParent {
        child: CanvasRecordId,
    },
    AddRecordToGroup {
        group: CanvasRecordId,
        member: CanvasRecordId,
    },
    RemoveRecordFromGroup {
        group: CanvasRecordId,
        member: CanvasRecordId,
    },
    SetRecordBinding(CanvasRecordBindingRelation),
    RemoveRecordBinding {
        id: BindingId,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CanvasDocumentDiff {
    #[serde(default)]
    pub inserted: IndexSet<CanvasRecordId>,
    #[serde(default)]
    pub updated: IndexSet<CanvasRecordId>,
    #[serde(default)]
    pub removed: IndexSet<CanvasRecordId>,
    #[serde(default)]
    pub metadata_changed: bool,
    #[serde(default)]
    pub relations_changed: bool,
}

impl CanvasDocumentDiff {
    pub fn is_empty(&self) -> bool {
        self.inserted.is_empty()
            && self.updated.is_empty()
            && self.removed.is_empty()
            && !self.metadata_changed
            && !self.relations_changed
    }

    pub fn record_insert(&mut self, id: impl Into<CanvasRecordId>) {
        let id = id.into();
        if self.removed.shift_remove(&id) {
            self.updated.insert(id);
        } else {
            self.inserted.insert(id);
        }
    }

    pub fn record_update(&mut self, id: impl Into<CanvasRecordId>) {
        let id = id.into();
        if !self.inserted.contains(&id) && !self.removed.contains(&id) {
            self.updated.insert(id);
        }
    }

    pub fn record_remove(&mut self, id: impl Into<CanvasRecordId>) {
        let id = id.into();
        if self.inserted.shift_remove(&id) {
            self.updated.shift_remove(&id);
        } else {
            self.updated.shift_remove(&id);
            self.removed.insert(id);
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CanvasTransaction {
    #[serde(default)]
    pub commands: Vec<DocumentCommand>,
    #[serde(default)]
    pub metadata: CanvasValue,
}

impl CanvasTransaction {
    pub fn new(commands: impl IntoIterator<Item = DocumentCommand>) -> Self {
        Self {
            commands: commands.into_iter().collect(),
            metadata: CanvasValue::new(),
        }
    }

    pub fn single(command: DocumentCommand) -> Self {
        Self::new([command])
    }

    pub fn push(&mut self, command: DocumentCommand) {
        self.commands.push(command);
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

impl From<DocumentCommand> for CanvasTransaction {
    fn from(value: DocumentCommand) -> Self {
        Self::single(value)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasSnapshot {
    #[serde(default = "default_document_format_version")]
    pub format_version: u32,
    #[serde(default)]
    pub nodes: Vec<CanvasNode>,
    #[serde(default)]
    pub edges: Vec<CanvasEdge>,
    #[serde(default)]
    pub shapes: Vec<CanvasShape>,
    #[serde(default)]
    pub metadata: CanvasValue,
    #[serde(default)]
    pub relations: CanvasRecordRelations,
}

impl Default for CanvasSnapshot {
    fn default() -> Self {
        Self {
            format_version: CANVAS_DOCUMENT_FORMAT_VERSION,
            nodes: Vec::new(),
            edges: Vec::new(),
            shapes: Vec::new(),
            metadata: CanvasValue::new(),
            relations: CanvasRecordRelations::default(),
        }
    }
}

impl CanvasSnapshot {
    pub fn migrate_to_current(self) -> Result<Self, DocumentError> {
        migrate_canvas_snapshot(self)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasDocument {
    #[serde(default = "default_document_format_version")]
    format_version: u32,
    #[serde(default)]
    nodes: IndexMap<NodeId, CanvasNode>,
    #[serde(default)]
    edges: IndexMap<EdgeId, CanvasEdge>,
    #[serde(default)]
    shapes: IndexMap<ShapeId, CanvasShape>,
    #[serde(default)]
    metadata: CanvasValue,
    #[serde(default)]
    relations: CanvasRecordRelations,
}

impl Default for CanvasDocument {
    fn default() -> Self {
        Self {
            format_version: CANVAS_DOCUMENT_FORMAT_VERSION,
            nodes: IndexMap::new(),
            edges: IndexMap::new(),
            shapes: IndexMap::new(),
            metadata: CanvasValue::new(),
            relations: CanvasRecordRelations::default(),
        }
    }
}

impl CanvasDocument {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn builder() -> CanvasDocumentBuilder {
        CanvasDocumentBuilder::new()
    }

    pub fn format_version(&self) -> u32 {
        self.format_version
    }

    pub fn metadata(&self) -> &CanvasValue {
        &self.metadata
    }

    pub fn relations(&self) -> &CanvasRecordRelations {
        &self.relations
    }

    pub fn node(&self, id: &NodeId) -> Option<&CanvasNode> {
        self.nodes.get(id)
    }

    pub fn edge(&self, id: &EdgeId) -> Option<&CanvasEdge> {
        self.edges.get(id)
    }

    pub fn shape(&self, id: &ShapeId) -> Option<&CanvasShape> {
        self.shapes.get(id)
    }

    pub fn contains_node(&self, id: &NodeId) -> bool {
        self.nodes.contains_key(id)
    }

    pub fn contains_edge(&self, id: &EdgeId) -> bool {
        self.edges.contains_key(id)
    }

    pub fn contains_shape(&self, id: &ShapeId) -> bool {
        self.shapes.contains_key(id)
    }

    pub fn contains_record(&self, id: &CanvasRecordId) -> bool {
        match id {
            CanvasRecordId::Node(id) => self.nodes.contains_key(id),
            CanvasRecordId::Edge(id) => self.edges.contains_key(id),
            CanvasRecordId::Shape(id) => self.shapes.contains_key(id),
        }
    }

    pub fn nodes(&self) -> impl Iterator<Item = &CanvasNode> + '_ {
        self.nodes.values()
    }

    pub fn edges(&self) -> impl Iterator<Item = &CanvasEdge> + '_ {
        self.edges.values()
    }

    pub fn shapes(&self) -> impl Iterator<Item = &CanvasShape> + '_ {
        self.shapes.values()
    }

    pub fn node_entries(&self) -> impl Iterator<Item = (&NodeId, &CanvasNode)> + '_ {
        self.nodes.iter()
    }

    pub fn edge_entries(&self) -> impl Iterator<Item = (&EdgeId, &CanvasEdge)> + '_ {
        self.edges.iter()
    }

    pub fn shape_entries(&self) -> impl Iterator<Item = (&ShapeId, &CanvasShape)> + '_ {
        self.shapes.iter()
    }

    pub fn node_ids(&self) -> impl Iterator<Item = &NodeId> + '_ {
        self.nodes.keys()
    }

    pub fn edge_ids(&self) -> impl Iterator<Item = &EdgeId> + '_ {
        self.edges.keys()
    }

    pub fn shape_ids(&self) -> impl Iterator<Item = &ShapeId> + '_ {
        self.shapes.keys()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn shape_count(&self) -> usize {
        self.shapes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.edges.is_empty() && self.shapes.is_empty()
    }
}

fn default_true() -> bool {
    true
}

fn default_kind() -> String {
    "default".to_string()
}

fn default_handle_size() -> Size<Pixels> {
    Size::new(Pixels::from(12.0), Pixels::from(12.0))
}

fn default_edge_interaction_width() -> Pixels {
    Pixels::from(12.0)
}

#[cfg(test)]
mod tests;
