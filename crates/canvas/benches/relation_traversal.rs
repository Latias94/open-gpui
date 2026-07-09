use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use open_gpui::{Bounds, point, px, size};
use open_gpui_canvas::advanced::{CanvasRecordScopeOptions, selection_record_scope};
use open_gpui_canvas::{
    CanvasDocument, CanvasEdge, CanvasEndpoint, CanvasNode, CanvasRecordId, CanvasSelection,
    CanvasShape, CanvasTransaction, DocumentCommand, NodeId, ShapeId,
};

const SHALLOW_WIDE_RECORDS: usize = 1_000;
const DEEP_CHAIN_RECORDS: usize = 256;
const MIXED_GROUPS: usize = 32;
const MIXED_MEMBERS_PER_GROUP: usize = 32;

fn relation_traversal_benches(c: &mut Criterion) {
    bench_scope(
        c,
        "parent/shallow_wide",
        &parent_wide_document(SHALLOW_WIDE_RECORDS),
    );
    bench_scope(
        c,
        "parent/deep_chain",
        &parent_chain_document(DEEP_CHAIN_RECORDS),
    );
    bench_scope(
        c,
        "group/shallow_wide",
        &group_wide_document(SHALLOW_WIDE_RECORDS),
    );
    bench_scope(
        c,
        "mixed/nested_parent_group",
        &mixed_parent_group_document(MIXED_GROUPS, MIXED_MEMBERS_PER_GROUP),
    );
}

fn bench_scope(c: &mut Criterion, name: &str, workload: &RelationWorkload) {
    c.bench_with_input(
        BenchmarkId::new(name, workload.record_count()),
        workload,
        |b, workload| {
            b.iter(|| {
                black_box(selection_record_scope(
                    black_box(&workload.document),
                    black_box(&workload.selection),
                    CanvasRecordScopeOptions::structural_with_internal_edges(),
                ))
            });
        },
    );
}

struct RelationWorkload {
    document: CanvasDocument,
    selection: CanvasSelection,
}

impl RelationWorkload {
    fn record_count(&self) -> usize {
        self.document.node_count() + self.document.edge_count() + self.document.shape_count()
    }
}

fn parent_wide_document(child_count: usize) -> RelationWorkload {
    let frame = ShapeId::from("frame");
    let mut commands = vec![DocumentCommand::InsertShape(CanvasShape::new(
        frame.as_str(),
        Bounds::new(point(px(0.0), px(0.0)), size(px(4_000.0), px(4_000.0))),
    ))];

    for index in 0..child_count {
        let child = NodeId::from(format!("child-{index}"));
        commands.push(DocumentCommand::InsertNode(CanvasNode::new(
            child.as_str(),
            grid_point(index),
            size(px(72.0), px(40.0)),
        )));
        commands.push(DocumentCommand::SetRecordParent {
            child: CanvasRecordId::Node(child),
            parent: CanvasRecordId::Shape(frame.clone()),
        });
    }

    workload(commands, selected_shape(frame))
}

fn parent_chain_document(depth: usize) -> RelationWorkload {
    let root = ShapeId::from("frame-0");
    let mut commands = Vec::new();

    for index in 0..depth {
        let shape = ShapeId::from(format!("frame-{index}"));
        commands.push(DocumentCommand::InsertShape(CanvasShape::new(
            shape.as_str(),
            Bounds::new(
                point(px(index as f32), px(index as f32)),
                size(px(4_000.0 - index as f32), px(4_000.0 - index as f32)),
            ),
        )));
        if index > 0 {
            commands.push(DocumentCommand::SetRecordParent {
                child: CanvasRecordId::Shape(shape),
                parent: CanvasRecordId::Shape(ShapeId::from(format!("frame-{}", index - 1))),
            });
        }
    }

    let leaf = NodeId::from("leaf");
    commands.push(DocumentCommand::InsertNode(CanvasNode::new(
        leaf.as_str(),
        point(px(64.0), px(64.0)),
        size(px(72.0), px(40.0)),
    )));
    commands.push(DocumentCommand::SetRecordParent {
        child: CanvasRecordId::Node(leaf),
        parent: CanvasRecordId::Shape(ShapeId::from(format!("frame-{}", depth - 1))),
    });

    workload(commands, selected_shape(root))
}

