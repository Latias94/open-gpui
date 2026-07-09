use crate::{
    CanvasDocument, CanvasDocumentDiff, CanvasKindRegistry, CanvasRecordId, HitRecord, HitTarget,
    geometry_facts::CanvasGeometryFacts, graph::CanvasGraphIndex, routing::CanvasEdgeRouter,
};
use indexmap::{IndexMap, IndexSet};
use open_gpui::{Bounds, Pixels, Point, Size, px};
use static_aabb2d_index::{StaticAABB2DIndex, StaticAABB2DIndexBuilder};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CanvasSpatialCache {
    base: StaticSpatialRecordSet,
    overlay: SpatialRecordSet,
    stale: IndexSet<CanvasRecordId>,
    ordinals: IndexMap<HitTarget, usize>,
    compact_after: usize,
}

impl Default for CanvasSpatialCache {
    fn default() -> Self {
        Self {
            base: StaticSpatialRecordSet::default(),
            overlay: SpatialRecordSet::default(),
            stale: IndexSet::new(),
            ordinals: IndexMap::new(),
            compact_after: 256,
        }
    }
}

impl CanvasSpatialCache {
    pub(crate) fn rebuild_with_router<R>(document: &CanvasDocument, router: &R) -> Self
    where
        R: CanvasEdgeRouter + ?Sized,
    {
        Self::rebuild_with_facts(CanvasGeometryFacts::with_router(document, router))
    }

    pub(crate) fn rebuild_with_router_and_kind_registry<R>(
        document: &CanvasDocument,
        router: &R,
        kind_registry: &CanvasKindRegistry,
    ) -> Self
    where
        R: CanvasEdgeRouter + ?Sized,
    {
        Self::rebuild_with_facts(CanvasGeometryFacts::with_router_and_kind_registry(
            document,
            router,
            Some(kind_registry),
        ))
    }

