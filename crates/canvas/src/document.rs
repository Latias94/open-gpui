use indexmap::IndexMap;
use open_gpui::{Bounds, Pixels, Point, Size};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fmt;
use thiserror::Error;

pub type CanvasValue = Map<String, Value>;

pub const CANVAS_DOCUMENT_FORMAT_VERSION: u32 = 1;

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum HandleRole {
    #[default]
    Any,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
}

impl Default for CanvasSnapshot {
    fn default() -> Self {
        Self {
            format_version: CANVAS_DOCUMENT_FORMAT_VERSION,
            nodes: Vec::new(),
            edges: Vec::new(),
            shapes: Vec::new(),
            metadata: CanvasValue::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasDocument {
    #[serde(default = "default_document_format_version")]
    pub format_version: u32,
    #[serde(default)]
    pub nodes: IndexMap<NodeId, CanvasNode>,
    #[serde(default)]
    pub edges: IndexMap<EdgeId, CanvasEdge>,
    #[serde(default)]
    pub shapes: IndexMap<ShapeId, CanvasShape>,
    #[serde(default)]
    pub metadata: CanvasValue,
}

impl Default for CanvasDocument {
    fn default() -> Self {
        Self {
            format_version: CANVAS_DOCUMENT_FORMAT_VERSION,
            nodes: IndexMap::new(),
            edges: IndexMap::new(),
            shapes: IndexMap::new(),
            metadata: CanvasValue::new(),
        }
    }
}

impl CanvasDocument {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_snapshot(snapshot: CanvasSnapshot) -> Result<Self, DocumentError> {
        if snapshot.format_version != CANVAS_DOCUMENT_FORMAT_VERSION {
            return Err(DocumentError::UnsupportedFormatVersion {
                expected: CANVAS_DOCUMENT_FORMAT_VERSION,
                found: snapshot.format_version,
            });
        }

        let mut document = Self {
            format_version: snapshot.format_version,
            metadata: snapshot.metadata,
            ..Self::default()
        };

        for node in snapshot.nodes {
            document.insert_node(node)?;
        }

        for shape in snapshot.shapes {
            document.insert_shape(shape)?;
        }

        for edge in snapshot.edges {
            document.insert_edge(edge)?;
        }

        Ok(document)
    }

    pub fn to_snapshot(&self) -> CanvasSnapshot {
        CanvasSnapshot {
            format_version: self.format_version,
            nodes: self.nodes.values().cloned().collect(),
            edges: self.edges.values().cloned().collect(),
            shapes: self.shapes.values().cloned().collect(),
            metadata: self.metadata.clone(),
        }
    }

    pub fn apply(&mut self, command: DocumentCommand) -> Result<(), DocumentError> {
        match command {
            DocumentCommand::InsertNode(node) => self.insert_node(node),
            DocumentCommand::UpdateNode(node) => self.update_node(node),
            DocumentCommand::RemoveNode(id) => self.remove_node(&id).map(drop),
            DocumentCommand::InsertEdge(edge) => self.insert_edge(edge),
            DocumentCommand::UpdateEdge(edge) => self.update_edge(edge),
            DocumentCommand::RemoveEdge(id) => self.remove_edge(&id).map(drop),
            DocumentCommand::InsertShape(shape) => self.insert_shape(shape),
            DocumentCommand::UpdateShape(shape) => self.update_shape(shape),
            DocumentCommand::RemoveShape(id) => self.remove_shape(&id).map(drop),
        }
    }

    pub fn insert_node(&mut self, node: CanvasNode) -> Result<(), DocumentError> {
        if self.nodes.contains_key(&node.id) {
            return Err(DocumentError::DuplicateNode(node.id));
        }

        self.nodes.insert(node.id.clone(), node);
        Ok(())
    }

    pub fn update_node(&mut self, node: CanvasNode) -> Result<(), DocumentError> {
        if !self.nodes.contains_key(&node.id) {
            return Err(DocumentError::MissingNode(node.id));
        }

        self.nodes.insert(node.id.clone(), node);
        Ok(())
    }

    pub fn remove_node(&mut self, id: &NodeId) -> Result<CanvasNode, DocumentError> {
        let Some(node) = self.nodes.shift_remove(id) else {
            return Err(DocumentError::MissingNode(id.clone()));
        };

        self.edges
            .retain(|_, edge| edge.source.node_id != *id && edge.target.node_id != *id);
        Ok(node)
    }

    pub fn insert_edge(&mut self, edge: CanvasEdge) -> Result<(), DocumentError> {
        if self.edges.contains_key(&edge.id) {
            return Err(DocumentError::DuplicateEdge(edge.id));
        }
        self.validate_endpoint(&edge.source)?;
        self.validate_endpoint(&edge.target)?;

        self.edges.insert(edge.id.clone(), edge);
        Ok(())
    }

    pub fn update_edge(&mut self, edge: CanvasEdge) -> Result<(), DocumentError> {
        if !self.edges.contains_key(&edge.id) {
            return Err(DocumentError::MissingEdge(edge.id));
        }
        self.validate_endpoint(&edge.source)?;
        self.validate_endpoint(&edge.target)?;

        self.edges.insert(edge.id.clone(), edge);
        Ok(())
    }

    pub fn remove_edge(&mut self, id: &EdgeId) -> Result<CanvasEdge, DocumentError> {
        self.edges
            .shift_remove(id)
            .ok_or_else(|| DocumentError::MissingEdge(id.clone()))
    }

    pub fn insert_shape(&mut self, shape: CanvasShape) -> Result<(), DocumentError> {
        if self.shapes.contains_key(&shape.id) {
            return Err(DocumentError::DuplicateShape(shape.id));
        }

        self.shapes.insert(shape.id.clone(), shape);
        Ok(())
    }

    pub fn update_shape(&mut self, shape: CanvasShape) -> Result<(), DocumentError> {
        if !self.shapes.contains_key(&shape.id) {
            return Err(DocumentError::MissingShape(shape.id));
        }

        self.shapes.insert(shape.id.clone(), shape);
        Ok(())
    }

    pub fn remove_shape(&mut self, id: &ShapeId) -> Result<CanvasShape, DocumentError> {
        self.shapes
            .shift_remove(id)
            .ok_or_else(|| DocumentError::MissingShape(id.clone()))
    }

    pub fn validate_endpoint(&self, endpoint: &CanvasEndpoint) -> Result<(), DocumentError> {
        let node = self
            .nodes
            .get(&endpoint.node_id)
            .ok_or_else(|| DocumentError::MissingNode(endpoint.node_id.clone()))?;

        if let Some(handle_id) = &endpoint.handle_id {
            if node.handle(Some(handle_id)).is_none() {
                return Err(DocumentError::MissingHandle {
                    node_id: endpoint.node_id.clone(),
                    handle_id: handle_id.clone(),
                });
            }
        }

        Ok(())
    }

    pub fn endpoint_position(
        &self,
        endpoint: &CanvasEndpoint,
    ) -> Result<Point<Pixels>, DocumentError> {
        let node = self
            .nodes
            .get(&endpoint.node_id)
            .ok_or_else(|| DocumentError::MissingNode(endpoint.node_id.clone()))?;

        if let Some(handle_id) = &endpoint.handle_id {
            let handle =
                node.handle(Some(handle_id))
                    .ok_or_else(|| DocumentError::MissingHandle {
                        node_id: endpoint.node_id.clone(),
                        handle_id: handle_id.clone(),
                    })?;
            return Ok(node.position + handle.position);
        }

        Ok(node.bounds().center())
    }

    pub fn edge_bounds(&self, edge: &CanvasEdge) -> Result<Bounds<Pixels>, DocumentError> {
        let source = self.endpoint_position(&edge.source)?;
        let target = self.endpoint_position(&edge.target)?;
        let min_x = source.x.min(target.x);
        let min_y = source.y.min(target.y);
        let max_x = source.x.max(target.x);
        let max_y = source.y.max(target.y);

        Ok(Bounds::from_corners(
            Point::new(min_x, min_y),
            Point::new(max_x, max_y),
        ))
    }
}

impl TryFrom<CanvasSnapshot> for CanvasDocument {
    type Error = DocumentError;

    fn try_from(value: CanvasSnapshot) -> Result<Self, Self::Error> {
        Self::from_snapshot(value)
    }
}

impl From<&CanvasDocument> for CanvasSnapshot {
    fn from(value: &CanvasDocument) -> Self {
        value.to_snapshot()
    }
}

fn default_true() -> bool {
    true
}

fn default_document_format_version() -> u32 {
    CANVAS_DOCUMENT_FORMAT_VERSION
}

fn default_kind() -> String {
    "default".to_string()
}

fn default_handle_size() -> Size<Pixels> {
    Size::new(Pixels::from(12.0), Pixels::from(12.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui::{point, px, size};

    #[test]
    fn defaults_to_current_format_version() {
        let document = CanvasDocument::default();

        assert_eq!(document.format_version, CANVAS_DOCUMENT_FORMAT_VERSION);
    }

    #[test]
    fn deserializes_missing_format_version_to_current_version() {
        let document: CanvasDocument = serde_json::from_str(
            r#"{
                "nodes": {},
                "edges": {},
                "shapes": {},
                "metadata": {}
            }"#,
        )
        .unwrap();

        assert_eq!(document.format_version, CANVAS_DOCUMENT_FORMAT_VERSION);
    }

    #[test]
    fn snapshot_round_trips_array_records() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();
        document
            .insert_node(CanvasNode::new(
                "b",
                point(px(20.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();
        document
            .insert_edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .unwrap();

        let snapshot = document.to_snapshot();
        assert_eq!(snapshot.nodes.len(), 2);
        assert_eq!(snapshot.edges.len(), 1);

        let restored = CanvasDocument::from_snapshot(snapshot).unwrap();
        assert_eq!(restored.nodes.len(), 2);
        assert_eq!(restored.edges.len(), 1);
    }

    #[test]
    fn rejects_unsupported_snapshot_version() {
        let snapshot = CanvasSnapshot {
            format_version: CANVAS_DOCUMENT_FORMAT_VERSION + 1,
            ..CanvasSnapshot::default()
        };

        assert_eq!(
            CanvasDocument::from_snapshot(snapshot).unwrap_err(),
            DocumentError::UnsupportedFormatVersion {
                expected: CANVAS_DOCUMENT_FORMAT_VERSION,
                found: CANVAS_DOCUMENT_FORMAT_VERSION + 1,
            }
        );
    }

    #[test]
    fn removes_edges_when_node_is_removed() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();
        document
            .insert_node(CanvasNode::new(
                "b",
                point(px(20.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();
        document
            .insert_edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .unwrap();

        document.remove_node(&NodeId::from("a")).unwrap();

        assert!(document.edges.is_empty());
    }

    #[test]
    fn validates_edge_handles() {
        let mut node = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        node.handles
            .push(CanvasHandle::new("out", point(px(10.0), px(5.0))));
        let mut document = CanvasDocument::default();
        document.insert_node(node).unwrap();
        document
            .insert_node(CanvasNode::new(
                "b",
                point(px(20.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();

        let err = document
            .insert_edge(CanvasEdge::new(
                "bad",
                CanvasEndpoint::new("a", Some("missing")),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .unwrap_err();

        assert_eq!(
            err,
            DocumentError::MissingHandle {
                node_id: NodeId::from("a"),
                handle_id: HandleId::from("missing")
            }
        );
    }
}
