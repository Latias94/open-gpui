use crate::{
    CanvasDefaultEdgeRouter, CanvasDocument, CanvasDocumentDiff, CanvasEdgeRouter,
    CanvasGeometryResolver, CanvasGraphIndex, CanvasIndexedGraph, CanvasRecordId,
    CanvasResolvedEdgeGeometry, EdgeId, HitOptions, HitRecord, SpatialIndex,
};
use indexmap::IndexMap;
use open_gpui::{Bounds, Pixels, Point};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CanvasRuntime {
    spatial_index: SpatialIndex,
    graph_index: CanvasGraphIndex,
    edge_geometries: IndexMap<EdgeId, CanvasResolvedEdgeGeometry>,
}

impl CanvasRuntime {
    pub fn rebuild(document: &CanvasDocument) -> Self {
        Self::rebuild_with_router(document, &CanvasDefaultEdgeRouter)
    }

    pub fn rebuild_with_router<R>(document: &CanvasDocument, router: &R) -> Self
    where
        R: CanvasEdgeRouter + ?Sized,
    {
        Self {
            spatial_index: SpatialIndex::rebuild_with_router(document, router),
            graph_index: CanvasGraphIndex::rebuild(document),
            edge_geometries: resolve_edge_geometries(document, router),
        }
    }

    pub fn from_spatial_index(document: &CanvasDocument, spatial_index: SpatialIndex) -> Self {
        Self::from_spatial_index_with_router(document, spatial_index, &CanvasDefaultEdgeRouter)
    }

    pub fn from_spatial_index_with_router<R>(
        document: &CanvasDocument,
        spatial_index: SpatialIndex,
        router: &R,
    ) -> Self
    where
        R: CanvasEdgeRouter + ?Sized,
    {
        Self {
            spatial_index,
            graph_index: CanvasGraphIndex::rebuild(document),
            edge_geometries: resolve_edge_geometries(document, router),
        }
    }

    pub fn apply_diff(&mut self, document: &CanvasDocument, diff: &CanvasDocumentDiff) {
        self.apply_diff_with_router(document, diff, &CanvasDefaultEdgeRouter);
    }

    pub fn apply_diff_with_router<R>(
        &mut self,
        document: &CanvasDocument,
        diff: &CanvasDocumentDiff,
        router: &R,
    ) where
        R: CanvasEdgeRouter + ?Sized,
    {
        if diff.is_empty() {
            return;
        }

        self.spatial_index
            .apply_diff_with_router(document, diff, router);
        self.graph_index.apply_diff(document, diff);
        self.apply_edge_geometry_diff(document, diff, router);
    }

    pub fn spatial_index(&self) -> &SpatialIndex {
        &self.spatial_index
    }

    pub fn graph_index(&self) -> &CanvasGraphIndex {
        &self.graph_index
    }

    pub fn edge_geometry(&self, id: &EdgeId) -> Option<&CanvasResolvedEdgeGeometry> {
        self.edge_geometries.get(id)
    }

