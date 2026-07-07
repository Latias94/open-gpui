use super::*;

#[test]
fn selected_records_are_marked_in_paint_frame() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "selected",
            point(px(10.0), px(10.0)),
            size(px(40.0), px(20.0)),
        ))
        .node(CanvasNode::new(
            "plain",
            point(px(70.0), px(10.0)),
            size(px(40.0), px(20.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor
        .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Node(
            crate::NodeId::from("selected"),
        )))
        .unwrap();
    let model = CanvasPaintModel::from(&editor);

    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(100.0))),
        CanvasPaintOptions::default(),
    );

    assert!(frame.records.iter().any(|record| {
        record.target == HitTarget::Node(crate::NodeId::from("selected")) && record.selected
    }));
    assert!(frame.records.iter().any(|record| {
        record.target == HitTarget::Node(crate::NodeId::from("plain")) && !record.selected
    }));
}

#[test]
fn structurally_selected_records_are_marked_in_paint_frame() {
    let mut document = document_fixture()
        .shape(CanvasShape::new(
            "frame",
            Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(120.0))),
        ))
        .node(CanvasNode::new(
            "child",
            point(px(20.0), px(20.0)),
            size(px(40.0), px(30.0)),
        ))
        .node(CanvasNode::new(
            "peer",
            point(px(80.0), px(20.0)),
            size(px(40.0), px(30.0)),
        ))
        .edge(CanvasEdge::new(
            "child-peer",
            CanvasEndpoint::new("child", None::<&str>),
            CanvasEndpoint::new("peer", None::<&str>),
        ))
        .build();
    document
        .apply_transaction(CanvasTransaction::new([
            DocumentCommand::SetRecordParent {
                child: CanvasRecordId::Node(crate::NodeId::from("child")),
                parent: CanvasRecordId::Shape(crate::ShapeId::from("frame")),
            },
            DocumentCommand::AddRecordToGroup {
                group: CanvasRecordId::Shape(crate::ShapeId::from("frame")),
                member: CanvasRecordId::Node(crate::NodeId::from("peer")),
            },
        ]))
        .unwrap();
    let mut editor = CanvasEditor::new(document);
    editor
        .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Shape(
            crate::ShapeId::from("frame"),
        )))
        .unwrap();
    let model = CanvasPaintModel::from(&editor);

    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(240.0), px(160.0))),
        CanvasPaintOptions::default(),
    );

    let frame_record = frame
        .records
        .iter()
        .find(|record| record.target == HitTarget::Shape(crate::ShapeId::from("frame")))
        .unwrap();
    assert!(frame_record.selected);
    assert!(!frame_record.structurally_selected);
    for target in [
        HitTarget::Node(crate::NodeId::from("child")),
        HitTarget::Node(crate::NodeId::from("peer")),
        HitTarget::Edge(crate::EdgeId::from("child-peer")),
    ] {
        let record = frame
            .records
            .iter()
            .find(|record| record.target == target)
            .unwrap();
        assert!(!record.selected);
        assert!(record.structurally_selected);
    }

    let overlay = frame.widget_overlay_frame(CanvasWidgetOverlayOptions::selected_records());
    assert_eq!(overlay.len(), 1);
    assert_eq!(
        overlay.placements[0].target,
        HitTarget::Shape(crate::ShapeId::from("frame"))
    );
}

