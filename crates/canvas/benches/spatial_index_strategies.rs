use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use open_gpui::{Bounds, Pixels, Point, point, px, size};
use open_gpui_canvas::{
    CanvasDocument, CanvasEdge, CanvasEndpoint, CanvasHandle, CanvasNode, CanvasPaintModel,
    CanvasPaintOptions, CanvasRecordId, CanvasRuntime, CanvasShape, CanvasTransaction,
    CanvasViewport, DocumentCommand, HitOptions, HitRecord, HitTarget, NodeId, SpatialIndex,
    collect_visible_records,
};
use rstar::{AABB as RStarAabb, RTree, RTreeObject};
use static_aabb2d_index::{StaticAABB2DIndex, StaticAABB2DIndexBuilder};
use std::{collections::HashSet, time::Duration};

const GRID_COLUMNS: usize = 120;
const GRID_ROWS: usize = 80;
const DRAG_GRID_COLUMNS: usize = 40;
const DRAG_GRID_ROWS: usize = 25;
const DRAG_FRAMES: usize = 120;

fn spatial_index_strategy_benches(c: &mut Criterion) {
    let workloads = [
        Workload::new("grid", grid_document(GRID_COLUMNS, GRID_ROWS)),
        Workload::new("dense_overlap", dense_overlap_document(2_500)),
        Workload::new("clustered", clustered_document(80, 64)),
        Workload::new("long_edges", long_edge_document(2_000)),
        Workload::new("mixed", mixed_document(1_200)),
    ];

    for workload in workloads {
        bench_workload(c, &workload);
    }

    let drag_workload = Workload::new(
        "drag_grid",
        grid_document(DRAG_GRID_COLUMNS, DRAG_GRID_ROWS),
    );
    bench_drag_workload(c, &drag_workload);
}

fn bench_workload(c: &mut Criterion, workload: &Workload) {
    let mut group = c.benchmark_group(format!("spatial_index/{}", workload.name));
    let viewport = workload.viewport;
    let hit_point = workload.hit_point;
    let options = HitOptions {
        include_locked: true,
        ..HitOptions::default()
    };

    group.bench_with_input(
        BenchmarkId::new("rebuild/vector", workload.records.len()),
        &workload.document,
        |b, document| {
            b.iter(|| SpatialIndex::rebuild(black_box(document)));
        },
    );

    group.bench_with_input(
        BenchmarkId::new("rebuild/rstar", workload.records.len()),
        &workload.records,
        |b, records| {
            b.iter(|| RStarCandidate::new(black_box(records)));
        },
    );

    group.bench_with_input(
        BenchmarkId::new("rebuild/static_aabb", workload.records.len()),
        &workload.records,
        |b, records| {
            b.iter(|| StaticAabbCandidate::new(black_box(records.clone())));
        },
    );

    group.bench_function("query/vector", |b| {
        b.iter(|| {
            black_box(
                workload
                    .oracle
                    .query_with_options(black_box(viewport), black_box(options))
                    .count(),
            )
        });
    });

    group.bench_function("query/runtime", |b| {
        b.iter(|| {
            black_box(
                workload
                    .runtime
                    .query_with_options(black_box(viewport), black_box(options))
                    .count(),
            )
        });
    });

    group.bench_function("query/rstar", |b| {
        b.iter(|| {
            black_box(
                workload
                    .rstar
                    .query_count(black_box(viewport), black_box(options)),
            )
        });
    });

    group.bench_function("query/static_aabb", |b| {
        b.iter(|| {
            black_box(
                workload
                    .static_aabb
                    .query_count(black_box(viewport), black_box(options)),
            )
        });
    });

    group.bench_function("hit_test/vector", |b| {
        b.iter(|| {
            black_box(
                workload
                    .oracle
                    .hit_test(black_box(hit_point), black_box(options))
                    .count(),
            )
        });
    });

    group.bench_function("hit_test/runtime", |b| {
        b.iter(|| {
            black_box(
                workload
                    .runtime
                    .hit_test(black_box(hit_point), black_box(options))
                    .count(),
            )
        });
    });

    group.bench_function("hit_test/rstar", |b| {
        b.iter(|| {
            black_box(
                workload
                    .rstar
                    .hit_test_count(black_box(hit_point), black_box(options)),
            )
        });
    });

    group.bench_function("hit_test/static_aabb", |b| {
        b.iter(|| {
            black_box(
                workload
                    .static_aabb
                    .hit_test_count(black_box(hit_point), black_box(options)),
            )
        });
    });

    let paint_model = CanvasPaintModel::new(
        workload.document.clone(),
        CanvasViewport::new(viewport.origin, 1.0).expect("benchmark viewport should be valid"),
    );
    let canvas_bounds = Bounds::new(point(px(0.0), px(0.0)), viewport.size);
    let paint_options = CanvasPaintOptions {
        cull_margin: px(128.0),
        ..CanvasPaintOptions::default()
    };
    group.bench_function("paint_frame_culling/current", |b| {
        b.iter(|| {
            black_box(collect_visible_records(
                black_box(&paint_model),
                black_box(canvas_bounds),
                black_box(paint_options),
            ))
        });
    });

    group.finish();
}

