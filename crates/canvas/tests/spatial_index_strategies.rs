use open_gpui::{Bounds, Pixels, Point, point, px, size};
use open_gpui_canvas::advanced::{
    CanvasCommittedMutation, CanvasEdgeRouter, CanvasRoutePath, CanvasRouteRequest, SpatialIndex,
};
use open_gpui_canvas::{
    CanvasDocument, CanvasEdge, CanvasEndpoint, CanvasHandle, CanvasKindRegistry, CanvasNode,
    CanvasNodeGeometryPolicy, CanvasNodeKind, CanvasRecordId, CanvasRuntime, CanvasShape,
    CanvasShapeGeometryPolicy, CanvasShapeKind, CanvasTransaction, DocumentCommand, HitOptions,
    HitRecord, HitTarget, NodeId,
};
use rstar::{AABB as RStarAabb, RTree, RTreeObject};
use static_aabb2d_index::{StaticAABB2DIndex, StaticAABB2DIndexBuilder};
use std::collections::HashSet;

#[test]
fn candidate_query_results_match_spatial_index() {
    let cases = [
        (
            "grid",
            MaterializedFixture::default_router(grid_document(8, 6)),
        ),
        (
            "dense_overlap",
            MaterializedFixture::default_router(dense_overlap_document()),
        ),
        (
            "clustered",
            MaterializedFixture::default_router(clustered_document()),
        ),
        (
            "long_edges",
            MaterializedFixture::custom_router(long_edge_document(), &VerticalDetourRouter),
        ),
        (
            "mixed_kind_geometry",
            MaterializedFixture::kind_registry(mixed_document(), geometry_registry()),
        ),
    ];
    let viewports = [
        bounds(0.0, 0.0, 320.0, 240.0),
        bounds(90.0, 70.0, 520.0, 360.0),
        bounds(900.0, 20.0, 560.0, 420.0),
        bounds(-40.0, -40.0, 160.0, 160.0),
    ];
    let options = hit_options();

    for (name, fixture) in cases {
        assert_query_parity(
            name,
            &fixture.index,
            &fixture.candidates,
            &viewports,
            &options,
        );
    }
}

#[test]
fn runtime_query_results_match_spatial_index() {
    let cases = [
        ("grid", RuntimeFixture::default_router(grid_document(8, 6))),
        (
            "dense_overlap",
            RuntimeFixture::default_router(dense_overlap_document()),
        ),
        (
            "clustered",
            RuntimeFixture::default_router(clustered_document()),
        ),
        (
            "long_edges",
            RuntimeFixture::custom_router(long_edge_document(), &VerticalDetourRouter),
        ),
        (
            "mixed_kind_geometry",
            RuntimeFixture::kind_registry(mixed_document(), geometry_registry()),
        ),
    ];
    let viewports = [
        bounds(0.0, 0.0, 320.0, 240.0),
        bounds(90.0, 70.0, 520.0, 360.0),
        bounds(900.0, 20.0, 560.0, 420.0),
        bounds(-40.0, -40.0, 160.0, 160.0),
    ];

    for (name, fixture) in cases {
        assert_runtime_query_parity(name, &fixture, &viewports, &hit_options());
    }
}

#[test]
fn candidate_hit_tests_match_spatial_index_ordering() {
    let cases = [
        (
            "dense_overlap",
            MaterializedFixture::default_router(dense_overlap_document()),
        ),
        (
            "mixed_kind_geometry",
            MaterializedFixture::kind_registry(mixed_document(), geometry_registry()),
        ),
        (
            "custom_router",
            MaterializedFixture::custom_router(long_edge_document(), &VerticalDetourRouter),
        ),
    ];
    let points = [
        point(px(52.0), px(52.0)),
        point(px(95.0), px(50.0)),
        point(px(130.0), px(90.0)),
        point(px(530.0), px(220.0)),
        point(px(1_220.0), px(240.0)),
    ];
    let options = hit_options();

    for (name, fixture) in cases {
        assert_hit_test_parity(name, &fixture.index, &fixture.candidates, &points, &options);
    }
}

