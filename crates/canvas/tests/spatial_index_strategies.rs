use open_gpui::{Bounds, Pixels, Point, point, px, size};
use open_gpui_canvas::{
    CanvasDocument, CanvasEdge, CanvasEdgeRouter, CanvasEndpoint, CanvasHandle, CanvasKindRegistry,
    CanvasNode, CanvasNodeKind, CanvasRoutePath, CanvasRouteRequest, CanvasShape, HitOptions,
    HitRecord, HitTarget, SpatialIndex,
};
use rstar::{AABB as RStarAabb, RTree, RTreeObject};
use static_aabb2d_index::{StaticAABB2DIndex, StaticAABB2DIndexBuilder};

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

struct MaterializedFixture {
    index: SpatialIndex,
    candidates: CandidateIndexes,
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
        Self {
            tree: RTree::bulk_load(indexed_records(records)),
        }
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
            .tree
            .locate_in_envelope_intersecting(rstar_envelope(viewport))
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
            .map(|record| record.record.target.clone())
            .collect()
    }

    fn hit_test_targets(&self, point: Point<Pixels>, options: HitOptions) -> Vec<HitTarget> {
        let viewport = point_query_bounds(point, options.margin);
        let mut records = self
            .tree
            .locate_in_envelope_intersecting(rstar_envelope(viewport))
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
            .map(|record| record.record.target.clone())
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

fn indexed_records(records: &[HitRecord]) -> Vec<IndexedRecord> {
    records
        .iter()
        .cloned()
        .enumerate()
        .map(|(ordinal, record)| IndexedRecord { ordinal, record })
        .collect()
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

    for row in 0..rows {
        for column in 0..columns {
            let id = format!("node-{row}-{column}");
            let mut node = CanvasNode::new(
                id.clone(),
                point(px(column as f32 * 130.0), px(row as f32 * 90.0)),
                size(px(96.0), px(56.0)),
            );
            node.z_index = (row * columns + column) as i32;
            document.insert_node(node).unwrap();

            if column > 0 {
                document
                    .insert_edge(CanvasEdge::new(
                        format!("edge-{row}-{}-{row}-{column}", column - 1),
                        CanvasEndpoint::new(format!("node-{row}-{}", column - 1), None::<String>),
                        CanvasEndpoint::new(id, None::<String>),
                    ))
                    .unwrap();
            }
        }
    }

    document
}

fn dense_overlap_document() -> CanvasDocument {
    let mut document = CanvasDocument::default();

    for index in 0..24 {
        let mut node = CanvasNode::new(
            format!("overlap-{index}"),
            point(px(40.0 + index as f32 * 2.0), px(40.0 + index as f32 * 2.0)),
            size(px(80.0), px(80.0)),
        );
        node.z_index = index;
        document.insert_node(node).unwrap();
    }

    let mut shape = CanvasShape::new("shape-top", bounds(52.0, 52.0, 70.0, 70.0));
    shape.z_index = 40;
    document.insert_shape(shape).unwrap();
    document
}

fn clustered_document() -> CanvasDocument {
    let mut document = CanvasDocument::default();

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
            document.insert_node(node).unwrap();
        }
    }

    document
}

fn long_edge_document() -> CanvasDocument {
    let mut document = CanvasDocument::default();
    for index in 0..8 {
        let node_id = format!("long-{index}");
        document
            .insert_node(CanvasNode::new(
                node_id.clone(),
                point(px(index as f32 * 420.0), px((index % 2) as f32 * 260.0)),
                size(px(80.0), px(52.0)),
            ))
            .unwrap();

        if index > 0 {
            let mut edge = CanvasEdge::new(
                format!("long-edge-{}-{index}", index - 1),
                CanvasEndpoint::new(format!("long-{}", index - 1), None::<String>),
                CanvasEndpoint::new(node_id, None::<String>),
            );
            edge.z_index = 100 + index as i32;
            document.insert_edge(edge).unwrap();
        }
    }
    document
}

fn mixed_document() -> CanvasDocument {
    let mut document = grid_document(4, 3);

    let mut wide = CanvasNode::new("wide", point(px(420.0), px(80.0)), size(px(40.0), px(40.0)));
    wide.kind = "wide".to_string();
    wide.z_index = 200;
    wide.handles
        .push(CanvasHandle::new("out", point(px(40.0), px(20.0))));
    document.insert_node(wide).unwrap();

    let mut shape = CanvasShape::new("shape-wide", bounds(500.0, 110.0, 48.0, 48.0));
    shape.kind = "padded".to_string();
    shape.z_index = 210;
    document.insert_shape(shape).unwrap();

    document
}

fn visibility_document() -> CanvasDocument {
    let mut document = CanvasDocument::default();

    let mut node = CanvasNode::new(
        "handles",
        point(px(0.0), px(0.0)),
        size(px(100.0), px(100.0)),
    );
    node.handles
        .push(CanvasHandle::new("out", point(px(96.0), px(50.0))));
    document.insert_node(node).unwrap();

    let mut hidden = CanvasNode::new(
        "hidden",
        point(px(20.0), px(20.0)),
        size(px(40.0), px(40.0)),
    );
    hidden.hidden = true;
    hidden.z_index = 10;
    document.insert_node(hidden).unwrap();

    let mut locked = CanvasNode::new(
        "locked",
        point(px(60.0), px(20.0)),
        size(px(40.0), px(40.0)),
    );
    locked.locked = true;
    locked.z_index = 20;
    document.insert_node(locked).unwrap();

    document
}

fn geometry_registry() -> CanvasKindRegistry {
    let mut registry = CanvasKindRegistry::open();
    registry.register_node_kind("wide", WideNodeKind);
    registry.register_shape_kind("padded", PaddedShapeKind);
    registry
}

struct WideNodeKind;

impl CanvasNodeKind for WideNodeKind {
    fn node_bounds(&self, node: &CanvasNode) -> Option<Bounds<Pixels>> {
        Some(node.bounds().dilate(px(30.0)))
    }
}

struct PaddedShapeKind;

impl open_gpui_canvas::CanvasShapeKind for PaddedShapeKind {
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
