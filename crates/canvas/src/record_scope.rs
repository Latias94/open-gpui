use crate::{CanvasDocument, CanvasRecordId, CanvasSelection};
use indexmap::IndexSet;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CanvasRecordScopeOptions {
    pub include_internal_edges: bool,
}

impl CanvasRecordScopeOptions {
    pub const fn structural() -> Self {
        Self {
            include_internal_edges: false,
        }
    }

    pub const fn structural_with_internal_edges() -> Self {
        Self {
            include_internal_edges: true,
        }
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
}

pub(crate) fn collect_selection_record_scope(
    document: &CanvasDocument,
    selection: &CanvasSelection,
    options: CanvasRecordScopeOptions,
    mut can_include: impl FnMut(&CanvasRecordId) -> bool,
) -> IndexSet<CanvasRecordId> {
    let mut records = document
        .relations()
        .collect_related_records(selected_record_ids(selection), |record_id| {
            can_include(record_id)
        });

    if options.include_internal_edges {
        include_internal_edges(document, &mut records, &mut can_include);
    }

    records
}

pub fn selection_record_scope(
    document: &CanvasDocument,
    selection: &CanvasSelection,
    options: CanvasRecordScopeOptions,
) -> CanvasRecordScope {
    CanvasRecordScope::new(collect_selection_record_scope(
        document,
        selection,
        options,
        |record_id| record_exists(document, record_id),
    ))
}

fn selected_record_ids(selection: &CanvasSelection) -> impl Iterator<Item = CanvasRecordId> + '_ {
    selection
        .selected_nodes()
        .cloned()
        .map(CanvasRecordId::Node)
        .chain(
            selection
                .selected_edges()
                .cloned()
                .map(CanvasRecordId::Edge),
        )
        .chain(
            selection
                .selected_shapes()
                .cloned()
                .map(CanvasRecordId::Shape),
        )
}

fn include_internal_edges(
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

fn record_exists(document: &CanvasDocument, record_id: &CanvasRecordId) -> bool {
    match record_id {
        CanvasRecordId::Node(id) => document.contains_node(id),
        CanvasRecordId::Edge(id) => document.contains_edge(id),
        CanvasRecordId::Shape(id) => document.contains_shape(id),
    }
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