#[test]
fn runtime_hit_tests_match_spatial_index_ordering() {
    let cases = [
        (
            "dense_overlap",
            RuntimeFixture::default_router(dense_overlap_document()),
        ),
        (
            "mixed_kind_geometry",
            RuntimeFixture::kind_registry(mixed_document(), geometry_registry()),
        ),
        (
            "custom_router",
            RuntimeFixture::custom_router(long_edge_document(), &VerticalDetourRouter),
        ),
    ];
    let points = [
        point(px(52.0), px(52.0)),
        point(px(95.0), px(50.0)),
        point(px(130.0), px(90.0)),
        point(px(530.0), px(220.0)),
        point(px(1_220.0), px(240.0)),
    ];

    for (name, fixture) in cases {
        assert_runtime_hit_test_parity(name, &fixture, &points, &hit_options());
    }
}

#[test]
fn candidates_preserve_hit_options_for_hidden_locked_handles_and_margin() {
    let fixture = MaterializedFixture::default_router(visibility_document());
    let viewport = bounds(0.0, 0.0, 180.0, 120.0);
    let handle_point = point(px(96.0), px(50.0));
    let margin_point = point(px(111.0), px(50.0));
    let option_sets = hit_options();

    assert_query_parity(
        "visibility",
        &fixture.index,
        &fixture.candidates,
        &[viewport],
        &option_sets,
    );
    assert_hit_test_parity(
        "visibility",
        &fixture.index,
        &fixture.candidates,
        &[handle_point, margin_point],
        &option_sets,
    );
}

#[test]
fn hybrid_overlay_matches_oracle_during_drag_frames() {
    let base_document = grid_document(12, 10);
    let base_index = SpatialIndex::rebuild(&base_document);
    let base_records = base_index.records().to_vec();
    let viewports = [
        bounds(0.0, 0.0, 640.0, 420.0),
        bounds(420.0, 0.0, 760.0, 500.0),
        bounds(1_080.0, 360.0, 760.0, 560.0),
    ];
    let points = [
        point(px(56.0), px(32.0)),
        point(px(510.0), px(124.0)),
        point(px(1_180.0), px(430.0)),
    ];
    let options = hit_options();

    for selected_count in [1, 10, 100] {
        let selected_nodes = base_document
            .node_ids()
            .take(selected_count)
            .cloned()
            .collect::<Vec<_>>();
        let stale_records = stale_record_ids_for_nodes(&base_document, &selected_nodes);
        let mut document = base_document.clone();

        for frame in [1, 60, 120] {
            move_selected_nodes(&mut document, &selected_nodes, frame as f32);
            let oracle = SpatialIndex::rebuild(&document);
            let overlay_records = indexed_overlay_records(&oracle, &stale_records);
            let hybrid = HybridOverlayCandidate::new(
                base_records.clone(),
                overlay_records,
                stale_records.clone(),
            );

            assert_hybrid_parity(
                &format!("hybrid_drag_{selected_count}_frame_{frame}"),
                &oracle,
                &hybrid,
                &viewports,
                &points,
                &options,
            );
        }
    }
}

#[test]
fn hybrid_overlay_suppresses_deleted_node_and_incident_edges() {
    let base_document = grid_document(4, 2);
    let base_index = SpatialIndex::rebuild(&base_document);
    let mut document = base_document.clone();
    let removed = NodeId::from("node-0-1");
    let stale_records = stale_record_ids_for_node_removal(&base_document, &removed);

    apply_command(&mut document, DocumentCommand::RemoveNode(removed));
    let oracle = SpatialIndex::rebuild(&document);
    let hybrid =
        HybridOverlayCandidate::new(base_index.records().to_vec(), Vec::new(), stale_records);

    assert_hybrid_parity(
        "hybrid_delete_node",
        &oracle,
        &hybrid,
        &[bounds(0.0, 0.0, 640.0, 320.0)],
        &[point(px(148.0), px(28.0)), point(px(286.0), px(28.0))],
        &hit_options(),
    );
}

#[test]
fn runtime_diff_updates_match_oracle_during_drag_frames() {
    let base_document = grid_document(12, 10);
    let mut runtime = CanvasRuntime::rebuild(&base_document);
    let viewports = [
        bounds(0.0, 0.0, 640.0, 420.0),
        bounds(420.0, 0.0, 760.0, 500.0),
        bounds(1_080.0, 360.0, 760.0, 560.0),
    ];
    let points = [
        point(px(56.0), px(32.0)),
        point(px(510.0), px(124.0)),
        point(px(1_180.0), px(430.0)),
    ];
    let selected_nodes = base_document
        .node_ids()
        .take(10)
        .cloned()
        .collect::<Vec<_>>();
    let mut document = base_document.clone();

    for frame in [1, 60, 120] {
        let committed = move_selected_nodes(&mut document, &selected_nodes, frame as f32);
        runtime.apply_committed_mutation(&document, &committed);
        let oracle = SpatialIndex::rebuild(&document);

        assert_runtime_against_oracle(
            &format!("runtime_drag_frame_{frame}"),
            &oracle,
            &runtime,
            &viewports,
            &points,
            &hit_options(),
        );
    }
}