fn bench_drag_workload(c: &mut Criterion, workload: &Workload) {
    let mut group = c.benchmark_group(format!("spatial_index/{}", workload.name));
    let viewport = workload.viewport;

    for selected_nodes in [1, 10, 100] {
        group.bench_function(format!("drag_rebuild/vector/{selected_nodes}"), |b| {
            b.iter(|| {
                black_box(simulate_drag_rebuild(
                    black_box(&workload.document),
                    selected_nodes,
                ))
            });
        });
        group.bench_function(format!("drag_rebuild/rstar/{selected_nodes}"), |b| {
            b.iter(|| {
                black_box(simulate_drag_candidate(
                    black_box(&workload.document),
                    selected_nodes,
                    CandidateKind::RStar,
                ))
            });
        });
        group.bench_function(format!("drag_rebuild/static_aabb/{selected_nodes}"), |b| {
            b.iter(|| {
                black_box(simulate_drag_candidate(
                    black_box(&workload.document),
                    selected_nodes,
                    CandidateKind::StaticAabb,
                ))
            });
        });
        group.bench_function(format!("drag_overlay/hybrid/{selected_nodes}"), |b| {
            b.iter(|| {
                black_box(simulate_drag_hybrid_overlay(
                    black_box(&workload.document),
                    black_box(&workload.records),
                    selected_nodes,
                    viewport,
                ))
            });
        });
        group.bench_function(format!("drag_update/runtime/{selected_nodes}"), |b| {
            b.iter(|| {
                black_box(simulate_drag_runtime(
                    black_box(&workload.document),
                    selected_nodes,
                    viewport,
                ))
            });
        });
    }

    group.finish();
}

struct Workload {
    name: &'static str,
    document: CanvasDocument,
    viewport: Bounds<Pixels>,
    hit_point: Point<Pixels>,
    oracle: SpatialIndex,
    runtime: CanvasRuntime,
    records: Vec<HitRecord>,
    rstar: RStarCandidate,
    static_aabb: StaticAabbCandidate,
}

impl Workload {
    fn new(name: &'static str, document: CanvasDocument) -> Self {
        let oracle = SpatialIndex::rebuild(&document);
        let records = oracle.records().to_vec();
        let runtime = CanvasRuntime::rebuild(&document);
        let viewport = Bounds::new(
            point(px(2_400.0), px(1_400.0)),
            size(px(1_280.0), px(720.0)),
        );
        let hit_point = point(px(2_560.0), px(1_520.0));

        Self {
            name,
            document,
            viewport,
            hit_point,
            runtime,
            oracle,
            rstar: RStarCandidate::new(&records),
            static_aabb: StaticAabbCandidate::new(records.clone()),
            records,
        }
    }
}

#[derive(Clone)]
struct IndexedRecord {
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

    fn query_count(&self, viewport: Bounds<Pixels>, options: HitOptions) -> usize {
        self.query_records(viewport)
            .into_iter()
            .filter(|record| query_matches(&record.record, viewport, options))
            .count()
    }

    fn hit_test_count(&self, point: Point<Pixels>, options: HitOptions) -> usize {
        let viewport = point_query_bounds(point, options.margin);
        self.query_records(viewport)
            .into_iter()
            .filter(|record| hit_matches(&record.record, point, options))
            .count()
    }
}

