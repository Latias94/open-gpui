use crate::{
    CanvasDocument, CanvasEdge, CanvasEndpoint, CanvasHandle, CanvasNode, CanvasStyle, CanvasValue,
    DocumentError, NodeId,
};
use indexmap::IndexMap;
use open_gpui::{Pixels, Point, point, px, size};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fmt, str::FromStr};
use thiserror::Error;

const TEXT_NODE_TYPE: &str = "text";
const FILE_NODE_TYPE: &str = "file";
const LINK_NODE_TYPE: &str = "link";
const GROUP_NODE_TYPE: &str = "group";

const TEXT_FIELD: &str = "text";
const FILE_FIELD: &str = "file";
const SUBPATH_FIELD: &str = "subpath";
const URL_FIELD: &str = "url";
const LABEL_FIELD: &str = "label";
const BACKGROUND_FIELD: &str = "background";
const BACKGROUND_STYLE_FIELD: &str = "backgroundStyle";
const FROM_END_FIELD: &str = "fromEnd";
const TO_END_FIELD: &str = "toEnd";

#[derive(Debug, Error)]
pub enum JsonCanvasError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Document(#[from] DocumentError),
    #[error("node `{node_id}` is missing required JSON Canvas field `{field}` for type `{kind}`")]
    MissingNodeField {
        node_id: String,
        kind: String,
        field: &'static str,
    },
    #[error("node `{node_id}` has unsupported JSON Canvas type `{kind}`")]
    UnsupportedNodeKind { node_id: String, kind: String },
    #[error("node `{node_id}` has invalid JSON Canvas size")]
    InvalidNodeSize { node_id: String },
    #[error("record `{id}` has invalid `{field}` geometry")]
    InvalidGeometry { id: String, field: &'static str },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct JsonCanvas {
    #[serde(default)]
    pub nodes: Vec<JsonCanvasNode>,
    #[serde(default)]
    pub edges: Vec<JsonCanvasEdge>,
}

impl JsonCanvas {
    pub fn parse(input: &str) -> Result<Self, JsonCanvasError> {
        Ok(serde_json::from_str(input)?)
    }

    pub fn to_string_pretty(&self) -> Result<String, JsonCanvasError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn into_document(self) -> Result<CanvasDocument, JsonCanvasError> {
        self.try_into()
    }

    pub fn from_document(document: &CanvasDocument) -> Result<Self, JsonCanvasError> {
        document.try_into()
    }
}

impl FromStr for JsonCanvas {
    type Err = JsonCanvasError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::parse(input)
    }
}

impl TryFrom<JsonCanvas> for CanvasDocument {
    type Error = JsonCanvasError;

    fn try_from(canvas: JsonCanvas) -> Result<Self, Self::Error> {
        let mut nodes = IndexMap::new();

        for (index, node) in canvas.nodes.into_iter().enumerate() {
            let id = NodeId::from(node.id.clone());
            if nodes.contains_key(&id) {
                return Err(DocumentError::DuplicateNode(id).into());
            }
            let canvas_node = node.into_canvas_node(index as i32)?;
            nodes.insert(id, canvas_node);
        }

        for edge in &canvas.edges {
            if let Some(side) = edge.from_side {
                ensure_side_handle(&mut nodes, &edge.from_node, side);
            }
            if let Some(side) = edge.to_side {
                ensure_side_handle(&mut nodes, &edge.to_node, side);
            }
        }

        let mut document = CanvasDocument::default();
        for (_, node) in nodes {
            document.insert_node(node)?;
        }

        for edge in canvas.edges {
            document.insert_edge(edge.into_canvas_edge())?;
        }

        Ok(document)
    }
}

impl TryFrom<&CanvasDocument> for JsonCanvas {
    type Error = JsonCanvasError;

