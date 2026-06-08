use crate::{
    CanvasDocument, CanvasDocumentDiff, CanvasGraphIndex, CanvasIndexedGraph, HitOptions,
    HitRecord, SpatialIndex,
};
use open_gpui::{Bounds, Pixels, Point};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CanvasRuntime {
    spatial_index: SpatialIndex,
    graph_index: CanvasGraphIndex,
}

impl CanvasRuntime {
    pub fn rebuild(document: &CanvasDocument) -> Self {
        Self {
            spatial_index: SpatialIndex::rebuild(document),
            graph_index: CanvasGraphIndex::rebuild(document),
        }
    }

    pub fn from_spatial_index(document: &CanvasDocument, spatial_index: SpatialIndex) -> Self {
        Self {
            spatial_index,
            graph_index: CanvasGraphIndex::rebuild(document),
        }
    }

    pub fn apply_diff(&mut self, document: &CanvasDocument, diff: &CanvasDocumentDiff) {
        if diff.is_empty() {
            return;
        }

        self.spatial_index.apply_diff(document, diff);
        self.graph_index.apply_diff(document, diff);
    }

    pub fn spatial_index(&self) -> &SpatialIndex {
        &self.spatial_index
    }

    pub fn graph_index(&self) -> &CanvasGraphIndex {
        &self.graph_index
    }

    pub fn graph<'a>(&'a self, document: &'a CanvasDocument) -> CanvasIndexedGraph<'a> {
        self.graph_index.graph(document)
    }

    pub fn query(&self, viewport: Bounds<Pixels>) -> impl Iterator<Item = &HitRecord> {
        self.spatial_index.query(viewport)
    }

    pub fn query_with_options(
        &self,
        viewport: Bounds<Pixels>,
        options: HitOptions,
    ) -> impl Iterator<Item = &HitRecord> {
        self.spatial_index.query_with_options(viewport, options)
    }

    pub fn hit_test(
        &self,
        point: Point<Pixels>,
        options: HitOptions,
    ) -> impl Iterator<Item = &HitRecord> {
        self.spatial_index.hit_test(point, options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CanvasEdge, CanvasEndpoint, CanvasNode, CanvasTransaction, DocumentCommand, EdgeId, NodeId,
    };
    use open_gpui::{point, px, size};

    #[test]
    fn runtime_rebuilds_spatial_and_graph_indexes() {
        let document = connected_document();
        let runtime = CanvasRuntime::rebuild(&document);

        assert!(runtime.graph_index().contains_edge(&EdgeId::from("a-b")));
        assert!(
            runtime
                .hit_test(point(px(1.0), px(1.0)), HitOptions::default())
                .any(|record| matches!(&record.target, crate::HitTarget::Node(id) if id == &NodeId::from("a")))
        );
    }

    #[test]
    fn runtime_applies_diff_to_spatial_and_graph_indexes() {
        let mut document = connected_document();
        let mut runtime = CanvasRuntime::rebuild(&document);
        let mut moved = document.nodes[&NodeId::from("a")].clone();
        moved.position = point(px(100.0), px(0.0));

        let diff = document
            .apply_transaction_with_diff(CanvasTransaction::single(DocumentCommand::UpdateNode(
                moved,
            )))
            .unwrap();
        runtime.apply_diff(&document, &diff);

        assert!(
            runtime
                .hit_test(point(px(101.0), px(1.0)), HitOptions::default())
                .any(|record| matches!(&record.target, crate::HitTarget::Node(id) if id == &NodeId::from("a")))
        );
        assert!(
            runtime
                .hit_test(point(px(1.0), px(1.0)), HitOptions::default())
                .next()
                .is_none()
        );
        assert_eq!(
            runtime
                .graph(&document)
                .incident_edge_count(&NodeId::from("a")),
            1
        );
    }

    #[test]
    fn runtime_removes_incident_edges_from_graph_after_node_removal() {
        let mut document = connected_document();
        let mut runtime = CanvasRuntime::rebuild(&document);

        let diff = document
            .apply_transaction_with_diff(CanvasTransaction::single(DocumentCommand::RemoveNode(
                NodeId::from("a"),
            )))
            .unwrap();
        runtime.apply_diff(&document, &diff);

        assert!(!runtime.graph_index().contains_edge(&EdgeId::from("a-b")));
        assert_eq!(
            runtime
                .graph(&document)
                .incident_edge_count(&NodeId::from("b")),
            0
        );
        assert!(
            runtime
                .hit_test(point(px(1.0), px(1.0)), HitOptions::default())
                .next()
                .is_none()
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
        document
    }
}
