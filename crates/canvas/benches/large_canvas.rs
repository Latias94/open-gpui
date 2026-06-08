use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use open_gpui::{Bounds, point, px, size};
use open_gpui_canvas::{
    CanvasDocument, CanvasEdge, CanvasEndpoint, CanvasNode, CanvasPaintModel, CanvasPaintOptions,
    CanvasViewport, SpatialIndex, collect_visible_records,
};
use std::sync::Arc;

const GRID_COLUMNS: usize = 200;
const GRID_ROWS: usize = 100;
const NODE_WIDTH: f32 = 96.0;
const NODE_HEIGHT: f32 = 56.0;
const COLUMN_GAP: f32 = 160.0;
const ROW_GAP: f32 = 120.0;

fn large_canvas_benches(c: &mut Criterion) {
    let document = build_grid_document(GRID_COLUMNS, GRID_ROWS);
    let index = SpatialIndex::rebuild(&document);
    let node_count = document.nodes.len();
    let edge_count = document.edges.len();

    c.bench_with_input(
        BenchmarkId::new("spatial_index_rebuild", node_count + edge_count),
        &document,
        |b, document| {
            b.iter(|| SpatialIndex::rebuild(black_box(document)));
        },
    );

    c.bench_function("spatial_index_visible_query", |b| {
        let viewport = Bounds::new(
            point(px(12_000.0), px(6_000.0)),
            size(px(1_280.0), px(720.0)),
        );

        b.iter(|| black_box(index.query(black_box(viewport)).count()));
    });

    c.bench_function("paint_frame_culling", |b| {
        let model = CanvasPaintModel::from_parts(
            Arc::new(document.clone()),
            Arc::new(index.clone()),
            CanvasViewport::new(point(px(12_000.0), px(6_000.0)), 1.0)
                .expect("benchmark viewport should be valid"),
        );
        let canvas_bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(1_280.0), px(720.0)));
        let options = CanvasPaintOptions {
            cull_margin: px(128.0),
            ..CanvasPaintOptions::default()
        };

        b.iter(|| collect_visible_records(black_box(&model), black_box(canvas_bounds), options));
    });
}

fn build_grid_document(columns: usize, rows: usize) -> CanvasDocument {
    let mut document = CanvasDocument::default();

    for row in 0..rows {
        for column in 0..columns {
            let id = node_id(row, column);
            let mut node = CanvasNode::new(
                id.clone(),
                point(px(column as f32 * COLUMN_GAP), px(row as f32 * ROW_GAP)),
                size(px(NODE_WIDTH), px(NODE_HEIGHT)),
            );
            node.z_index = (row * columns + column) as i32;
            document
                .insert_node(node)
                .expect("benchmark grid node ids should be unique");

            if column > 0 {
                document
                    .insert_edge(CanvasEdge::new(
                        edge_id(row, column - 1, row, column),
                        CanvasEndpoint::new(node_id(row, column - 1), None::<String>),
                        CanvasEndpoint::new(id, None::<String>),
                    ))
                    .expect("benchmark horizontal edge endpoints should exist");
            }
        }
    }

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

criterion_group!(benches, large_canvas_benches);
criterion_main!(benches);