struct MaterializedFixture {
    index: SpatialIndex,
    candidates: CandidateIndexes,
}

struct RuntimeFixture {
    oracle: SpatialIndex,
    runtime: CanvasRuntime,
}

impl RuntimeFixture {
    fn default_router(document: CanvasDocument) -> Self {
        Self {
            oracle: SpatialIndex::rebuild(&document),
            runtime: CanvasRuntime::rebuild(&document),
        }
    }

    fn custom_router<R>(document: CanvasDocument, router: &R) -> Self
    where
        R: CanvasEdgeRouter + ?Sized,
    {
        Self {
            oracle: SpatialIndex::rebuild_with_router(&document, router),
            runtime: CanvasRuntime::rebuild_with_router(&document, router),
        }
    }

    fn kind_registry(document: CanvasDocument, kind_registry: CanvasKindRegistry) -> Self {
        Self {
            oracle: SpatialIndex::rebuild_with_kind_registry(&document, &kind_registry),
            runtime: CanvasRuntime::rebuild_with_kind_registry(&document, &kind_registry),
        }
    }
}

impl MaterializedFixture {
    fn default_router(document: CanvasDocument) -> Self {
        Self::from_index(SpatialIndex::rebuild(&document))
    }

    fn custom_router<R>(document: CanvasDocument, router: &R) -> Self
    where
        R: CanvasEdgeRouter + ?Sized,
    {
        Self::from_index(SpatialIndex::rebuild_with_router(&document, router))
    }

    fn kind_registry(document: CanvasDocument, kind_registry: CanvasKindRegistry) -> Self {
        Self::from_index(SpatialIndex::rebuild_with_kind_registry(
            &document,
            &kind_registry,
        ))
    }

    fn from_index(index: SpatialIndex) -> Self {
        let records = index.records().to_vec();

        Self {
            index,
            candidates: CandidateIndexes::new(records),
        }
    }
}

struct CandidateIndexes {
    rstar: RStarCandidate,
    static_aabb: StaticAabbCandidate,
}

impl CandidateIndexes {
    fn new(records: Vec<HitRecord>) -> Self {
        Self {
            rstar: RStarCandidate::new(&records),
            static_aabb: StaticAabbCandidate::new(records),
        }
    }

    fn iter(&self) -> [(&'static str, &dyn CandidateSpatialIndex); 2] {
        [
            ("rstar", &self.rstar as &dyn CandidateSpatialIndex),
            (
                "static_aabb",
                &self.static_aabb as &dyn CandidateSpatialIndex,
            ),
        ]
    }
}

trait CandidateSpatialIndex {
    fn query_targets(&self, viewport: Bounds<Pixels>, options: HitOptions) -> Vec<HitTarget>;

    fn hit_test_targets(&self, point: Point<Pixels>, options: HitOptions) -> Vec<HitTarget>;
}

#[derive(Clone)]
struct IndexedRecord {
    ordinal: usize,
    record: HitRecord,
}

struct RStarCandidate {
    tree: RTree<IndexedRecord>,
}

impl RStarCandidate {
    fn new(records: &[HitRecord]) -> Self {
        Self::new_indexed(indexed_records(records))
    }

    fn new_indexed(records: Vec<IndexedRecord>) -> Self {
        Self {
            tree: RTree::bulk_load(records),
        }
    }

    fn query_records(&self, viewport: Bounds<Pixels>) -> Vec<IndexedRecord> {
        self.tree
            .locate_in_envelope_intersecting(rstar_envelope(viewport))
            .cloned()
            .collect()
    }
}

impl RTreeObject for IndexedRecord {
    type Envelope = RStarAabb<[f32; 2]>;

