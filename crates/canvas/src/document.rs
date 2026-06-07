use indexmap::{IndexMap, IndexSet};
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
}

impl CanvasDocumentDiff {
    pub fn is_empty(&self) -> bool {
        self.inserted.is_empty()
            && self.updated.is_empty()
            && self.removed.is_empty()
            && !self.metadata_changed
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

    pub fn apply_transaction(
        &mut self,
        transaction: CanvasTransaction,
    ) -> Result<(), DocumentError> {
        self.apply_transaction_with_diff(transaction).map(drop)
    }

    pub fn apply_transaction_with_diff(
        &mut self,
        transaction: CanvasTransaction,
    ) -> Result<CanvasDocumentDiff, DocumentError> {
        let previous = self.clone();
        let mut draft = previous.clone();
        for command in transaction.commands {
            draft.apply(command)?;
        }
        let diff = draft.diff_against(&previous);
        *self = draft;
        Ok(diff)
    }

    pub fn invert_transaction(
        &self,
        transaction: &CanvasTransaction,
    ) -> Result<CanvasTransaction, DocumentError> {
        let mut draft = self.clone();
        let mut inverse_segments = Vec::new();

        for command in &transaction.commands {
            inverse_segments.push(draft.inverse_for(command)?);
            draft.apply(command.clone())?;
        }

        Ok(CanvasTransaction {
            commands: inverse_segments.into_iter().rev().flatten().collect(),
            metadata: CanvasValue::new(),
        })
    }

    pub fn insert_node(&mut self, node: CanvasNode) -> Result<(), DocumentError> {
        if self.nodes.contains_key(&node.id) {
            return Err(DocumentError::DuplicateNode(node.id));
        }
        Self::validate_node(&node)?;

        self.nodes.insert(node.id.clone(), node);
        Ok(())
    }

    pub fn update_node(&mut self, node: CanvasNode) -> Result<(), DocumentError> {
        if !self.nodes.contains_key(&node.id) {
            return Err(DocumentError::MissingNode(node.id));
        }
        Self::validate_node(&node)?;

        let mut draft = self.clone();
        draft.nodes.insert(node.id.clone(), node);
        draft.validate_integrity()?;
        *self = draft;
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
        self.validate_edge(&edge)?;

        self.edges.insert(edge.id.clone(), edge);
        Ok(())
    }

    pub fn update_edge(&mut self, edge: CanvasEdge) -> Result<(), DocumentError> {
        if !self.edges.contains_key(&edge.id) {
            return Err(DocumentError::MissingEdge(edge.id));
        }
        self.validate_edge(&edge)?;

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
        self.endpoint_parts(endpoint)?;
        Ok(())
    }

    pub fn validate_edge(&self, edge: &CanvasEdge) -> Result<(), DocumentError> {
        Self::validate_edge_route(edge)?;
        self.validate_source_endpoint(&edge.source)?;
        self.validate_target_endpoint(&edge.target)?;
        Ok(())
    }

    pub fn validate_integrity(&self) -> Result<(), DocumentError> {
        for node in self.nodes.values() {
            Self::validate_node(node)?;
        }

        for edge in self.edges.values() {
            self.validate_edge(edge)?;
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
        let route_points = edge
            .route
            .waypoints
            .iter()
            .chain(edge.route.control_points.iter());
        let (min_x, min_y, max_x, max_y) = route_points.fold(
            (
                source.x.min(target.x),
                source.y.min(target.y),
                source.x.max(target.x),
                source.y.max(target.y),
            ),
            |(min_x, min_y, max_x, max_y), point| {
                (
                    min_x.min(point.x),
                    min_y.min(point.y),
                    max_x.max(point.x),
                    max_y.max(point.y),
                )
            },
        );
        let bounds = Bounds::from_corners(Point::new(min_x, min_y), Point::new(max_x, max_y));
        let stroke_width = if edge.style.stroke_width.as_f32().is_finite()
            && edge.style.stroke_width > Pixels::ZERO
        {
            edge.style.stroke_width
        } else {
            Pixels::ZERO
        };
        let interaction_width = if edge.route.interaction_width > stroke_width {
            edge.route.interaction_width
        } else {
            stroke_width
        };

        Ok(bounds.dilate(interaction_width * 0.5))
    }

    pub fn edge_route_points(
        &self,
        edge: &CanvasEdge,
    ) -> Result<Vec<Point<Pixels>>, DocumentError> {
        let source = self.endpoint_position(&edge.source)?;
        let target = self.endpoint_position(&edge.target)?;
        let mut points = Vec::with_capacity(edge.route.waypoints.len() + 2);
        points.push(source);
        points.extend(edge.route.waypoints.iter().copied());
        points.push(target);
        Ok(points)
    }

    pub fn diff_against(&self, previous: &CanvasDocument) -> CanvasDocumentDiff {
        let mut diff = CanvasDocumentDiff::default();

        for id in previous.nodes.keys() {
            if !self.nodes.contains_key(id) {
                diff.record_remove(id.clone());
            }
        }

        for (id, node) in &self.nodes {
            match previous.nodes.get(id) {
                None => diff.record_insert(id.clone()),
                Some(previous_node) if previous_node != node => diff.record_update(id.clone()),
                Some(_) => {}
            }
        }

        for id in previous.edges.keys() {
            if !self.edges.contains_key(id) {
                diff.record_remove(id.clone());
            }
        }

        for (id, edge) in &self.edges {
            match previous.edges.get(id) {
                None => diff.record_insert(id.clone()),
                Some(previous_edge) if previous_edge != edge => diff.record_update(id.clone()),
                Some(_) => {}
            }
        }

        for id in previous.shapes.keys() {
            if !self.shapes.contains_key(id) {
                diff.record_remove(id.clone());
            }
        }

        for (id, shape) in &self.shapes {
            match previous.shapes.get(id) {
                None => diff.record_insert(id.clone()),
                Some(previous_shape) if previous_shape != shape => diff.record_update(id.clone()),
                Some(_) => {}
            }
        }

        diff.metadata_changed = self.metadata != previous.metadata;
        diff
    }

    fn validate_node(node: &CanvasNode) -> Result<(), DocumentError> {
        let mut handle_ids = IndexSet::new();
        for handle in &node.handles {
            if !handle_ids.insert(handle.id.clone()) {
                return Err(DocumentError::DuplicateHandle {
                    node_id: node.id.clone(),
                    handle_id: handle.id.clone(),
                });
            }
        }

        Ok(())
    }

    fn validate_edge_route(edge: &CanvasEdge) -> Result<(), DocumentError> {
        if edge.route.kind.as_str().trim().is_empty() {
            return Err(DocumentError::EmptyEdgeRouteKind(edge.id.clone()));
        }

        if !edge.route.interaction_width.as_f32().is_finite()
            || edge.route.interaction_width < Pixels::ZERO
        {
            return Err(DocumentError::InvalidEdgeInteractionWidth(edge.id.clone()));
        }

        for point in edge
            .route
            .waypoints
            .iter()
            .chain(edge.route.control_points.iter())
        {
            if !point.x.as_f32().is_finite() || !point.y.as_f32().is_finite() {
                return Err(DocumentError::InvalidEdgeRoutePoint(edge.id.clone()));
            }
        }

        Ok(())
    }

    fn validate_source_endpoint(&self, endpoint: &CanvasEndpoint) -> Result<(), DocumentError> {
        let Some(handle) = self.endpoint_parts(endpoint)?.1 else {
            return Ok(());
        };
        self.validate_connectable_handle(endpoint, handle)?;

        if handle.role == HandleRole::Target {
            return Err(DocumentError::InvalidSourceHandle {
                node_id: endpoint.node_id.clone(),
                handle_id: handle.id.clone(),
            });
        }

        Ok(())
    }

    fn validate_target_endpoint(&self, endpoint: &CanvasEndpoint) -> Result<(), DocumentError> {
        let Some(handle) = self.endpoint_parts(endpoint)?.1 else {
            return Ok(());
        };
        self.validate_connectable_handle(endpoint, handle)?;

        if handle.role == HandleRole::Source {
            return Err(DocumentError::InvalidTargetHandle {
                node_id: endpoint.node_id.clone(),
                handle_id: handle.id.clone(),
            });
        }

        Ok(())
    }

    fn validate_connectable_handle(
        &self,
        endpoint: &CanvasEndpoint,
        handle: &CanvasHandle,
    ) -> Result<(), DocumentError> {
        if !handle.connectable {
            return Err(DocumentError::NonConnectableHandle {
                node_id: endpoint.node_id.clone(),
                handle_id: handle.id.clone(),
            });
        }

        Ok(())
    }

    fn endpoint_parts(
        &self,
        endpoint: &CanvasEndpoint,
    ) -> Result<(&CanvasNode, Option<&CanvasHandle>), DocumentError> {
        let node = self
            .nodes
            .get(&endpoint.node_id)
            .ok_or_else(|| DocumentError::MissingNode(endpoint.node_id.clone()))?;

        let Some(handle_id) = &endpoint.handle_id else {
            return Ok((node, None));
        };

        let handle = node
            .handle(Some(handle_id))
            .ok_or_else(|| DocumentError::MissingHandle {
                node_id: endpoint.node_id.clone(),
                handle_id: handle_id.clone(),
            })?;

        Ok((node, Some(handle)))
    }

    fn inverse_for(
        &self,
        command: &DocumentCommand,
    ) -> Result<Vec<DocumentCommand>, DocumentError> {
        match command {
            DocumentCommand::InsertNode(node) => {
                if self.nodes.contains_key(&node.id) {
                    return Err(DocumentError::DuplicateNode(node.id.clone()));
                }
                Self::validate_node(node)?;
                Ok(vec![DocumentCommand::RemoveNode(node.id.clone())])
            }
            DocumentCommand::UpdateNode(node) => Ok(vec![DocumentCommand::UpdateNode(
                self.nodes
                    .get(&node.id)
                    .ok_or_else(|| DocumentError::MissingNode(node.id.clone()))?
                    .clone(),
            )]),
            DocumentCommand::RemoveNode(id) => {
                let node = self
                    .nodes
                    .get(id)
                    .ok_or_else(|| DocumentError::MissingNode(id.clone()))?
                    .clone();
                let mut inverse = vec![DocumentCommand::InsertNode(node)];
                inverse.extend(
                    self.edges
                        .values()
                        .filter(|edge| edge.source.node_id == *id || edge.target.node_id == *id)
                        .cloned()
                        .map(DocumentCommand::InsertEdge),
                );
                Ok(inverse)
            }
            DocumentCommand::InsertEdge(edge) => {
                if self.edges.contains_key(&edge.id) {
                    return Err(DocumentError::DuplicateEdge(edge.id.clone()));
                }
                self.validate_edge(edge)?;
                Ok(vec![DocumentCommand::RemoveEdge(edge.id.clone())])
            }
            DocumentCommand::UpdateEdge(edge) => Ok(vec![DocumentCommand::UpdateEdge(
                self.edges
                    .get(&edge.id)
                    .ok_or_else(|| DocumentError::MissingEdge(edge.id.clone()))?
                    .clone(),
            )]),
            DocumentCommand::RemoveEdge(id) => Ok(vec![DocumentCommand::InsertEdge(
                self.edges
                    .get(id)
                    .ok_or_else(|| DocumentError::MissingEdge(id.clone()))?
                    .clone(),
            )]),
            DocumentCommand::InsertShape(shape) => {
                if self.shapes.contains_key(&shape.id) {
                    return Err(DocumentError::DuplicateShape(shape.id.clone()));
                }
                Ok(vec![DocumentCommand::RemoveShape(shape.id.clone())])
            }
            DocumentCommand::UpdateShape(shape) => Ok(vec![DocumentCommand::UpdateShape(
                self.shapes
                    .get(&shape.id)
                    .ok_or_else(|| DocumentError::MissingShape(shape.id.clone()))?
                    .clone(),
            )]),
            DocumentCommand::RemoveShape(id) => Ok(vec![DocumentCommand::InsertShape(
                self.shapes
                    .get(id)
                    .ok_or_else(|| DocumentError::MissingShape(id.clone()))?
                    .clone(),
            )]),
        }
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

fn default_edge_interaction_width() -> Pixels {
    Pixels::from(12.0)
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

    #[test]
    fn rejects_duplicate_handles_on_node_insert() {
        let mut node = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        node.handles
            .push(CanvasHandle::new("out", point(px(10.0), px(5.0))));
        node.handles
            .push(CanvasHandle::new("out", point(px(0.0), px(5.0))));

        let err = CanvasDocument::default().insert_node(node).unwrap_err();

        assert_eq!(
            err,
            DocumentError::DuplicateHandle {
                node_id: NodeId::from("a"),
                handle_id: HandleId::from("out")
            }
        );
    }

    #[test]
    fn validates_handle_roles_for_edges() {
        let mut source = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        let mut target_only = CanvasHandle::new("in", point(px(10.0), px(5.0)));
        target_only.role = HandleRole::Target;
        source.handles.push(target_only);

        let mut target = CanvasNode::new("b", point(px(20.0), px(0.0)), size(px(10.0), px(10.0)));
        let mut source_only = CanvasHandle::new("out", point(px(0.0), px(5.0)));
        source_only.role = HandleRole::Source;
        target.handles.push(source_only);

        let mut document = CanvasDocument::default();
        document.insert_node(source).unwrap();
        document.insert_node(target).unwrap();

        let err = document
            .insert_edge(CanvasEdge::new(
                "bad",
                CanvasEndpoint::new("a", Some("in")),
                CanvasEndpoint::new("b", Some("out")),
            ))
            .unwrap_err();

        assert_eq!(
            err,
            DocumentError::InvalidSourceHandle {
                node_id: NodeId::from("a"),
                handle_id: HandleId::from("in")
            }
        );
    }

    #[test]
    fn rejects_non_connectable_edge_handles() {
        let mut source = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        let mut handle = CanvasHandle::new("out", point(px(10.0), px(5.0)));
        handle.connectable = false;
        source.handles.push(handle);

        let mut document = CanvasDocument::default();
        document.insert_node(source).unwrap();
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
                CanvasEndpoint::new("a", Some("out")),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .unwrap_err();

        assert_eq!(
            err,
            DocumentError::NonConnectableHandle {
                node_id: NodeId::from("a"),
                handle_id: HandleId::from("out")
            }
        );
    }

    #[test]
    fn edge_route_defaults_keep_legacy_edges_readable() {
        let edge: CanvasEdge = serde_json::from_str(
            r#"{
                "id": "a-b",
                "source": { "node_id": "a" },
                "target": { "node_id": "b" }
            }"#,
        )
        .unwrap();

        assert_eq!(
            edge.route.kind,
            CanvasEdgeRouteKind::from(CanvasEdgeRouteKind::STRAIGHT)
        );
        assert!(edge.route.waypoints.is_empty());
        assert!(edge.route.control_points.is_empty());
        assert_eq!(edge.route.interaction_width, px(12.0));
    }

    #[test]
    fn edge_route_points_include_waypoints_between_endpoints() {
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
                point(px(100.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();
        let mut edge = CanvasEdge::new(
            "a-b",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        );
        edge.route =
            CanvasEdgeRoute::polyline([point(px(40.0), px(50.0)), point(px(80.0), px(50.0))]);

        let route_points = document.edge_route_points(&edge).unwrap();

        assert_eq!(
            route_points,
            vec![
                point(px(5.0), px(5.0)),
                point(px(40.0), px(50.0)),
                point(px(80.0), px(50.0)),
                point(px(105.0), px(5.0)),
            ]
        );
    }

    #[test]
    fn edge_bounds_include_route_points_and_interaction_width() {
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
                point(px(100.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();
        let mut edge = CanvasEdge::new(
            "a-b",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        );
        edge.route = CanvasEdgeRoute::polyline([point(px(40.0), px(50.0))]);
        edge.route.interaction_width = px(20.0);

        let bounds = document.edge_bounds(&edge).unwrap();

        assert_eq!(bounds.origin, point(px(-5.0), px(-5.0)));
        assert_eq!(bounds.size, size(px(120.0), px(65.0)));
    }

    #[test]
    fn rejects_invalid_edge_route_metadata() {
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
                point(px(100.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();

        let mut empty_kind = CanvasEdge::new(
            "empty-kind",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        );
        empty_kind.route.kind = CanvasEdgeRouteKind::new("");
        assert_eq!(
            document.insert_edge(empty_kind).unwrap_err(),
            DocumentError::EmptyEdgeRouteKind(EdgeId::from("empty-kind"))
        );

        let mut negative_width = CanvasEdge::new(
            "negative-width",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        );
        negative_width.route.interaction_width = px(-1.0);
        assert_eq!(
            document.insert_edge(negative_width).unwrap_err(),
            DocumentError::InvalidEdgeInteractionWidth(EdgeId::from("negative-width"))
        );

        let mut invalid_point = CanvasEdge::new(
            "invalid-point",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        );
        invalid_point
            .route
            .waypoints
            .push(point(px(f32::NAN), px(0.0)));
        assert_eq!(
            document.insert_edge(invalid_point).unwrap_err(),
            DocumentError::InvalidEdgeRoutePoint(EdgeId::from("invalid-point"))
        );
    }

    #[test]
    fn node_update_cannot_break_existing_edge_endpoints() {
        let mut node = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        node.handles
            .push(CanvasHandle::new("out", point(px(10.0), px(5.0))));

        let mut document = CanvasDocument::default();
        document.insert_node(node.clone()).unwrap();
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
                CanvasEndpoint::new("a", Some("out")),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .unwrap();

        node.handles.clear();
        let err = document.update_node(node).unwrap_err();

        assert_eq!(
            err,
            DocumentError::MissingHandle {
                node_id: NodeId::from("a"),
                handle_id: HandleId::from("out")
            }
        );
        assert!(
            document.nodes[&NodeId::from("a")]
                .handle(Some(&HandleId::from("out")))
                .is_some()
        );
    }

    #[test]
    fn applies_transaction_atomically() {
        let mut document = CanvasDocument::default();
        let transaction = CanvasTransaction::new([
            DocumentCommand::InsertNode(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            )),
            DocumentCommand::InsertEdge(CanvasEdge::new(
                "bad",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("missing", None::<&str>),
            )),
        ]);

        let err = document.apply_transaction(transaction).unwrap_err();

        assert_eq!(err, DocumentError::MissingNode(NodeId::from("missing")));
        assert!(document.nodes.is_empty());
        assert!(document.edges.is_empty());
    }

    #[test]
    fn transaction_inverse_restores_document() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();

        let before = document.clone();
        let transaction = CanvasTransaction::new([
            DocumentCommand::InsertNode(CanvasNode::new(
                "b",
                point(px(20.0), px(0.0)),
                size(px(10.0), px(10.0)),
            )),
            DocumentCommand::InsertEdge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("b", None::<&str>),
            )),
        ]);
        let inverse = document.invert_transaction(&transaction).unwrap();

        document.apply_transaction(transaction).unwrap();
        assert_ne!(document, before);

        document.apply_transaction(inverse).unwrap();
        assert_eq!(document, before);
    }

    #[test]
    fn transaction_diff_tracks_record_changes() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();

        let moved_a = CanvasNode::new("a", point(px(5.0), px(0.0)), size(px(10.0), px(10.0)));
        let transaction = CanvasTransaction::new([
            DocumentCommand::UpdateNode(moved_a),
            DocumentCommand::InsertNode(CanvasNode::new(
                "b",
                point(px(20.0), px(0.0)),
                size(px(10.0), px(10.0)),
            )),
            DocumentCommand::InsertEdge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("b", None::<&str>),
            )),
        ]);

        let diff = document.apply_transaction_with_diff(transaction).unwrap();

        assert_eq!(
            diff.updated.iter().cloned().collect::<Vec<_>>(),
            vec![CanvasRecordId::Node(NodeId::from("a"))]
        );
        assert_eq!(
            diff.inserted.iter().cloned().collect::<Vec<_>>(),
            vec![
                CanvasRecordId::Node(NodeId::from("b")),
                CanvasRecordId::Edge(EdgeId::from("a-b")),
            ]
        );
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn transaction_diff_compacts_insert_then_remove() {
        let mut document = CanvasDocument::default();
        let transaction = CanvasTransaction::new([
            DocumentCommand::InsertNode(CanvasNode::new(
                "temp",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            )),
            DocumentCommand::RemoveNode(NodeId::from("temp")),
        ]);

        let diff = document.apply_transaction_with_diff(transaction).unwrap();

        assert!(diff.is_empty());
        assert!(document.nodes.is_empty());
    }

    #[test]
    fn transaction_diff_includes_edges_removed_with_node() {
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

        let diff = document
            .apply_transaction_with_diff(CanvasTransaction::single(DocumentCommand::RemoveNode(
                NodeId::from("a"),
            )))
            .unwrap();

        assert_eq!(
            diff.removed.iter().cloned().collect::<Vec<_>>(),
            vec![
                CanvasRecordId::Node(NodeId::from("a")),
                CanvasRecordId::Edge(EdgeId::from("a-b")),
            ]
        );
        assert!(document.edges.is_empty());
    }

    #[test]
    fn document_diff_tracks_metadata_changes() {
        let previous = CanvasDocument::default();
        let mut document = previous.clone();
        document
            .metadata
            .insert("title".to_string(), serde_json::json!("Canvas"));

        let diff = document.diff_against(&previous);

        assert!(diff.metadata_changed);
        assert!(!diff.is_empty());
    }
}