#[test]
fn structural_selection_bounds_do_not_replace_transform_handles() {
    let mut document = document_fixture()
        .shape(CanvasShape::new(
            "frame",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(80.0))),
        ))
        .node(CanvasNode::new(
            "child",
            point(px(160.0), px(20.0)),
            size(px(40.0), px(30.0)),
        ))
        .build();
    document
        .apply_transaction(CanvasTransaction::new([DocumentCommand::SetRecordParent {
            child: CanvasRecordId::Node(crate::NodeId::from("child")),
            parent: CanvasRecordId::Shape(crate::ShapeId::from("frame")),
        }]))
        .unwrap();
    let mut editor = CanvasEditor::new(document);
    editor
        .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Shape(
            crate::ShapeId::from("frame"),
        )))
        .unwrap();
    let model = CanvasPaintModel::from(&editor);

    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(240.0), px(140.0))),
        CanvasPaintOptions::default(),
    );

    assert_eq!(
        frame.interaction.structural_selection_bounds,
        Some(Bounds::new(
            point(px(160.0), px(20.0)),
            size(px(40.0), px(30.0))
        ))
    );
    assert_eq!(frame.interaction.transform_handles.len(), 4);
    assert!(frame.interaction.transform_handles.iter().any(|handle| {
        handle.target == CanvasTransformTarget::Shape(crate::ShapeId::from("frame"))
            && handle.handle == crate::CanvasResizeHandle::BottomRight
            && handle.view_bounds.contains(&point(px(100.0), px(80.0)))
    }));
    assert!(
        !frame
            .interaction
            .transform_handles
            .iter()
            .any(|handle| handle.view_bounds.contains(&point(px(200.0), px(80.0))))
    );
}

#[test]
fn child_only_selection_has_no_structural_selection_bounds() {
    let mut document = document_fixture()
        .shape(CanvasShape::new(
            "frame",
            Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(120.0))),
        ))
        .node(CanvasNode::new(
            "child",
            point(px(20.0), px(20.0)),
            size(px(40.0), px(30.0)),
        ))
        .build();
    document
        .apply_transaction(CanvasTransaction::new([DocumentCommand::SetRecordParent {
            child: CanvasRecordId::Node(crate::NodeId::from("child")),
            parent: CanvasRecordId::Shape(crate::ShapeId::from("frame")),
        }]))
        .unwrap();
    let mut editor = CanvasEditor::new(document);
    editor
        .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Shape(
            crate::ShapeId::from("frame"),
        )))
        .unwrap();
    editor
        .apply_tool_effect(CanvasToolEffect::ToggleSelection(HitTarget::Shape(
            crate::ShapeId::from("frame"),
        )))
        .unwrap();
    editor
        .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Node(
            crate::NodeId::from("child"),
        )))
        .unwrap();
    let model = CanvasPaintModel::from(&editor);

    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(240.0), px(160.0))),
        CanvasPaintOptions::default(),
    );

    assert_eq!(frame.interaction.structural_selection_bounds, None);
}

#[test]
fn widget_overlay_frame_uses_selected_visible_record_placements() {
    let mut selected = CanvasNode::new(
        "selected",
        point(px(10.0), px(10.0)),
        size(px(40.0), px(20.0)),
    );
    selected.z_index = 5;
    let mut locked = CanvasNode::new(
        "locked",
        point(px(70.0), px(10.0)),
        size(px(40.0), px(20.0)),
    );
    locked.locked = true;
    let mut hidden = CanvasNode::new(
        "hidden",
        point(px(130.0), px(10.0)),
        size(px(40.0), px(20.0)),
    );
    hidden.hidden = true;
    let mut shape = CanvasShape::new(
        "shape",
        Bounds::new(point(px(10.0), px(60.0)), size(px(60.0), px(30.0))),
    );
    shape.z_index = 4;

    let document = document_fixture()
        .node(selected)
        .node(locked)
        .node(hidden)
        .shape(shape)
        .build();
    let mut editor = CanvasEditor::new(document);
    editor
        .apply_tool_effects([
            CanvasToolEffect::AddSelection(HitTarget::Node(crate::NodeId::from("selected"))),
            CanvasToolEffect::AddSelection(HitTarget::Node(crate::NodeId::from("locked"))),
            CanvasToolEffect::AddSelection(HitTarget::Node(crate::NodeId::from("hidden"))),
            CanvasToolEffect::AddSelection(HitTarget::Shape(crate::ShapeId::from("shape"))),
        ])
        .unwrap();
    let model = CanvasPaintModel::from(&editor);
    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(240.0), px(120.0))),
        CanvasPaintOptions {
            include_hidden: true,
            ..CanvasPaintOptions::default()
        },
    );

    let overlay = frame.widget_overlay_frame(
        CanvasWidgetOverlayOptions::selected_records()
            .with_hit_priority(CanvasWidgetOverlayHitPriority::CanvasFirst),
    );

    assert_eq!(overlay.len(), 2);
    assert_eq!(
        overlay.placements[0],
        CanvasWidgetOverlayPlacement {
            target: HitTarget::Shape(crate::ShapeId::from("shape")),
            document_bounds: Bounds::new(point(px(10.0), px(60.0)), size(px(60.0), px(30.0))),
            view_bounds: Bounds::new(point(px(10.0), px(60.0)), size(px(60.0), px(30.0))),
            z_index: 4,
            hit_priority: CanvasWidgetOverlayHitPriority::CanvasFirst,
        }
    );
    assert_eq!(
        overlay.placements[1],
        CanvasWidgetOverlayPlacement {
            target: HitTarget::Node(crate::NodeId::from("selected")),
            document_bounds: Bounds::new(point(px(10.0), px(10.0)), size(px(40.0), px(20.0))),
            view_bounds: Bounds::new(point(px(10.0), px(10.0)), size(px(40.0), px(20.0))),
            z_index: 5,
            hit_priority: CanvasWidgetOverlayHitPriority::CanvasFirst,
        }
    );

    let including_locked = frame.widget_overlay_frame(
        CanvasWidgetOverlayOptions::selected_nodes()
            .with_locked(true)
            .with_hit_priority(CanvasWidgetOverlayHitPriority::WidgetFirst),
    );
    assert_eq!(including_locked.len(), 2);
    assert!(including_locked.placements.iter().any(|placement| {
        placement.target == HitTarget::Node(crate::NodeId::from("locked"))
            && placement.hit_priority == CanvasWidgetOverlayHitPriority::WidgetFirst
    }));
}