    fn envelope(&self) -> Self::Envelope {
        rstar_envelope(self.record.bounds)
    }
}

impl CandidateSpatialIndex for RStarCandidate {
    fn query_targets(&self, viewport: Bounds<Pixels>, options: HitOptions) -> Vec<HitTarget> {
        let mut records = self
            .query_records(viewport)
            .into_iter()
            .filter(|record| query_matches(&record.record, viewport, options))
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            left.record
                .z_index
                .cmp(&right.record.z_index)
                .then_with(|| left.ordinal.cmp(&right.ordinal))
        });
        records
            .into_iter()
            .map(|record| record.record.target)
            .collect()
    }

    fn hit_test_targets(&self, point: Point<Pixels>, options: HitOptions) -> Vec<HitTarget> {
        let viewport = point_query_bounds(point, options.margin);
        let mut records = self
            .query_records(viewport)
            .into_iter()
            .filter(|record| hit_matches(&record.record, point, options))
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            right
                .record
                .z_index
                .cmp(&left.record.z_index)
                .then_with(|| right.ordinal.cmp(&left.ordinal))
        });
        records
            .into_iter()
            .map(|record| record.record.target)
            .collect()
    }
}

struct StaticAabbCandidate {
    records: Vec<HitRecord>,
    index: StaticAABB2DIndex<f32>,
}

impl StaticAabbCandidate {
    fn new(records: Vec<HitRecord>) -> Self {
        let mut builder = StaticAABB2DIndexBuilder::new(records.len());
        for record in &records {
            let [min_x, min_y, max_x, max_y] = aabb_extents(record.bounds);
            builder.add(min_x, min_y, max_x, max_y);
        }

        Self {
            records,
            index: builder
                .build()
                .expect("static AABB candidate should receive every record extent"),
        }
    }

    fn query_records(&self, viewport: Bounds<Pixels>) -> Vec<IndexedRecord> {
        let [min_x, min_y, max_x, max_y] = aabb_extents(viewport);
        self.index
            .query(min_x, min_y, max_x, max_y)
            .into_iter()
            .map(|ordinal| IndexedRecord {
                ordinal,
                record: self.records[ordinal].clone(),
            })
            .collect()
    }
}

impl CandidateSpatialIndex for StaticAabbCandidate {
    fn query_targets(&self, viewport: Bounds<Pixels>, options: HitOptions) -> Vec<HitTarget> {
        let mut records = self
            .query_records(viewport)
            .into_iter()
            .filter(|record| query_matches(&record.record, viewport, options))
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            left.record
                .z_index
                .cmp(&right.record.z_index)
                .then_with(|| left.ordinal.cmp(&right.ordinal))
        });
        records
            .into_iter()
            .map(|record| record.record.target)
            .collect()
    }

    fn hit_test_targets(&self, point: Point<Pixels>, options: HitOptions) -> Vec<HitTarget> {
        let viewport = point_query_bounds(point, options.margin);
        let mut records = self
            .query_records(viewport)
            .into_iter()
            .filter(|record| hit_matches(&record.record, point, options))
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            right
                .record
                .z_index
                .cmp(&left.record.z_index)
                .then_with(|| right.ordinal.cmp(&left.ordinal))
        });
        records
            .into_iter()
            .map(|record| record.record.target)
            .collect()
    }
}

struct HybridOverlayCandidate {
    base: StaticAabbCandidate,
    overlay: RStarCandidate,
    stale_records: HashSet<CanvasRecordId>,
}

impl HybridOverlayCandidate {
    fn new(
        base_records: Vec<HitRecord>,
        overlay_records: Vec<IndexedRecord>,
        stale_records: HashSet<CanvasRecordId>,
    ) -> Self {
        Self {
            base: StaticAabbCandidate::new(base_records),
            overlay: RStarCandidate::new_indexed(overlay_records),
            stale_records,
        }
    }

    fn query_records(&self, viewport: Bounds<Pixels>) -> Vec<IndexedRecord> {
        let mut records = self
            .base
            .query_records(viewport)
            .into_iter()
            .filter(|record| !self.is_stale(record))
            .collect::<Vec<_>>();
        records.extend(self.overlay.query_records(viewport));
        records
    }

    fn is_stale(&self, record: &IndexedRecord) -> bool {
        self.stale_records
            .contains(&record_id_for_target(&record.record.target))
    }
}

