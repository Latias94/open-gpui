use super::*;

#[test]
fn selected_records_add_transform_handles_to_paint_frame() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "selected",
            point(px(10.0), px(10.0)),
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

    assert_eq!(frame.interaction.transform_handles.len(), 4);
    assert!(frame.interaction.transform_handles.iter().any(|handle| {
        handle.target == CanvasTransformTarget::Node(crate::NodeId::from("selected"))
            && handle.handle == crate::CanvasResizeHandle::BottomRight
            && handle.view_bounds.contains(&point(px(50.0), px(30.0)))
    }));
}

#[test]
fn selected_edge_adds_reconnect_handles_to_paint_frame() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "a",
            point(px(10.0), px(10.0)),
            size(px(40.0), px(20.0)),
        ))
        .node(CanvasNode::new(
            "b",
            point(px(90.0), px(10.0)),
            size(px(40.0), px(20.0)),
        ))
        .edge(CanvasEdge::new(
            "edge",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor
        .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Edge(
            crate::EdgeId::from("edge"),
        )))
        .unwrap();
    let model = CanvasPaintModel::from(&editor);

    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(100.0))),
        CanvasPaintOptions::default(),
    );

    assert_eq!(frame.interaction.reconnect_handles.len(), 2);
    let source_handle = frame
        .interaction
        .reconnect_handles
        .iter()
        .find(|handle| {
            handle.edge_id == crate::EdgeId::from("edge")
                && handle.endpoint == CanvasPaintReconnectEndpoint::Source
        })
        .expect("source reconnect handle");
    assert_eq!(
        source_handle.shape,
        CanvasPaintReconnectHandleShape::SourcePlug
    );
    assert_eq!(source_handle.view_bounds, source_handle.hit_bounds);
    assert!(
        source_handle
            .hit_bounds
            .contains(&point(px(30.0), px(20.0)))
    );
    assert!(
        source_handle
            .visual_bounds
            .contains(&point(px(30.0), px(20.0)))
    );
    assert!(source_handle.visual_bounds.size.width < source_handle.hit_bounds.size.width);

    let target_handle = frame
        .interaction
        .reconnect_handles
        .iter()
        .find(|handle| {
            handle.edge_id == crate::EdgeId::from("edge")
                && handle.endpoint == CanvasPaintReconnectEndpoint::Target
        })
        .expect("target reconnect handle");
    assert_eq!(
        target_handle.shape,
        CanvasPaintReconnectHandleShape::TargetSocket
    );
    assert_eq!(target_handle.view_bounds, target_handle.hit_bounds);
    assert!(
        target_handle
            .hit_bounds
            .contains(&point(px(110.0), px(20.0)))
    );
    assert!(
        target_handle
            .visual_bounds
            .contains(&point(px(110.0), px(20.0)))
    );
    assert!(target_handle.visual_bounds.size.width < target_handle.hit_bounds.size.width);

    let edge_record = frame
        .records
        .iter()
        .find(|record| record.target == HitTarget::Edge(crate::EdgeId::from("edge")))
        .expect("selected edge record");
    assert_eq!(
        edge_record
            .edge_geometry
            .as_ref()
            .map(|geometry| geometry.visual_state),
        Some(CanvasPaintWireVisualState::Selected)
    );
}

#[test]
fn hovered_selected_edge_state_keeps_reconnect_geometry() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "a",
            point(px(10.0), px(10.0)),
            size(px(40.0), px(20.0)),
        ))
        .node(CanvasNode::new(
            "b",
            point(px(90.0), px(10.0)),
            size(px(40.0), px(20.0)),
        ))
        .edge(CanvasEdge::new(
            "edge",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        ))
        .build();
    let edge_target = HitTarget::Edge(crate::EdgeId::from("edge"));
    let mut selection = CanvasSelection::default();
    selection.insert_edge(crate::EdgeId::from("edge"));
    let model = CanvasPaintModel::new(document, CanvasViewport::default()).with_interaction(
        CanvasPaintInteraction::new(selection).with_hovered_target(Some(edge_target.clone())),
    );

    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(100.0))),
        CanvasPaintOptions::default(),
    );

    let edge_record = frame
        .records
        .iter()
        .find(|record| record.target == edge_target)
        .expect("hovered selected edge record");
    assert!(edge_record.selected);
    assert!(edge_record.hovered);
    assert_eq!(
        edge_record
            .edge_geometry
            .as_ref()
            .map(|geometry| geometry.visual_state),
        Some(CanvasPaintWireVisualState::SelectedHovered)
    );
    assert_eq!(frame.interaction.reconnect_handles.len(), 2);
    assert!(
        frame
            .interaction
            .reconnect_handles
            .iter()
            .all(|handle| handle.hit_bounds.contains(&handle.visual_bounds.center()))
    );
}

