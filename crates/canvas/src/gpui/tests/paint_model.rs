use super::*;

#[test]
fn collect_visible_records_culls_and_transforms_bounds() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "inside",
            point(px(60.0), px(10.0)),
            size(px(20.0), px(10.0)),
        ))
        .node(CanvasNode::new(
            "outside",
            point(px(200.0), px(10.0)),
            size(px(20.0), px(10.0)),
        ))
        .build();
    let model = CanvasPaintModel::new(
        document,
        CanvasViewport::new(point(px(50.0), px(0.0)), 2.0).unwrap(),
    );

    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(100.0), px(100.0)), size(px(100.0), px(100.0))),
        CanvasPaintOptions::default(),
    );

    assert_eq!(frame.records.len(), 1);
    assert_eq!(
        frame.records[0].target,
        HitTarget::Node(crate::NodeId::from("inside"))
    );
    assert_eq!(
        frame.records[0].view_bounds,
        Bounds::new(point(px(20.0), px(20.0)), size(px(40.0), px(20.0)))
    );
}

#[test]
fn collect_visible_records_keeps_large_canvas_frame_bounded() {
    let document = large_grid_document(128, 96);
    let total_records = document.node_count();
    let model = CanvasPaintModel::new(
        document,
        CanvasViewport::new(point(px(2_400.0), px(1_800.0)), 1.0).unwrap(),
    );

    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(800.0), px(600.0))),
        CanvasPaintOptions::default(),
    );

    assert_eq!(total_records, 12_288);
    assert!(!frame.records.is_empty());
    assert!(frame.records.len() < 80);
    assert!(frame.records.iter().all(|record| {
        frame
            .visible_document_bounds
            .intersects(&record.document_bounds)
    }));
}

#[test]
fn collect_visible_records_keeps_locked_records_visible() {
    let mut node = CanvasNode::new("locked", point(px(0.0), px(0.0)), size(px(20.0), px(20.0)));
    node.locked = true;
    let document = document_fixture().node(node).build();
    let model = CanvasPaintModel::new(document, CanvasViewport::default());

    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        CanvasPaintOptions::default(),
    );

    assert_eq!(frame.records.len(), 1);
    assert_eq!(
        frame.records[0].target,
        HitTarget::Node(crate::NodeId::from("locked"))
    );
    assert!(frame.records[0].locked);
}

#[test]
fn handles_are_only_collected_when_requested() {
    let mut node = CanvasNode::new("node", point(px(0.0), px(0.0)), size(px(40.0), px(40.0)));
    node.handles
        .push(CanvasHandle::new("out", point(px(40.0), px(20.0))));
    let document = document_fixture().node(node).build();
    let model = CanvasPaintModel::new(document, CanvasViewport::default());
    let canvas_bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));

    let frame = collect_visible_records(&model, canvas_bounds, CanvasPaintOptions::default());
    assert_eq!(frame.records.len(), 1);

    let frame = collect_visible_records(
        &model,
        canvas_bounds,
        CanvasPaintOptions {
            include_handles: true,
            ..CanvasPaintOptions::default()
        },
    );

    assert!(frame.records.iter().any(|record| {
        matches!(
            &record.target,
            HitTarget::Handle { node_id, handle_id }
                if node_id.as_str() == "node" && handle_id.as_str() == "out"
        )
    }));
}

#[test]
fn paint_model_culls_edges_with_custom_router_geometry() {
    let document = connected_edge_document();
    let model = CanvasPaintModel::new_with_router(
        document,
        CanvasViewport::default(),
        &VerticalDetourRouter,
    );

    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(76.0)), size(px(12.0), px(12.0))),
        CanvasPaintOptions::default(),
    );

    assert!(frame.records.iter().any(|record| {
        record.target == HitTarget::Edge(EdgeId::from("a-b"))
            && record.document_bounds.origin == point(px(-1.0), px(-1.0))
            && record.document_bounds.size == size(px(32.0), px(87.0))
    }));
    assert_eq!(
        model
            .runtime
            .edge_geometry(&EdgeId::from("a-b"))
            .unwrap()
            .path
            .document_points(),
        vec![
            point(px(5.0), px(5.0)),
            point(px(5.0), px(80.0)),
            point(px(25.0), px(5.0)),
        ]
    );
}

