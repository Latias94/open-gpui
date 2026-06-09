use crate::{
    CanvasDocument, CanvasDocumentDiff, CanvasEdgeRouter, CanvasGeometryResolver,
    CanvasKindRegistry, CanvasRecordId, HitRecord, HitTarget,
};
use indexmap::{IndexMap, IndexSet};
use open_gpui::{Bounds, Pixels, Point};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CanvasSpatialCache {
    base: SpatialRecordSet,
    overlay: SpatialRecordSet,
    merged: SpatialRecordSet,
    stale: IndexSet<CanvasRecordId>,
    ordinals: IndexMap<HitTarget, usize>,
    compact_after: usize,
}

impl Default for CanvasSpatialCache {
    fn default() -> Self {
        Self {
            base: SpatialRecordSet::default(),
            overlay: SpatialRecordSet::default(),
            merged: SpatialRecordSet::default(),
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
        Self::rebuild_with_resolver(CanvasGeometryResolver::with_router(document, router))
    }

    pub(crate) fn rebuild_with_router_and_kind_registry<R>(
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
        let records = materialize_records(resolver);
        let ordinals = ordinals_for_records(&records);
        let base = SpatialRecordSet::new(records);
        Self {
            merged: base.clone(),
            base,
            overlay: SpatialRecordSet::default(),
            stale: IndexSet::new(),
            ordinals,
            compact_after: 256,
        }
    }

    pub(crate) fn apply_diff_with_router<R>(
        &mut self,
        document: &CanvasDocument,
        diff: &CanvasDocumentDiff,
        router: &R,
    ) where
        R: CanvasEdgeRouter + ?Sized,
    {
        self.apply_diff_with_resolver(CanvasGeometryResolver::with_router(document, router), diff);
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

        let dirty = dirty_record_ids(resolver.document(), diff);
        if dirty.is_empty() {
            return;
        }

        for record_id in dirty {
            self.stale.insert(record_id.clone());
            self.overlay.remove_record(&record_id);
            let records = refresh_records_with_resolver(resolver, &record_id);
            self.assign_ordinals(&records);
            self.overlay.extend(records, &self.ordinals);
        }
        self.overlay.sort();
        self.refresh_merged();

        if self.stale.len() > self.compact_after {
            self.compact(resolver);
        }
    }

    pub(crate) fn query_candidates(
        &self,
        viewport: Bounds<Pixels>,
    ) -> impl Iterator<Item = &IndexedHitRecord> {
        self.merged
            .records
            .iter()
            .filter(move |record| record.record.bounds.intersects(&viewport))
    }

    pub(crate) fn hit_test_candidates(
        &self,
        point: Point<Pixels>,
        margin: Pixels,
    ) -> impl Iterator<Item = &IndexedHitRecord> {
        self.merged.records.iter().filter(move |record| {
            let bounds = if margin == Pixels::ZERO {
                record.record.bounds
            } else {
                record.record.bounds.dilate(margin)
            };
            bounds.contains(&point)
        })
    }

    fn refresh_merged(&mut self) {
        let records = self
            .base
            .records
            .iter()
            .filter(|record| {
                !self
                    .stale
                    .contains(&record_id_for_target(&record.record.target))
            })
            .cloned()
            .chain(self.overlay.records.iter().cloned())
            .collect::<Vec<_>>();
        self.merged = SpatialRecordSet { records };
        self.merged.sort();
    }

    fn compact<R>(&mut self, resolver: CanvasGeometryResolver<'_, R>)
    where
        R: CanvasEdgeRouter + Copy,
    {
        self.base = SpatialRecordSet::new(materialize_records(resolver));
        self.overlay = SpatialRecordSet::default();
        self.merged = self.base.clone();
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

pub(crate) fn materialize_records<R>(resolver: CanvasGeometryResolver<'_, R>) -> Vec<HitRecord>
where
    R: CanvasEdgeRouter + Copy,
{
    let document = resolver.document();
    let mut records = Vec::new();

    for node in document.nodes() {
        records.extend(refresh_records_with_resolver(
            resolver,
            &CanvasRecordId::Node(node.id.clone()),
        ));
    }

    for shape in document.shapes() {
        records.extend(refresh_records_with_resolver(
            resolver,
            &CanvasRecordId::Shape(shape.id.clone()),
        ));
    }

    for edge in document.edges() {
        records.extend(refresh_records_with_resolver(
            resolver,
            &CanvasRecordId::Edge(edge.id.clone()),
        ));
    }

    records
}

pub(crate) fn refresh_records_with_resolver<R>(
    resolver: CanvasGeometryResolver<'_, R>,
    record_id: &CanvasRecordId,
) -> Vec<HitRecord>
where
    R: CanvasEdgeRouter + Copy,
{
    let document = resolver.document();
    let mut records = Vec::new();

    match record_id {
        CanvasRecordId::Node(id) => {
            let Some(node) = document.node(id) else {
                return records;
            };

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
        CanvasRecordId::Edge(id) => {
            let Some(edge) = document.edge(id) else {
                return records;
            };

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
        CanvasRecordId::Shape(id) => {
            let Some(shape) = document.shape(id) else {
                return records;
            };

            records.push(HitRecord {
                target: HitTarget::Shape(shape.id.clone()),
                bounds: resolver.shape_bounds(shape),
                z_index: shape.z_index,
                hidden: shape.hidden,
                locked: shape.locked,
            });
        }
    }

    records
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
    let mut dirty = IndexSet::new();

    for record_id in diff
        .removed
        .iter()
        .chain(&diff.updated)
        .chain(&diff.inserted)
    {
        dirty.insert(record_id.clone());
        if let CanvasRecordId::Node(id) = record_id {
            dirty_incident_edges(document, id, &mut dirty);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanvasEdge, CanvasEndpoint, CanvasNode, CanvasShape, DocumentCommand, NodeId};
    use open_gpui::{point, px, size};

    #[test]
    fn base_only_cache_matches_spatial_index_query_order() {
        let mut document = CanvasDocument::default();
        let mut back = CanvasNode::new("back", point(px(0.0), px(0.0)), size(px(50.0), px(50.0)));
        back.z_index = 1;
        let mut front = CanvasShape::new(
            "front",
            Bounds::new(point(px(10.0), px(10.0)), size(px(50.0), px(50.0))),
        );
        front.z_index = 2;
        document.insert_node(back).unwrap();
        document.insert_shape(front).unwrap();

        let cache =
            CanvasSpatialCache::rebuild_with_router(&document, &crate::CanvasDefaultEdgeRouter);

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
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .unwrap();
        let mut cache =
            CanvasSpatialCache::rebuild_with_router(&document, &crate::CanvasDefaultEdgeRouter);

        let mut moved = document.node(&NodeId::from("a")).unwrap().clone();
        moved.position = point(px(200.0), px(0.0));
        let diff = document
            .apply_transaction_with_diff(crate::CanvasTransaction::single(
                DocumentCommand::UpdateNode(moved),
            ))
            .unwrap();
        cache.apply_diff_with_router(&document, &diff, &crate::CanvasDefaultEdgeRouter);

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
        let mut cache =
            CanvasSpatialCache::rebuild_with_router(&document, &crate::CanvasDefaultEdgeRouter);

        let mut moved = document.node(&NodeId::from("a")).unwrap().clone();
        moved.position = point(px(40.0), px(0.0));
        let diff = document
            .apply_transaction_with_diff(crate::CanvasTransaction::single(
                DocumentCommand::UpdateNode(moved),
            ))
            .unwrap();
        cache.apply_diff_with_router(&document, &diff, &crate::CanvasDefaultEdgeRouter);

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