#[test]
fn widget_overlay_bounds_come_from_paint_frame_geometry() {
    let mut node = CanvasNode::new("wide", point(px(10.0), px(10.0)), size(px(20.0), px(20.0)));
    node.kind = "wide".to_string();
    let document = document_fixture().node(node).build();
    let mut editor =
        CanvasEditor::try_new_with_kind_registry(document, geometry_registry()).unwrap();
    editor
        .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Node(
            crate::NodeId::from("wide"),
        )))
        .unwrap();
    let model = CanvasPaintModel::from(&editor);
    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        CanvasPaintOptions::default(),
    );
    let paint_record = frame
        .records
        .iter()
        .find(|record| record.target == HitTarget::Node(crate::NodeId::from("wide")))
        .unwrap();

    let overlay =
        collect_widget_overlay_frame(&frame, CanvasWidgetOverlayOptions::selected_nodes());

    assert_eq!(overlay.len(), 1);
    assert_eq!(
        overlay.placements[0].document_bounds,
        paint_record.document_bounds
    );
    assert_eq!(overlay.placements[0].view_bounds, paint_record.view_bounds);
    assert_eq!(
        overlay.placements[0].document_bounds,
        Bounds::new(point(px(5.0), px(5.0)), size(px(30.0), px(30.0)))
    );
}

#[test]
fn scene_record_groups_keep_node_widgets_atomic_with_z_order() {
    let mut low = CanvasNode::new("low", point(px(0.0), px(0.0)), size(px(100.0), px(80.0)));
    low.z_index = 1;
    let mut high = CanvasNode::new("high", point(px(20.0), px(10.0)), size(px(100.0), px(80.0)));
    high.z_index = 10;
    let document = document_fixture().node(low).node(high).build();
    let model = CanvasPaintModel::new(document, CanvasViewport::default());
    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(160.0))),
        CanvasPaintOptions::default(),
    );

    let scene = CanvasSceneFrame::from_paint_frame(&frame);
    let layers = scene.ordered_layer_items();
    let low_widget = scene_layer_index(
        &layers,
        HitTarget::Node(crate::NodeId::from("low")),
        CanvasSceneLayerPhase::RecordWidget,
    );
    let high_body = scene_layer_index(
        &layers,
        HitTarget::Node(crate::NodeId::from("high")),
        CanvasSceneLayerPhase::RecordBody,
    );
    let high_chrome = scene_layer_index(
        &layers,
        HitTarget::Node(crate::NodeId::from("high")),
        CanvasSceneLayerPhase::RecordChrome,
    );

    assert!(
        low_widget < high_body,
        "a lower z node widget must not render above a higher z node body"
    );
    assert!(
        high_body < high_chrome,
        "node-local chrome must stay inside the same record group above body/widget"
    );
}