#[test]
fn edge_paint_geometry_comes_from_runtime_geometry_as_view_path() {
    let document = connected_edge_document();
    let model = CanvasPaintModel::new_with_router(
        document,
        CanvasViewport::new(point(px(0.0), px(0.0)), 2.0).unwrap(),
        &VerticalDetourRouter,
    );
    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(120.0), px(200.0))),
        CanvasPaintOptions::default(),
    );
    let edge_record = frame
        .records
        .iter()
        .find(|record| record.target == HitTarget::Edge(EdgeId::from("a-b")))
        .unwrap();

    assert_eq!(
        edge_record
            .edge_geometry
            .as_ref()
            .unwrap()
            .view_path
            .document_points(),
        vec![
            point(px(10.0), px(10.0)),
            point(px(10.0), px(160.0)),
            point(px(50.0), px(10.0)),
        ]
    );
}

#[test]
fn paint_model_uses_kind_registry_bounds_in_frame_records() {
    let mut node = CanvasNode::new("wide", point(px(10.0), px(10.0)), size(px(20.0), px(20.0)));
    node.kind = "wide".to_string();
    let document = document_fixture().node(node).build();
    let model = CanvasPaintModel::new_with_kind_registry(
        document,
        CanvasViewport::default(),
        geometry_registry(),
    );

    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        CanvasPaintOptions::default(),
    );

    let record = frame
        .records
        .iter()
        .find(|record| record.target == HitTarget::Node(crate::NodeId::from("wide")))
        .unwrap();
    assert_eq!(
        record.document_bounds,
        Bounds::new(point(px(5.0), px(5.0)), size(px(30.0), px(30.0)))
    );
    assert_eq!(record.view_bounds, record.document_bounds);
}

#[test]
fn paint_frame_carries_kind_label_metadata_for_nodes_and_shapes() {
    let mut node = CanvasNode::new(
        "painted",
        point(px(10.0), px(20.0)),
        size(px(100.0), px(80.0)),
    );
    node.kind = "painted-node".to_string();
    let mut shape = CanvasShape::new(
        "shape",
        Bounds::new(point(px(150.0), px(20.0)), size(px(90.0), px(70.0))),
    );
    shape.kind = "painted-shape".to_string();
    let document = document_fixture().node(node).shape(shape).build();
    let model = CanvasPaintModel::new_with_kind_registry(
        document,
        CanvasViewport::new(point(px(10.0), px(10.0)), 2.0).unwrap(),
        paint_registry(),
    );

    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(1_000.0), px(1_000.0))),
        CanvasPaintOptions::default(),
    );

    let node_label = frame
        .records
        .iter()
        .find(|record| record.target == HitTarget::Node(crate::NodeId::from("painted")))
        .and_then(|record| record.label.as_ref())
        .unwrap();
    assert_eq!(node_label.text, "Node label");
    assert_eq!(
        node_label.document_bounds,
        Bounds::new(point(px(18.0), px(28.0)), size(px(84.0), px(64.0)))
    );
    assert_eq!(
        node_label.view_bounds,
        Bounds::new(point(px(16.0), px(36.0)), size(px(168.0), px(128.0)))
    );
    assert_eq!(node_label.color, parse_color("#24292f"));

    let shape_label = frame
        .records
        .iter()
        .find(|record| record.target == HitTarget::Shape(crate::ShapeId::from("shape")))
        .and_then(|record| record.label.as_ref())
        .unwrap();
    assert_eq!(shape_label.text, "Shape label");
    assert_eq!(
        shape_label.document_bounds,
        Bounds::new(point(px(154.0), px(24.0)), size(px(82.0), px(62.0)))
    );
    assert_eq!(shape_label.color, parse_color("#0969da"));
}