impl RTreeObject for IndexedRecord {
    type Envelope = RStarAabb<[f32; 2]>;

    fn envelope(&self) -> Self::Envelope {
        rstar_envelope(self.record.bounds)
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

    fn query_count(&self, viewport: Bounds<Pixels>, options: HitOptions) -> usize {
        self.query_records(viewport)
            .into_iter()
            .filter(|record| query_matches(&record.record, viewport, options))
            .count()
    }

    fn hit_test_count(&self, point: Point<Pixels>, options: HitOptions) -> usize {
        let viewport = point_query_bounds(point, options.margin);
        self.query_records(viewport)
            .into_iter()
            .filter(|record| hit_matches(&record.record, point, options))
            .count()
    }

    fn query_records(&self, viewport: Bounds<Pixels>) -> Vec<IndexedRecord> {
        let [min_x, min_y, max_x, max_y] = aabb_extents(viewport);
        self.index
            .query_iter(min_x, min_y, max_x, max_y)
            .map(|ordinal| IndexedRecord {
                record: self.records[ordinal].clone(),
            })
            .collect()
    }
}

struct HybridOverlayCandidate<'a> {
    base: &'a StaticAabbCandidate,
    overlay: RStarCandidate,
    stale_records: HashSet<CanvasRecordId>,
}

impl<'a> HybridOverlayCandidate<'a> {
    fn new(
        base: &'a StaticAabbCandidate,
        overlay_records: Vec<IndexedRecord>,
        stale_records: HashSet<CanvasRecordId>,
    ) -> Self {
        Self {
            base,
            stale_records,
            overlay: RStarCandidate::new_indexed(overlay_records),
        }
    }