    fn try_from(document: &CanvasDocument) -> Result<Self, Self::Error> {
        let mut nodes = document.nodes.values().collect::<Vec<_>>();
        nodes.sort_by(|a, b| a.z_index.cmp(&b.z_index));

        Ok(Self {
            nodes: nodes
                .into_iter()
                .map(JsonCanvasNode::from_canvas_node)
                .collect::<Result<Vec<_>, _>>()?,
            edges: document
                .edges
                .values()
                .map(|edge| JsonCanvasEdge::from_canvas_edge(document, edge))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JsonCanvasNode {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subpath: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(rename = "backgroundStyle")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_style: Option<String>,
    #[serde(flatten)]
    pub extra: CanvasValue,
}

impl JsonCanvasNode {
    fn into_canvas_node(self, z_index: i32) -> Result<CanvasNode, JsonCanvasError> {
        self.validate_required_fields()?;

        if self.width < 0 || self.height < 0 {
            return Err(JsonCanvasError::InvalidNodeSize { node_id: self.id });
        }

        let mut data = self.extra;
        insert_string(&mut data, TEXT_FIELD, self.text);
        insert_string(&mut data, FILE_FIELD, self.file);
        insert_string(&mut data, SUBPATH_FIELD, self.subpath);
        insert_string(&mut data, URL_FIELD, self.url);
        insert_string(&mut data, LABEL_FIELD, self.label);
        insert_string(&mut data, BACKGROUND_FIELD, self.background);
        insert_string(&mut data, BACKGROUND_STYLE_FIELD, self.background_style);

        let mut node = CanvasNode::new(
            self.id,
            point(px(self.x as f32), px(self.y as f32)),
            size(px(self.width as f32), px(self.height as f32)),
        );
        node.kind = self.kind;
        node.z_index = z_index;
        node.data = data;
        node.style = CanvasStyle {
            fill: self.color,
            ..CanvasStyle::default()
        };
        Ok(node)
    }

    fn from_canvas_node(node: &CanvasNode) -> Result<Self, JsonCanvasError> {
        let mut extra = node.data.clone();
        let text = remove_string(&mut extra, TEXT_FIELD);
        let file = remove_string(&mut extra, FILE_FIELD);
        let subpath = remove_string(&mut extra, SUBPATH_FIELD);
        let url = remove_string(&mut extra, URL_FIELD);
        let label = remove_string(&mut extra, LABEL_FIELD);
        let background = remove_string(&mut extra, BACKGROUND_FIELD);
        let background_style = remove_string(&mut extra, BACKGROUND_STYLE_FIELD);

        let json_node = Self {
            id: node.id.as_str().to_string(),
            kind: node.kind.clone(),
            x: pixel_to_i64(&node.id, "x", node.position.x)?,
            y: pixel_to_i64(&node.id, "y", node.position.y)?,
            width: nonnegative_pixel_to_i64(&node.id, "width", node.size.width)?,
            height: nonnegative_pixel_to_i64(&node.id, "height", node.size.height)?,
            color: node.style.fill.clone(),
            text,
            file,
            subpath,
            url,
            label,
            background,
            background_style,
            extra,
        };
        json_node.validate_required_fields()?;
        Ok(json_node)
    }

    fn validate_required_fields(&self) -> Result<(), JsonCanvasError> {
        match self.kind.as_str() {
            TEXT_NODE_TYPE if self.text.is_none() => self.missing_field(TEXT_FIELD),
            FILE_NODE_TYPE if self.file.is_none() => self.missing_field(FILE_FIELD),
            LINK_NODE_TYPE if self.url.is_none() => self.missing_field(URL_FIELD),
            TEXT_NODE_TYPE | FILE_NODE_TYPE | LINK_NODE_TYPE | GROUP_NODE_TYPE => Ok(()),
            _ => Err(JsonCanvasError::UnsupportedNodeKind {
                node_id: self.id.clone(),
                kind: self.kind.clone(),
            }),
        }
    }

    fn missing_field(&self, field: &'static str) -> Result<(), JsonCanvasError> {
        Err(JsonCanvasError::MissingNodeField {
            node_id: self.id.clone(),
            kind: self.kind.clone(),
            field,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JsonCanvasSide {
    Top,
    Right,
    Bottom,
    Left,
}

impl JsonCanvasSide {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Right => "right",
            Self::Bottom => "bottom",
            Self::Left => "left",
        }
    }

    pub fn handle_id(self) -> String {
        format!("json_canvas:{}", self.as_str())
    }

    fn handle_position(self, node: &CanvasNode) -> Point<Pixels> {
        match self {
            Self::Top => point(node.size.width * 0.5, Pixels::ZERO),
            Self::Right => point(node.size.width, node.size.height * 0.5),
            Self::Bottom => point(node.size.width * 0.5, node.size.height),
            Self::Left => point(Pixels::ZERO, node.size.height * 0.5),
        }
    }
}

impl fmt::Display for JsonCanvasSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JsonCanvasEndpointShape {
    None,
    Arrow,
}

impl JsonCanvasEndpointShape {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Arrow => "arrow",
        }
    }
}

impl fmt::Display for JsonCanvasEndpointShape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JsonCanvasEdge {
    pub id: String,
    #[serde(rename = "fromNode")]
    pub from_node: String,
    #[serde(rename = "fromSide")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_side: Option<JsonCanvasSide>,
    #[serde(rename = "fromEnd")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_end: Option<JsonCanvasEndpointShape>,
    #[serde(rename = "toNode")]
    pub to_node: String,
    #[serde(rename = "toSide")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_side: Option<JsonCanvasSide>,
    #[serde(rename = "toEnd")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_end: Option<JsonCanvasEndpointShape>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(flatten)]
    pub extra: CanvasValue,
}

impl JsonCanvasEdge {
    fn into_canvas_edge(self) -> CanvasEdge {
        let mut data = self.extra;
        insert_string(
            &mut data,
            FROM_END_FIELD,
            self.from_end.map(|shape| shape.as_str().to_string()),
        );
        insert_string(
            &mut data,
            TO_END_FIELD,
            self.to_end.map(|shape| shape.as_str().to_string()),
        );
        insert_string(&mut data, LABEL_FIELD, self.label);

        let mut edge = CanvasEdge::new(
            self.id,
            CanvasEndpoint {
                node_id: NodeId::from(self.from_node),
                handle_id: self.from_side.map(|side| side.handle_id().into()),
            },
            CanvasEndpoint {
                node_id: NodeId::from(self.to_node),
                handle_id: self.to_side.map(|side| side.handle_id().into()),
            },
        );
        edge.data = data;
        edge.style = CanvasStyle {
            stroke: self.color,
            ..CanvasStyle::default()
        };
        edge
    }

    fn from_canvas_edge(
        document: &CanvasDocument,
        edge: &CanvasEdge,
    ) -> Result<Self, JsonCanvasError> {
        let mut extra = edge.data.clone();
        let from_end = remove_endpoint_shape(&mut extra, FROM_END_FIELD);
        let to_end = remove_endpoint_shape(&mut extra, TO_END_FIELD);
        let label = remove_string(&mut extra, LABEL_FIELD);

        Ok(Self {
            id: edge.id.as_str().to_string(),
            from_node: edge.source.node_id.as_str().to_string(),
            from_side: endpoint_side(document, &edge.source),
            from_end,
            to_node: edge.target.node_id.as_str().to_string(),
            to_side: endpoint_side(document, &edge.target),
            to_end,
            color: edge.style.stroke.clone(),
            label,
            extra,
        })
    }
}

pub fn document_from_json_canvas_str(input: &str) -> Result<CanvasDocument, JsonCanvasError> {
    JsonCanvas::parse(input)?.into_document()
}

pub fn document_to_json_canvas_string(
    document: &CanvasDocument,
) -> Result<String, JsonCanvasError> {
    JsonCanvas::from_document(document)?.to_string_pretty()
}

fn ensure_side_handle(
    nodes: &mut IndexMap<NodeId, CanvasNode>,
    node_id: &str,
    side: JsonCanvasSide,
) {
    let Some(node) = nodes.get_mut(&NodeId::from(node_id)) else {
        return;
    };
    let handle_id = side.handle_id();
    if node
        .handles
        .iter()
        .any(|handle| handle.id.as_str() == handle_id)
    {
        return;
    }

    node.handles
        .push(CanvasHandle::new(handle_id, side.handle_position(node)));
}

fn endpoint_side(document: &CanvasDocument, endpoint: &CanvasEndpoint) -> Option<JsonCanvasSide> {
    let handle_id = endpoint.handle_id.as_ref()?;
    side_from_handle_id(handle_id.as_str()).or_else(|| {
        let node = document.nodes.get(&endpoint.node_id)?;
        let handle = node.handle(Some(handle_id))?;
        side_from_handle_position(node, handle)
    })
}

fn side_from_handle_id(handle_id: &str) -> Option<JsonCanvasSide> {
    match handle_id.strip_prefix("json_canvas:")? {
        "top" => Some(JsonCanvasSide::Top),
        "right" => Some(JsonCanvasSide::Right),
        "bottom" => Some(JsonCanvasSide::Bottom),
        "left" => Some(JsonCanvasSide::Left),
        _ => None,
    }
}

fn side_from_handle_position(node: &CanvasNode, handle: &CanvasHandle) -> Option<JsonCanvasSide> {
    let top = handle.position.y.abs();
    let right = (node.size.width - handle.position.x).abs();
    let bottom = (node.size.height - handle.position.y).abs();
    let left = handle.position.x.abs();
    let mut sides = [
        (top, JsonCanvasSide::Top),
        (right, JsonCanvasSide::Right),
        (bottom, JsonCanvasSide::Bottom),
        (left, JsonCanvasSide::Left),
    ];
    sides.sort_by(|(a, _), (b, _)| a.cmp(b));
    Some(sides[0].1)
}

fn insert_string(data: &mut CanvasValue, key: &'static str, value: Option<String>) {
    if let Some(value) = value {
        data.insert(key.to_string(), Value::String(value));
    }
}

fn remove_string(data: &mut CanvasValue, key: &'static str) -> Option<String> {
    match data.remove(key) {
        Some(Value::String(value)) => Some(value),
        Some(value) => {
            data.insert(key.to_string(), value);
            None
        }
        None => None,
    }
}

fn remove_endpoint_shape(
    data: &mut CanvasValue,
    key: &'static str,
) -> Option<JsonCanvasEndpointShape> {
    match data.remove(key) {
        Some(Value::String(value)) if value == "none" => Some(JsonCanvasEndpointShape::None),
        Some(Value::String(value)) if value == "arrow" => Some(JsonCanvasEndpointShape::Arrow),
        Some(value) => {
            data.insert(key.to_string(), value);
            None
        }
        None => None,
    }
}

fn pixel_to_i64(
    id: impl fmt::Display,
    field: &'static str,
    value: Pixels,
) -> Result<i64, JsonCanvasError> {
    if !value.as_f32().is_finite() {
        return Err(JsonCanvasError::InvalidGeometry {
            id: id.to_string(),
            field,
        });
    }

    Ok(value.as_f32().round() as i64)
}

fn nonnegative_pixel_to_i64(
    id: impl fmt::Display,
    field: &'static str,
    value: Pixels,
) -> Result<i64, JsonCanvasError> {
    if !value.as_f32().is_finite() || value < Pixels::ZERO {
        return Err(JsonCanvasError::InvalidGeometry {
            id: id.to_string(),
            field,
        });
    }

    Ok(value.as_f32().round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EdgeId, HandleId};
    use open_gpui::{point, px, size};
    use serde_json::json;

    #[test]
    fn imports_json_canvas_text_file_group_and_edges() {
        let input = r##"{
            "nodes": [
                {
                    "id": "note",
                    "type": "text",
                    "x": -10,
                    "y": 20,
                    "width": 200,
                    "height": 120,
                    "color": "#ff0000",
                    "text": "Hello"
                },
                {
                    "id": "file",
                    "type": "file",
                    "x": 300,
                    "y": 20,
                    "width": 180,
                    "height": 90,
                    "file": "docs/spec.md",
                    "subpath": "#section"
                },
                {
                    "id": "group",
                    "type": "group",
                    "x": -40,
                    "y": 0,
                    "width": 560,
                    "height": 180,
                    "label": "Cluster"
                }
            ],
            "edges": [
                {
                    "id": "edge",
                    "fromNode": "note",
                    "fromSide": "right",
                    "fromEnd": "none",
                    "toNode": "file",
                    "toSide": "left",
                    "toEnd": "arrow",
                    "color": "1",
                    "label": "reads"
                }
            ]
        }"##;

        let document = document_from_json_canvas_str(input).unwrap();

        let note = document.nodes.get(&NodeId::from("note")).unwrap();
        assert_eq!(note.kind, TEXT_NODE_TYPE);
        assert_eq!(note.position, point(px(-10.0), px(20.0)));
        assert_eq!(note.size, size(px(200.0), px(120.0)));
        assert_eq!(note.z_index, 0);
        assert_eq!(note.style.fill.as_deref(), Some("#ff0000"));
        assert_eq!(note.data.get(TEXT_FIELD), Some(&json!("Hello")));
        assert!(
            note.handle(Some(&HandleId::from("json_canvas:right")))
                .is_some()
        );

        let file = document.nodes.get(&NodeId::from("file")).unwrap();
        assert_eq!(file.data.get(FILE_FIELD), Some(&json!("docs/spec.md")));
        assert_eq!(file.data.get(SUBPATH_FIELD), Some(&json!("#section")));
        assert!(
            file.handle(Some(&HandleId::from("json_canvas:left")))
                .is_some()
        );

        let edge = document.edges.get(&EdgeId::from("edge")).unwrap();
        assert_eq!(edge.source.node_id, NodeId::from("note"));
        assert_eq!(
            edge.source.handle_id,
            Some(HandleId::from("json_canvas:right"))
        );
        assert_eq!(edge.target.node_id, NodeId::from("file"));
        assert_eq!(
            edge.target.handle_id,
            Some(HandleId::from("json_canvas:left"))
        );
        assert_eq!(edge.style.stroke.as_deref(), Some("1"));
        assert_eq!(edge.data.get(LABEL_FIELD), Some(&json!("reads")));
        assert_eq!(edge.data.get(FROM_END_FIELD), Some(&json!("none")));
        assert_eq!(edge.data.get(TO_END_FIELD), Some(&json!("arrow")));
    }

    #[test]
    fn exports_json_canvas_nodes_by_z_index() {
        let mut document = CanvasDocument::default();
        let mut front = CanvasNode::new(
            "front",
            point(px(100.0), px(0.0)),
            size(px(120.0), px(80.0)),
        );
        front.kind = TEXT_NODE_TYPE.to_string();
        front.z_index = 10;
        front.data.insert(TEXT_FIELD.to_string(), json!("Front"));
        let mut back = CanvasNode::new("back", point(px(0.0), px(0.0)), size(px(120.0), px(80.0)));
        back.kind = GROUP_NODE_TYPE.to_string();
        back.z_index = -1;
        back.data.insert(LABEL_FIELD.to_string(), json!("Back"));

        document.insert_node(front).unwrap();
        document.insert_node(back).unwrap();

        let json_canvas = JsonCanvas::from_document(&document).unwrap();

        assert_eq!(json_canvas.nodes[0].id, "back");
        assert_eq!(json_canvas.nodes[0].label.as_deref(), Some("Back"));
        assert_eq!(json_canvas.nodes[1].id, "front");
        assert_eq!(json_canvas.nodes[1].text.as_deref(), Some("Front"));
    }

    #[test]
    fn exports_json_canvas_edge_sides_from_handles() {
        let mut document = CanvasDocument::default();
        let mut source = CanvasNode::new(
            "source",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        );
        source.kind = TEXT_NODE_TYPE.to_string();
        source.data.insert(TEXT_FIELD.to_string(), json!("Source"));
        source.handles.push(CanvasHandle::new(
            "json_canvas:right",
            point(px(100.0), px(50.0)),
        ));
        let mut target = CanvasNode::new(
            "target",
            point(px(200.0), px(0.0)),
            size(px(100.0), px(100.0)),
        );
        target.kind = TEXT_NODE_TYPE.to_string();
        target.data.insert(TEXT_FIELD.to_string(), json!("Target"));
        target.handles.push(CanvasHandle::new(
            "json_canvas:left",
            point(px(0.0), px(50.0)),
        ));

        document.insert_node(source).unwrap();
        document.insert_node(target).unwrap();
        let mut edge = CanvasEdge::new(
            "edge",
            CanvasEndpoint::new("source", Some("json_canvas:right")),
            CanvasEndpoint::new("target", Some("json_canvas:left")),
        );
        edge.style.stroke = Some("2".to_string());
        edge.data
            .insert(LABEL_FIELD.to_string(), json!("depends on"));
        edge.data.insert(
            TO_END_FIELD.to_string(),
            json!(JsonCanvasEndpointShape::Arrow.as_str()),
        );
        document.insert_edge(edge).unwrap();

        let json_canvas = JsonCanvas::from_document(&document).unwrap();
        let edge = &json_canvas.edges[0];

        assert_eq!(edge.from_side, Some(JsonCanvasSide::Right));
        assert_eq!(edge.to_side, Some(JsonCanvasSide::Left));
        assert_eq!(edge.to_end, Some(JsonCanvasEndpointShape::Arrow));
        assert_eq!(edge.color.as_deref(), Some("2"));
        assert_eq!(edge.label.as_deref(), Some("depends on"));
    }

    #[test]
    fn rejects_missing_required_node_fields() {
        let input = r#"{
            "nodes": [
                { "id": "note", "type": "text", "x": 0, "y": 0, "width": 100, "height": 100 }
            ]
        }"#;

        let err = document_from_json_canvas_str(input).unwrap_err();

        assert!(matches!(
            err,
            JsonCanvasError::MissingNodeField {
                node_id,
                kind,
                field: TEXT_FIELD
            } if node_id == "note" && kind == TEXT_NODE_TYPE
        ));
    }

    #[test]
    fn rejects_unsupported_node_types() {
        let input = r#"{
            "nodes": [
                { "id": "shape", "type": "shape", "x": 0, "y": 0, "width": 100, "height": 100 }
            ]
        }"#;

        let err = document_from_json_canvas_str(input).unwrap_err();

        assert!(matches!(
            err,
            JsonCanvasError::UnsupportedNodeKind { node_id, kind }
                if node_id == "shape" && kind == "shape"
        ));
    }

    #[test]
    fn rejects_duplicate_node_ids_on_import() {
        let input = r#"{
            "nodes": [
                { "id": "note", "type": "text", "x": 0, "y": 0, "width": 100, "height": 100, "text": "A" },
                { "id": "note", "type": "text", "x": 120, "y": 0, "width": 100, "height": 100, "text": "B" }
            ]
        }"#;

        let err = document_from_json_canvas_str(input).unwrap_err();

        assert!(matches!(
            err,
            JsonCanvasError::Document(DocumentError::DuplicateNode(id)) if id == NodeId::from("note")
        ));
    }

    #[test]
    fn rejects_exporting_incomplete_json_canvas_nodes() {
        let mut document = CanvasDocument::default();
        let mut node = CanvasNode::new("note", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        node.kind = TEXT_NODE_TYPE.to_string();
        document.insert_node(node).unwrap();

        let err = JsonCanvas::from_document(&document).unwrap_err();

        assert!(matches!(
            err,
            JsonCanvasError::MissingNodeField {
                node_id,
                kind,
                field: TEXT_FIELD
            } if node_id == "note" && kind == TEXT_NODE_TYPE
        ));
    }
}