impl CandidateSpatialIndex for HybridOverlayCandidate {
    fn query_targets(&self, viewport: Bounds<Pixels>, options: HitOptions) -> Vec<HitTarget> {
        let mut records = self
            .query_records(viewport)
            .into_iter()
            .filter(|record| query_matches(&record.record, viewport, options))
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            left.record
                .z_index
                .cmp(&right.record.z_index)
                .then_with(|| left.ordinal.cmp(&right.ordinal))
        });
        records
            .into_iter()
            .map(|record| record.record.target)
            .collect()
    }

    fn hit_test_targets(&self, point: Point<Pixels>, options: HitOptions) -> Vec<HitTarget> {
        let viewport = point_query_bounds(point, options.margin);
        let mut records = self
            .query_records(viewport)
            .into_iter()
            .filter(|record| hit_matches(&record.record, point, options))
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            right
                .record
                .z_index
                .cmp(&left.record.z_index)
                .then_with(|| right.ordinal.cmp(&left.ordinal))
        });
        records
            .into_iter()
            .map(|record| record.record.target)
            .collect()
    }
}

fn assert_query_parity(
    fixture_name: &str,
    oracle: &SpatialIndex,
    candidates: &CandidateIndexes,
    viewports: &[Bounds<Pixels>],
    option_sets: &[HitOptions],
) {
    for viewport in viewports {
        for options in option_sets {
            let expected = oracle
                .query_with_options(*viewport, *options)
                .map(|record| record.target.clone())
                .collect::<Vec<_>>();

            for (candidate_name, candidate) in candidates.iter() {
                assert_eq!(
                    candidate.query_targets(*viewport, *options),
                    expected,
                    "{candidate_name} query mismatch in {fixture_name} for {viewport:?} {options:?}",
                );
            }
        }
    }
}

fn assert_hit_test_parity(
    fixture_name: &str,
    oracle: &SpatialIndex,
    candidates: &CandidateIndexes,
    points: &[Point<Pixels>],
    option_sets: &[HitOptions],
) {
    for point in points {
        for options in option_sets {
            let expected = oracle
                .hit_test(*point, *options)
                .map(|record| record.target.clone())
                .collect::<Vec<_>>();

            for (candidate_name, candidate) in candidates.iter() {
                assert_eq!(
                    candidate.hit_test_targets(*point, *options),
                    expected,
                    "{candidate_name} hit-test mismatch in {fixture_name} for {point:?} {options:?}",
                );
            }
        }
    }
}

fn assert_runtime_query_parity(
    fixture_name: &str,
    fixture: &RuntimeFixture,
    viewports: &[Bounds<Pixels>],
    option_sets: &[HitOptions],
) {
    for viewport in viewports {
        for options in option_sets {
            assert_eq!(
                fixture
                    .runtime
                    .query_with_options(*viewport, *options)
                    .map(|record| record.target.clone())
                    .collect::<Vec<_>>(),
                fixture
                    .oracle
                    .query_with_options(*viewport, *options)
                    .map(|record| record.target.clone())
                    .collect::<Vec<_>>(),
                "runtime query mismatch in {fixture_name} for {viewport:?} {options:?}",
            );
        }
    }
}

fn assert_runtime_hit_test_parity(
    fixture_name: &str,
    fixture: &RuntimeFixture,
    points: &[Point<Pixels>],
    option_sets: &[HitOptions],
) {
    for point in points {
        for options in option_sets {
            assert_eq!(
                fixture
                    .runtime
                    .hit_test(*point, *options)
                    .map(|record| record.target.clone())
                    .collect::<Vec<_>>(),
                fixture
                    .oracle
                    .hit_test(*point, *options)
                    .map(|record| record.target.clone())
                    .collect::<Vec<_>>(),
                "runtime hit-test mismatch in {fixture_name} for {point:?} {options:?}",
            );
        }
    }
}

