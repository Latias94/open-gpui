use crate::{
    BindingId, CanvasDocument, CanvasEdge, CanvasEndpoint, CanvasNode, CanvasRecordId,
    CanvasSelection, CanvasShape, CanvasTransaction, DocumentCommand, EdgeId, NodeId, ShapeId,
    record_scope::{CanvasRecordScopeOptions, resolve_selection_scope_with_predicates},
    relations::CanvasRecordRelations,
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
    #[serde(default)]
    pub relations: CanvasRecordRelations,
}

impl CanvasClipboardPayload {
    pub fn from_document_selection(document: &CanvasDocument, selection: &CanvasSelection) -> Self {
        let scope = resolve_selection_scope_with_predicates(
            document,
            selection,
            CanvasRecordScopeOptions::structural_with_internal_edges(),
            |record_id| document.contains_record(record_id),
            |record_id| is_copyable_record(document, record_id),
        );
        let copied_records = scope.action_records();
        let selection = copied_selection(scope.normalized_selection(), copied_records);

        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut shapes = Vec::new();
        for record_id in copied_records.records() {
            match record_id {
                CanvasRecordId::Node(id) => {
                    if let Some(node) = document.node(id) {
                        nodes.push(node.clone());
                    }
                }
                CanvasRecordId::Edge(id) => {
                    if let Some(edge) = document.edge(id) {
                        edges.push(edge.clone());
                    }
                }
                CanvasRecordId::Shape(id) => {
                    if let Some(shape) = document.shape(id) {
                        shapes.push(shape.clone());
                    }
                }
            }
        }

        Self {
            nodes,
            edges,
            shapes,
            selection,
            relations: document
                .relations()
                .subset_for_records(|record_id| copied_records.contains(record_id)),
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

        let mut fallback_selection = CanvasSelection::default();
        let mut commands = Vec::new();

        for node in &self.nodes {
            let mut node = node.clone();
            node.id = node_ids[&node.id].clone();
            node.position += offset;
            fallback_selection.insert_node(node.id.clone());
            commands.push(DocumentCommand::InsertNode(node));
        }

        for shape in &self.shapes {
            let mut shape = shape.clone();
            shape.id = shape_ids[&shape.id].clone();
            shape.bounds.origin += offset;
            fallback_selection.insert_shape(shape.id.clone());
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
            fallback_selection.insert_edge(edge.id.clone());
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

        let selection = remap_selection(&self.selection, &node_ids, &pasted_edge_ids, &shape_ids)
            .filter(|selection| !selection.is_empty())
            .unwrap_or(fallback_selection);

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

fn is_copyable_record(document: &CanvasDocument, record_id: &CanvasRecordId) -> bool {
    match record_id {
        CanvasRecordId::Node(id) => document.node(id).is_some_and(|node| !node.locked),
        CanvasRecordId::Edge(id) => document.edge(id).is_some_and(|edge| !edge.locked),
        CanvasRecordId::Shape(id) => document.shape(id).is_some_and(|shape| !shape.locked),
    }
}

fn copied_selection(
    selection: &CanvasSelection,
    copied_records: &crate::record_scope::CanvasRecordScope,
) -> CanvasSelection {
    let mut copied_selection = CanvasSelection::default();
    for record_id in selection
        .selected_records()
        .filter(|record_id| copied_records.contains(record_id))
    {
        copied_selection.insert_record(record_id);
    }
    for endpoint in selection.selected_handles() {
        if copied_records.contains(&CanvasRecordId::Node(endpoint.node_id.clone())) {
            copied_selection.insert_handle(endpoint.clone());
        }
    }
    copied_selection
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

fn remap_selection(
    selection: &CanvasSelection,
    node_ids: &IndexMap<NodeId, NodeId>,
    edge_ids: &IndexMap<EdgeId, EdgeId>,
    shape_ids: &IndexMap<ShapeId, ShapeId>,
) -> Option<CanvasSelection> {
    let mut remapped = CanvasSelection::default();
    for record_id in selection.selected_records() {
        remapped.insert_record(remap_record_id(&record_id, node_ids, edge_ids, shape_ids)?);
    }
    for endpoint in selection.selected_handles() {
        remapped.insert_handle(remap_endpoint(endpoint, node_ids)?);
    }

    Some(remapped)
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
        BindingId, CanvasEdge, CanvasEndpoint, CanvasNode, CanvasShape,
        relations::CanvasRecordBindingRelation,
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
            Vec::<EdgeId>::new()
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
        assert!(draft.contains_edge(&EdgeId::from("a-b-copy")));
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
    fn copy_selection_expands_related_descendants() {
        let document = related_tree_document();
        let mut selection = CanvasSelection::default();
        selection.insert_shape(ShapeId::from("frame"));

        let payload = CanvasClipboardPayload::from_document_selection(&document, &selection);

        assert_eq!(
            payload
                .nodes
                .iter()
                .map(|node| &node.id)
                .collect::<Vec<_>>(),
            vec![&NodeId::from("child"), &NodeId::from("peer")]
        );
        assert_eq!(
            payload
                .edges
                .iter()
                .map(|edge| &edge.id)
                .collect::<Vec<_>>(),
            vec![&EdgeId::from("child-peer")]
        );
        assert_eq!(
            payload
                .shapes
                .iter()
                .map(|shape| &shape.id)
                .collect::<Vec<_>>(),
            vec![&ShapeId::from("frame"), &ShapeId::from("group")]
        );

        let frame = CanvasRecordId::Shape(ShapeId::from("frame"));
        let group = CanvasRecordId::Shape(ShapeId::from("group"));
        let child = CanvasRecordId::Node(NodeId::from("child"));
        let peer = CanvasRecordId::Node(NodeId::from("peer"));
        assert_eq!(payload.relations.parent_of(&group), Some(&frame));
        assert_eq!(
            payload
                .relations
                .members_of(&group)
                .cloned()
                .collect::<Vec<_>>(),
            vec![child.clone(), peer.clone()]
        );
        assert_eq!(
            payload.relations.binding(&BindingId::from("binding")),
            Some(&CanvasRecordBindingRelation::new("binding", child, group))
        );
        assert_eq!(
            payload
                .selection
                .selected_shapes()
                .cloned()
                .collect::<Vec<_>>(),
            vec![ShapeId::from("frame")]
        );
        assert!(payload.selection.selected_nodes().next().is_none());
        assert!(payload.selection.selected_edges().next().is_none());
    }

    #[test]
    fn paste_payload_selects_remapped_explicit_roots() {
        let document = related_tree_document();
        let mut selection = CanvasSelection::default();
        selection.insert_shape(ShapeId::from("frame"));
        let payload = CanvasClipboardPayload::from_document_selection(&document, &selection);

        let pasted = payload.paste_transaction(&document, point(px(16.0), px(24.0)));

        assert_eq!(
            pasted
                .selection
                .selected_shapes()
                .cloned()
                .collect::<Vec<_>>(),
            vec![ShapeId::from("frame-copy")]
        );
        assert!(pasted.selection.selected_nodes().next().is_none());
        assert!(pasted.selection.selected_edges().next().is_none());
    }

    #[test]
    fn copy_selection_filters_payload_selection_to_copied_records() {
        let mut locked =
            CanvasNode::new("locked", point(px(40.0), px(0.0)), size(px(10.0), px(10.0)));
        locked.locked = true;
        let document = document_fixture()
            .node(CanvasNode::new(
                "copyable",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .node(locked)
            .build();
        let mut selection = CanvasSelection::default();
        selection.insert_node(NodeId::from("copyable"));
        selection.insert_node(NodeId::from("locked"));

        let payload = CanvasClipboardPayload::from_document_selection(&document, &selection);
        let pasted = payload.paste_transaction(&document, point(px(16.0), px(24.0)));

        assert_eq!(
            payload
                .nodes
                .iter()
                .map(|node| &node.id)
                .collect::<Vec<_>>(),
            vec![&NodeId::from("copyable")]
        );
        assert_eq!(
            payload
                .selection
                .selected_nodes()
                .cloned()
                .collect::<Vec<_>>(),
            vec![NodeId::from("copyable")]
        );
        assert_eq!(
            pasted
                .selection
                .selected_nodes()
                .cloned()
                .collect::<Vec<_>>(),
            vec![NodeId::from("copyable-copy")]
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

    fn related_tree_document() -> CanvasDocument {
        let mut document = document_fixture()
            .shape(CanvasShape::new(
                "frame",
                Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(200.0))),
            ))
            .shape(CanvasShape::new(
                "group",
                Bounds::new(point(px(20.0), px(20.0)), size(px(100.0), px(100.0))),
            ))
            .node(CanvasNode::new(
                "child",
                point(px(40.0), px(40.0)),
                size(px(10.0), px(10.0)),
            ))
            .node(CanvasNode::new(
                "peer",
                point(px(80.0), px(40.0)),
                size(px(10.0), px(10.0)),
            ))
            .edge(CanvasEdge::new(
                "child-peer",
                CanvasEndpoint::new("child", None::<&str>),
                CanvasEndpoint::new("peer", None::<&str>),
            ))
            .build();
        document
            .apply_transaction(CanvasTransaction::new([
                DocumentCommand::SetRecordParent {
                    child: CanvasRecordId::Shape(ShapeId::from("group")),
                    parent: CanvasRecordId::Shape(ShapeId::from("frame")),
                },
                DocumentCommand::AddRecordToGroup {
                    group: CanvasRecordId::Shape(ShapeId::from("group")),
                    member: CanvasRecordId::Node(NodeId::from("child")),
                },
                DocumentCommand::AddRecordToGroup {
                    group: CanvasRecordId::Shape(ShapeId::from("group")),
                    member: CanvasRecordId::Node(NodeId::from("peer")),
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
