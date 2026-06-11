use crate::{
    CanvasDefaultEdgeRouter, CanvasDocument, CanvasDocumentDiff, CanvasEdgeRouter,
    CanvasGeometryFacts, CanvasGraphIndex, CanvasIndexedGraph, CanvasKindRegistry, CanvasRecordId,
    CanvasResolvedEdgeGeometry, EdgeId, HitOptions, HitRecord, mutation::CanvasCommittedMutation,
    runtime_query::CanvasRuntimeQuery,
};
use indexmap::IndexMap;
use open_gpui::{Bounds, Pixels, Point};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CanvasRuntime {
    query: CanvasRuntimeQuery,
    graph_index: CanvasGraphIndex,
    edge_geometries: IndexMap<EdgeId, CanvasResolvedEdgeGeometry>,
}

impl CanvasRuntime {
    pub fn rebuild(document: &CanvasDocument) -> Self {
        Self::rebuild_with_router(document, &CanvasDefaultEdgeRouter)
    }

    pub fn rebuild_with_kind_registry(
        document: &CanvasDocument,
        kind_registry: &CanvasKindRegistry,
    ) -> Self {
        Self::rebuild_with_router_and_optional_kind_registry(
            document,
            &CanvasDefaultEdgeRouter,
            Some(kind_registry),
        )
    }

    pub fn rebuild_with_router<R>(document: &CanvasDocument, router: &R) -> Self
    where
        R: CanvasEdgeRouter + ?Sized,
    {
        Self::rebuild_with_router_and_optional_kind_registry(document, router, None)
    }

    pub fn rebuild_with_router_and_kind_registry<R>(
        document: &CanvasDocument,
        router: &R,
        kind_registry: &CanvasKindRegistry,
    ) -> Self
    where
        R: CanvasEdgeRouter + ?Sized,
    {
        Self::rebuild_with_router_and_optional_kind_registry(document, router, Some(kind_registry))
    }

    fn rebuild_with_router_and_optional_kind_registry<R>(
        document: &CanvasDocument,
        router: &R,
        kind_registry: Option<&CanvasKindRegistry>,
    ) -> Self
    where
        R: CanvasEdgeRouter + ?Sized,
    {
        let query = match kind_registry {
            Some(kind_registry) => CanvasRuntimeQuery::rebuild_with_router_and_kind_registry(
                document,
                router,
                kind_registry,
            ),
            None => CanvasRuntimeQuery::rebuild_with_router(document, router),
        };

        Self {
            query,
            graph_index: CanvasGraphIndex::rebuild(document),
            edge_geometries: resolve_edge_geometries(document, router, kind_registry),
        }
    }

    pub fn apply_committed_mutation(
        &mut self,
        document: &CanvasDocument,
        committed: &CanvasCommittedMutation,
    ) {
        self.apply_committed_mutation_with_router(document, committed, &CanvasDefaultEdgeRouter);
    }

    pub fn apply_committed_mutation_with_kind_registry(
        &mut self,
        document: &CanvasDocument,
        committed: &CanvasCommittedMutation,
        kind_registry: &CanvasKindRegistry,
    ) {
        self.apply_committed_mutation_with_router_and_kind_registry(
            document,
            committed,
            &CanvasDefaultEdgeRouter,
            kind_registry,
        );
    }

    pub fn apply_committed_mutation_with_router<R>(
        &mut self,
        document: &CanvasDocument,
        committed: &CanvasCommittedMutation,
        router: &R,
    ) where
        R: CanvasEdgeRouter + ?Sized,
    {
        self.apply_diff_with_router_and_optional_kind_registry(
            document,
            committed.diff(),
            router,
            None,
        );
    }

    pub fn apply_committed_mutation_with_router_and_kind_registry<R>(
        &mut self,
        document: &CanvasDocument,
        committed: &CanvasCommittedMutation,
        router: &R,
        kind_registry: &CanvasKindRegistry,
    ) where
        R: CanvasEdgeRouter + ?Sized,
    {
        self.apply_diff_with_router_and_optional_kind_registry(
            document,
            committed.diff(),
            router,
            Some(kind_registry),
        );
    }