fn assert_runtime_against_oracle(
    fixture_name: &str,
    oracle: &SpatialIndex,
    runtime: &CanvasRuntime,
    viewports: &[Bounds<Pixels>],
    points: &[Point<Pixels>],
    option_sets: &[HitOptions],
) {
    for viewport in viewports {
        for options in option_sets {
            assert_eq!(
                runtime
                    .query_with_options(*viewport, *options)
                    .map(|record| record.target.clone())
                    .collect::<Vec<_>>(),
                oracle
                    .query_with_options(*viewport, *options)
                    .map(|record| record.target.clone())
                    .collect::<Vec<_>>(),
                "runtime query mismatch in {fixture_name} for {viewport:?} {options:?}",
            );
        }
    }

    for point in points {
        for options in option_sets {
            assert_eq!(
                runtime
                    .hit_test(*point, *options)
                    .map(|record| record.target.clone())
                    .collect::<Vec<_>>(),
                oracle
                    .hit_test(*point, *options)
                    .map(|record| record.target.clone())
                    .collect::<Vec<_>>(),
                "runtime hit-test mismatch in {fixture_name} for {point:?} {options:?}",
            );
        }
    }
}

fn assert_hybrid_parity(
    fixture_name: &str,
    oracle: &SpatialIndex,
    hybrid: &HybridOverlayCandidate,
    viewports: &[Bounds<Pixels>],
    points: &[Point<Pixels>],
    option_sets: &[HitOptions],
) {
    for viewport in viewports {
        for options in option_sets {
            assert_eq!(
                hybrid.query_targets(*viewport, *options),
                oracle
                    .query_with_options(*viewport, *options)
                    .map(|record| record.target.clone())
                    .collect::<Vec<_>>(),
                "hybrid query mismatch in {fixture_name} for {viewport:?} {options:?}",
            );
        }
    }

    for point in points {
        for options in option_sets {
            assert_eq!(
                hybrid.hit_test_targets(*point, *options),
                oracle
                    .hit_test(*point, *options)
                    .map(|record| record.target.clone())
                    .collect::<Vec<_>>(),
                "hybrid hit-test mismatch in {fixture_name} for {point:?} {options:?}",
            );
        }
    }
}

fn indexed_records(records: &[HitRecord]) -> Vec<IndexedRecord> {
    records
        .iter()
        .cloned()
        .enumerate()
        .map(|(ordinal, record)| IndexedRecord { ordinal, record })
        .collect()
}

fn indexed_overlay_records(
    index: &SpatialIndex,
    stale_records: &HashSet<CanvasRecordId>,
) -> Vec<IndexedRecord> {
    index
        .records()
        .iter()
        .cloned()
        .enumerate()
        .filter(|(_, record)| stale_records.contains(&record_id_for_target(&record.target)))
        .map(|(ordinal, record)| IndexedRecord { ordinal, record })
        .collect()
}

fn stale_record_ids_for_nodes(
    document: &CanvasDocument,
    selected_nodes: &[NodeId],
) -> HashSet<CanvasRecordId> {
    let selected = selected_nodes.iter().cloned().collect::<HashSet<_>>();
    let mut stale_records = selected
        .iter()
        .cloned()
        .map(CanvasRecordId::Node)
        .collect::<HashSet<_>>();

    for edge in document.edges() {
        if selected.contains(&edge.source.node_id) || selected.contains(&edge.target.node_id) {
            stale_records.insert(CanvasRecordId::Edge(edge.id.clone()));
        }
    }

    stale_records
}

fn stale_record_ids_for_node_removal(
    document: &CanvasDocument,
    removed_node: &NodeId,
) -> HashSet<CanvasRecordId> {
    let mut stale_records = HashSet::from([CanvasRecordId::Node(removed_node.clone())]);

    for edge in document.edges() {
        if edge.source.node_id == *removed_node || edge.target.node_id == *removed_node {
            stale_records.insert(CanvasRecordId::Edge(edge.id.clone()));
        }
    }

    stale_records
}

fn move_selected_nodes(
    document: &mut CanvasDocument,
    selected_nodes: &[NodeId],
    frame: f32,
) -> CanvasCommittedMutation {
    let mut commands = Vec::new();
    for (index, id) in selected_nodes.iter().enumerate() {
        let mut node = document.node(id).unwrap().clone();
        node.position.x += px(frame * 0.75 + index as f32 * 0.01);
        node.position.y += px(frame * 0.25);
        commands.push(DocumentCommand::UpdateNode(node));
    }
    document
        .commit_transaction(CanvasTransaction::new(commands))
        .unwrap()
}

fn record_id_for_target(target: &HitTarget) -> CanvasRecordId {
    match target {
        HitTarget::Node(id) => CanvasRecordId::Node(id.clone()),
        HitTarget::Handle { node_id, .. } => CanvasRecordId::Node(node_id.clone()),
        HitTarget::Shape(id) => CanvasRecordId::Shape(id.clone()),
        HitTarget::Edge(id) => CanvasRecordId::Edge(id.clone()),
    }
}

