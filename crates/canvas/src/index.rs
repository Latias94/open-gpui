use crate::{
    CanvasDocument, CanvasDocumentDiff, CanvasEdgeRouter, CanvasGeometryResolver,
    CanvasKindRegistry, CanvasRecordId, EdgeId, HandleId, NodeId, ShapeId,
};
use open_gpui::{Bounds, Pixels, Point};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum HitTarget {
    Node(NodeId),
    Handle {
        node_id: NodeId,
        handle_id: HandleId,
    },
    Shape(ShapeId),
    Edge(EdgeId),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HitRecord {
    pub target: HitTarget,
    pub bounds: Bounds<Pixels>,
    pub z_index: i32,
    pub hidden: bool,
    pub locked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct HitOptions {
    pub include_hidden: bool,
    pub include_locked: bool,
    pub include_handles: bool,
    pub margin: Pixels,
}

impl Default for HitOptions {
    fn default() -> Self {
        Self {
            include_hidden: false,
            include_locked: false,
            include_handles: false,
            margin: Pixels::ZERO,
        }
    }
}

pub trait CanvasSpatialIndex {
    fn visit_query(
        &self,
        viewport: Bounds<Pixels>,
        options: HitOptions,
        visitor: &mut dyn FnMut(&HitRecord),
    );

    fn visit_hit_test(
        &self,
        point: Point<Pixels>,
        options: HitOptions,
        visitor: &mut dyn FnMut(&HitRecord),
    );
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpatialIndex {
    records: Vec<HitRecord>,
}

impl SpatialIndex {
    pub fn rebuild(document: &CanvasDocument) -> Self {
        Self::rebuild_with_resolver(CanvasGeometryResolver::new(document))
    }

    pub fn rebuild_with_kind_registry(
        document: &CanvasDocument,
        kind_registry: &CanvasKindRegistry,
    ) -> Self {
        Self::rebuild_with_resolver(CanvasGeometryResolver::with_kind_registry(
            document,
            kind_registry,
        ))
    }

    pub fn rebuild_with_router<R>(document: &CanvasDocument, router: &R) -> Self
    where
        R: CanvasEdgeRouter + ?Sized,
    {
        Self::rebuild_with_resolver(CanvasGeometryResolver::with_router(document, router))
    }

    pub fn rebuild_with_router_and_kind_registry<R>(
        document: &CanvasDocument,
        router: &R,
        kind_registry: &CanvasKindRegistry,
    ) -> Self
    where
        R: CanvasEdgeRouter + ?Sized,
    {
        Self::rebuild_with_resolver(CanvasGeometryResolver::with_router_and_kind_registry(
            document,
            router,
            Some(kind_registry),
        ))
    }

    fn rebuild_with_resolver<R>(resolver: CanvasGeometryResolver<'_, R>) -> Self
    where
        R: CanvasEdgeRouter + Copy,
    {
        let document = resolver.document();
        let mut records = Vec::new();

        for node in document.nodes.values() {
            records.push(HitRecord {
                target: HitTarget::Node(node.id.clone()),
                bounds: resolver.node_bounds(node),
                z_index: node.z_index,
                hidden: node.hidden,
                locked: node.locked,
            });

            for handle in &node.handles {
                records.push(HitRecord {
                    target: HitTarget::Handle {
                        node_id: node.id.clone(),
                        handle_id: handle.id.clone(),
                    },
                    bounds: resolver.handle_bounds(node, handle),
                    z_index: node.z_index,
                    hidden: node.hidden || handle.hidden || !handle.connectable,
                    locked: node.locked,
                });
            }
        }

        for shape in document.shapes.values() {
            records.push(HitRecord {
                target: HitTarget::Shape(shape.id.clone()),
                bounds: resolver.shape_bounds(shape),
                z_index: shape.z_index,
                hidden: shape.hidden,
                locked: shape.locked,
            });
        }

        for edge in document.edges.values() {
            if let Ok(bounds) = resolver.edge_bounds(edge) {
                records.push(HitRecord {
                    target: HitTarget::Edge(edge.id.clone()),
                    bounds,
                    z_index: edge.z_index,
                    hidden: edge.hidden,
                    locked: edge.locked,
                });
            }
        }

        records.sort_by(|a, b| a.z_index.cmp(&b.z_index));
        Self { records }
    }

    pub fn apply_diff(&mut self, document: &CanvasDocument, diff: &CanvasDocumentDiff) {
        self.apply_diff_with_resolver(CanvasGeometryResolver::new(document), diff);
    }

    pub fn apply_diff_with_kind_registry(
        &mut self,
        document: &CanvasDocument,
        diff: &CanvasDocumentDiff,
        kind_registry: &CanvasKindRegistry,
    ) {
        self.apply_diff_with_resolver(
            CanvasGeometryResolver::with_kind_registry(document, kind_registry),
            diff,
        );
    }

    pub fn apply_diff_with_router<R>(
        &mut self,
        document: &CanvasDocument,
        diff: &CanvasDocumentDiff,
        router: &R,
    ) where
        R: CanvasEdgeRouter + ?Sized,
    {
        self.apply_diff_with_resolver(CanvasGeometryResolver::with_router(document, router), diff);
    }

    pub fn apply_diff_with_router_and_kind_registry<R>(
        &mut self,
        document: &CanvasDocument,
        diff: &CanvasDocumentDiff,
        router: &R,
        kind_registry: &CanvasKindRegistry,
    ) where
        R: CanvasEdgeRouter + ?Sized,
    {
        self.apply_diff_with_resolver(
            CanvasGeometryResolver::with_router_and_kind_registry(
                document,
                router,
                Some(kind_registry),
            ),
            diff,
        );
    }

    fn apply_diff_with_resolver<R>(
        &mut self,
        resolver: CanvasGeometryResolver<'_, R>,
        diff: &CanvasDocumentDiff,
    ) where
        R: CanvasEdgeRouter + Copy,
    {
        if diff.is_empty() {
            return;
        }

        for record_id in &diff.removed {
            self.remove_record(record_id);
        }

        for record_id in diff.updated.iter().chain(&diff.inserted) {
            self.refresh_record_with_resolver(resolver, record_id);
        }

        self.records.sort_by(|a, b| a.z_index.cmp(&b.z_index));
    }

    pub fn query(&self, viewport: Bounds<Pixels>) -> impl Iterator<Item = &HitRecord> {
        self.query_with_options(
            viewport,
            HitOptions {
                include_locked: true,
                ..HitOptions::default()
            },
        )
    }

    pub fn query_with_options(
        &self,
        viewport: Bounds<Pixels>,
        options: HitOptions,
    ) -> impl Iterator<Item = &HitRecord> {
        self.records.iter().filter(move |record| {
            (options.include_hidden || !record.hidden)
                && (options.include_locked || !record.locked)
                && (options.include_handles || !matches!(record.target, HitTarget::Handle { .. }))
                && record.bounds.intersects(&viewport)
        })
    }

    pub fn hit_test(
        &self,
        point: Point<Pixels>,
        options: HitOptions,
    ) -> impl Iterator<Item = &HitRecord> {
        self.records
            .iter()
            .rev()
            .filter(move |record| options.include_hidden || !record.hidden)
            .filter(move |record| options.include_locked || !record.locked)
            .filter(move |record| {
                options.include_handles || !matches!(record.target, HitTarget::Handle { .. })
            })
            .filter(move |record| {
                let bounds = if options.margin == Pixels::ZERO {
                    record.bounds
                } else {
                    record.bounds.dilate(options.margin)
                };
                bounds.contains(&point)
            })
    }

    pub fn records(&self) -> &[HitRecord] {
        &self.records
    }

    fn refresh_record_with_resolver<R>(
        &mut self,
        resolver: CanvasGeometryResolver<'_, R>,
        record_id: &CanvasRecordId,
    ) where
        R: CanvasEdgeRouter + Copy,
    {
        let document = resolver.document();
        self.remove_record(record_id);

        match record_id {
            CanvasRecordId::Node(id) => {
                let Some(node) = document.nodes.get(id) else {
                    return;
                };

                self.records.push(HitRecord {
                    target: HitTarget::Node(node.id.clone()),
                    bounds: resolver.node_bounds(node),
                    z_index: node.z_index,
                    hidden: node.hidden,
                    locked: node.locked,
                });

                for handle in &node.handles {
                    self.records.push(HitRecord {
                        target: HitTarget::Handle {
                            node_id: node.id.clone(),
                            handle_id: handle.id.clone(),
                        },
                        bounds: resolver.handle_bounds(node, handle),
                        z_index: node.z_index,
                        hidden: node.hidden || handle.hidden || !handle.connectable,
                        locked: node.locked,
                    });
                }

                for edge in document
                    .edges
                    .values()
                    .filter(|edge| edge.source.node_id == *id || edge.target.node_id == *id)
                {
                    self.refresh_record_with_resolver(
                        resolver,
                        &CanvasRecordId::Edge(edge.id.clone()),
                    );
                }
            }
            CanvasRecordId::Edge(id) => {
                let Some(edge) = document.edges.get(id) else {
                    return;
                };

                if let Ok(bounds) = resolver.edge_bounds(edge) {
                    self.records.push(HitRecord {
                        target: HitTarget::Edge(edge.id.clone()),
                        bounds,
                        z_index: edge.z_index,
                        hidden: edge.hidden,
                        locked: edge.locked,
                    });
                }
            }
            CanvasRecordId::Shape(id) => {
                let Some(shape) = document.shapes.get(id) else {
                    return;
                };

                self.records.push(HitRecord {
                    target: HitTarget::Shape(shape.id.clone()),
                    bounds: resolver.shape_bounds(shape),
                    z_index: shape.z_index,
                    hidden: shape.hidden,
                    locked: shape.locked,
                });
            }
        }
    }

    fn remove_record(&mut self, record_id: &CanvasRecordId) {
        self.records
            .retain(|record| match (record_id, &record.target) {
                (CanvasRecordId::Node(id), HitTarget::Node(target_id)) => target_id != id,
                (CanvasRecordId::Node(id), HitTarget::Handle { node_id, .. }) => node_id != id,
                (CanvasRecordId::Node(_), _) => true,
                (CanvasRecordId::Edge(id), HitTarget::Edge(target_id)) => target_id != id,
                (CanvasRecordId::Edge(_), _) => true,
                (CanvasRecordId::Shape(id), HitTarget::Shape(target_id)) => target_id != id,
                (CanvasRecordId::Shape(_), _) => true,
            });
    }
}

impl CanvasSpatialIndex for SpatialIndex {
    fn visit_query(
        &self,
        viewport: Bounds<Pixels>,
        options: HitOptions,
        visitor: &mut dyn FnMut(&HitRecord),
    ) {
        for record in self.query_with_options(viewport, options) {
            visitor(record);
        }
    }

    fn visit_hit_test(
        &self,
        point: Point<Pixels>,
        options: HitOptions,
        visitor: &mut dyn FnMut(&HitRecord),
    ) {
        for record in self.hit_test(point, options) {
            visitor(record);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CanvasDocument, CanvasEdge, CanvasEdgeRouter, CanvasEndpoint, CanvasNode, CanvasRoutePath,
        CanvasRouteRequest, CanvasShape, CanvasTransaction,
        test_support::{CanvasCommandGenerator, TestRng},
    };
    use open_gpui::{Bounds, point, px, size};
    use std::cmp::Ordering;

    #[test]
    fn hit_test_returns_topmost_first() {
        let mut document = CanvasDocument::default();
        let mut back = CanvasNode::new("back", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        back.z_index = 1;
        let mut front = CanvasShape::new(
            "front",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        );
        front.z_index = 2;
        document.insert_node(back).unwrap();
        document.insert_shape(front).unwrap();

        let index = SpatialIndex::rebuild(&document);
        let hits = index
            .hit_test(point(px(50.0), px(50.0)), HitOptions::default())
            .map(|record| record.target.clone())
            .collect::<Vec<_>>();

        assert_eq!(
            hits,
            vec![
                HitTarget::Shape(ShapeId::from("front")),
                HitTarget::Node(NodeId::from("back"))
            ]
        );
    }

    #[test]
    fn spatial_index_trait_visits_hit_tests_in_order() {
        let mut document = CanvasDocument::default();
        let mut back = CanvasNode::new("back", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        back.z_index = 1;
        let mut front =
            CanvasNode::new("front", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        front.z_index = 2;
        document.insert_node(back).unwrap();
        document.insert_node(front).unwrap();

        let index = SpatialIndex::rebuild(&document);
        let query: &dyn CanvasSpatialIndex = &index;
        let mut hits = Vec::new();
        query.visit_hit_test(
            point(px(50.0), px(50.0)),
            HitOptions::default(),
            &mut |record| hits.push(record.target.clone()),
        );

        assert_eq!(
            hits,
            vec![
                HitTarget::Node(NodeId::from("front")),
                HitTarget::Node(NodeId::from("back"))
            ]
        );
    }

    #[test]
    fn query_culls_outside_records() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "inside",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();
        document
            .insert_node(CanvasNode::new(
                "outside",
                point(px(100.0), px(100.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();

        let index = SpatialIndex::rebuild(&document);
        let visible = index
            .query(Bounds::new(
                point(px(0.0), px(0.0)),
                size(px(50.0), px(50.0)),
            ))
            .map(|record| record.target.clone())
            .collect::<Vec<_>>();

        assert_eq!(visible, vec![HitTarget::Node(NodeId::from("inside"))]);
    }

    #[test]
    fn spatial_index_trait_visits_query_records_with_options() {
        let mut document = CanvasDocument::default();
        let mut locked =
            CanvasNode::new("locked", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        locked.locked = true;
        document.insert_node(locked).unwrap();

        let index = SpatialIndex::rebuild(&document);
        let query: &dyn CanvasSpatialIndex = &index;
        let mut default_records = Vec::new();
        query.visit_query(
            Bounds::new(point(px(0.0), px(0.0)), size(px(20.0), px(20.0))),
            HitOptions::default(),
            &mut |record| default_records.push(record.target.clone()),
        );
        assert!(default_records.is_empty());

        let mut locked_records = Vec::new();
        query.visit_query(
            Bounds::new(point(px(0.0), px(0.0)), size(px(20.0), px(20.0))),
            HitOptions {
                include_locked: true,
                ..HitOptions::default()
            },
            &mut |record| locked_records.push(record.target.clone()),
        );

        assert_eq!(
            locked_records,
            vec![HitTarget::Node(NodeId::from("locked"))]
        );
    }

    #[test]
    fn hidden_records_are_only_returned_when_requested() {
        let mut document = CanvasDocument::default();
        let mut node = CanvasNode::new("hidden", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        node.hidden = true;
        document.insert_node(node).unwrap();

        let index = SpatialIndex::rebuild(&document);
        assert!(
            index
                .hit_test(point(px(5.0), px(5.0)), HitOptions::default())
                .next()
                .is_none()
        );

        let options = HitOptions {
            include_hidden: true,
            ..HitOptions::default()
        };
        assert_eq!(
            index
                .hit_test(point(px(5.0), px(5.0)), options)
                .map(|record| record.target.clone())
                .collect::<Vec<_>>(),
            vec![HitTarget::Node(NodeId::from("hidden"))]
        );
    }

    #[test]
    fn locked_records_are_skipped_by_hit_test_unless_requested() {
        let mut document = CanvasDocument::default();
        let mut node = CanvasNode::new("locked", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        node.locked = true;
        document.insert_node(node).unwrap();

        let index = SpatialIndex::rebuild(&document);
        assert!(
            index
                .hit_test(point(px(5.0), px(5.0)), HitOptions::default())
                .next()
                .is_none()
        );

        let options = HitOptions {
            include_locked: true,
            ..HitOptions::default()
        };
        assert_eq!(
            index
                .hit_test(point(px(5.0), px(5.0)), options)
                .map(|record| (record.target.clone(), record.locked))
                .collect::<Vec<_>>(),
            vec![(HitTarget::Node(NodeId::from("locked")), true)]
        );
    }

    #[test]
    fn query_returns_locked_records_for_culling() {
        let mut document = CanvasDocument::default();
        let mut node = CanvasNode::new("locked", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        node.locked = true;
        document.insert_node(node).unwrap();

        let index = SpatialIndex::rebuild(&document);
        let records = index
            .query(Bounds::new(
                point(px(0.0), px(0.0)),
                size(px(20.0), px(20.0)),
            ))
            .map(|record| (record.target.clone(), record.locked))
            .collect::<Vec<_>>();

        assert_eq!(
            records,
            vec![(HitTarget::Node(NodeId::from("locked")), true)]
        );
    }

    #[test]
    fn edge_bounds_are_indexed_from_route_hit_area() {
        use crate::{CanvasEdge, CanvasEndpoint};

        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(20.0), px(20.0)),
            ))
            .unwrap();
        document
            .insert_node(CanvasNode::new(
                "b",
                point(px(100.0), px(0.0)),
                size(px(20.0), px(20.0)),
            ))
            .unwrap();
        document
            .insert_edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .unwrap();

        let index = SpatialIndex::rebuild(&document);
        assert!(index.records().iter().any(|record| {
            record.target == HitTarget::Edge(EdgeId::from("a-b"))
                && record.bounds.origin == point(px(4.0), px(4.0))
                && record.bounds.size.width == px(112.0)
                && record.bounds.size.height == px(12.0)
        }));
    }

    #[test]
    fn custom_router_flows_through_edge_culling_and_hit_testing() {
        let document = connected_document_for_router();
        let index = SpatialIndex::rebuild_with_router(&document, &VerticalDetourRouter);

        let visible = index
            .query(Bounds::new(
                point(px(0.0), px(76.0)),
                size(px(12.0), px(12.0)),
            ))
            .map(|record| record.target.clone())
            .collect::<Vec<_>>();
        assert_eq!(visible, vec![HitTarget::Edge(EdgeId::from("a-b"))]);

        let hits = index
            .hit_test(point(px(5.0), px(80.0)), HitOptions::default())
            .map(|record| record.target.clone())
            .collect::<Vec<_>>();
        assert_eq!(hits, vec![HitTarget::Edge(EdgeId::from("a-b"))]);
    }

    #[test]
    fn custom_router_flows_through_incremental_edge_refresh() {
        let mut document = connected_document_for_router();
        let mut index = SpatialIndex::rebuild_with_router(&document, &VerticalDetourRouter);
        let mut target = document.nodes[&NodeId::from("b")].clone();
        target.position = point(px(40.0), px(0.0));

        let previous = document.clone();
        document.update_node(target).unwrap();
        let diff = document.diff_against(&previous);
        index.apply_diff_with_router(&document, &diff, &VerticalDetourRouter);

        assert!(index.records().iter().any(|record| {
            record.target == HitTarget::Edge(EdgeId::from("a-b"))
                && record.bounds.origin == point(px(-1.0), px(-1.0))
                && record.bounds.size == size(px(52.0), px(87.0))
        }));
    }

    #[test]
    fn handles_are_hit_only_when_requested() {
        use crate::CanvasHandle;

        let mut document = CanvasDocument::default();
        let mut node = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        node.handles
            .push(CanvasHandle::new("out", point(px(95.0), px(50.0))));
        document.insert_node(node).unwrap();

        let index = SpatialIndex::rebuild(&document);
        let point = point(px(95.0), px(50.0));
        assert_eq!(
            index
                .hit_test(point, HitOptions::default())
                .map(|record| record.target.clone())
                .collect::<Vec<_>>(),
            vec![HitTarget::Node(NodeId::from("a"))]
        );

        let options = HitOptions {
            include_handles: true,
            ..HitOptions::default()
        };
        assert_eq!(
            index
                .hit_test(point, options)
                .map(|record| record.target.clone())
                .collect::<Vec<_>>(),
            vec![
                HitTarget::Handle {
                    node_id: NodeId::from("a"),
                    handle_id: HandleId::from("out"),
                },
                HitTarget::Node(NodeId::from("a")),
            ]
        );
    }

    #[test]
    fn hidden_handles_are_only_hit_when_hidden_records_are_requested() {
        use crate::CanvasHandle;

        let mut document = CanvasDocument::default();
        let mut node = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        let mut handle = CanvasHandle::new("hidden", point(px(95.0), px(50.0)));
        handle.hidden = true;
        node.handles.push(handle);
        document.insert_node(node).unwrap();

        let index = SpatialIndex::rebuild(&document);
        let point = point(px(95.0), px(50.0));
        let visible_options = HitOptions {
            include_handles: true,
            ..HitOptions::default()
        };

        assert_eq!(
            index
                .hit_test(point, visible_options)
                .map(|record| record.target.clone())
                .collect::<Vec<_>>(),
            vec![HitTarget::Node(NodeId::from("a"))]
        );

        let hidden_options = HitOptions {
            include_hidden: true,
            include_handles: true,
            ..HitOptions::default()
        };
        assert_eq!(
            index
                .hit_test(point, hidden_options)
                .map(|record| record.target.clone())
                .collect::<Vec<_>>(),
            vec![
                HitTarget::Handle {
                    node_id: NodeId::from("a"),
                    handle_id: HandleId::from("hidden"),
                },
                HitTarget::Node(NodeId::from("a")),
            ]
        );
    }

    #[test]
    fn applies_diff_for_inserted_records() {
        let previous = CanvasDocument::default();
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();

        let mut index = SpatialIndex::rebuild(&previous);
        let diff = document.diff_against(&previous);
        index.apply_diff(&document, &diff);

        assert_eq!(
            index
                .hit_test(point(px(5.0), px(5.0)), HitOptions::default())
                .map(|record| record.target.clone())
                .collect::<Vec<_>>(),
            vec![HitTarget::Node(NodeId::from("a"))]
        );
    }

    #[test]
    fn applies_diff_for_moved_node_and_incident_edge() {
        use crate::{CanvasEdge, CanvasEndpoint};

        let mut previous = CanvasDocument::default();
        previous
            .insert_node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(20.0), px(20.0)),
            ))
            .unwrap();
        previous
            .insert_node(CanvasNode::new(
                "b",
                point(px(100.0), px(0.0)),
                size(px(20.0), px(20.0)),
            ))
            .unwrap();
        previous
            .insert_edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .unwrap();

        let mut document = previous.clone();
        let mut node = document.nodes.get(&NodeId::from("a")).unwrap().clone();
        node.position = point(px(40.0), px(0.0));
        document.update_node(node).unwrap();

        let mut index = SpatialIndex::rebuild(&previous);
        let diff = document.diff_against(&previous);
        index.apply_diff(&document, &diff);

        assert!(index.records().iter().any(|record| {
            record.target == HitTarget::Edge(EdgeId::from("a-b"))
                && record.bounds.origin == point(px(44.0), px(4.0))
                && record.bounds.size.width == px(72.0)
                && record.bounds.size.height == px(12.0)
        }));
    }

    #[test]
    fn applies_diff_for_updated_edge_route() {
        use crate::{CanvasEdge, CanvasEdgeRoute, CanvasEndpoint};

        let mut previous = CanvasDocument::default();
        previous
            .insert_node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(20.0), px(20.0)),
            ))
            .unwrap();
        previous
            .insert_node(CanvasNode::new(
                "b",
                point(px(100.0), px(0.0)),
                size(px(20.0), px(20.0)),
            ))
            .unwrap();
        previous
            .insert_edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .unwrap();

        let mut document = previous.clone();
        let mut edge = document.edges.get(&EdgeId::from("a-b")).unwrap().clone();
        edge.route = CanvasEdgeRoute::polyline([point(px(60.0), px(80.0))]);
        edge.route.interaction_width = px(20.0);
        document.update_edge(edge).unwrap();

        let mut index = SpatialIndex::rebuild(&previous);
        let diff = document.diff_against(&previous);
        index.apply_diff(&document, &diff);

        assert!(index.records().iter().any(|record| {
            record.target == HitTarget::Edge(EdgeId::from("a-b"))
                && record.bounds.origin == point(px(0.0), px(0.0))
                && record.bounds.size == size(px(120.0), px(90.0))
        }));
    }

    #[test]
    fn incremental_index_matches_rebuild_after_random_diffs() {
        let mut rng = TestRng::new(0x4b65_9072_e9c1_fab3);
        let mut generator = CanvasCommandGenerator::default();
        let mut document = CanvasDocument::default();
        let mut index = SpatialIndex::rebuild(&document);

        for _ in 0..192 {
            let command = generator.next_command(&document, &mut rng);
            let diff = document
                .apply_transaction_with_diff(CanvasTransaction::single(command))
                .unwrap();
            index.apply_diff(&document, &diff);

            let rebuilt = SpatialIndex::rebuild(&document);
            assert_eq!(sorted_records(&index), sorted_records(&rebuilt));
        }
    }

    fn sorted_records(index: &SpatialIndex) -> Vec<HitRecord> {
        let mut records = index.records().to_vec();
        records.sort_by(compare_hit_records);
        records
    }

    fn compare_hit_records(left: &HitRecord, right: &HitRecord) -> Ordering {
        target_key(&left.target)
            .cmp(&target_key(&right.target))
            .then_with(|| left.z_index.cmp(&right.z_index))
            .then_with(|| left.hidden.cmp(&right.hidden))
            .then_with(|| left.locked.cmp(&right.locked))
    }

    fn target_key(target: &HitTarget) -> (u8, String, String) {
        match target {
            HitTarget::Node(id) => (0, id.to_string(), String::new()),
            HitTarget::Handle { node_id, handle_id } => {
                (1, node_id.to_string(), handle_id.to_string())
            }
            HitTarget::Shape(id) => (2, id.to_string(), String::new()),
            HitTarget::Edge(id) => (3, id.to_string(), String::new()),
        }
    }

    fn connected_document_for_router() -> CanvasDocument {
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