    fn rebuild_with_facts<R>(facts: CanvasGeometryFacts<'_, R>) -> Self
    where
        R: CanvasEdgeRouter + Copy,
    {
        let records = facts.hit_records();
        let ordinals = ordinals_for_records(&records);
        let base = StaticSpatialRecordSet::new(records);
        Self {
            base,
            overlay: SpatialRecordSet::default(),
            stale: IndexSet::new(),
            ordinals,
            compact_after: 256,
        }
    }

    pub(crate) fn apply_diff_with_graph_index_and_router<R>(
        &mut self,
        document: &CanvasDocument,
        diff: &CanvasDocumentDiff,
        graph_index: &CanvasGraphIndex,
        router: &R,
    ) where
        R: CanvasEdgeRouter + ?Sized,
    {
        self.apply_diff_with_graph_index_and_facts(
            CanvasGeometryFacts::with_router(document, router),
            diff,
            graph_index,
        );
    }

    pub(crate) fn apply_diff_with_graph_index_router_and_kind_registry<R>(
        &mut self,
        document: &CanvasDocument,
        diff: &CanvasDocumentDiff,
        graph_index: &CanvasGraphIndex,
        router: &R,
        kind_registry: &CanvasKindRegistry,
    ) where
        R: CanvasEdgeRouter + ?Sized,
    {
        self.apply_diff_with_graph_index_and_facts(
            CanvasGeometryFacts::with_router_and_kind_registry(
                document,
                router,
                Some(kind_registry),
            ),
            diff,
            graph_index,
        );
    }

    fn apply_diff_with_graph_index_and_facts<R>(
        &mut self,
        facts: CanvasGeometryFacts<'_, R>,
        diff: &CanvasDocumentDiff,
        graph_index: &CanvasGraphIndex,
    ) where
        R: CanvasEdgeRouter + Copy,
    {
        if diff.is_empty() {
            return;
        }

        let dirty = dirty_record_ids_with_graph_index(diff, graph_index);
        if dirty.is_empty() {
            return;
        }

        for record_id in dirty {
            self.stale.insert(record_id.clone());
            self.overlay.remove_record(&record_id);
            let records = facts.hit_records_for_record(&record_id);
            self.assign_ordinals(&records);
            self.overlay.extend(records, &self.ordinals);
        }
        self.overlay.sort();

        if self.stale.len() > self.compact_after {
            self.compact(facts);
        }
    }

    pub(crate) fn query_candidates(
        &self,
        viewport: Bounds<Pixels>,
    ) -> impl Iterator<Item = &IndexedHitRecord> {
        self.base
            .query(viewport)
            .filter(|record| !self.is_stale(record))
            .chain(self.overlay.query(viewport))
    }

    pub(crate) fn hit_test_candidates(
        &self,
        point: Point<Pixels>,
        margin: Pixels,
    ) -> impl Iterator<Item = &IndexedHitRecord> {
        let viewport = point_query_bounds(point, margin);
        self.base
            .query(viewport)
            .filter(move |record| {
                !self.is_stale(record) && record_contains_point(record, point, margin)
            })
            .chain(
                self.overlay
                    .query(viewport)
                    .filter(move |record| record_contains_point(record, point, margin)),
            )
    }

    fn compact<R>(&mut self, facts: CanvasGeometryFacts<'_, R>)
    where
        R: CanvasEdgeRouter + Copy,
    {
        self.base = StaticSpatialRecordSet::new(facts.hit_records());
        self.overlay = SpatialRecordSet::default();
        self.stale.clear();
    }

    fn assign_ordinals(&mut self, records: &[HitRecord]) {
        for record in records {
            if self.ordinals.contains_key(&record.target) {
                continue;
            }
            self.ordinals
                .insert(record.target.clone(), self.ordinals.len());
        }
    }

    fn is_stale(&self, record: &IndexedHitRecord) -> bool {
        self.stale
            .contains(&record_id_for_target(&record.record.target))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct StaticSpatialRecordSet {
    records: Vec<IndexedHitRecord>,
    index: StaticAABB2DIndex<f32>,
}

impl PartialEq for StaticSpatialRecordSet {
    fn eq(&self, other: &Self) -> bool {
        self.records == other.records
    }
}

impl Default for StaticSpatialRecordSet {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl StaticSpatialRecordSet {
    pub(crate) fn new(records: Vec<HitRecord>) -> Self {
        let records = SpatialRecordSet::new(records).records;
        let mut builder = StaticAABB2DIndexBuilder::new(records.len());
        for record in &records {
            let [min_x, min_y, max_x, max_y] = aabb_extents(record.record.bounds);
            builder.add(min_x, min_y, max_x, max_y);
        }

        Self {
            records,
            index: builder
                .build()
                .expect("static spatial cache should add every hit record extent"),
        }
    }

    pub(crate) fn query(
        &self,
        viewport: Bounds<Pixels>,
    ) -> impl Iterator<Item = &IndexedHitRecord> {
        let [min_x, min_y, max_x, max_y] = aabb_extents(viewport);
        self.index
            .query_iter(min_x, min_y, max_x, max_y)
            .map(|index| &self.records[index])
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SpatialRecordSet {
    records: Vec<IndexedHitRecord>,
}

impl SpatialRecordSet {
    pub(crate) fn new(records: Vec<HitRecord>) -> Self {
        let ordinals = ordinals_for_records(&records);
        let mut set = Self {
            records: records
                .into_iter()
                .map(|record| IndexedHitRecord {
                    ordinal: ordinals[&record.target],
                    record,
                })
                .collect(),
        };
        set.sort();
        set
    }

    pub(crate) fn extend(
        &mut self,
        records: Vec<HitRecord>,
        ordinals: &IndexMap<HitTarget, usize>,
    ) {
        self.records
            .extend(records.into_iter().map(|record| IndexedHitRecord {
                ordinal: ordinals[&record.target],
                record,
            }));
    }

    pub(crate) fn remove_record(&mut self, record_id: &CanvasRecordId) {
        self.records
            .retain(|record| record_id_for_target(&record.record.target) != *record_id);
    }

    pub(crate) fn query(
        &self,
        viewport: Bounds<Pixels>,
    ) -> impl Iterator<Item = &IndexedHitRecord> {
        self.records
            .iter()
            .filter(move |record| record.record.bounds.intersects(&viewport))
    }

    pub(crate) fn sort(&mut self) {
        self.records.sort_by(|left, right| {
            left.record
                .z_index
                .cmp(&right.record.z_index)
                .then_with(|| left.ordinal.cmp(&right.ordinal))
        });
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct IndexedHitRecord {
    pub(crate) ordinal: usize,
    pub(crate) record: HitRecord,
}

pub(crate) fn remove_record(records: &mut Vec<HitRecord>, record_id: &CanvasRecordId) {
    records.retain(|record| record_id_for_target(&record.target) != *record_id);
}

pub(crate) fn record_id_for_target(target: &HitTarget) -> CanvasRecordId {
    match target {
        HitTarget::Node(id) => CanvasRecordId::Node(id.clone()),
        HitTarget::Handle { node_id, .. } => CanvasRecordId::Node(node_id.clone()),
        HitTarget::Shape(id) => CanvasRecordId::Shape(id.clone()),
        HitTarget::Edge(id) => CanvasRecordId::Edge(id.clone()),
    }
}

pub(crate) fn dirty_record_ids(
    document: &CanvasDocument,
    diff: &CanvasDocumentDiff,
) -> IndexSet<CanvasRecordId> {
    dirty_record_ids_with_edges(diff, |node_id, dirty| {
        dirty_incident_edges(document, node_id, dirty);
    })
}

pub(crate) fn dirty_record_ids_with_graph_index(
    diff: &CanvasDocumentDiff,
    graph_index: &CanvasGraphIndex,
) -> IndexSet<CanvasRecordId> {
    dirty_record_ids_with_edges(diff, |node_id, dirty| {
        for edge_id in graph_index.incident_edge_ids(node_id) {
            dirty.insert(CanvasRecordId::Edge(edge_id.clone()));
        }
    })
}

fn dirty_record_ids_with_edges(
    diff: &CanvasDocumentDiff,
    mut dirty_edges_for_node: impl FnMut(&crate::NodeId, &mut IndexSet<CanvasRecordId>),
) -> IndexSet<CanvasRecordId> {
    let mut dirty = IndexSet::new();

    for record_id in diff
        .removed
        .iter()
        .chain(&diff.updated)
        .chain(&diff.inserted)
    {
        dirty.insert(record_id.clone());
        if let CanvasRecordId::Node(id) = record_id {
            dirty_edges_for_node(id, &mut dirty);
        }
    }

    dirty
}

fn dirty_incident_edges(
    document: &CanvasDocument,
    node_id: &crate::NodeId,
    dirty: &mut IndexSet<CanvasRecordId>,
) {
    for edge in document.edges() {
        if edge.source.node_id == *node_id || edge.target.node_id == *node_id {
            dirty.insert(CanvasRecordId::Edge(edge.id.clone()));
        }
    }
}

fn ordinals_for_records(records: &[HitRecord]) -> IndexMap<HitTarget, usize> {
    records
        .iter()
        .enumerate()
        .map(|(ordinal, record)| (record.target.clone(), ordinal))
        .collect()
}

fn record_contains_point(record: &IndexedHitRecord, point: Point<Pixels>, margin: Pixels) -> bool {
    let bounds = if margin == Pixels::ZERO {
        record.record.bounds
    } else {
        record.record.bounds.dilate(margin)
    };
    bounds.contains(&point)
}

fn point_query_bounds(point: Point<Pixels>, margin: Pixels) -> Bounds<Pixels> {
    let extent = margin.max(px(1.0));
    Bounds::centered_at(point, Size::new(extent * 2.0, extent * 2.0))
}

fn aabb_extents(bounds: Bounds<Pixels>) -> [f32; 4] {
    let bottom_right = bounds.bottom_right();
    [
        bounds.origin.x.as_f32(),
        bounds.origin.y.as_f32(),
        bottom_right.x.as_f32(),
        bottom_right.y.as_f32(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::CanvasDefaultEdgeRouter;
    use crate::test_support::document_fixture;
    use crate::{CanvasEdge, CanvasEndpoint, CanvasNode, CanvasShape, DocumentCommand, NodeId};
    use open_gpui::{point, px, size};

    #[test]
    fn base_only_cache_matches_spatial_index_query_order() {
        let mut back = CanvasNode::new("back", point(px(0.0), px(0.0)), size(px(50.0), px(50.0)));
        back.z_index = 1;
        let mut front = CanvasShape::new(
            "front",
            Bounds::new(point(px(10.0), px(10.0)), size(px(50.0), px(50.0))),
        );
        front.z_index = 2;
        let document = document_fixture().node(back).shape(front).build();

        let cache = CanvasSpatialCache::rebuild_with_router(&document, &CanvasDefaultEdgeRouter);

        assert_eq!(
            cache
                .query_candidates(Bounds::new(
                    point(px(0.0), px(0.0)),
                    size(px(100.0), px(100.0))
                ))
                .map(|record| record.record.target.clone())
                .collect::<Vec<_>>(),
            vec![
                HitTarget::Node("back".into()),
                HitTarget::Shape("front".into())
            ]
        );
    }

    #[test]
    fn overlay_records_replace_stale_base_records() {
        let mut document = document_fixture()
            .node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .build();
        let mut cache =
            CanvasSpatialCache::rebuild_with_router(&document, &CanvasDefaultEdgeRouter);

        let mut moved = document.node(&NodeId::from("a")).unwrap().clone();
        moved.position = point(px(200.0), px(0.0));
        let diff = document
            .apply_transaction_with_diff(crate::CanvasTransaction::single(
                DocumentCommand::UpdateNode(moved),
            ))
            .unwrap();
        let graph_index = document.graph_index();
        cache.apply_diff_with_graph_index_and_router(
            &document,
            &diff,
            &graph_index,
            &CanvasDefaultEdgeRouter,
        );

        assert!(
            cache
                .hit_test_candidates(point(px(10.0), px(10.0)), Pixels::ZERO)
                .next()
                .is_none()
        );
        assert!(
            cache
                .hit_test_candidates(point(px(210.0), px(10.0)), Pixels::ZERO)
                .any(|record| record.record.target == HitTarget::Node(NodeId::from("a")))
        );
    }

    #[test]
    fn moving_node_refreshes_incident_edge_overlay_record() {
        let mut document = document_fixture()
            .node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(20.0), px(20.0)),
            ))
            .node(CanvasNode::new(
                "b",
                point(px(100.0), px(0.0)),
                size(px(20.0), px(20.0)),
            ))
            .edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .build();
        let mut cache =
            CanvasSpatialCache::rebuild_with_router(&document, &CanvasDefaultEdgeRouter);

        let mut moved = document.node(&NodeId::from("a")).unwrap().clone();
        moved.position = point(px(40.0), px(0.0));
        let diff = document
            .apply_transaction_with_diff(crate::CanvasTransaction::single(
                DocumentCommand::UpdateNode(moved),
            ))
            .unwrap();
        let graph_index = document.graph_index();
        cache.apply_diff_with_graph_index_and_router(
            &document,
            &diff,
            &graph_index,
            &CanvasDefaultEdgeRouter,
        );

        assert!(
            cache
                .query_candidates(Bounds::new(
                    point(px(0.0), px(0.0)),
                    size(px(160.0), px(40.0))
                ))
                .any(|record| {
                    record.record.target == HitTarget::Edge("a-b".into())
                        && record.record.bounds.origin == point(px(44.0), px(4.0))
                        && record.record.bounds.size.width == px(72.0)
                        && record.record.bounds.size.height == px(12.0)
                })
        );
    }
}