    fn apply_diff_with_router_and_optional_kind_registry<R>(
        &mut self,
        document: &CanvasDocument,
        diff: &CanvasDocumentDiff,
        router: &R,
        kind_registry: Option<&CanvasKindRegistry>,
    ) where
        R: CanvasEdgeRouter + ?Sized,
    {
        if diff.is_empty() {
            return;
        }

        match kind_registry {
            Some(kind_registry) => {
                self.query.apply_diff_with_router_and_kind_registry(
                    document,
                    diff,
                    router,
                    kind_registry,
                );
            }
            None => {
                self.query.apply_diff_with_router(document, diff, router);
            }
        }
        self.graph_index.apply_diff(document, diff);
        self.apply_edge_geometry_diff(document, diff, router, kind_registry);
    }

    pub fn edge_geometry(&self, id: &EdgeId) -> Option<&CanvasResolvedEdgeGeometry> {
        self.edge_geometries.get(id)
    }

    pub fn graph<'a>(&'a self, document: &'a CanvasDocument) -> CanvasIndexedGraph<'a> {
        self.graph_index.graph(document)
    }

    pub fn query(&self, viewport: Bounds<Pixels>) -> impl Iterator<Item = &HitRecord> {
        self.query.query(viewport)
    }

    pub fn query_with_options(
        &self,
        viewport: Bounds<Pixels>,
        options: HitOptions,
    ) -> impl Iterator<Item = &HitRecord> {
        self.query.query_with_options(viewport, options)
    }

    pub fn hit_test(
        &self,
        point: Point<Pixels>,
        options: HitOptions,
    ) -> impl Iterator<Item = &HitRecord> {
        self.query.hit_test(point, options)
    }

