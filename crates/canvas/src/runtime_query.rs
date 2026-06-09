use crate::spatial_cache::{CanvasSpatialCache, IndexedHitRecord};
use crate::{
    CanvasDocument, CanvasDocumentDiff, CanvasEdgeRouter, CanvasGeometryResolver,
    CanvasKindRegistry, CanvasResolvedEdgeGeometry, EdgeId, HitOptions, HitRecord, HitTarget,
};
use indexmap::IndexMap;
use open_gpui::{Bounds, Pixels, Point};
use std::cmp::Ordering;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CanvasRuntimeQuery {
    spatial_cache: CanvasSpatialCache,
}

impl CanvasRuntimeQuery {
    pub(crate) fn rebuild_with_router<R>(document: &CanvasDocument, router: &R) -> Self
    where
        R: CanvasEdgeRouter + ?Sized,
    {
        Self::rebuild_with_router_and_optional_kind_registry(document, router, None)
    }

    pub(crate) fn rebuild_with_router_and_kind_registry<R>(
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
        let spatial_cache = match kind_registry {
            Some(kind_registry) => CanvasSpatialCache::rebuild_with_router_and_kind_registry(
                document,
                router,
                kind_registry,
            ),
            None => CanvasSpatialCache::rebuild_with_router(document, router),
        };

        Self { spatial_cache }
    }

    pub(crate) fn apply_diff_with_router<R>(
        &mut self,
        document: &CanvasDocument,
        diff: &CanvasDocumentDiff,
        router: &R,
    ) where
        R: CanvasEdgeRouter + ?Sized,
    {
        self.apply_diff_with_router_and_optional_kind_registry(document, diff, router, None);
    }

    pub(crate) fn apply_diff_with_router_and_kind_registry<R>(
        &mut self,
        document: &CanvasDocument,
        diff: &CanvasDocumentDiff,
        router: &R,
        kind_registry: &CanvasKindRegistry,
    ) where
        R: CanvasEdgeRouter + ?Sized,
    {
        self.apply_diff_with_router_and_optional_kind_registry(
            document,
            diff,
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
        match kind_registry {
            Some(kind_registry) => self.spatial_cache.apply_diff_with_router_and_kind_registry(
                document,
                diff,
                router,
                kind_registry,
            ),
            None => self
                .spatial_cache
                .apply_diff_with_router(document, diff, router),
        }
    }

    pub(crate) fn query(&self, viewport: Bounds<Pixels>) -> impl Iterator<Item = &HitRecord> {
        self.query_with_options(
            viewport,
            HitOptions {
                include_locked: true,
                ..HitOptions::default()
            },
        )
    }

    pub(crate) fn query_with_options(
        &self,
        viewport: Bounds<Pixels>,
        options: HitOptions,
    ) -> impl Iterator<Item = &HitRecord> {
        let mut records = self
            .spatial_cache
            .query_candidates(viewport)
            .filter(move |record| query_matches(&record.record, viewport, options))
            .collect::<Vec<_>>();
        records.sort_by(|left, right| compare_query_records(left, right));
        records.into_iter().map(|record| &record.record)
    }

    pub(crate) fn hit_test(
        &self,
        point: Point<Pixels>,
        options: HitOptions,
    ) -> impl Iterator<Item = &HitRecord> {
        let mut records = self
            .spatial_cache
            .hit_test_candidates(point, options.margin)
            .filter(move |record| hit_matches(&record.record, point, options))
            .collect::<Vec<_>>();
        records.sort_by(|left, right| compare_hit_records(left, right));
        records.into_iter().map(|record| &record.record)
    }

    pub(crate) fn precise_hit_test_with_resolver<'a, R>(
        &'a self,
        resolver: CanvasGeometryResolver<'a, R>,
        edge_geometries: &'a IndexMap<EdgeId, CanvasResolvedEdgeGeometry>,
        point: Point<Pixels>,
        options: HitOptions,
    ) -> impl Iterator<Item = &'a HitRecord> + 'a
    where
        R: CanvasEdgeRouter + 'a,
    {
        self.hit_test(point, options).filter(move |record| {
            let edge_geometry = match &record.target {
                HitTarget::Edge(id) => edge_geometries.get(id),
                _ => None,
            };
            resolver.record_contains_point_with_edge_geometry(record, point, options, edge_geometry)
        })
    }
}