fn query_matches(record: &HitRecord, viewport: Bounds<Pixels>, options: HitOptions) -> bool {
    options_match(record, options) && record.bounds.intersects(&viewport)
}

fn hit_matches(record: &HitRecord, point: Point<Pixels>, options: HitOptions) -> bool {
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

fn options_match(record: &HitRecord, options: HitOptions) -> bool {
    (options.include_hidden || !record.hidden)
        && (options.include_locked || !record.locked)
        && (options.include_handles || !matches!(record.target, HitTarget::Handle { .. }))
}

fn point_query_bounds(point: Point<Pixels>, margin: Pixels) -> Bounds<Pixels> {
    let extent = margin.max(px(1.0));
    Bounds::centered_at(point, size(extent * 2.0, extent * 2.0))
}

fn apply_command(document: &mut CanvasDocument, command: DocumentCommand) {
    apply_commands(document, [command]);
}

fn apply_commands(
    document: &mut CanvasDocument,
    commands: impl IntoIterator<Item = DocumentCommand>,
) {
    document
        .apply_transaction(CanvasTransaction::new(commands))
        .unwrap();
}

fn rstar_envelope(bounds: Bounds<Pixels>) -> RStarAabb<[f32; 2]> {
    let [min_x, min_y, max_x, max_y] = aabb_extents(bounds);
    RStarAabb::from_corners([min_x, min_y], [max_x, max_y])
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

fn hit_options() -> [HitOptions; 6] {
    [
        HitOptions::default(),
        HitOptions {
            include_locked: true,
            ..HitOptions::default()
        },
        HitOptions {
            include_hidden: true,
            ..HitOptions::default()
        },
        HitOptions {
            include_handles: true,
            ..HitOptions::default()
        },
        HitOptions {
            include_hidden: true,
            include_locked: true,
            include_handles: true,
            ..HitOptions::default()
        },
        HitOptions {
            include_handles: true,
            margin: px(12.0),
            ..HitOptions::default()
        },
    ]
}

fn grid_document(columns: usize, rows: usize) -> CanvasDocument {
    let mut document = CanvasDocument::default();
    let mut commands = Vec::new();

    for row in 0..rows {
        for column in 0..columns {
            let id = format!("node-{row}-{column}");
            let mut node = CanvasNode::new(
                id.clone(),
                point(px(column as f32 * 130.0), px(row as f32 * 90.0)),
                size(px(96.0), px(56.0)),
            );
            node.z_index = (row * columns + column) as i32;
            commands.push(DocumentCommand::InsertNode(node));

            if column > 0 {
                commands.push(DocumentCommand::InsertEdge(CanvasEdge::new(
                    format!("edge-{row}-{}-{row}-{column}", column - 1),
                    CanvasEndpoint::new(format!("node-{row}-{}", column - 1), None::<String>),
                    CanvasEndpoint::new(id, None::<String>),
                )));
            }
        }
    }

    apply_commands(&mut document, commands);
    document
}

fn dense_overlap_document() -> CanvasDocument {
    let mut document = CanvasDocument::default();
    let mut commands = Vec::new();

    for index in 0..24 {
        let mut node = CanvasNode::new(
            format!("overlap-{index}"),
            point(px(40.0 + index as f32 * 2.0), px(40.0 + index as f32 * 2.0)),
            size(px(80.0), px(80.0)),
        );
        node.z_index = index;
        commands.push(DocumentCommand::InsertNode(node));
    }

    let mut shape = CanvasShape::new("shape-top", bounds(52.0, 52.0, 70.0, 70.0));
    shape.z_index = 40;
    commands.push(DocumentCommand::InsertShape(shape));
    apply_commands(&mut document, commands);
    document
}

fn clustered_document() -> CanvasDocument {
    let mut document = CanvasDocument::default();
    let mut commands = Vec::new();

    for cluster in 0..4 {
        let base_x = cluster as f32 * 600.0;
        let base_y = (cluster % 2) as f32 * 420.0;
        for index in 0..18 {
            let mut node = CanvasNode::new(
                format!("cluster-{cluster}-{index}"),
                point(
                    px(base_x + (index % 6) as f32 * 84.0),
                    px(base_y + (index / 6) as f32 * 72.0),
                ),
                size(px(64.0), px(44.0)),
            );
            node.z_index = (cluster * 100 + index) as i32;
            commands.push(DocumentCommand::InsertNode(node));
        }
    }

    apply_commands(&mut document, commands);
    document
}

fn long_edge_document() -> CanvasDocument {
    let mut document = CanvasDocument::default();
    let mut commands = Vec::new();
    for index in 0..8 {
        let node_id = format!("long-{index}");
        commands.push(DocumentCommand::InsertNode(CanvasNode::new(
            node_id.clone(),
            point(px(index as f32 * 420.0), px((index % 2) as f32 * 260.0)),
            size(px(80.0), px(52.0)),
        )));

        if index > 0 {
            let mut edge = CanvasEdge::new(
                format!("long-edge-{}-{index}", index - 1),
                CanvasEndpoint::new(format!("long-{}", index - 1), None::<String>),
                CanvasEndpoint::new(node_id, None::<String>),
            );
            edge.z_index = 100 + index as i32;
            commands.push(DocumentCommand::InsertEdge(edge));
        }
    }
    apply_commands(&mut document, commands);
    document
}

fn mixed_document() -> CanvasDocument {
    let mut document = grid_document(4, 3);

    let mut wide = CanvasNode::new("wide", point(px(420.0), px(80.0)), size(px(40.0), px(40.0)));
    wide.kind = "wide".to_string();
    wide.z_index = 200;
    wide.handles
        .push(CanvasHandle::new("out", point(px(40.0), px(20.0))));

    let mut shape = CanvasShape::new("shape-wide", bounds(500.0, 110.0, 48.0, 48.0));
    shape.kind = "padded".to_string();
    shape.z_index = 210;
    apply_commands(
        &mut document,
        [
            DocumentCommand::InsertNode(wide),
            DocumentCommand::InsertShape(shape),
        ],
    );

    document
}

fn visibility_document() -> CanvasDocument {
    let mut document = CanvasDocument::default();
    let mut commands = Vec::new();

    let mut node = CanvasNode::new(
        "handles",
        point(px(0.0), px(0.0)),
        size(px(100.0), px(100.0)),
    );
    node.handles
        .push(CanvasHandle::new("out", point(px(96.0), px(50.0))));
    commands.push(DocumentCommand::InsertNode(node));

    let mut hidden = CanvasNode::new(
        "hidden",
        point(px(20.0), px(20.0)),
        size(px(40.0), px(40.0)),
    );
    hidden.hidden = true;
    hidden.z_index = 10;
    commands.push(DocumentCommand::InsertNode(hidden));

    let mut locked = CanvasNode::new(
        "locked",
        point(px(60.0), px(20.0)),
        size(px(40.0), px(40.0)),
    );
    locked.locked = true;
    locked.z_index = 20;
    commands.push(DocumentCommand::InsertNode(locked));

    apply_commands(&mut document, commands);
    document
}

fn geometry_registry() -> CanvasKindRegistry {
    let mut registry = CanvasKindRegistry::open();
    registry.register_node_kind(
        "wide",
        CanvasNodeKind::new().with_geometry_policy(WideNodeKind),
    );
    registry.register_shape_kind(
        "padded",
        CanvasShapeKind::new().with_geometry_policy(PaddedShapeKind),
    );
    registry
}

struct WideNodeKind;

impl CanvasNodeGeometryPolicy for WideNodeKind {
    fn node_bounds(&self, node: &CanvasNode) -> Option<Bounds<Pixels>> {
        Some(node.bounds().dilate(px(30.0)))
    }
}

struct PaddedShapeKind;

impl CanvasShapeGeometryPolicy for PaddedShapeKind {
    fn shape_bounds(&self, shape: &CanvasShape) -> Option<Bounds<Pixels>> {
        Some(shape.bounds.dilate(px(18.0)))
    }
}

struct VerticalDetourRouter;

impl CanvasEdgeRouter for VerticalDetourRouter {
    fn route_edge(&self, request: CanvasRouteRequest<'_>) -> CanvasRoutePath {
        CanvasRoutePath::polyline([
            request.source,
            point(request.source.x, px(220.0)),
            point(request.target.x, px(220.0)),
            request.target,
        ])
    }
}

fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}