fn group_wide_document(member_count: usize) -> RelationWorkload {
    let group = ShapeId::from("group");
    let mut commands = vec![DocumentCommand::InsertShape(CanvasShape::new(
        group.as_str(),
        Bounds::new(point(px(0.0), px(0.0)), size(px(4_000.0), px(4_000.0))),
    ))];

    for index in 0..member_count {
        let member = NodeId::from(format!("member-{index}"));
        commands.push(DocumentCommand::InsertNode(CanvasNode::new(
            member.as_str(),
            grid_point(index),
            size(px(72.0), px(40.0)),
        )));
        commands.push(DocumentCommand::AddRecordToGroup {
            group: CanvasRecordId::Shape(group.clone()),
            member: CanvasRecordId::Node(member),
        });
    }

    workload(commands, selected_shape(group))
}

fn mixed_parent_group_document(group_count: usize, members_per_group: usize) -> RelationWorkload {
    let root = ShapeId::from("root");
    let mut commands = vec![DocumentCommand::InsertShape(CanvasShape::new(
        root.as_str(),
        Bounds::new(point(px(0.0), px(0.0)), size(px(8_000.0), px(8_000.0))),
    ))];

    for group_index in 0..group_count {
        let group = ShapeId::from(format!("group-{group_index}"));
        commands.push(DocumentCommand::InsertShape(CanvasShape::new(
            group.as_str(),
            Bounds::new(
                point(px((group_index * 180) as f32), px(0.0)),
                size(px(160.0), px(4_000.0)),
            ),
        )));
        commands.push(DocumentCommand::SetRecordParent {
            child: CanvasRecordId::Shape(group.clone()),
            parent: CanvasRecordId::Shape(root.clone()),
        });

        for member_index in 0..members_per_group {
            let member = NodeId::from(format!("member-{group_index}-{member_index}"));
            commands.push(DocumentCommand::InsertNode(CanvasNode::new(
                member.as_str(),
                point(
                    px((group_index * 180) as f32),
                    px((member_index * 64) as f32),
                ),
                size(px(72.0), px(40.0)),
            )));
            commands.push(DocumentCommand::SetRecordParent {
                child: CanvasRecordId::Node(member.clone()),
                parent: CanvasRecordId::Shape(group.clone()),
            });
            commands.push(DocumentCommand::AddRecordToGroup {
                group: CanvasRecordId::Shape(group.clone()),
                member: CanvasRecordId::Node(member),
            });
        }
    }

    for group_index in 0..group_count.saturating_sub(1) {
        commands.push(DocumentCommand::InsertEdge(CanvasEdge::new(
            format!("edge-{group_index}-{}", group_index + 1),
            CanvasEndpoint::new(format!("member-{group_index}-0"), None::<String>),
            CanvasEndpoint::new(format!("member-{}-0", group_index + 1), None::<String>),
        )));
    }

    workload(commands, selected_shape(root))
}

fn workload(commands: Vec<DocumentCommand>, selection: CanvasSelection) -> RelationWorkload {
    let mut document = CanvasDocument::default();
    document
        .apply_transaction(CanvasTransaction::new(commands))
        .expect("benchmark relation fixture should be valid");
    RelationWorkload {
        document,
        selection,
    }
}

fn selected_shape(id: ShapeId) -> CanvasSelection {
    let mut selection = CanvasSelection::default();
    selection.insert_shape(id);
    selection
}

fn grid_point(index: usize) -> open_gpui::Point<open_gpui::Pixels> {
    let column = index % 50;
    let row = index / 50;
    point(px((column * 96) as f32), px((row * 64) as f32))
}

criterion_group!(benches, relation_traversal_benches);
criterion_main!(benches);