    pub fn precise_hit_test<'a>(
        &'a self,
        document: &'a CanvasDocument,
        point: Point<Pixels>,
        options: HitOptions,
    ) -> impl Iterator<Item = &'a HitRecord> + 'a {
        self.precise_hit_test_with_facts(CanvasGeometryFacts::new(document), point, options)
    }

    pub fn precise_hit_test_with_kind_registry<'a>(
        &'a self,
        document: &'a CanvasDocument,
        kind_registry: &'a CanvasKindRegistry,
        point: Point<Pixels>,
        options: HitOptions,
    ) -> impl Iterator<Item = &'a HitRecord> + 'a {
        self.precise_hit_test_with_facts(
            CanvasGeometryFacts::with_kind_registry(document, kind_registry),
            point,
            options,
        )
    }

    pub fn precise_hit_test_with_facts<'a, R>(
        &'a self,
        facts: CanvasGeometryFacts<'a, R>,
        point: Point<Pixels>,
        options: HitOptions,
    ) -> impl Iterator<Item = &'a HitRecord> + 'a
    where
        R: CanvasEdgeRouter + 'a,
    {
        self.query
            .precise_hit_test_with_facts(facts, &self.edge_geometries, point, options)
    }

    fn apply_edge_geometry_diff<R>(
        &mut self,
        document: &CanvasDocument,
        diff: &CanvasDocumentDiff,
        router: &R,
        kind_registry: Option<&CanvasKindRegistry>,
    ) where
        R: CanvasEdgeRouter + ?Sized,
    {
        for record_id in &diff.removed {
            self.remove_edge_geometry(document, record_id);
        }

        for record_id in diff.updated.iter().chain(&diff.inserted) {
            self.refresh_edge_geometry(document, record_id, router, kind_registry);
        }
    }

    fn refresh_edge_geometry<R>(
        &mut self,
        document: &CanvasDocument,
        record_id: &CanvasRecordId,
        router: &R,
        kind_registry: Option<&CanvasKindRegistry>,
    ) where
        R: CanvasEdgeRouter + ?Sized,
    {
        match record_id {
            CanvasRecordId::Node(id) => {
                for edge in document
                    .edges()
                    .filter(|edge| edge.source.node_id == *id || edge.target.node_id == *id)
                {
                    self.refresh_edge_geometry(
                        document,
                        &CanvasRecordId::Edge(edge.id.clone()),
                        router,
                        kind_registry,
                    );
                }
            }
            CanvasRecordId::Edge(id) => {
                let Some(edge) = document.edge(id) else {
                    self.edge_geometries.shift_remove(id);
                    return;
                };
                let facts = CanvasGeometryFacts::with_router_and_kind_registry(
                    document,
                    router,
                    kind_registry,
                );
                match facts.edge_geometry(edge) {
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
                    document.edge(edge_id).is_some_and(|edge| {
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
    kind_registry: Option<&CanvasKindRegistry>,
) -> IndexMap<EdgeId, CanvasResolvedEdgeGeometry>
where
    R: CanvasEdgeRouter + ?Sized,
{
    let facts = CanvasGeometryFacts::with_router_and_kind_registry(document, router, kind_registry);
    document
        .edges()
        .filter_map(|edge| {
            facts
                .edge_geometry(edge)
                .ok()
                .map(|geometry| (edge.id.clone(), geometry))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::document_fixture;
    use crate::{
        CanvasEdge, CanvasEndpoint, CanvasHandle, CanvasKindRegistry, CanvasNode,
        CanvasNodeGeometryPolicy, CanvasNodeHitTest, CanvasNodeInteractionPolicy, CanvasNodeKind,
        CanvasRoutePath, CanvasRouteRequest, CanvasTransaction, DocumentCommand, EdgeId, NodeId,
    };
    use open_gpui::{Bounds, point, px, size};

    #[test]
    fn runtime_rebuilds_spatial_and_graph_indexes() {
        let document = connected_document();
        let runtime = CanvasRuntime::rebuild(&document);

        assert!(runtime.graph_index.contains_edge(&EdgeId::from("a-b")));
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
        let mut moved = document.node(&NodeId::from("a")).unwrap().clone();
        moved.position = point(px(100.0), px(0.0));

        let committed = document
            .commit_transaction(CanvasTransaction::single(DocumentCommand::UpdateNode(
                moved,
            )))
            .unwrap();
        runtime.apply_committed_mutation(&document, &committed);

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

        let committed = document
            .commit_transaction(CanvasTransaction::single(DocumentCommand::RemoveNode(
                NodeId::from("a"),
            )))
            .unwrap();
        runtime.apply_committed_mutation(&document, &committed);

        assert!(!runtime.graph_index.contains_edge(&EdgeId::from("a-b")));
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
        let mut moved = document.node(&NodeId::from("b")).unwrap().clone();
        moved.position = point(px(40.0), px(0.0));

        let committed = document
            .commit_transaction(CanvasTransaction::single(DocumentCommand::UpdateNode(
                moved,
            )))
            .unwrap();
        runtime.apply_committed_mutation_with_router(&document, &committed, &VerticalDetourRouter);

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

    #[test]
    fn runtime_uses_kind_registry_geometry_for_index_and_routes() {
        let mut source = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        source.kind = "wide".to_string();
        source
            .handles
            .push(CanvasHandle::new("out", point(px(10.0), px(5.0))));
        let document = document_fixture()
            .node(source)
            .node(CanvasNode::new(
                "b",
                point(px(20.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", Some("out")),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .build();
        let registry = geometry_registry();
        let runtime = CanvasRuntime::rebuild_with_kind_registry(&document, &registry);

        assert!(runtime.hit_test(point(px(-4.0), px(5.0)), HitOptions::default()).any(
            |record| matches!(&record.target, crate::HitTarget::Node(id) if id == &NodeId::from("a"))
        ));
        assert!(
            runtime
                .hit_test(
                    point(px(30.0), px(5.0)),
                    HitOptions {
                        include_handles: true,
                        ..HitOptions::default()
                    }
                )
                .any(|record| matches!(
                    &record.target,
                    crate::HitTarget::Handle { node_id, handle_id }
                        if node_id == &NodeId::from("a") && handle_id.as_str() == "out"
                ))
        );
        assert_eq!(
            runtime
                .edge_geometry(&EdgeId::from("a-b"))
                .unwrap()
                .path
                .document_points(),
            vec![point(px(30.0), px(5.0)), point(px(25.0), px(5.0))]
        );
    }

    #[test]
    fn runtime_precise_hit_test_uses_kind_policy() {
        let mut node = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        node.kind = "right-half".to_string();
        let document = document_fixture().node(node).build();
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind(
            "right-half",
            CanvasNodeKind::new().with_interaction_policy(RightHalfNodeKind),
        );
        let runtime = CanvasRuntime::rebuild_with_kind_registry(&document, &registry);

        assert!(
            runtime
                .hit_test(point(px(25.0), px(25.0)), HitOptions::default())
                .any(|record| matches!(&record.target, crate::HitTarget::Node(id) if id == &NodeId::from("a")))
        );
        assert!(
            runtime
                .precise_hit_test_with_kind_registry(
                    &document,
                    &registry,
                    point(px(25.0), px(25.0)),
                    HitOptions::default(),
                )
                .next()
                .is_none()
        );
        assert!(
            runtime
                .precise_hit_test_with_kind_registry(
                    &document,
                    &registry,
                    point(px(75.0), px(25.0)),
                    HitOptions::default(),
                )
                .any(|record| matches!(&record.target, crate::HitTarget::Node(id) if id == &NodeId::from("a")))
        );
    }

    #[test]
    fn runtime_precise_hit_test_uses_cached_edge_geometry() {
        let document = connected_document();
        let router = VerticalDetourRouter;
        let runtime = CanvasRuntime::rebuild_with_router(&document, &router);

        assert!(
            runtime
                .hit_test(point(px(25.0), px(80.0)), HitOptions::default())
                .any(|record| matches!(&record.target, crate::HitTarget::Edge(id) if id == &EdgeId::from("a-b")))
        );
        assert!(
            runtime
                .precise_hit_test_with_facts(
                    CanvasGeometryFacts::with_router(&document, &router),
                    point(px(25.0), px(80.0)),
                    HitOptions::default(),
                )
                .next()
                .is_none()
        );
        assert!(
            runtime
                .precise_hit_test_with_facts(
                    CanvasGeometryFacts::with_router(&document, &router),
                    point(px(5.0), px(80.0)),
                    HitOptions::default(),
                )
                .any(|record| matches!(&record.target, crate::HitTarget::Edge(id) if id == &EdgeId::from("a-b")))
        );
    }

    #[test]
    fn runtime_applies_kind_registry_geometry_after_diff() {
        let mut document = connected_registry_document();
        let registry = geometry_registry();
        let mut runtime = CanvasRuntime::rebuild_with_kind_registry(&document, &registry);
        let mut moved = document.node(&NodeId::from("a")).unwrap().clone();
        moved.position = point(px(10.0), px(0.0));

        let committed = document
            .commit_transaction(CanvasTransaction::single(DocumentCommand::UpdateNode(
                moved,
            )))
            .unwrap();
        runtime.apply_committed_mutation_with_kind_registry(&document, &committed, &registry);

        assert_eq!(
            runtime
                .edge_geometry(&EdgeId::from("a-b"))
                .unwrap()
                .path
                .document_points(),
            vec![point(px(40.0), px(5.0)), point(px(25.0), px(5.0))]
        );
        assert!(
            runtime
                .hit_test(
                    point(px(40.0), px(5.0)),
                    HitOptions {
                        include_handles: true,
                        ..HitOptions::default()
                    }
                )
                .any(|record| matches!(
                    &record.target,
                    crate::HitTarget::Handle { node_id, handle_id }
                        if node_id == &NodeId::from("a") && handle_id.as_str() == "out"
                ))
        );
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
                point(px(20.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .build()
    }

    fn connected_registry_document() -> CanvasDocument {
        let mut source = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        source.kind = "wide".to_string();
        source
            .handles
            .push(CanvasHandle::new("out", point(px(10.0), px(5.0))));
        document_fixture()
            .node(source)
            .node(CanvasNode::new(
                "b",
                point(px(20.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", Some("out")),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .build()
    }

    fn geometry_registry() -> CanvasKindRegistry {
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind(
            "wide",
            CanvasNodeKind::new().with_geometry_policy(WideNodeKind),
        );
        registry
    }

    struct WideNodeKind;

    impl CanvasNodeGeometryPolicy for WideNodeKind {
        fn node_bounds(&self, node: &CanvasNode) -> Option<Bounds<open_gpui::Pixels>> {
            Some(node.bounds().dilate(px(5.0)))
        }

        fn handle_position(
            &self,
            node: &CanvasNode,
            handle_id: &crate::HandleId,
        ) -> Option<open_gpui::Point<open_gpui::Pixels>> {
            (handle_id.as_str() == "out").then(|| {
                point(
                    node.position.x + node.size.width + px(20.0),
                    node.position.y + px(5.0),
                )
            })
        }
    }

    struct RightHalfNodeKind;

    impl CanvasNodeInteractionPolicy for RightHalfNodeKind {
        fn node_contains_point(&self, hit: CanvasNodeHitTest<'_>) -> Option<bool> {
            Some(hit.point.x >= hit.bounds.center().x)
        }
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