    pub fn edge_geometries(&self) -> &IndexMap<EdgeId, CanvasResolvedEdgeGeometry> {
        &self.edge_geometries
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

    fn apply_edge_geometry_diff<R>(
        &mut self,
        document: &CanvasDocument,
        diff: &CanvasDocumentDiff,
        router: &R,
    ) where
        R: CanvasEdgeRouter + ?Sized,
    {
        for record_id in &diff.removed {
            self.remove_edge_geometry(document, record_id);
        }

        for record_id in diff.updated.iter().chain(&diff.inserted) {
            self.refresh_edge_geometry(document, record_id, router);
        }
    }

    fn refresh_edge_geometry<R>(
        &mut self,
        document: &CanvasDocument,
        record_id: &CanvasRecordId,
        router: &R,
    ) where
        R: CanvasEdgeRouter + ?Sized,
    {
        match record_id {
            CanvasRecordId::Node(id) => {
                for edge in document
                    .edges
                    .values()
                    .filter(|edge| edge.source.node_id == *id || edge.target.node_id == *id)
                {
                    self.refresh_edge_geometry(
                        document,
                        &CanvasRecordId::Edge(edge.id.clone()),
                        router,
                    );
                }
            }
            CanvasRecordId::Edge(id) => {
                let Some(edge) = document.edges.get(id) else {
                    self.edge_geometries.shift_remove(id);
                    return;
                };
                let resolver = CanvasGeometryResolver::with_router(document, router);
                match resolver.edge_geometry(edge) {
                    Ok(geometry) => {
                        self.edge_geometries.insert(id.clone(), geometry);
                    }
                    Err(_) => {
                        self.edge_geometries.shift_remove(id);
                    }
                }
            }
            CanvasRecordId::Shape(_) => {}
        }
    }

    fn remove_edge_geometry(&mut self, document: &CanvasDocument, record_id: &CanvasRecordId) {
        match record_id {
            CanvasRecordId::Node(id) => {
                self.edge_geometries.retain(|edge_id, _| {
                    document.edges.get(edge_id).is_some_and(|edge| {
                        edge.source.node_id != *id && edge.target.node_id != *id
                    })
                });
            }
            CanvasRecordId::Edge(id) => {
                self.edge_geometries.shift_remove(id);
            }
            CanvasRecordId::Shape(_) => {}
        }
    }
}

fn resolve_edge_geometries<R>(
    document: &CanvasDocument,
    router: &R,
) -> IndexMap<EdgeId, CanvasResolvedEdgeGeometry>
where
    R: CanvasEdgeRouter + ?Sized,
{
    let resolver = CanvasGeometryResolver::with_router(document, router);
    document
        .edges
        .values()
        .filter_map(|edge| {
            resolver
                .edge_geometry(edge)
                .ok()
                .map(|geometry| (edge.id.clone(), geometry))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CanvasEdge, CanvasEndpoint, CanvasNode, CanvasRoutePath, CanvasRouteRequest,
        CanvasTransaction, DocumentCommand, EdgeId, NodeId,
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

    #[test]
    fn runtime_caches_edge_geometry_from_custom_router() {
        let document = connected_document();
        let runtime = CanvasRuntime::rebuild_with_router(&document, &VerticalDetourRouter);
        let geometry = runtime.edge_geometry(&EdgeId::from("a-b")).unwrap();

        assert_eq!(
            geometry.path.document_points(),
            vec![
                point(px(5.0), px(5.0)),
                point(px(5.0), px(80.0)),
                point(px(25.0), px(5.0)),
            ]
        );
        assert_eq!(geometry.bounds.origin, point(px(-1.0), px(-1.0)));
        assert_eq!(geometry.bounds.size, size(px(32.0), px(87.0)));
    }

    #[test]
    fn runtime_updates_edge_geometry_with_router_after_node_diff() {
        let mut document = connected_document();
        let mut runtime = CanvasRuntime::rebuild_with_router(&document, &VerticalDetourRouter);
        let mut moved = document.nodes[&NodeId::from("b")].clone();
        moved.position = point(px(40.0), px(0.0));

        let diff = document
            .apply_transaction_with_diff(CanvasTransaction::single(DocumentCommand::UpdateNode(
                moved,
            )))
            .unwrap();
        runtime.apply_diff_with_router(&document, &diff, &VerticalDetourRouter);

        assert_eq!(
            runtime
                .edge_geometry(&EdgeId::from("a-b"))
                .unwrap()
                .path
                .document_points(),
            vec![
                point(px(5.0), px(5.0)),
                point(px(5.0), px(80.0)),
                point(px(45.0), px(5.0)),
            ]
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

    struct VerticalDetourRouter;

    impl CanvasEdgeRouter for VerticalDetourRouter {
        fn route_edge(&self, request: CanvasRouteRequest<'_>) -> CanvasRoutePath {
            CanvasRoutePath::polyline([
                request.source,
                point(request.source.x, px(80.0)),
                request.target,
            ])
        }
    }
}
