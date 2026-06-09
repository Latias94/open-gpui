use crate::{
    CanvasDocument, CanvasEdge, CanvasEndpoint, CanvasNode, CanvasSelection, CanvasShape,
    CanvasTransaction, DocumentCommand, EdgeId, NodeId, ShapeId,
};
use indexmap::IndexMap;
use open_gpui::{Pixels, Point};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasClipboardPayload {
    #[serde(default)]
    pub nodes: Vec<CanvasNode>,
    #[serde(default)]
    pub edges: Vec<CanvasEdge>,
    #[serde(default)]
    pub shapes: Vec<CanvasShape>,
    #[serde(default)]
    pub selection: CanvasSelection,
}

impl CanvasClipboardPayload {
    pub fn from_document_selection(document: &CanvasDocument, selection: &CanvasSelection) -> Self {
        let nodes = selection
            .selected_nodes()
            .filter_map(|id| document.node(id))
            .filter(|node| !node.locked)
            .cloned()
            .collect::<Vec<_>>();
        let shapes = selection
            .selected_shapes()
            .filter_map(|id| document.shape(id))
            .filter(|shape| !shape.locked)
            .cloned()
            .collect::<Vec<_>>();

        let selected_node_ids = nodes
            .iter()
            .map(|node| node.id.clone())
            .collect::<indexmap::IndexSet<_>>();
        let mut edges = selection
            .selected_edges()
            .filter_map(|id| document.edge(id))
            .filter(|edge| !edge.locked)
            .cloned()
            .collect::<Vec<_>>();
        let selected_edge_ids = edges
            .iter()
            .map(|edge| edge.id.clone())
            .collect::<indexmap::IndexSet<_>>();
        edges.extend(
            document
                .edges()
                .filter(|edge| {
                    !edge.locked
                        && selected_node_ids.contains(&edge.source.node_id)
                        && selected_node_ids.contains(&edge.target.node_id)
                        && !selected_edge_ids.contains(&edge.id)
                })
                .cloned(),
        );

        let mut payload_selection = CanvasSelection::default();
        payload_selection
            .nodes
            .extend(nodes.iter().map(|node| node.id.clone()));
        payload_selection
            .edges
            .extend(edges.iter().map(|edge| edge.id.clone()));
        payload_selection
            .shapes
            .extend(shapes.iter().map(|shape| shape.id.clone()));

        Self {
            nodes,
            edges,
            shapes,
            selection: payload_selection,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.edges.is_empty() && self.shapes.is_empty()
    }

    pub fn paste_transaction(
        &self,
        document: &CanvasDocument,
        offset: Point<Pixels>,
    ) -> CanvasPasteTransaction {
        let node_ids = remap_ids(
            self.nodes.iter().map(|node| node.id.as_str()),
            |id| document.contains_node(&NodeId::from(id.to_owned())),
            unique_node_id,
        );
        let edge_ids = remap_ids(
            self.edges.iter().map(|edge| edge.id.as_str()),
            |id| document.contains_edge(&EdgeId::from(id.to_owned())),
            unique_edge_id,
        );
        let shape_ids = remap_ids(
            self.shapes.iter().map(|shape| shape.id.as_str()),
            |id| document.contains_shape(&ShapeId::from(id.to_owned())),
            unique_shape_id,
        );

        let mut selection = CanvasSelection::default();
        let mut commands = Vec::new();

        for node in &self.nodes {
            let mut node = node.clone();
            node.id = node_ids[&node.id].clone();
            node.position += offset;
            selection.nodes.insert(node.id.clone());
            commands.push(DocumentCommand::InsertNode(node));
        }

        for shape in &self.shapes {
            let mut shape = shape.clone();
            shape.id = shape_ids[&shape.id].clone();
            shape.bounds.origin += offset;
            selection.shapes.insert(shape.id.clone());
            commands.push(DocumentCommand::InsertShape(shape));
        }

        for edge in &self.edges {
            let Some(source) = remap_endpoint(&edge.source, &node_ids) else {
                continue;
            };
            let Some(target) = remap_endpoint(&edge.target, &node_ids) else {
                continue;
            };
            let mut edge = edge.clone();
            edge.id = edge_ids[&edge.id].clone();
            edge.source = source;
            edge.target = target;
            selection.edges.insert(edge.id.clone());
            commands.push(DocumentCommand::InsertEdge(edge));
        }

        CanvasPasteTransaction {
            transaction: CanvasTransaction::new(commands),
            selection,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPasteTransaction {
    pub transaction: CanvasTransaction,
    pub selection: CanvasSelection,
}

fn remap_ids<'a, I, T, Exists, Unique>(ids: I, exists: Exists, unique: Unique) -> IndexMap<T, T>
where
    I: IntoIterator<Item = &'a str>,
    T: Clone + From<String> + std::hash::Hash + Eq,
    Exists: Fn(&str) -> bool,
    Unique: Fn(&str, &mut dyn FnMut(&str) -> bool) -> T,
{
    let mut remapped = IndexMap::new();
    for id in ids {
        let mut taken = |candidate: &str| {
            exists(candidate)
                || remapped
                    .values()
                    .any(|mapped: &T| mapped.clone() == T::from(candidate.to_owned()))
        };
        remapped.insert(T::from(id.to_owned()), unique(id, &mut taken));
    }
    remapped
}

fn unique_node_id(base: &str, taken: &mut dyn FnMut(&str) -> bool) -> NodeId {
    NodeId::new(unique_id(base, taken))
}

fn unique_edge_id(base: &str, taken: &mut dyn FnMut(&str) -> bool) -> EdgeId {
    EdgeId::new(unique_id(base, taken))
}

fn unique_shape_id(base: &str, taken: &mut dyn FnMut(&str) -> bool) -> ShapeId {
    ShapeId::new(unique_id(base, taken))
}

fn unique_id(base: &str, taken: &mut dyn FnMut(&str) -> bool) -> String {
    let copy = format!("{base}-copy");
    if !taken(&copy) {
        return copy;
    }

    for index in 2.. {
        let candidate = format!("{base}-copy-{index}");
        if !taken(&candidate) {
            return candidate;
        }
    }

    unreachable!("unbounded id search should always return")
}

fn remap_endpoint(
    endpoint: &CanvasEndpoint,
    node_ids: &IndexMap<NodeId, NodeId>,
) -> Option<CanvasEndpoint> {
    node_ids
        .get(&endpoint.node_id)
        .cloned()
        .map(|node_id| CanvasEndpoint {
            node_id,
            handle_id: endpoint.handle_id.clone(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanvasEdge, CanvasEndpoint, CanvasNode, CanvasShape};
    use open_gpui::{Bounds, point, px, size};

    #[test]
    fn copy_selection_includes_internal_edges() {
        let document = connected_document();
        let mut selection = CanvasSelection::default();
        selection.nodes.insert(NodeId::from("a"));
        selection.nodes.insert(NodeId::from("b"));

        let payload = CanvasClipboardPayload::from_document_selection(&document, &selection);

        assert_eq!(
            payload
                .nodes
                .iter()
                .map(|node| &node.id)
                .collect::<Vec<_>>(),
            vec![&NodeId::from("a"), &NodeId::from("b")]
        );
        assert_eq!(
            payload
                .edges
                .iter()
                .map(|edge| &edge.id)
                .collect::<Vec<_>>(),
            vec![&EdgeId::from("a-b")]
        );
    }

    #[test]
    fn copy_selection_omits_external_edges() {
        let document = connected_document();
        let mut selection = CanvasSelection::default();
        selection.nodes.insert(NodeId::from("a"));

        let payload = CanvasClipboardPayload::from_document_selection(&document, &selection);

        assert_eq!(payload.nodes.len(), 1);
        assert!(payload.edges.is_empty());
    }

    #[test]
    fn paste_payload_remaps_ids_and_offsets_records() {
        let mut document = connected_document();
        document
            .insert_shape(CanvasShape::new(
                "note",
                Bounds::new(point(px(0.0), px(20.0)), size(px(30.0), px(30.0))),
            ))
            .unwrap();
        let mut selection = CanvasSelection::default();
        selection.nodes.insert(NodeId::from("a"));
        selection.nodes.insert(NodeId::from("b"));
        selection.shapes.insert(ShapeId::from("note"));
        let payload = CanvasClipboardPayload::from_document_selection(&document, &selection);

        let pasted = payload.paste_transaction(&document, point(px(16.0), px(24.0)));

        assert_eq!(
            pasted.selection.nodes.iter().cloned().collect::<Vec<_>>(),
            vec![NodeId::from("a-copy"), NodeId::from("b-copy")]
        );
        assert_eq!(
            pasted.selection.edges.iter().cloned().collect::<Vec<_>>(),
            vec![EdgeId::from("a-b-copy")]
        );
        assert_eq!(
            pasted.selection.shapes.iter().cloned().collect::<Vec<_>>(),
            vec![ShapeId::from("note-copy")]
        );

        let mut draft = document.clone();
        draft.apply_transaction(pasted.transaction).unwrap();
        assert_eq!(
            draft.node(&NodeId::from("a-copy")).unwrap().position,
            point(px(16.0), px(24.0))
        );
        assert_eq!(
            draft
                .edge(&EdgeId::from("a-b-copy"))
                .unwrap()
                .source
                .node_id,
            NodeId::from("a-copy")
        );
        assert_eq!(
            draft
                .shape(&ShapeId::from("note-copy"))
                .unwrap()
                .bounds
                .origin,
            point(px(16.0), px(44.0))
        );
    }

    fn connected_document() -> CanvasDocument {
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
                point(px(40.0), px(0.0)),
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
        document
    }
}