#[test]
fn translating_state_adds_snap_guides_to_paint_frame() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "selected",
            point(px(10.0), px(10.0)),
            size(px(40.0), px(20.0)),
        ))
        .build();
    let mut selection = CanvasSelection::default();
    selection.insert_node(crate::NodeId::from("selected"));
    let model = CanvasPaintModel::new(
        document,
        CanvasViewport::new(point(px(10.0), px(20.0)), 2.0).unwrap(),
    )
    .with_interaction(
        CanvasPaintInteraction::new(selection).with_internal_tool_state(ToolState::Translating {
            origin: point(px(10.0), px(10.0)),
            last: point(px(20.0), px(20.0)),
            constraint_axis: None,
            node_ids: vec![crate::NodeId::from("selected")],
            shape_ids: Vec::new(),
            snap_guides: vec![CanvasSnapGuide {
                axis: CanvasSnapAxis::Horizontal,
                document_start: point(px(40.0), px(10.0)),
                document_end: point(px(40.0), px(90.0)),
            }],
        }),
    );

    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(200.0))),
        CanvasPaintOptions::default(),
    );

    assert_eq!(
        frame.interaction.snap_guides,
        vec![CanvasPaintSnapGuide {
            axis: CanvasSnapAxis::Horizontal,
            view_start: point(px(60.0), px(-20.0)),
            view_end: point(px(60.0), px(140.0)),
        }]
    );
}

#[test]
fn interaction_feedback_can_be_disabled() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "selected",
            point(px(10.0), px(10.0)),
            size(px(40.0), px(20.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor
        .apply_tool_effects([
            CanvasToolEffect::AddSelection(HitTarget::Node(crate::NodeId::from("selected"))),
            CanvasToolEffect::SetState(ToolState::Selecting {
                origin: point(px(10.0), px(10.0)),
                current: point(px(40.0), px(50.0)),
                selection_mode: CanvasSelectionMode::Replace,
                base_selection: CanvasSelection::default(),
            }),
        ])
        .unwrap();
    let model = CanvasPaintModel::from(&editor);

    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(100.0))),
        CanvasPaintOptions {
            include_interaction_feedback: false,
            ..CanvasPaintOptions::default()
        },
    );

    assert!(frame.records.iter().all(|record| !record.selected));
    assert!(frame.records.iter().all(|record| !record.hovered));
    assert_eq!(frame.interaction, CanvasPaintInteractionFrame::default());
}

#[test]
fn selecting_state_adds_selection_bounds_feedback() {
    let model = CanvasPaintModel::new(
        document_fixture().build(),
        CanvasViewport::new(point(px(10.0), px(20.0)), 2.0).unwrap(),
    )
    .with_interaction(CanvasPaintInteraction::default().with_internal_tool_state(
        ToolState::Selecting {
            origin: point(px(40.0), px(80.0)),
            current: point(px(20.0), px(50.0)),
            selection_mode: CanvasSelectionMode::Replace,
            base_selection: CanvasSelection::default(),
        },
    ));

    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(200.0))),
        CanvasPaintOptions::default(),
    );

    assert_eq!(
        frame.interaction.selection_bounds,
        Some(Bounds::new(
            point(px(20.0), px(60.0)),
            size(px(40.0), px(60.0))
        ))
    );
}

