use crate::{CanvasDocument, CanvasRecordId, CanvasSelection};
use indexmap::IndexSet;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CanvasRecordScopeOptions {
    pub include_structural_descendants: bool,
    pub include_internal_edges: bool,
}

impl Default for CanvasRecordScopeOptions {
    fn default() -> Self {
        Self::structural()
    }
}

impl CanvasRecordScopeOptions {
    pub const fn explicit() -> Self {
        Self {
            include_structural_descendants: false,
            include_internal_edges: false,
        }
    }

    pub const fn structural() -> Self {
        Self {
            include_structural_descendants: true,
            include_internal_edges: false,
        }
    }

    pub const fn structural_with_internal_edges() -> Self {
        Self {
            include_structural_descendants: true,
            include_internal_edges: true,
        }
    }

    pub const fn with_structural_descendants(
        mut self,
        include_structural_descendants: bool,
    ) -> Self {
        self.include_structural_descendants = include_structural_descendants;
        self
    }

    pub const fn with_internal_edges(mut self, include_internal_edges: bool) -> Self {
        self.include_internal_edges = include_internal_edges;
        self
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CanvasRecordScope {
    records: IndexSet<CanvasRecordId>,
}

impl CanvasRecordScope {
    pub fn new(records: impl IntoIterator<Item = CanvasRecordId>) -> Self {
        Self {
            records: records.into_iter().collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn contains(&self, record_id: &CanvasRecordId) -> bool {
        self.records.contains(record_id)
    }

    pub fn records(&self) -> impl Iterator<Item = &CanvasRecordId> {
        self.records.iter()
    }

    pub fn into_records(self) -> Vec<CanvasRecordId> {
        self.records.into_iter().collect()
    }

    pub(crate) fn into_index_set(self) -> IndexSet<CanvasRecordId> {
        self.records
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CanvasResolvedSelectionScope {
    normalized_selection: CanvasSelection,
    explicit_records: CanvasRecordScope,
    structural_records: CanvasRecordScope,
    action_records: CanvasRecordScope,
}

impl CanvasResolvedSelectionScope {
    pub fn normalized_selection(&self) -> &CanvasSelection {
        &self.normalized_selection
    }

    pub fn explicit_records(&self) -> &CanvasRecordScope {
        &self.explicit_records
    }

    pub fn structural_records(&self) -> &CanvasRecordScope {
        &self.structural_records
    }

    pub fn action_records(&self) -> &CanvasRecordScope {
        &self.action_records
    }

    pub(crate) fn contains_action_record(&self, record_id: &CanvasRecordId) -> bool {
        self.action_records.contains(record_id)
    }

    pub(crate) fn contains_paint_structural_record(&self, record_id: &CanvasRecordId) -> bool {
        !self.explicit_records.contains(record_id) && self.action_records.contains(record_id)
    }

    pub(crate) fn paint_structural_records(&self) -> impl Iterator<Item = &CanvasRecordId> {
        let explicit_records = &self.explicit_records;
        self.action_records
            .records()
            .filter(move |record_id| !explicit_records.contains(record_id))
    }

    pub fn into_action_records(self) -> CanvasRecordScope {
        self.action_records
    }
}

pub fn normalize_selection(
    document: &CanvasDocument,
    selection: &CanvasSelection,
) -> CanvasSelection {
    let explicit_records = normalize_record_candidates(document, selection.selected_records());
    let mut normalized = CanvasSelection::default();
    for record_id in explicit_records {
        normalized.insert_record(record_id);
    }
    for endpoint in selection.selected_handles() {
        if document.validate_endpoint(endpoint).is_ok() {
            normalized.insert_handle(endpoint.clone());
        }
    }
    normalized
}

pub(crate) fn normalize_record_candidates(
    document: &CanvasDocument,
    candidates: impl IntoIterator<Item = CanvasRecordId>,
) -> IndexSet<CanvasRecordId> {
    let mut candidates = candidates
        .into_iter()
        .filter(|record_id| document.contains_record(record_id))
        .collect::<IndexSet<_>>();
    let snapshot = candidates.clone();
    candidates.retain(|record_id| !has_candidate_ancestor(document, record_id, &snapshot));
    if candidates
        .iter()
        .any(|record_id| matches!(record_id, CanvasRecordId::Edge(_)))
    {
        suppress_internal_edge_candidates(document, &mut candidates);
    }
    candidates
}

pub fn resolve_selection_scope(
    document: &CanvasDocument,
    selection: &CanvasSelection,
    options: CanvasRecordScopeOptions,
) -> CanvasResolvedSelectionScope {
    resolve_selection_scope_with_predicates(
        document,
        selection,
        options,
        |record_id| document.contains_record(record_id),
        |record_id| document.contains_record(record_id),
    )
}

pub(crate) fn resolve_selection_scope_with_predicates(
    document: &CanvasDocument,
    selection: &CanvasSelection,
    options: CanvasRecordScopeOptions,
    mut can_traverse: impl FnMut(&CanvasRecordId) -> bool,
    mut can_include: impl FnMut(&CanvasRecordId) -> bool,
) -> CanvasResolvedSelectionScope {
    let normalized_selection = normalize_selection(document, selection);
    let explicit_records = normalized_selection
        .selected_records()
        .collect::<IndexSet<_>>();
    let structural_records = if options.include_structural_descendants {
        document
            .relations()
            .collect_descendant_records(explicit_records.iter().cloned(), |record_id| {
                can_traverse(record_id)
            })
    } else {
        IndexSet::new()
    };

    let mut action_records = IndexSet::new();
    for record_id in explicit_records.iter().chain(structural_records.iter()) {
        if can_include(record_id) {
            action_records.insert(record_id.clone());
        }
    }

    if options.include_internal_edges {
        include_internal_edges(document, &mut action_records, &mut can_include);
    }

    CanvasResolvedSelectionScope {
        normalized_selection,
        explicit_records: CanvasRecordScope::new(explicit_records),
        structural_records: CanvasRecordScope::new(structural_records),
        action_records: CanvasRecordScope::new(action_records),
    }
}

pub(crate) fn collect_selection_record_scope(
    document: &CanvasDocument,
    selection: &CanvasSelection,
    options: CanvasRecordScopeOptions,
    mut can_include: impl FnMut(&CanvasRecordId) -> bool,
) -> IndexSet<CanvasRecordId> {
    resolve_selection_scope_with_predicates(
        document,
        selection,
        options,
        |record_id| document.contains_record(record_id),
        |record_id| can_include(record_id),
    )
    .into_action_records()
    .into_index_set()
}

pub fn selection_record_scope(
    document: &CanvasDocument,
    selection: &CanvasSelection,
    options: CanvasRecordScopeOptions,
) -> CanvasRecordScope {
    resolve_selection_scope(document, selection, options).into_action_records()
}

pub(crate) fn include_internal_edges(
    document: &CanvasDocument,
    records: &mut IndexSet<CanvasRecordId>,
    can_include: &mut impl FnMut(&CanvasRecordId) -> bool,
) {
    let selected_node_ids = records
        .iter()
        .filter_map(|record_id| match record_id {
            CanvasRecordId::Node(id) => Some(id.clone()),
            CanvasRecordId::Edge(_) | CanvasRecordId::Shape(_) => None,
        })
        .collect::<IndexSet<_>>();

    for edge in document.edges().filter(|edge| {
        selected_node_ids.contains(&edge.source.node_id)
            && selected_node_ids.contains(&edge.target.node_id)
    }) {
        let record_id = CanvasRecordId::Edge(edge.id.clone());
        if can_include(&record_id) {
            records.insert(record_id);
        }
    }
}

fn has_candidate_ancestor(
    document: &CanvasDocument,
    record_id: &CanvasRecordId,
    candidates: &IndexSet<CanvasRecordId>,
) -> bool {
    let mut pending = document
        .relations()
        .parent_of(record_id)
        .cloned()
        .into_iter()
        .chain(document.relations().groups_for(record_id).cloned())
        .collect::<Vec<_>>();
    let mut visited = IndexSet::new();

    while let Some(current) = pending.pop() {
        if !visited.insert(current.clone()) {
            continue;
        }
        if candidates.contains(&current) {
            return true;
        }
        pending.extend(document.relations().parent_of(&current).cloned());
        pending.extend(document.relations().groups_for(&current).cloned());
    }

    false
}

fn suppress_internal_edge_candidates(
    document: &CanvasDocument,
    candidates: &mut IndexSet<CanvasRecordId>,
) {
    let node_scope = document.relations().collect_related_records(
        candidates
            .iter()
            .filter(|record_id| !matches!(record_id, CanvasRecordId::Edge(_)))
            .cloned(),
        |record_id| document.contains_record(record_id),
    );
    let selected_node_ids = node_scope
        .iter()
        .filter_map(|record_id| match record_id {
            CanvasRecordId::Node(id) => Some(id.clone()),
            CanvasRecordId::Edge(_) | CanvasRecordId::Shape(_) => None,
        })
        .collect::<IndexSet<_>>();

    candidates.retain(|record_id| match record_id {
        CanvasRecordId::Edge(id) => document.edge(id).is_none_or(|edge| {
            !selected_node_ids.contains(&edge.source.node_id)
                || !selected_node_ids.contains(&edge.target.node_id)
        }),
        CanvasRecordId::Node(_) | CanvasRecordId::Shape(_) => true,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::document_fixture;
    use crate::{
        CanvasEdge, CanvasEndpoint, CanvasNode, CanvasShape, CanvasTransaction, DocumentCommand,
        EdgeId, NodeId, ShapeId,
    };
    use open_gpui::{Bounds, point, px, size};

    #[test]
    fn selection_candidates_normalize_descendants_to_selected_ancestor() {
        let mut document = document_fixture()
            .shape(CanvasShape::new(
                "frame",
                Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
            ))
            .node(CanvasNode::new(
                "child",
                point(px(10.0), px(10.0)),
                size(px(10.0), px(10.0)),
            ))
            .node(CanvasNode::new(
                "outside",
                point(px(120.0), px(10.0)),
                size(px(10.0), px(10.0)),
            ))
            .build();
        document
            .apply_transaction(CanvasTransaction::single(
                DocumentCommand::SetRecordParent {
                    child: CanvasRecordId::Node(NodeId::from("child")),
                    parent: CanvasRecordId::Shape(ShapeId::from("frame")),
                },
            ))
            .unwrap();
        let mut selection = CanvasSelection::default();
        selection.insert_shape(ShapeId::from("frame"));
        selection.insert_node(NodeId::from("child"));
        selection.insert_node(NodeId::from("outside"));

        let normalized = normalize_selection(&document, &selection);

        assert_eq!(
            normalized.selected_shapes().cloned().collect::<Vec<_>>(),
            vec![ShapeId::from("frame")]
        );
        assert_eq!(
            normalized.selected_nodes().cloned().collect::<Vec<_>>(),
            vec![NodeId::from("outside")]
        );
    }

    #[test]
    fn child_only_selection_stays_explicit() {
        let mut document = document_fixture()
            .shape(CanvasShape::new(
                "frame",
                Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
            ))
            .node(CanvasNode::new(
                "child",
                point(px(10.0), px(10.0)),
                size(px(10.0), px(10.0)),
            ))
            .build();
        document
            .apply_transaction(CanvasTransaction::single(
                DocumentCommand::SetRecordParent {
                    child: CanvasRecordId::Node(NodeId::from("child")),
                    parent: CanvasRecordId::Shape(ShapeId::from("frame")),
                },
            ))
            .unwrap();
        let mut selection = CanvasSelection::default();
        selection.insert_node(NodeId::from("child"));

        let normalized = normalize_selection(&document, &selection);

        assert_eq!(
            normalized.selected_nodes().cloned().collect::<Vec<_>>(),
            vec![NodeId::from("child")]
        );
        assert!(normalized.selected_shapes().next().is_none());
    }

    #[test]
    fn resolved_scope_separates_explicit_structural_and_action_records() {
        let mut document = document_fixture()
            .shape(CanvasShape::new(
                "frame",
                Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
            ))
            .node(CanvasNode::new(
                "child",
                point(px(10.0), px(10.0)),
                size(px(10.0), px(10.0)),
            ))
            .node(CanvasNode::new(
                "peer",
                point(px(30.0), px(10.0)),
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
                    child: CanvasRecordId::Node(NodeId::from("child")),
                    parent: CanvasRecordId::Shape(ShapeId::from("frame")),
                },
                DocumentCommand::AddRecordToGroup {
                    group: CanvasRecordId::Shape(ShapeId::from("frame")),
                    member: CanvasRecordId::Node(NodeId::from("peer")),
                },
            ]))
            .unwrap();
        let mut selection = CanvasSelection::default();
        selection.insert_shape(ShapeId::from("frame"));

        let scope = resolve_selection_scope(
            &document,
            &selection,
            CanvasRecordScopeOptions::structural_with_internal_edges(),
        );

        assert_eq!(
            scope
                .normalized_selection()
                .selected_shapes()
                .cloned()
                .collect::<Vec<_>>(),
            vec![ShapeId::from("frame")]
        );
        assert_eq!(
            scope
                .explicit_records()
                .records()
                .cloned()
                .collect::<Vec<_>>(),
            vec![CanvasRecordId::Shape(ShapeId::from("frame"))]
        );
        assert_eq!(
            scope
                .structural_records()
                .records()
                .cloned()
                .collect::<Vec<_>>(),
            vec![
                CanvasRecordId::Node(NodeId::from("child")),
                CanvasRecordId::Node(NodeId::from("peer")),
            ]
        );
        assert_eq!(
            scope
                .action_records()
                .records()
                .cloned()
                .collect::<Vec<_>>(),
            vec![
                CanvasRecordId::Shape(ShapeId::from("frame")),
                CanvasRecordId::Node(NodeId::from("child")),
                CanvasRecordId::Node(NodeId::from("peer")),
                CanvasRecordId::Edge(EdgeId::from("child-peer")),
            ]
        );
    }

    #[test]
    fn internal_edges_are_only_included_when_requested() {
        let mut document = document_fixture()
            .shape(CanvasShape::new(
                "frame",
                Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
            ))
            .node(CanvasNode::new(
                "child",
                point(px(10.0), px(10.0)),
                size(px(10.0), px(10.0)),
            ))
            .node(CanvasNode::new(
                "peer",
                point(px(30.0), px(10.0)),
                size(px(10.0), px(10.0)),
            ))
            .node(CanvasNode::new(
                "outside",
                point(px(60.0), px(10.0)),
                size(px(10.0), px(10.0)),
            ))
            .edge(CanvasEdge::new(
                "internal",
                CanvasEndpoint::new("child", None::<&str>),
                CanvasEndpoint::new("peer", None::<&str>),
            ))
            .edge(CanvasEdge::new(
                "external",
                CanvasEndpoint::new("child", None::<&str>),
                CanvasEndpoint::new("outside", None::<&str>),
            ))
            .build();
        document
            .apply_transaction(CanvasTransaction::new([
                DocumentCommand::SetRecordParent {
                    child: CanvasRecordId::Node(NodeId::from("child")),
                    parent: CanvasRecordId::Shape(ShapeId::from("frame")),
                },
                DocumentCommand::SetRecordParent {
                    child: CanvasRecordId::Node(NodeId::from("peer")),
                    parent: CanvasRecordId::Shape(ShapeId::from("frame")),
                },
            ]))
            .unwrap();
        let mut selection = CanvasSelection::default();
        selection.insert_shape(ShapeId::from("frame"));

        let without_edges = selection_record_scope(
            &document,
            &selection,
            CanvasRecordScopeOptions::structural(),
        );
        let with_edges = selection_record_scope(
            &document,
            &selection,
            CanvasRecordScopeOptions::structural_with_internal_edges(),
        );

        assert!(!without_edges.contains(&CanvasRecordId::Edge(EdgeId::from("internal"))));
        assert!(with_edges.contains(&CanvasRecordId::Edge(EdgeId::from("internal"))));
        assert!(!with_edges.contains(&CanvasRecordId::Edge(EdgeId::from("external"))));
    }

    #[test]
    fn traversal_and_action_predicates_are_separate() {
        let mut locked_frame = CanvasShape::new(
            "locked-frame",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        );
        locked_frame.locked = true;
        let mut document = document_fixture()
            .shape(locked_frame)
            .node(CanvasNode::new(
                "child",
                point(px(10.0), px(10.0)),
                size(px(10.0), px(10.0)),
            ))
            .build();
        document
            .apply_transaction(CanvasTransaction::single(
                DocumentCommand::SetRecordParent {
                    child: CanvasRecordId::Node(NodeId::from("child")),
                    parent: CanvasRecordId::Shape(ShapeId::from("locked-frame")),
                },
            ))
            .unwrap();
        let mut selection = CanvasSelection::default();
        selection.insert_shape(ShapeId::from("locked-frame"));

        let scope = resolve_selection_scope_with_predicates(
            &document,
            &selection,
            CanvasRecordScopeOptions::structural(),
            |record_id| document.contains_record(record_id),
            |record_id| match record_id {
                CanvasRecordId::Shape(id) => document.shape(id).is_some_and(|shape| !shape.locked),
                CanvasRecordId::Node(id) => document.node(id).is_some_and(|node| !node.locked),
                CanvasRecordId::Edge(_) => false,
            },
        );

        assert!(
            !scope
                .action_records()
                .contains(&CanvasRecordId::Shape(ShapeId::from("locked-frame")))
        );
        assert!(
            scope
                .action_records()
                .contains(&CanvasRecordId::Node(NodeId::from("child")))
        );
    }

    #[test]
    fn handles_are_retained_in_selection_but_excluded_from_record_scope() {
        let mut node = CanvasNode::new("node", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        node.handles.push(crate::CanvasHandle::new(
            "handle",
            point(px(50.0), px(50.0)),
        ));
        let document = document_fixture().node(node).build();
        let mut selection = CanvasSelection::default();
        selection.insert_target(crate::HitTarget::Handle {
            node_id: NodeId::from("node"),
            handle_id: crate::HandleId::from("handle"),
        });

        let normalized = normalize_selection(&document, &selection);
        let scope = selection_record_scope(
            &document,
            &normalized,
            CanvasRecordScopeOptions::structural_with_internal_edges(),
        );

        assert_eq!(normalized.selected_handles().count(), 1);
        assert!(scope.is_empty());
    }

    #[test]
    fn selection_scope_expands_related_descendants_and_internal_edges() {
        let mut document = document_fixture()
            .shape(CanvasShape::new(
                "frame",
                Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
            ))
            .node(CanvasNode::new(
                "child",
                point(px(10.0), px(10.0)),
                size(px(10.0), px(10.0)),
            ))
            .node(CanvasNode::new(
                "peer",
                point(px(30.0), px(10.0)),
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
                    child: CanvasRecordId::Node(NodeId::from("child")),
                    parent: CanvasRecordId::Shape(ShapeId::from("frame")),
                },
                DocumentCommand::AddRecordToGroup {
                    group: CanvasRecordId::Shape(ShapeId::from("frame")),
                    member: CanvasRecordId::Node(NodeId::from("peer")),
                },
            ]))
            .unwrap();
        let mut selection = CanvasSelection::default();
        selection.insert_shape(ShapeId::from("frame"));

        let records = collect_selection_record_scope(
            &document,
            &selection,
            CanvasRecordScopeOptions::structural_with_internal_edges(),
            |_| true,
        );

        assert!(records.contains(&CanvasRecordId::Shape(ShapeId::from("frame"))));
        assert!(records.contains(&CanvasRecordId::Node(NodeId::from("child"))));
        assert!(records.contains(&CanvasRecordId::Node(NodeId::from("peer"))));
        assert!(records.contains(&CanvasRecordId::Edge(EdgeId::from("child-peer"))));
    }
}
