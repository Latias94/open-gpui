use crate::{CanvasDocument, CanvasRecordId, CanvasSelection};
use indexmap::IndexSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CanvasRecordScopeOptions {
    pub include_internal_edges: bool,
}

impl CanvasRecordScopeOptions {
    pub(crate) const fn structural() -> Self {
        Self {
            include_internal_edges: false,
        }
    }

    pub(crate) const fn structural_with_internal_edges() -> Self {
        Self {
            include_internal_edges: true,
        }
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