#[test]
fn connecting_state_adds_connection_preview_feedback() {
    let mut node = CanvasNode::new(
        "source",
        point(px(10.0), px(20.0)),
        size(px(100.0), px(80.0)),
    );
    node.handles
        .push(CanvasHandle::new("out", point(px(100.0), px(40.0))));
    let document = document_fixture().node(node).build();
    let mut editor = CanvasEditor::new(document);
    editor
        .apply_tool_effect(CanvasToolEffect::SetState(ToolState::Connecting {
            source: CanvasEndpoint::new("source", Some("out")),
            current: point(px(180.0), px(120.0)),
        }))
        .unwrap();
    let model = CanvasPaintModel::from(&editor);

    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(240.0), px(180.0))),
        CanvasPaintOptions::default(),
    );

    assert_eq!(
        frame.interaction.connection_preview,
        Some(straight_preview(
            point(px(110.0), px(60.0)),
            point(px(180.0), px(120.0)),
            CanvasPaintConnectionTargetState::Free,
            point(px(180.0), px(120.0)),
        ))
    );
}

#[test]
fn connecting_preview_uses_configured_route_policy() {
    let mut node = CanvasNode::new(
        "source",
        point(px(10.0), px(20.0)),
        size(px(100.0), px(80.0)),
    );
    node.handles
        .push(CanvasHandle::new("out", point(px(100.0), px(40.0))));
    let document = document_fixture().node(node).build();
    let mut editor = CanvasEditor::new(document);
    editor
        .apply_tool_effect(CanvasToolEffect::SetState(ToolState::Connecting {
            source: CanvasEndpoint::new("source", Some("out")),
            current: point(px(180.0), px(120.0)),
        }))
        .unwrap();
    let model = CanvasPaintModel::from(&editor);

    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(240.0), px(180.0))),
        CanvasPaintOptions {
            connection_preview_route: CanvasConnectionPreviewRoute::Orthogonal,
            ..CanvasPaintOptions::default()
        },
    );

    let preview = frame.interaction.connection_preview.unwrap();
    assert_eq!(
        preview.edge_geometry.view_path.document_points(),
        vec![
            point(px(110.0), px(60.0)),
            point(px(145.0), px(60.0)),
            point(px(145.0), px(120.0)),
            point(px(180.0), px(120.0)),
        ]
    );
}

#[test]
fn connecting_preview_uses_kind_registry_endpoint_positions() {
    let mut source = CanvasNode::new("source", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
    source.kind = "wide".to_string();
    let mut source_handle = CanvasHandle::new("out", point(px(10.0), px(5.0)));
    source_handle.role = HandleRole::Source;
    source.handles.push(source_handle);

    let mut target = CanvasNode::new("target", point(px(60.0), px(0.0)), size(px(10.0), px(10.0)));
    target.kind = "wide".to_string();
    let mut target_handle = CanvasHandle::new("in", point(px(0.0), px(5.0)));
    target_handle.role = HandleRole::Target;
    target.handles.push(target_handle);

    let document = document_fixture().node(source).node(target).build();
    let model = CanvasPaintModel::new_with_kind_registry(
        document,
        CanvasViewport::default(),
        geometry_registry(),
    )
    .with_interaction(CanvasPaintInteraction::default().with_internal_tool_state(
        ToolState::Connecting {
            source: CanvasEndpoint::new("source", Some("out")),
            current: point(px(40.0), px(5.0)),
        },
    ));

    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        CanvasPaintOptions::default(),
    );

    assert_eq!(
        frame.interaction.connection_preview,
        Some(straight_preview(
            point(px(30.0), px(5.0)),
            point(px(40.0), px(5.0)),
            CanvasPaintConnectionTargetState::Valid,
            point(px(40.0), px(5.0)),
        ))
    );
}

#[test]
fn connecting_preview_snaps_to_valid_target_handle() {
    let mut source = CanvasNode::new(
        "source",
        point(px(10.0), px(20.0)),
        size(px(100.0), px(80.0)),
    );
    let mut source_handle = CanvasHandle::new("out", point(px(100.0), px(40.0)));
    source_handle.role = HandleRole::Source;
    source.handles.push(source_handle);

    let mut target = CanvasNode::new(
        "target",
        point(px(200.0), px(20.0)),
        size(px(100.0), px(80.0)),
    );
    let mut target_handle = CanvasHandle::new("in", point(px(0.0), px(40.0)));
    target_handle.role = HandleRole::Target;
    target.handles.push(target_handle);

    let document = document_fixture().node(source).node(target).build();
    let mut editor = CanvasEditor::new(document);
    editor
        .apply_tool_effect(CanvasToolEffect::SetState(ToolState::Connecting {
            source: CanvasEndpoint::new("source", Some("out")),
            current: point(px(204.0), px(64.0)),
        }))
        .unwrap();
    let model = CanvasPaintModel::from(&editor);

    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(320.0), px(140.0))),
        CanvasPaintOptions::default(),
    );

    assert_eq!(
        frame.interaction.connection_preview,
        Some(straight_preview(
            point(px(110.0), px(60.0)),
            point(px(200.0), px(60.0)),
            CanvasPaintConnectionTargetState::Valid,
            point(px(200.0), px(60.0)),
        ))
    );
}

