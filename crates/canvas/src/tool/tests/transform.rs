use super::*;

#[test]
fn select_tool_translates_node() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "n1",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(10.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(20.0), px(25.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(20.0), px(25.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    let node = editor.document().node(&NodeId::from("n1")).unwrap();
    assert_eq!(node.position, point(px(10.0), px(15.0)));
    assert_eq!(
        editor
            .session
            .selection
            .nodes
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![NodeId::from("n1")]
    );
    assert_eq!(editor.session.state, ToolState::Idle);
    assert_eq!(editor.history().undo_depth(), 1);

    assert!(editor.undo().unwrap());
    let node = editor.document().node(&NodeId::from("n1")).unwrap();
    assert_eq!(node.position, point(px(0.0), px(0.0)));
    assert_eq!(editor.history().redo_depth(), 1);

    assert!(editor.redo().unwrap());
    let node = editor.document().node(&NodeId::from("n1")).unwrap();
    assert_eq!(node.position, point(px(10.0), px(15.0)));
}

#[test]
fn select_tool_waits_for_drag_threshold_before_translating_node() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "n1",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(10.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert!(!editor.is_tool_state_idle());
    assert!(matches!(
        editor.session.state,
        ToolState::PendingTranslation { .. }
    ));
    assert_eq!(editor.history().undo_depth(), 0);

    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(12.0), px(12.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert!(matches!(
        editor.session.state,
        ToolState::PendingTranslation { .. }
    ));
    assert_eq!(
        editor
            .document()
            .node(&NodeId::from("n1"))
            .unwrap()
            .position,
        point(px(0.0), px(0.0))
    );
    assert_eq!(editor.history().undo_depth(), 0);

    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(12.0), px(12.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert_eq!(editor.session.state, ToolState::Idle);
    assert_eq!(editor.history().undo_depth(), 0);
    assert_eq!(
        editor
            .session
            .selection
            .nodes
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![NodeId::from("n1")]
    );
}

#[test]
fn select_tool_cancel_pending_translation_restores_base_selection() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "base",
            point(px(120.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .node(CanvasNode::new(
            "next",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("base"));

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(10.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    assert!(matches!(
        editor.session.state,
        ToolState::PendingTranslation { .. }
    ));

    editor.handle_event(CanvasEvent::Cancel).unwrap();

    assert_eq!(editor.session.state, ToolState::Idle);
    assert_eq!(
        editor
            .session
            .selection
            .nodes
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![NodeId::from("base")]
    );
    assert_eq!(editor.history().undo_depth(), 0);
}

#[test]
fn select_tool_translates_shape() {
    let document = document_fixture()
        .shape(CanvasShape::new(
            "shape",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(80.0))),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(10.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(30.0), px(25.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(30.0), px(25.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    let shape = editor.document().shape(&ShapeId::from("shape")).unwrap();
    assert_eq!(shape.bounds.origin, point(px(20.0), px(15.0)));
    assert_eq!(
        editor
            .session
            .selection
            .shapes
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![ShapeId::from("shape")]
    );
    assert_eq!(editor.history().undo_depth(), 1);

    assert!(editor.undo().unwrap());
    let shape = editor.document().shape(&ShapeId::from("shape")).unwrap();
    assert_eq!(shape.bounds.origin, point(px(0.0), px(0.0)));
}

#[test]
fn canvas_transform_handles_follow_selected_record_bounds() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "node",
            point(px(10.0), px(20.0)),
            size(px(100.0), px(80.0)),
        ))
        .shape(CanvasShape::new(
            "shape",
            Bounds::new(point(px(200.0), px(40.0)), size(px(50.0), px(30.0))),
        ))
        .build();
    let mut selection = CanvasSelection::default();
    selection.nodes.insert(NodeId::from("node"));
    selection.shapes.insert(ShapeId::from("shape"));

    let handles = canvas_transform_handles(&document, &selection, CanvasViewport::default(), None);

    assert_eq!(handles.len(), 8);
    assert!(handles.iter().any(|handle| {
        handle.target == CanvasTransformTarget::Node(NodeId::from("node"))
            && handle.handle == CanvasResizeHandle::BottomRight
            && handle
                .document_bounds
                .contains(&point(px(110.0), px(100.0)))
    }));
    assert!(handles.iter().any(|handle| {
        handle.target == CanvasTransformTarget::Shape(ShapeId::from("shape"))
            && handle.handle == CanvasResizeHandle::TopLeft
            && handle.document_bounds.contains(&point(px(200.0), px(40.0)))
    }));
}

#[test]
fn canvas_transform_handles_use_registered_geometry_bounds() {
    let mut node = CanvasNode::new("node", point(px(10.0), px(20.0)), size(px(100.0), px(80.0)));
    node.kind = "wide".to_string();
    let document = document_fixture().node(node).build();
    let mut selection = CanvasSelection::default();
    selection.nodes.insert(NodeId::from("node"));
    let mut registry = CanvasKindRegistry::open();
    registry.register_node_kind("wide", wide_bounds_node_kind());

    let handles = canvas_transform_handles(
        &document,
        &selection,
        CanvasViewport::default(),
        Some(&registry),
    );

    assert_eq!(handles.len(), 4);
    assert!(handles.iter().any(|handle| {
        handle.target == CanvasTransformTarget::Node(NodeId::from("node"))
            && handle.handle == CanvasResizeHandle::BottomRight
            && handle
                .document_bounds
                .contains(&point(px(140.0), px(100.0)))
    }));
}

#[test]
fn select_tool_resizes_selected_node_with_one_undo_entry() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "node",
            point(px(10.0), px(20.0)),
            size(px(100.0), px(80.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("node"));

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(110.0), px(100.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(130.0), px(125.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(130.0), px(125.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    let node = &editor.document().node(&NodeId::from("node")).unwrap();
    assert_eq!(node.position, point(px(10.0), px(20.0)));
    assert_eq!(node.size, size(px(120.0), px(105.0)));
    assert_eq!(editor.history().undo_depth(), 1);
    assert!(matches!(editor.session.state, ToolState::Idle));
    assert!(
        editor
            .runtime()
            .hit_test(point(px(128.0), px(123.0)), HitOptions::default())
            .any(|record| record.target == HitTarget::Node(NodeId::from("node")))
    );

    assert!(editor.undo().unwrap());
    let node = &editor.document().node(&NodeId::from("node")).unwrap();
    assert_eq!(node.size, size(px(100.0), px(80.0)));
}

#[test]
fn select_tool_resizes_group_and_structural_descendants() {
    let mut edge = CanvasEdge::new(
        "a-b",
        CanvasEndpoint::new("a", None::<&str>),
        CanvasEndpoint::new("b", None::<&str>),
    );
    edge.route = crate::CanvasEdgeRoute::polyline([point(px(30.0), px(70.0))]);
    edge.route.control_points = vec![point(px(40.0), px(20.0))];
    let document = document_fixture()
        .node(CanvasNode::new(
            "a",
            point(px(10.0), px(10.0)),
            size(px(20.0), px(20.0)),
        ))
        .node(CanvasNode::new(
            "b",
            point(px(50.0), px(50.0)),
            size(px(20.0), px(20.0)),
        ))
        .edge(edge)
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("a"));
    editor.session.selection.nodes.insert(NodeId::from("b"));
    assert!(editor.group_selection("group").unwrap());
    editor.handle_event(CanvasEvent::Cancel).unwrap();
    editor
        .session
        .selection
        .shapes
        .insert(ShapeId::from("group"));

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(70.0), px(70.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(130.0), px(130.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(130.0), px(130.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    let a = editor.document().node(&NodeId::from("a")).unwrap();
    let b = editor.document().node(&NodeId::from("b")).unwrap();
    let edge = editor.document().edge(&EdgeId::from("a-b")).unwrap();
    let group = editor.document().shape(&ShapeId::from("group")).unwrap();
    assert_eq!(a.position, point(px(10.0), px(10.0)));
    assert_eq!(a.size, size(px(40.0), px(40.0)));
    assert_eq!(b.position, point(px(90.0), px(90.0)));
    assert_eq!(b.size, size(px(40.0), px(40.0)));
    assert_eq!(edge.route.waypoints, vec![point(px(50.0), px(130.0))]);
    assert_eq!(edge.route.control_points, vec![point(px(70.0), px(30.0))]);
    assert_eq!(
        group.bounds,
        Bounds::new(point(px(10.0), px(10.0)), size(px(120.0), px(120.0)))
    );
    assert_eq!(editor.history().undo_depth(), 2);

    assert!(editor.undo().unwrap());
    let a = editor.document().node(&NodeId::from("a")).unwrap();
    let b = editor.document().node(&NodeId::from("b")).unwrap();
    let edge = editor.document().edge(&EdgeId::from("a-b")).unwrap();
    let group = editor.document().shape(&ShapeId::from("group")).unwrap();
    assert_eq!(a.position, point(px(10.0), px(10.0)));
    assert_eq!(a.size, size(px(20.0), px(20.0)));
    assert_eq!(b.position, point(px(50.0), px(50.0)));
    assert_eq!(b.size, size(px(20.0), px(20.0)));
    assert_eq!(edge.route.waypoints, vec![point(px(30.0), px(70.0))]);
    assert_eq!(edge.route.control_points, vec![point(px(40.0), px(20.0))]);
    assert_eq!(
        group.bounds,
        Bounds::new(point(px(10.0), px(10.0)), size(px(60.0), px(60.0)))
    );
}

#[test]
fn select_tool_direct_multi_select_resize_stays_per_record() {
    let mut edge = CanvasEdge::new(
        "a-b",
        CanvasEndpoint::new("a", None::<&str>),
        CanvasEndpoint::new("b", None::<&str>),
    );
    edge.route = crate::CanvasEdgeRoute::polyline([point(px(30.0), px(70.0))]);
    let document = document_fixture()
        .node(CanvasNode::new(
            "a",
            point(px(10.0), px(10.0)),
            size(px(20.0), px(20.0)),
        ))
        .node(CanvasNode::new(
            "b",
            point(px(50.0), px(50.0)),
            size(px(20.0), px(20.0)),
        ))
        .edge(edge)
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("a"));
    editor.session.selection.nodes.insert(NodeId::from("b"));

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(70.0), px(70.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(90.0), px(90.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(90.0), px(90.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    let a = editor.document().node(&NodeId::from("a")).unwrap();
    let b = editor.document().node(&NodeId::from("b")).unwrap();
    let edge = editor.document().edge(&EdgeId::from("a-b")).unwrap();
    assert_eq!(a.position, point(px(10.0), px(10.0)));
    assert_eq!(a.size, size(px(40.0), px(40.0)));
    assert_eq!(b.position, point(px(50.0), px(50.0)));
    assert_eq!(b.size, size(px(40.0), px(40.0)));
    assert_eq!(edge.route.waypoints, vec![point(px(30.0), px(70.0))]);
}

#[test]
fn select_tool_resize_uses_registered_kind_policy() {
    let mut node = CanvasNode::new("node", point(px(10.0), px(20.0)), size(px(100.0), px(80.0)));
    node.kind = "min-resize".to_string();
    let document = document_fixture().node(node).build();
    let mut registry = CanvasKindRegistry::open();
    registry.register_node_kind("min-resize", minimum_resize_node_kind());
    let mut editor = CanvasEditor::try_new_with_kind_registry(document, registry).unwrap();
    editor.session.selection.nodes.insert(NodeId::from("node"));

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(110.0), px(100.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(-100.0), px(-100.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(-100.0), px(-100.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    let node = &editor.document().node(&NodeId::from("node")).unwrap();
    assert_eq!(node.position, point(px(10.0), px(20.0)));
    assert_eq!(node.size, size(px(64.0), px(48.0)));
    assert_eq!(editor.history().undo_depth(), 1);
}

#[test]
fn select_tool_resize_policy_rejection_is_atomic() {
    let mut node = CanvasNode::new("node", point(px(10.0), px(20.0)), size(px(100.0), px(80.0)));
    node.kind = "reject-resize".to_string();
    let original = node.clone();
    let document = document_fixture().node(node).build();
    let mut registry = CanvasKindRegistry::open();
    registry.register_node_kind("reject-resize", reject_resize_node_kind());
    let mut editor = CanvasEditor::try_new_with_kind_registry(document, registry).unwrap();
    editor.session.selection.nodes.insert(NodeId::from("node"));

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(110.0), px(100.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    let err = editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(130.0), px(125.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap_err();

    assert!(matches!(
        err,
        DocumentError::Schema(CanvasSchemaError::InvalidData {
            record_kind: CanvasRecordKind::Node,
            record_id: crate::CanvasRecordId::Node(id),
            kind,
            message,
        }) if id == NodeId::from("node")
            && kind == "reject-resize"
            && message == "resize is disabled"
    ));
    assert_eq!(
        editor.document().node(&NodeId::from("node")).unwrap(),
        &original
    );
    assert_eq!(editor.history().undo_depth(), 0);
    assert!(
        editor
            .runtime()
            .hit_test(point(px(108.0), px(98.0)), HitOptions::default())
            .any(|record| record.target == HitTarget::Node(NodeId::from("node")))
    );
}

#[test]
fn select_tool_cancel_restores_resize_baseline() {
    let document = document_fixture()
        .shape(CanvasShape::new(
            "shape",
            Bounds::new(point(px(10.0), px(20.0)), size(px(100.0), px(80.0))),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor
        .session
        .selection
        .shapes
        .insert(ShapeId::from("shape"));

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(10.0), px(20.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(30.0), px(45.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    assert_eq!(
        editor
            .document()
            .shape(&ShapeId::from("shape"))
            .unwrap()
            .bounds,
        Bounds::new(point(px(30.0), px(45.0)), size(px(80.0), px(55.0)))
    );

    editor.handle_event(CanvasEvent::Cancel).unwrap();

    assert_eq!(
        editor
            .document()
            .shape(&ShapeId::from("shape"))
            .unwrap()
            .bounds,
        Bounds::new(point(px(10.0), px(20.0)), size(px(100.0), px(80.0)))
    );
    assert_eq!(editor.history().undo_depth(), 0);
    assert!(matches!(editor.session.state, ToolState::Idle));
}

#[test]
fn translating_selected_record_moves_node_and_shape_selection() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .node(CanvasNode::new(
            "b",
            point(px(200.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .shape(CanvasShape::new(
            "shape",
            Bounds::new(point(px(400.0), px(0.0)), size(px(100.0), px(100.0))),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("a"));
    editor.session.selection.nodes.insert(NodeId::from("b"));
    editor
        .session
        .selection
        .shapes
        .insert(ShapeId::from("shape"));

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(10.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(20.0), px(30.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(20.0), px(30.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert_eq!(
        editor.document().node(&NodeId::from("a")).unwrap().position,
        point(px(10.0), px(20.0))
    );
    assert_eq!(
        editor.document().node(&NodeId::from("b")).unwrap().position,
        point(px(210.0), px(20.0))
    );
    assert_eq!(
        editor
            .document()
            .shape(&ShapeId::from("shape"))
            .unwrap()
            .bounds
            .origin,
        point(px(410.0), px(20.0))
    );
    assert_eq!(editor.history().undo_depth(), 1);
}

#[test]
fn translating_selected_parent_moves_related_descendants() {
    let mut document = document_fixture()
        .shape(CanvasShape::new(
            "frame",
            Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(200.0))),
        ))
        .shape(CanvasShape::new(
            "group",
            Bounds::new(point(px(40.0), px(0.0)), size(px(50.0), px(50.0))),
        ))
        .node(CanvasNode::new(
            "leaf",
            point(px(60.0), px(0.0)),
            size(px(20.0), px(20.0)),
        ))
        .build();
    document
        .apply_transaction(CanvasTransaction::new([
            DocumentCommand::SetRecordParent {
                child: CanvasRecordId::Shape(ShapeId::from("group")),
                parent: CanvasRecordId::Shape(ShapeId::from("frame")),
            },
            DocumentCommand::AddRecordToGroup {
                group: CanvasRecordId::Shape(ShapeId::from("group")),
                member: CanvasRecordId::Node(NodeId::from("leaf")),
            },
        ]))
        .unwrap();
    let mut editor = CanvasEditor::new(document);
    editor
        .session
        .selection
        .shapes
        .insert(ShapeId::from("frame"));

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(150.0), px(150.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(160.0), px(170.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(160.0), px(170.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert_eq!(
        editor
            .document()
            .shape(&ShapeId::from("frame"))
            .unwrap()
            .bounds
            .origin,
        point(px(10.0), px(20.0))
    );
    assert_eq!(
        editor
            .document()
            .shape(&ShapeId::from("group"))
            .unwrap()
            .bounds
            .origin,
        point(px(50.0), px(20.0))
    );
    assert_eq!(
        editor
            .document()
            .node(&NodeId::from("leaf"))
            .unwrap()
            .position,
        point(px(70.0), px(20.0))
    );
    assert_eq!(editor.history().undo_depth(), 1);
}

#[test]
fn translating_from_related_descendant_keeps_parent_selection() {
    let mut document = document_fixture()
        .shape(CanvasShape::new(
            "frame",
            Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(200.0))),
        ))
        .node(CanvasNode::new(
            "leaf",
            point(px(60.0), px(40.0)),
            size(px(20.0), px(20.0)),
        ))
        .build();
    document
        .apply_transaction(CanvasTransaction::single(
            DocumentCommand::SetRecordParent {
                child: CanvasRecordId::Node(NodeId::from("leaf")),
                parent: CanvasRecordId::Shape(ShapeId::from("frame")),
            },
        ))
        .unwrap();
    let mut editor = CanvasEditor::new(document);
    editor
        .session
        .selection
        .shapes
        .insert(ShapeId::from("frame"));

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(65.0), px(45.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(75.0), px(65.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(75.0), px(65.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert_eq!(
        editor
            .session
            .selection
            .shapes
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![ShapeId::from("frame")]
    );
    assert!(editor.session.selection.nodes.is_empty());
    assert_eq!(
        editor
            .document()
            .shape(&ShapeId::from("frame"))
            .unwrap()
            .bounds
            .origin,
        point(px(10.0), px(20.0))
    );
    assert_eq!(
        editor
            .document()
            .node(&NodeId::from("leaf"))
            .unwrap()
            .position,
        point(px(70.0), px(60.0))
    );
    assert_eq!(editor.history().undo_depth(), 1);
}

#[test]
fn translating_selected_node_with_shift_locks_to_dominant_axis() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("a"));

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(10.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(20.0), px(30.0)),
            modifiers: CanvasKeyModifiers {
                shift: true,
                ..CanvasKeyModifiers::default()
            },
        })
        .unwrap();

    assert_eq!(
        editor.document().node(&NodeId::from("a")).unwrap().position,
        point(px(0.0), px(20.0))
    );

    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(80.0), px(35.0)),
            modifiers: CanvasKeyModifiers {
                shift: true,
                ..CanvasKeyModifiers::default()
            },
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(80.0), px(35.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers {
                shift: true,
                ..CanvasKeyModifiers::default()
            },
        })
        .unwrap();

    assert_eq!(
        editor.document().node(&NodeId::from("a")).unwrap().position,
        point(px(0.0), px(25.0))
    );
    assert_eq!(editor.history().undo_depth(), 1);
}

#[test]
fn translating_selected_node_snaps_to_nearby_alignment() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "active",
            point(px(0.0), px(0.0)),
            size(px(40.0), px(40.0)),
        ))
        .node(CanvasNode::new(
            "target",
            point(px(100.0), px(0.0)),
            size(px(40.0), px(40.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor
        .session
        .selection
        .nodes
        .insert(NodeId::from("active"));

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(10.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(106.0), px(10.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert_eq!(
        editor
            .document()
            .node(&NodeId::from("active"))
            .unwrap()
            .position,
        point(px(100.0), px(0.0))
    );
    assert!(matches!(
        &editor.session.state,
        ToolState::Translating { snap_guides, .. } if !snap_guides.is_empty()
    ));

    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(106.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    assert_eq!(editor.history().undo_depth(), 1);
}

#[test]
fn translating_selected_nodes_skips_locked_nodes() {
    let mut locked = CanvasNode::new(
        "locked",
        point(px(200.0), px(0.0)),
        size(px(100.0), px(100.0)),
    );
    locked.locked = true;
    let document = document_fixture()
        .node(CanvasNode::new(
            "free",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .node(locked)
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("free"));
    editor
        .session
        .selection
        .nodes
        .insert(NodeId::from("locked"));

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(10.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(20.0), px(30.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(20.0), px(30.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert_eq!(
        editor
            .document()
            .node(&NodeId::from("free"))
            .unwrap()
            .position,
        point(px(10.0), px(20.0))
    );
    assert_eq!(
        editor
            .document()
            .node(&NodeId::from("locked"))
            .unwrap()
            .position,
        point(px(200.0), px(0.0))
    );
    assert_eq!(editor.history().undo_depth(), 1);
}

#[test]
fn pan_tool_moves_viewport() {
    let mut editor = CanvasEditor::default();
    editor.set_tool(CanvasTool::Pan).unwrap();

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(10.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(20.0), px(25.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert_eq!(editor.session.viewport.origin, point(px(-10.0), px(-15.0)));
}