    fn query_count(&self, viewport: Bounds<Pixels>, options: HitOptions) -> usize {
        self.query_records(viewport)
            .into_iter()
            .filter(|record| query_matches(&record.record, viewport, options))
            .count()
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

enum CandidateKind {
    RStar,
    StaticAabb,
}

fn simulate_drag_rebuild(document: &CanvasDocument, selected_nodes: usize) -> usize {
    let mut document = document.clone();
    let ids = document
        .node_ids()
        .take(selected_nodes)
        .cloned()
        .collect::<Vec<_>>();
    let mut count = 0;

    for frame in 0..DRAG_FRAMES {
        move_selected_nodes(&mut document, &ids, frame);

        count += SpatialIndex::rebuild(&document).records().len();
    }

    count
}

fn simulate_drag_candidate(
    document: &CanvasDocument,
    selected_nodes: usize,
    kind: CandidateKind,
) -> usize {
    let mut document = document.clone();
    let ids = document
        .node_ids()
        .take(selected_nodes)
        .cloned()
        .collect::<Vec<_>>();
    let mut count = 0;

    for frame in 0..DRAG_FRAMES {
        move_selected_nodes(&mut document, &ids, frame);

        let records = SpatialIndex::rebuild(&document).records().to_vec();
        count += match kind {
            CandidateKind::RStar => RStarCandidate::new(&records).query_count(
                Bounds::new(
                    point(px(2_400.0), px(1_400.0)),
                    size(px(1_280.0), px(720.0)),
                ),
                HitOptions::default(),
            ),
            CandidateKind::StaticAabb => StaticAabbCandidate::new(records).query_count(
                Bounds::new(
                    point(px(2_400.0), px(1_400.0)),
                    size(px(1_280.0), px(720.0)),
                ),
                HitOptions::default(),
            ),
        };
    }

    count
}

fn simulate_drag_hybrid_overlay(
    document: &CanvasDocument,
    base_records: &[HitRecord],
    selected_nodes: usize,
    viewport: Bounds<Pixels>,
) -> usize {
    let selected = selected_node_ids(document, selected_nodes);
    let base = StaticAabbCandidate::new(base_records.to_vec());
    let stale_records = stale_record_ids_for_nodes(document, &selected);
    let mut document = document.clone();
    let mut count = 0;

    for frame in 0..DRAG_FRAMES {
        move_selected_nodes(&mut document, &selected, frame);

        let oracle = SpatialIndex::rebuild(&document);
        let overlay_records = overlay_records_for_stale_ids(&oracle, &stale_records);
        let hybrid = HybridOverlayCandidate::new(&base, overlay_records, stale_records.clone());
        count += hybrid.query_count(viewport, HitOptions::default());
    }

    count
}

fn simulate_drag_runtime(
    document: &CanvasDocument,
    selected_nodes: usize,
    viewport: Bounds<Pixels>,
) -> usize {
    let selected = selected_node_ids(document, selected_nodes);
    let mut document = document.clone();
    let mut runtime = CanvasRuntime::rebuild(&document);
    let mut count = 0;

    for frame in 0..DRAG_FRAMES {
        let previous = document.clone();
        move_selected_nodes(&mut document, &selected, frame);

        let diff = document.diff_against(&previous);
        runtime.apply_diff(&document, &diff);
        count += runtime.query(viewport).count();
    }

    count
}

fn indexed_records(records: &[HitRecord]) -> Vec<IndexedRecord> {
    records
        .iter()
        .cloned()
        .map(|record| IndexedRecord { record })
        .collect()
}

fn overlay_records_for_stale_ids(
    index: &SpatialIndex,
    stale_records: &HashSet<CanvasRecordId>,
) -> Vec<IndexedRecord> {
    index
        .records()
        .iter()
        .cloned()
        .enumerate()
        .filter(|(_, record)| stale_records.contains(&record_id_for_target(&record.target)))
        .map(|(_, record)| IndexedRecord { record })
        .collect()
}

fn stale_record_ids_for_nodes(
    document: &CanvasDocument,
    selected_nodes: &HashSet<NodeId>,
) -> HashSet<CanvasRecordId> {
    let mut stale_records = HashSet::new();

    for node_id in selected_nodes {
        stale_records.insert(CanvasRecordId::Node(node_id.clone()));
    }

    for edge in document.edges() {
        if selected_nodes.contains(&edge.source.node_id)
            || selected_nodes.contains(&edge.target.node_id)
        {
            stale_records.insert(CanvasRecordId::Edge(edge.id.clone()));
        }
    }

    stale_records
}

fn record_id_for_target(target: &HitTarget) -> CanvasRecordId {
    match target {
        HitTarget::Node(id) => CanvasRecordId::Node(id.clone()),
        HitTarget::Handle { node_id, .. } => CanvasRecordId::Node(node_id.clone()),
        HitTarget::Shape(id) => CanvasRecordId::Shape(id.clone()),
        HitTarget::Edge(id) => CanvasRecordId::Edge(id.clone()),
    }
}

fn selected_node_ids(document: &CanvasDocument, selected_nodes: usize) -> HashSet<NodeId> {
    document.node_ids().take(selected_nodes).cloned().collect()
}

fn move_selected_nodes<'a>(
    document: &mut CanvasDocument,
    selected: impl IntoIterator<Item = &'a NodeId>,
    frame: usize,
) {
    let commands = selected
        .into_iter()
        .map(|id| {
            let mut node = document.node(id).unwrap().clone();
            node.position.x += px(1.0 + frame as f32 * 0.01);
            DocumentCommand::UpdateNode(node)
        })
        .collect::<Vec<_>>();
    apply_commands(document, commands);
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

fn apply_commands(
    document: &mut CanvasDocument,
    commands: impl IntoIterator<Item = DocumentCommand>,
) {
    document
        .apply_transaction(CanvasTransaction::new(commands))
        .expect("benchmark document commands should be valid");
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

fn grid_document(columns: usize, rows: usize) -> CanvasDocument {
    let mut document = CanvasDocument::default();
    let mut commands = Vec::new();

    for row in 0..rows {
        for column in 0..columns {
            let id = node_id(row, column);
            let mut node = CanvasNode::new(
                id.clone(),
                point(px(column as f32 * 160.0), px(row as f32 * 120.0)),
                size(px(96.0), px(56.0)),
            );
            node.z_index = (row * columns + column) as i32;
            commands.push(DocumentCommand::InsertNode(node));

            if column > 0 {
                commands.push(DocumentCommand::InsertEdge(CanvasEdge::new(
                    edge_id(row, column - 1, row, column),
                    CanvasEndpoint::new(node_id(row, column - 1), None::<String>),
                    CanvasEndpoint::new(id, None::<String>),
                )));
            }
        }
    }

    apply_commands(&mut document, commands);
    document
}

fn dense_overlap_document(count: usize) -> CanvasDocument {
    let mut document = CanvasDocument::default();
    let mut commands = Vec::new();

    for index in 0..count {
        let mut node = CanvasNode::new(
            format!("overlap-{index}"),
            point(
                px(2_400.0 + (index % 48) as f32 * 2.0),
                px(1_400.0 + (index % 48) as f32 * 2.0),
            ),
            size(px(96.0), px(96.0)),
        );
        node.z_index = index as i32;
        commands.push(DocumentCommand::InsertNode(node));
    }

    apply_commands(&mut document, commands);
    document
}

fn clustered_document(clusters: usize, nodes_per_cluster: usize) -> CanvasDocument {
    let mut document = CanvasDocument::default();
    let mut commands = Vec::new();

    for cluster in 0..clusters {
        let base_x = (cluster % 10) as f32 * 700.0;
        let base_y = (cluster / 10) as f32 * 520.0;
        for index in 0..nodes_per_cluster {
            let mut node = CanvasNode::new(
                format!("cluster-{cluster}-{index}"),
                point(
                    px(base_x + (index % 8) as f32 * 84.0),
                    px(base_y + (index / 8) as f32 * 72.0),
                ),
                size(px(64.0), px(44.0)),
            );
            node.z_index = (cluster * nodes_per_cluster + index) as i32;
            commands.push(DocumentCommand::InsertNode(node));
        }
    }

    apply_commands(&mut document, commands);
    document
}

fn long_edge_document(count: usize) -> CanvasDocument {
    let mut document = CanvasDocument::default();
    let mut commands = Vec::new();

    for index in 0..count {
        let id = format!("long-{index}");
        commands.push(DocumentCommand::InsertNode(CanvasNode::new(
            id.clone(),
            point(px(index as f32 * 24.0), px((index % 20) as f32 * 140.0)),
            size(px(72.0), px(44.0)),
        )));

        if index > 0 {
            commands.push(DocumentCommand::InsertEdge(CanvasEdge::new(
                format!("long-edge-{}-{index}", index - 1),
                CanvasEndpoint::new(format!("long-{}", index - 1), None::<String>),
                CanvasEndpoint::new(id, None::<String>),
            )));
        }
    }

    apply_commands(&mut document, commands);
    document
}

fn mixed_document(count: usize) -> CanvasDocument {
    let mut document = CanvasDocument::default();
    let mut commands = Vec::new();

    for index in 0..count {
        let mut node = CanvasNode::new(
            format!("mixed-{index}"),
            point(
                px((index % 80) as f32 * 92.0),
                px((index / 80) as f32 * 84.0),
            ),
            size(px(68.0), px(42.0)),
        );
        node.z_index = index as i32;
        if index % 11 == 0 {
            node.locked = true;
        }
        if index % 17 == 0 {
            node.hidden = true;
        }
        if index % 5 == 0 {
            node.handles
                .push(CanvasHandle::new("out", point(px(68.0), px(21.0))));
        }
        commands.push(DocumentCommand::InsertNode(node));

        if index % 7 == 0 {
            let mut shape = CanvasShape::new(
                format!("mixed-shape-{index}"),
                Bounds::new(
                    point(
                        px((index % 80) as f32 * 92.0 + 24.0),
                        px((index / 80) as f32 * 84.0),
                    ),
                    size(px(44.0), px(44.0)),
                ),
            );
            shape.z_index = index as i32 + 10_000;
            commands.push(DocumentCommand::InsertShape(shape));
        }
    }

    apply_commands(&mut document, commands);
    document
}

fn node_id(row: usize, column: usize) -> String {
    format!("node-{row}-{column}")
}

fn edge_id(
    source_row: usize,
    source_column: usize,
    target_row: usize,
    target_column: usize,
) -> String {
    format!("edge-{source_row}-{source_column}-{target_row}-{target_column}")
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_millis(250))
        .measurement_time(Duration::from_secs(2));
    targets = spatial_index_strategy_benches
}
criterion_main!(benches);