#[test]
fn connecting_preview_does_not_snap_to_invalid_target_handle() {
    let mut source = CanvasNode::new(
        "source",
        point(px(10.0), px(20.0)),
        size(px(100.0), px(80.0)),
    );
    let mut source_handle = CanvasHandle::new("out", point(px(100.0), px(40.0)));
    source_handle.role = HandleRole::Source;
    source.handles.push(source_handle);

    let mut target = CanvasNode::new(
        "target",
        point(px(200.0), px(20.0)),
        size(px(100.0), px(80.0)),
    );
    let mut invalid_target_handle = CanvasHandle::new("out", point(px(0.0), px(40.0)));
    invalid_target_handle.role = HandleRole::Source;
    target.handles.push(invalid_target_handle);

    let document = document_fixture().node(source).node(target).build();
    let mut editor = CanvasEditor::new(document);
    editor
        .apply_tool_effect(CanvasToolEffect::SetState(ToolState::Connecting {
            source: CanvasEndpoint::new("source", Some("out")),
            current: point(px(204.0), px(64.0)),
        }))
        .unwrap();
    let model = CanvasPaintModel::from(&editor);

    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(320.0), px(140.0))),
        CanvasPaintOptions::default(),
    );

    assert_eq!(
        frame.interaction.connection_preview,
        Some(straight_preview(
            point(px(110.0), px(60.0)),
            point(px(204.0), px(64.0)),
            CanvasPaintConnectionTargetState::Invalid,
            point(px(200.0), px(60.0)),
        ))
    );
}

#[test]
fn reconnecting_preview_reuses_selected_edge_route_path() {
    let mut source = CanvasNode::new(
        "source",
        point(px(0.0), px(0.0)),
        size(px(100.0), px(100.0)),
    );
    let mut source_handle = CanvasHandle::new("out", point(px(100.0), px(50.0)));
    source_handle.role = HandleRole::Source;
    source.handles.push(source_handle);

    let mut target = CanvasNode::new(
        "target",
        point(px(200.0), px(0.0)),
        size(px(100.0), px(100.0)),
    );
    let mut target_handle = CanvasHandle::new("in", point(px(0.0), px(50.0)));
    target_handle.role = HandleRole::Target;
    target.handles.push(target_handle);

    let mut edge = CanvasEdge::new(
        "edge",
        CanvasEndpoint::new("source", Some("out")),
        CanvasEndpoint::new("target", Some("in")),
    );
    edge.route = crate::CanvasEdgeRoute::orthogonal();
    let document = document_fixture()
        .node(source)
        .node(target)
        .edge(edge)
        .build();
    let model = CanvasPaintModel::new(document, CanvasViewport::default()).with_interaction(
        CanvasPaintInteraction::default().with_internal_tool_state(ToolState::Reconnecting {
            edge_id: EdgeId::from("edge"),
            endpoint: crate::CanvasConnectionEndpointRole::Target,
            fixed: CanvasEndpoint::new("source", Some("out")),
            current: point(px(260.0), px(120.0)),
        }),
    );

    let frame = collect_visible_records(
        &model,
        Bounds::new(point(px(0.0), px(0.0)), size(px(360.0), px(180.0))),
        CanvasPaintOptions::default(),
    );

    let preview = frame
        .interaction
        .connection_preview
        .expect("reconnecting should expose a preview route");

    assert_eq!(
        preview.edge_geometry.view_path.document_points(),
        vec![
            point(px(100.0), px(50.0)),
            point(px(180.0), px(50.0)),
            point(px(180.0), px(120.0)),
            point(px(260.0), px(120.0)),
        ]
    );
}
