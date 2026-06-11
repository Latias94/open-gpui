use crate::{
    BindingId, CanvasDocument, CanvasEdge, CanvasEndpoint, CanvasNode, CanvasRecordId,
    CanvasRecordRelations, CanvasSelection, CanvasShape, CanvasTransaction, DocumentCommand,
    EdgeId, NodeId, ShapeId,
};
use indexmap::{IndexMap, IndexSet};
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
    #[serde(default)]
    pub relations: CanvasRecordRelations,
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
            .collect::<IndexSet<_>>();
        let mut edges = selection
            .selected_edges()
            .filter_map(|id| document.edge(id))
            .filter(|edge| !edge.locked)
            .cloned()
            .collect::<Vec<_>>();
        let selected_edge_ids = edges
            .iter()
            .map(|edge| edge.id.clone())
            .collect::<IndexSet<_>>();
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
        for node in &nodes {
            payload_selection.insert_node(node.id.clone());
        }
        for edge in &edges {
            payload_selection.insert_edge(edge.id.clone());
        }
        for shape in &shapes {
            payload_selection.insert_shape(shape.id.clone());
        }

        let copied_record_ids = copied_record_ids(&nodes, &edges, &shapes);
        let mut relations = CanvasRecordRelations::builder();
        for relation in document.relations().parents() {
            if copied_record_ids.contains(&relation.child)
                && copied_record_ids.contains(&relation.parent)
            {
                relations.add_parent(relation.child.clone(), relation.parent.clone());
            }
        }
        for relation in document.relations().groups() {
            if copied_record_ids.contains(&relation.group)
                && copied_record_ids.contains(&relation.member)
            {
                relations.add_group_member(relation.group.clone(), relation.member.clone());
            }
        }
        for relation in document.relations().bindings() {
            if copied_record_ids.contains(&relation.source)
                && copied_record_ids.contains(&relation.target)
            {
                relations.add_binding(relation.clone());
            }
        }

        Self {
            nodes,
            edges,
            shapes,
            selection: payload_selection,
            relations: relations.build(),
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
        let binding_ids = remap_ids(
            self.relations.bindings().map(|binding| binding.id.as_str()),
            |id| {
                document
                    .relations()
                    .binding(&BindingId::from(id.to_owned()))
                    .is_some()
            },
            unique_binding_id,
        );

        let mut selection = CanvasSelection::default();
        let mut commands = Vec::new();

        for node in &self.nodes {
            let mut node = node.clone();
            node.id = node_ids[&node.id].clone();
            node.position += offset;
            selection.insert_node(node.id.clone());
            commands.push(DocumentCommand::InsertNode(node));
        }

        for shape in &self.shapes {
            let mut shape = shape.clone();
            shape.id = shape_ids[&shape.id].clone();
            shape.bounds.origin += offset;
            selection.insert_shape(shape.id.clone());
            commands.push(DocumentCommand::InsertShape(shape));
        }

        let mut pasted_edge_ids = IndexMap::new();
        for edge in &self.edges {
            let Some(source) = remap_endpoint(&edge.source, &node_ids) else {
                continue;
            };
            let Some(target) = remap_endpoint(&edge.target, &node_ids) else {
                continue;
            };
            let mut edge = edge.clone();
            let original_edge_id = edge.id.clone();
            edge.id = edge_ids[&original_edge_id].clone();
            edge.source = source;
            edge.target = target;
            selection.insert_edge(edge.id.clone());
            pasted_edge_ids.insert(original_edge_id, edge.id.clone());
            commands.push(DocumentCommand::InsertEdge(edge));
        }

        for relation in self.relations.parents() {
            let Some(child) =
                remap_record_id(&relation.child, &node_ids, &pasted_edge_ids, &shape_ids)
            else {
                continue;
            };
            let Some(parent) =
                remap_record_id(&relation.parent, &node_ids, &pasted_edge_ids, &shape_ids)
            else {
                continue;
            };
            commands.push(DocumentCommand::SetRecordParent { child, parent });
        }

        for relation in self.relations.groups() {
            let Some(group) =
                remap_record_id(&relation.group, &node_ids, &pasted_edge_ids, &shape_ids)
            else {
                continue;
            };
            let Some(member) =
                remap_record_id(&relation.member, &node_ids, &pasted_edge_ids, &shape_ids)
            else {
                continue;
            };
            commands.push(DocumentCommand::AddRecordToGroup { group, member });
        }

        for relation in self.relations.bindings() {
            let Some(source) =
                remap_record_id(&relation.source, &node_ids, &pasted_edge_ids, &shape_ids)
            else {
                continue;
            };
            let Some(target) =
                remap_record_id(&relation.target, &node_ids, &pasted_edge_ids, &shape_ids)
            else {
                continue;
            };
            let mut binding = relation.clone();
            binding.id = binding_ids[&binding.id].clone();
            binding.source = source;
            binding.target = target;
            commands.push(DocumentCommand::SetRecordBinding(binding));
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

fn copied_record_ids(
    nodes: &[CanvasNode],
    edges: &[CanvasEdge],
    shapes: &[CanvasShape],
) -> IndexSet<CanvasRecordId> {
    nodes
        .iter()
        .map(|node| CanvasRecordId::Node(node.id.clone()))
        .chain(
            edges
                .iter()
                .map(|edge| CanvasRecordId::Edge(edge.id.clone())),
        )
        .chain(
            shapes
                .iter()
                .map(|shape| CanvasRecordId::Shape(shape.id.clone())),
        )
        .collect()
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

fn unique_binding_id(base: &str, taken: &mut dyn FnMut(&str) -> bool) -> BindingId {
    BindingId::new(unique_id(base, taken))
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

fn remap_record_id(
    id: &CanvasRecordId,
    node_ids: &IndexMap<NodeId, NodeId>,
    edge_ids: &IndexMap<EdgeId, EdgeId>,
    shape_ids: &IndexMap<ShapeId, ShapeId>,
) -> Option<CanvasRecordId> {
    match id {
        CanvasRecordId::Node(id) => node_ids.get(id).cloned().map(CanvasRecordId::Node),
        CanvasRecordId::Edge(id) => edge_ids.get(id).cloned().map(CanvasRecordId::Edge),
        CanvasRecordId::Shape(id) => shape_ids.get(id).cloned().map(CanvasRecordId::Shape),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::document_fixture;
    use crate::{
        BindingId, CanvasEdge, CanvasEndpoint, CanvasNode, CanvasRecordBindingRelation, CanvasShape,
    };
    use open_gpui::{Bounds, point, px, size};

    #[test]
    fn copy_selection_includes_internal_edges() {
        let document = connected_document();
        let mut selection = CanvasSelection::default();
        selection.insert_node(NodeId::from("a"));
        selection.insert_node(NodeId::from("b"));

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
        selection.insert_node(NodeId::from("a"));

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
        selection.insert_node(NodeId::from("a"));
        selection.insert_node(NodeId::from("b"));
        selection.insert_shape(ShapeId::from("note"));
        let payload = CanvasClipboardPayload::from_document_selection(&document, &selection);

        let pasted = payload.paste_transaction(&document, point(px(16.0), px(24.0)));

        assert_eq!(
            pasted
                .selection
                .selected_nodes()
                .cloned()
                .collect::<Vec<_>>(),
            vec![NodeId::from("a-copy"), NodeId::from("b-copy")]
        );
        assert_eq!(
            pasted
                .selection
                .selected_edges()
                .cloned()
                .collect::<Vec<_>>(),
            vec![EdgeId::from("a-b-copy")]
        );
        assert_eq!(
            pasted
                .selection
                .selected_shapes()
                .cloned()
                .collect::<Vec<_>>(),
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

    #[test]
    fn copy_selection_includes_internal_relations() {
        let document = related_document();
        let mut selection = CanvasSelection::default();
        selection.insert_node(NodeId::from("child"));
        selection.insert_shape(ShapeId::from("group"));

        let payload = CanvasClipboardPayload::from_document_selection(&document, &selection);

        let child = CanvasRecordId::Node(NodeId::from("child"));
        let group = CanvasRecordId::Shape(ShapeId::from("group"));
        assert_eq!(payload.relations.parent_of(&child), Some(&group));
        assert_eq!(
            payload
                .relations
                .members_of(&group)
                .cloned()
                .collect::<Vec<_>>(),
            vec![child.clone()]
        );
        assert_eq!(
            payload.relations.binding(&BindingId::from("binding")),
            Some(&CanvasRecordBindingRelation::new(
                "binding",
                child,
                group.clone()
            ))
        );
    }

    #[test]
    fn copy_selection_omits_external_relations() {
        let document = related_document();
        let mut selection = CanvasSelection::default();
        selection.insert_node(NodeId::from("child"));

        let payload = CanvasClipboardPayload::from_document_selection(&document, &selection);

        assert!(payload.relations.is_empty());
    }

    #[test]
    fn paste_payload_remaps_internal_relations() {
        let document = related_document();
        let mut selection = CanvasSelection::default();
        selection.insert_node(NodeId::from("child"));
        selection.insert_shape(ShapeId::from("group"));
        let payload = CanvasClipboardPayload::from_document_selection(&document, &selection);

        let pasted = payload.paste_transaction(&document, point(px(16.0), px(24.0)));

        let mut draft = document.clone();
        draft.apply_transaction(pasted.transaction).unwrap();

        let child = CanvasRecordId::Node(NodeId::from("child-copy"));
        let group = CanvasRecordId::Shape(ShapeId::from("group-copy"));
        assert_eq!(draft.relations().parent_of(&child), Some(&group));
        assert_eq!(
            draft
                .relations()
                .members_of(&group)
                .cloned()
                .collect::<Vec<_>>(),
            vec![child.clone()]
        );
        assert_eq!(
            draft.relations().binding(&BindingId::from("binding-copy")),
            Some(&CanvasRecordBindingRelation::new(
                "binding-copy",
                child,
                group
            ))
        );
    }

    #[test]
    fn deserializes_clipboard_payload_without_relations() {
        let payload = serde_json::from_str::<CanvasClipboardPayload>(
            r#"{"nodes":[],"edges":[],"shapes":[],"selection":{"nodes":[],"edges":[],"shapes":[],"handles":[]}}"#,
        )
        .unwrap();

        assert!(payload.relations.is_empty());
    }

    fn connected_document() -> CanvasDocument {
        document_fixture()
            .node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .node(CanvasNode::new(
                "b",
                point(px(40.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .build()
    }

    fn related_document() -> CanvasDocument {
        let mut document = document_fixture()
            .node(CanvasNode::new(
                "child",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .shape(CanvasShape::new(
                "group",
                Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
            ))
            .build();
        document
            .apply_transaction(CanvasTransaction::new([
                DocumentCommand::SetRecordParent {
                    child: CanvasRecordId::Node(NodeId::from("child")),
                    parent: CanvasRecordId::Shape(ShapeId::from("group")),
                },
                DocumentCommand::AddRecordToGroup {
                    group: CanvasRecordId::Shape(ShapeId::from("group")),
                    member: CanvasRecordId::Node(NodeId::from("child")),
                },
                DocumentCommand::SetRecordBinding(CanvasRecordBindingRelation::new(
                    "binding",
                    CanvasRecordId::Node(NodeId::from("child")),
                    CanvasRecordId::Shape(ShapeId::from("group")),
                )),
            ]))
            .unwrap();
        document
    }
}