#[test]
fn prepared_frame_preserves_snapshot_when_no_labels_are_visible() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "plain",
            point(px(10.0), px(20.0)),
            size(px(100.0), px(80.0)),
        ))
        .build();
    let model = CanvasPaintModel::new(document, CanvasViewport::default());
    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(200.0))),
        CanvasPaintOptions::default(),
    );

    let prepared = CanvasPreparedPaintFrame {
        label_indices: frame.records.iter().map(|_| None).collect(),
        labels: Vec::new(),
        frame,
    };

    assert_eq!(prepared.record_count(), 1);
    assert_eq!(prepared.prepared_label_count(), 0);
    assert!(!prepared.has_prepared_label(0));
    assert_eq!(
        prepared.frame().records[0].target,
        HitTarget::Node("plain".into())
    );
}

#[test]
fn paint_model_from_editor_keeps_an_immutable_editor_snapshot() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "stable",
            point(px(10.0), px(10.0)),
            size(px(40.0), px(20.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    let snapshot = CanvasPaintModel::from(&editor);

    editor
        .apply(DocumentCommand::InsertNode(CanvasNode::new(
            "after-snapshot",
            point(px(70.0), px(10.0)),
            size(px(40.0), px(20.0)),
        )))
        .unwrap();

    let canvas_bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(160.0), px(80.0)));
    let old_frame =
        collect_visible_records(&snapshot, canvas_bounds, CanvasPaintOptions::default());
    let new_frame = collect_visible_records(
        &CanvasPaintModel::from(&editor),
        canvas_bounds,
        CanvasPaintOptions::default(),
    );

    assert_eq!(snapshot.document().node_count(), 1);
    assert!(
        !snapshot
            .document()
            .contains_node(&crate::NodeId::from("after-snapshot"))
    );
    assert!(
        old_frame.records.iter().all(|record| {
            record.target != HitTarget::Node(crate::NodeId::from("after-snapshot"))
        })
    );
    assert!(
        new_frame.records.iter().any(|record| {
            record.target == HitTarget::Node(crate::NodeId::from("after-snapshot"))
        })
    );
}

#[test]
fn paint_model_from_editor_keeps_an_immutable_session_snapshot() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "selected",
            point(px(10.0), px(10.0)),
            size(px(40.0), px(20.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    let viewport = CanvasViewport::new(point(px(5.0), px(-3.0)), 1.5).unwrap();
    editor.set_viewport(viewport);
    editor
        .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Node(
            crate::NodeId::from("selected"),
        )))
        .unwrap();
    editor
        .apply_tool_effect(CanvasToolEffect::SetState(ToolState::Selecting {
            origin: point(px(0.0), px(0.0)),
            current: point(px(20.0), px(20.0)),
            selection_mode: CanvasSelectionMode::Replace,
            base_selection: CanvasSelection::default(),
        }))
        .unwrap();

    let snapshot = CanvasPaintModel::from(&editor);

    editor.set_viewport(CanvasViewport::default());
    editor
        .apply_tool_effect(CanvasToolEffect::ClearSelection)
        .unwrap();
    editor
        .apply_tool_effect(CanvasToolEffect::SetState(ToolState::Idle))
        .unwrap();

    assert_eq!(snapshot.viewport(), viewport);
    assert!(
        snapshot
            .interaction()
            .selection()
            .contains_node(&crate::NodeId::from("selected"))
    );
    assert!(matches!(
        snapshot.interaction().state(),
        CanvasPaintInteractionState::Selecting { .. }
    ));

    let new_snapshot = CanvasPaintModel::from(&editor);
    assert_eq!(new_snapshot.viewport(), CanvasViewport::default());
    assert!(new_snapshot.interaction().selection().is_empty());
    assert!(matches!(
        new_snapshot.interaction().state(),
        CanvasPaintInteractionState::Idle
    ));
}