pub(crate) fn query_matches(
    record: &HitRecord,
    viewport: Bounds<Pixels>,
    options: HitOptions,
) -> bool {
    options_match(record, options) && record.bounds.intersects(&viewport)
}

pub(crate) fn hit_matches(record: &HitRecord, point: Point<Pixels>, options: HitOptions) -> bool {
    if !options_match(record, options) {
        return false;
    }

    let bounds = if options.margin == Pixels::ZERO {
        record.bounds
    } else {
        record.bounds.dilate(options.margin)
    };
    bounds.contains(&point)
}

fn compare_query_records(left: &IndexedHitRecord, right: &IndexedHitRecord) -> Ordering {
    left.record
        .z_index
        .cmp(&right.record.z_index)
        .then_with(|| left.ordinal.cmp(&right.ordinal))
}

fn compare_hit_records(left: &IndexedHitRecord, right: &IndexedHitRecord) -> Ordering {
    right
        .record
        .z_index
        .cmp(&left.record.z_index)
        .then_with(|| right.ordinal.cmp(&left.ordinal))
}

fn options_match(record: &HitRecord, options: HitOptions) -> bool {
    (options.include_hidden || !record.hidden)
        && (options.include_locked || !record.locked)
        && (options.include_handles || !matches!(record.target, HitTarget::Handle { .. }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CanvasDefaultEdgeRouter, CanvasHandle, CanvasNode, CanvasShape, CanvasTransaction,
        DocumentCommand, NodeId,
    };
    use open_gpui::{point, px, size};

    #[test]
    fn runtime_query_owns_filtering_and_ordering_over_cache_candidates() {
        let mut document = CanvasDocument::default();
        let mut back = CanvasNode::new("back", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        back.z_index = 1;
        back.handles
            .push(CanvasHandle::new("out", point(px(95.0), px(50.0))));
        let mut front = CanvasShape::new(
            "front",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        );
        front.z_index = 2;
        document.insert_node(back).unwrap();
        document.insert_shape(front).unwrap();

        let query = CanvasRuntimeQuery::rebuild_with_router(&document, &CanvasDefaultEdgeRouter);

        assert_eq!(
            query
                .hit_test(point(px(95.0), px(50.0)), HitOptions::default())
                .map(|record| record.target.clone())
                .collect::<Vec<_>>(),
            vec![
                HitTarget::Shape("front".into()),
                HitTarget::Node("back".into())
            ]
        );
        assert_eq!(
            query
                .hit_test(
                    point(px(95.0), px(50.0)),
                    HitOptions {
                        include_handles: true,
                        ..HitOptions::default()
                    },
                )
                .map(|record| record.target.clone())
                .collect::<Vec<_>>(),
            vec![
                HitTarget::Shape("front".into()),
                HitTarget::Handle {
                    node_id: "back".into(),
                    handle_id: "out".into(),
                },
                HitTarget::Node("back".into()),
            ]
        );
    }

    #[test]
    fn runtime_query_suppresses_stale_cache_records_after_diff() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .unwrap();
        let mut query =
            CanvasRuntimeQuery::rebuild_with_router(&document, &CanvasDefaultEdgeRouter);

        let mut moved = document.node(&NodeId::from("a")).unwrap().clone();
        moved.position = point(px(200.0), px(0.0));
        let diff = document
            .apply_transaction_with_diff(CanvasTransaction::single(DocumentCommand::UpdateNode(
                moved,
            )))
            .unwrap();
        query.apply_diff_with_router(&document, &diff, &CanvasDefaultEdgeRouter);

        assert!(
            query
                .hit_test(point(px(10.0), px(10.0)), HitOptions::default())
                .next()
                .is_none()
        );
        assert!(
            query
                .hit_test(point(px(210.0), px(10.0)), HitOptions::default())
                .any(|record| record.target == HitTarget::Node(NodeId::from("a")))
        );
    }
}