#[test]
fn scene_record_groups_promote_selected_nodes_atomically() {
    let mut selected = CanvasNode::new(
        "selected",
        point(px(0.0), px(0.0)),
        size(px(100.0), px(80.0)),
    );
    selected.z_index = 1;
    let mut covering = CanvasNode::new(
        "covering",
        point(px(20.0), px(10.0)),
        size(px(100.0), px(80.0)),
    );
    covering.z_index = 10;
    let document = document_fixture().node(selected).node(covering).build();
    let mut editor = CanvasEditor::new(document);
    editor
        .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Node(
            crate::NodeId::from("selected"),
        )))
        .unwrap();
    let model = CanvasPaintModel::from(&editor);
    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(160.0))),
        CanvasPaintOptions::default(),
    );

    let scene = CanvasSceneFrame::from_paint_frame(&frame);
    let layers = scene.ordered_layer_items();
    let covering_chrome = scene_layer_index(
        &layers,
        HitTarget::Node(crate::NodeId::from("covering")),
        CanvasSceneLayerPhase::RecordChrome,
    );
    let selected_widget = scene_layer_index(
        &layers,
        HitTarget::Node(crate::NodeId::from("selected")),
        CanvasSceneLayerPhase::RecordWidget,
    );

    assert!(
        covering_chrome < selected_widget,
        "selection promotion must move the whole node group, not only top chrome"
    );
}

#[test]
fn scene_preserves_canvas_only_record_ordering() {
    let mut low = CanvasNode::new("low", point(px(0.0), px(0.0)), size(px(40.0), px(30.0)));
    low.z_index = 1;
    let mut middle = CanvasShape::new(
        "middle",
        Bounds::new(point(px(50.0), px(0.0)), size(px(40.0), px(30.0))),
    );
    middle.z_index = 3;
    let mut high = CanvasNode::new("high", point(px(100.0), px(0.0)), size(px(40.0), px(30.0)));
    high.z_index = 5;
    let document = document_fixture()
        .node(high)
        .shape(middle)
        .node(low)
        .build();
    let model = CanvasPaintModel::new(document, CanvasViewport::default());
    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(80.0))),
        CanvasPaintOptions::default(),
    );

    let scene = CanvasSceneFrame::from_paint_frame(&frame);
    let record_targets = scene
        .record_groups()
        .iter()
        .map(|group| group.target.clone())
        .collect::<Vec<_>>();

    assert_eq!(
        record_targets,
        vec![
            HitTarget::Node(crate::NodeId::from("low")),
            HitTarget::Shape(crate::ShapeId::from("middle")),
            HitTarget::Node(crate::NodeId::from("high")),
        ]
    );
}

#[test]
fn scene_exposes_selected_node_chrome_and_tool_chrome_separately() {
    let node = CanvasNode::new(
        "selected",
        point(px(10.0), px(10.0)),
        size(px(80.0), px(60.0)),
    );
    let document = document_fixture().node(node).build();
    let mut editor = CanvasEditor::new(document);
    editor
        .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Node(
            crate::NodeId::from("selected"),
        )))
        .unwrap();
    let model = CanvasPaintModel::from(&editor);
    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(160.0), px(120.0))),
        CanvasPaintOptions::default(),
    );

    let scene = CanvasSceneFrame::from_paint_frame(&frame);
    let selected = scene
        .record_groups()
        .iter()
        .find(|group| group.target == HitTarget::Node(crate::NodeId::from("selected")))
        .unwrap();

    assert!(selected.has_phase(CanvasSceneLayerPhase::RecordBody));
    assert!(selected.has_phase(CanvasSceneLayerPhase::RecordWidget));
    assert!(selected.has_phase(CanvasSceneLayerPhase::RecordChrome));
    assert!(
        !scene.tool_chrome().transform_handles.is_empty(),
        "transform handles remain explicit tool chrome instead of being hidden behind widgets"
    );
}
