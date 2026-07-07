use super::*;

#[test]
fn canvas_selection_adds_removes_and_toggles_targets() {
    let mut selection = CanvasSelection::default();
    let node = HitTarget::Node(NodeId::from("node"));
    let handle = HitTarget::Handle {
        node_id: NodeId::from("node"),
        handle_id: HandleId::from("handle"),
    };
    let edge = HitTarget::Edge(EdgeId::from("edge"));
    let shape = HitTarget::Shape(ShapeId::from("shape"));

    assert!(selection.insert_target(node.clone()));
    assert!(!selection.insert_target(node.clone()));
    assert!(selection.contains_target(&node));
    assert!(!selection.toggle_target(node.clone()));
    assert!(!selection.contains_target(&node));

    assert!(selection.toggle_target(handle.clone()));
    assert!(selection.insert_target(edge.clone()));
    assert!(selection.insert_target(shape.clone()));
    assert!(selection.contains_target(&handle));
    assert!(selection.remove_target(&edge));
    assert!(!selection.contains_target(&edge));
    assert!(selection.contains_target(&shape));
}

#[test]
fn select_tool_ignores_locked_node_hits() {
    let mut node = CanvasNode::new(
        "locked",
        point(px(0.0), px(0.0)),
        size(px(100.0), px(100.0)),
    );
    node.locked = true;
    let document = document_fixture().node(node).build();
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
            position: point(px(30.0), px(30.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(30.0), px(30.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert!(editor.session.selection.is_empty());
    assert_eq!(
        editor
            .document()
            .node(&NodeId::from("locked"))
            .unwrap()
            .position,
        point(px(0.0), px(0.0))
    );
    assert_eq!(editor.history().undo_depth(), 0);
}

#[test]
fn select_tool_clears_selection_when_canvas_is_pressed() {
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
    assert!(!editor.session.selection.is_empty());

    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(10.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(300.0), px(300.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert!(editor.session.selection.is_empty());
}

#[test]
fn select_tool_cancel_restores_selection_after_canvas_press() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "base",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("base"));

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(300.0), px(300.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert!(editor.session.selection.is_empty());

    editor.handle_event(CanvasEvent::Cancel).unwrap();

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
    assert_eq!(editor.session.state, ToolState::Idle);
}

#[test]
fn select_tool_cancel_clears_selection_when_idle() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "base",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("base"));

    editor.handle_event(CanvasEvent::Cancel).unwrap();

    assert!(editor.session.selection.is_empty());
    assert_eq!(editor.session.state, ToolState::Idle);
    assert_eq!(editor.history().undo_depth(), 0);
}

#[test]
fn select_tool_shift_click_toggles_selection() {
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
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("a"));

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(210.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers {
                shift: true,
                ..CanvasKeyModifiers::default()
            },
        })
        .unwrap();

    assert_eq!(
        editor
            .session
            .selection
            .nodes
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![NodeId::from("a"), NodeId::from("b")]
    );
    assert_eq!(editor.session.state, ToolState::Idle);
    assert_eq!(editor.history().undo_depth(), 0);

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(210.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers {
                shift: true,
                ..CanvasKeyModifiers::default()
            },
        })
        .unwrap();

    assert_eq!(
        editor
            .session
            .selection
            .nodes
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![NodeId::from("a")]
    );
    assert_eq!(editor.session.state, ToolState::Idle);
    assert_eq!(editor.history().undo_depth(), 0);
}

#[test]
fn select_tool_uses_registered_precise_hit_policy() {
    let mut node = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    node.kind = "right-half".to_string();
    let document = document_fixture().node(node).build();
    let mut registry = CanvasKindRegistry::open();
    registry.register_node_kind("right-half", right_half_node_kind());
    let mut editor =
        CanvasEditor::try_new_with_kind_registry(document.clone(), registry.clone()).unwrap();

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(25.0), px(25.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert!(editor.session.selection.is_empty());
    assert!(
        editor
            .runtime()
            .hit_test(point(px(25.0), px(25.0)), HitOptions::default())
            .next()
            .is_some()
    );
    assert!(
        editor
            .runtime()
            .precise_hit_test_with_kind_registry(
                editor.document(),
                editor.kind_registry(),
                point(px(25.0), px(25.0)),
                HitOptions::default(),
            )
            .next()
            .is_none()
    );

    let mut editor = CanvasEditor::try_new_with_kind_registry(document, registry).unwrap();
    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(75.0), px(25.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert_eq!(
        editor
            .session
            .selection
            .nodes
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![NodeId::from("a")]
    );
}

#[test]
fn select_tool_delete_key_removes_selected_records() {
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
        .edge(CanvasEdge::new(
            "a-b",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        ))
        .shape(CanvasShape::new(
            "shape",
            Bounds::new(point(px(0.0), px(200.0)), size(px(40.0), px(40.0))),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("a"));
    editor.session.selection.edges.insert(EdgeId::from("a-b"));
    editor
        .session
        .selection
        .shapes
        .insert(ShapeId::from("shape"));

    editor
        .handle_event(CanvasEvent::KeyDown {
            key: CanvasKey::Delete,
            modifiers: CanvasKeyModifiers::default(),
            repeat: false,
        })
        .unwrap();

    assert!(!editor.document().contains_node(&NodeId::from("a")));
    assert!(editor.document().contains_node(&NodeId::from("b")));
    assert!(editor.document().edge_count() == 0);
    assert!(editor.document().shape_count() == 0);
    assert!(editor.session.selection.is_empty());
    assert_eq!(editor.history().undo_depth(), 1);

    assert!(editor.undo().unwrap());
    assert!(editor.document().contains_node(&NodeId::from("a")));
    assert!(editor.document().contains_edge(&EdgeId::from("a-b")));
    assert!(editor.document().contains_shape(&ShapeId::from("shape")));
}

#[test]
fn select_tool_delete_key_removes_related_descendants() {
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
        .node(CanvasNode::new(
            "outside",
            point(px(260.0), px(0.0)),
            size(px(20.0), px(20.0)),
        ))
        .edge(CanvasEdge::new(
            "leaf-outside",
            CanvasEndpoint::new("leaf", None::<&str>),
            CanvasEndpoint::new("outside", None::<&str>),
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
        .handle_event(CanvasEvent::KeyDown {
            key: CanvasKey::Delete,
            modifiers: CanvasKeyModifiers::default(),
            repeat: false,
        })
        .unwrap();

    assert!(!editor.document().contains_shape(&ShapeId::from("frame")));
    assert!(!editor.document().contains_shape(&ShapeId::from("group")));
    assert!(!editor.document().contains_node(&NodeId::from("leaf")));
    assert!(editor.document().contains_node(&NodeId::from("outside")));
    assert!(
        !editor
            .document()
            .contains_edge(&EdgeId::from("leaf-outside"))
    );
    assert!(editor.document().relations().is_empty());
    assert!(editor.session.selection.is_empty());

    assert!(editor.undo().unwrap());
    let group = CanvasRecordId::Shape(ShapeId::from("group"));
    let frame = CanvasRecordId::Shape(ShapeId::from("frame"));
    let leaf = CanvasRecordId::Node(NodeId::from("leaf"));
    assert!(editor.document().contains_shape(&ShapeId::from("frame")));
    assert!(editor.document().contains_shape(&ShapeId::from("group")));
    assert!(editor.document().contains_node(&NodeId::from("leaf")));
    assert!(
        editor
            .document()
            .contains_edge(&EdgeId::from("leaf-outside"))
    );
    assert_eq!(
        editor.document().relations().parent_of(&group),
        Some(&frame)
    );
    assert_eq!(
        editor
            .document()
            .relations()
            .members_of(&group)
            .cloned()
            .collect::<Vec<_>>(),
        vec![leaf]
    );
}

#[test]
fn select_tool_delete_key_skips_locked_selected_records() {
    let mut locked_node = CanvasNode::new(
        "locked-node",
        point(px(0.0), px(0.0)),
        size(px(100.0), px(100.0)),
    );
    locked_node.locked = true;
    let mut locked_edge = CanvasEdge::new(
        "locked-edge",
        CanvasEndpoint::new("a", None::<&str>),
        CanvasEndpoint::new("b", None::<&str>),
    );
    locked_edge.locked = true;
    let mut locked_shape = CanvasShape::new(
        "locked-shape",
        Bounds::new(point(px(0.0), px(200.0)), size(px(40.0), px(40.0))),
    );
    locked_shape.locked = true;
    let document = document_fixture()
        .node(locked_node)
        .node(CanvasNode::new(
            "a",
            point(px(200.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .node(CanvasNode::new(
            "b",
            point(px(400.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .edge(locked_edge)
        .shape(locked_shape)
        .build();
    let mut editor = CanvasEditor::new(document);
    editor
        .session
        .selection
        .nodes
        .insert(NodeId::from("locked-node"));
    editor
        .session
        .selection
        .edges
        .insert(EdgeId::from("locked-edge"));
    editor
        .session
        .selection
        .shapes
        .insert(ShapeId::from("locked-shape"));

    editor
        .handle_event(CanvasEvent::KeyDown {
            key: CanvasKey::Backspace,
            modifiers: CanvasKeyModifiers::default(),
            repeat: false,
        })
        .unwrap();

    assert!(
        editor
            .document()
            .contains_node(&NodeId::from("locked-node"))
    );
    assert!(
        editor
            .document()
            .contains_edge(&EdgeId::from("locked-edge"))
    );
    assert!(
        editor
            .document()
            .contains_shape(&ShapeId::from("locked-shape"))
    );
    assert_eq!(editor.history().undo_depth(), 0);
}

#[test]
fn select_tool_hits_group_border_but_not_transparent_interior() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "a",
            point(px(10.0), px(10.0)),
            size(px(40.0), px(40.0)),
        ))
        .node(CanvasNode::new(
            "b",
            point(px(120.0), px(10.0)),
            size(px(40.0), px(40.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("a"));
    editor.session.selection.nodes.insert(NodeId::from("b"));
    assert!(editor.group_selection("group").unwrap());
    editor.handle_event(CanvasEvent::Cancel).unwrap();

    assert_eq!(
        editor
            .runtime()
            .precise_hit_test_with_kind_registry(
                editor.document(),
                editor.kind_registry(),
                point(px(20.0), px(20.0)),
                HitOptions::default(),
            )
            .map(|record| record.target.clone())
            .collect::<Vec<_>>(),
        vec![HitTarget::Node(NodeId::from("a"))]
    );

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(20.0), px(20.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert_eq!(
        editor
            .selection()
            .selected_nodes()
            .cloned()
            .collect::<Vec<_>>(),
        vec![NodeId::from("a")]
    );

    editor.handle_event(CanvasEvent::Cancel).unwrap();
    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(85.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert_eq!(
        editor
            .selection()
            .selected_shapes()
            .cloned()
            .collect::<Vec<_>>(),
        vec![ShapeId::from("group")]
    );
}

#[test]
fn select_tool_box_selects_intersecting_records() {
    let mut locked = CanvasNode::new(
        "locked",
        point(px(15.0), px(15.0)),
        size(px(20.0), px(20.0)),
    );
    locked.locked = true;
    let document = document_fixture()
        .node(CanvasNode::new(
            "inside",
            point(px(10.0), px(10.0)),
            size(px(20.0), px(20.0)),
        ))
        .node(CanvasNode::new(
            "outside",
            point(px(200.0), px(200.0)),
            size(px(20.0), px(20.0)),
        ))
        .node(locked)
        .build();
    let mut editor = CanvasEditor::new(document);

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(0.0), px(0.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(50.0), px(50.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(50.0), px(50.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert_eq!(
        editor
            .session
            .selection
            .nodes
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![NodeId::from("inside")]
    );
    assert_eq!(editor.session.state, ToolState::Idle);
}

#[test]
fn select_tool_box_select_respects_group_transparent_interior() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "a",
            point(px(80.0), px(80.0)),
            size(px(20.0), px(20.0)),
        ))
        .node(CanvasNode::new(
            "b",
            point(px(10.0), px(10.0)),
            size(px(20.0), px(20.0)),
        ))
        .node(CanvasNode::new(
            "c",
            point(px(170.0), px(170.0)),
            size(px(20.0), px(20.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("a"));
    editor.session.selection.nodes.insert(NodeId::from("b"));
    editor.session.selection.nodes.insert(NodeId::from("c"));
    assert!(editor.group_selection("group").unwrap());
    editor.handle_event(CanvasEvent::Cancel).unwrap();

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(75.0), px(75.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(115.0), px(115.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(115.0), px(115.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert_eq!(
        editor
            .selection()
            .selected_nodes()
            .cloned()
            .collect::<Vec<_>>(),
        vec![NodeId::from("a")]
    );
    assert!(editor.selection().selected_shapes().next().is_none());
}

#[test]
fn select_tool_cancel_restores_selection_after_box_select() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "base",
            point(px(200.0), px(200.0)),
            size(px(20.0), px(20.0)),
        ))
        .node(CanvasNode::new(
            "inside",
            point(px(10.0), px(10.0)),
            size(px(20.0), px(20.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("base"));

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(0.0), px(0.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(40.0), px(40.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert_eq!(
        editor
            .session
            .selection
            .nodes
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![NodeId::from("inside")]
    );

    editor.handle_event(CanvasEvent::Cancel).unwrap();

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
    assert_eq!(editor.session.state, ToolState::Idle);
}

#[test]
fn select_tool_shift_box_adds_to_base_selection_without_accumulating() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "base",
            point(px(200.0), px(200.0)),
            size(px(20.0), px(20.0)),
        ))
        .node(CanvasNode::new(
            "inside",
            point(px(10.0), px(10.0)),
            size(px(20.0), px(20.0)),
        ))
        .node(CanvasNode::new(
            "outside",
            point(px(100.0), px(100.0)),
            size(px(20.0), px(20.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("base"));

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(0.0), px(0.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers {
                shift: true,
                ..CanvasKeyModifiers::default()
            },
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(40.0), px(40.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert_eq!(
        editor
            .session
            .selection
            .nodes
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![NodeId::from("base"), NodeId::from("inside")]
    );

    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(-40.0), px(-40.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

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
}

#[test]
fn select_tool_reports_dropped_reconnect_release_for_empty_canvas() {
    use crate::{CanvasHandle, HandleId, HandleRole};

    let mut source = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    let mut source_handle = CanvasHandle::new("out", point(px(100.0), px(50.0)));
    source_handle.role = HandleRole::Source;
    source.handles.push(source_handle);

    let mut target = CanvasNode::new("b", point(px(200.0), px(0.0)), size(px(100.0), px(100.0)));
    let mut target_handle = CanvasHandle::new("in", point(px(0.0), px(50.0)));
    target_handle.role = HandleRole::Target;
    target.handles.push(target_handle);

    let document = document_fixture()
        .node(source)
        .node(target)
        .edge(CanvasEdge::new(
            "edge",
            CanvasEndpoint::new("a", Some("out")),
            CanvasEndpoint::new("b", Some("in")),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor
        .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Edge(
            EdgeId::from("edge"),
        )))
        .unwrap();

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(200.0), px(50.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(340.0), px(180.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    let edge = editor.document().edge(&EdgeId::from("edge")).unwrap();
    assert_eq!(edge.source.node_id, NodeId::from("a"));
    assert_eq!(edge.source.handle_id, Some(HandleId::from("out")));
    assert_eq!(edge.target.node_id, NodeId::from("b"));
    assert_eq!(edge.target.handle_id, Some(HandleId::from("in")));
    assert_eq!(editor.history().undo_depth(), 0);
    assert_eq!(
        editor.take_connection_release(),
        Some(CanvasConnectionRelease::ReconnectDropped(
            CanvasDroppedReconnectRelease {
                edge_id: EdgeId::from("edge"),
                endpoint: CanvasConnectionEndpointRole::Target,
                fixed: CanvasEndpoint::new("a", Some("out")),
                position: point(px(340.0), px(180.0)),
            }
        ))
    );
}

#[test]
fn selection_effects_normalize_selected_ancestor_and_descendant() {
    let mut document = document_fixture()
        .shape(CanvasShape::new(
            "frame",
            Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(200.0))),
        ))
        .node(CanvasNode::new(
            "child",
            point(px(20.0), px(20.0)),
            size(px(40.0), px(40.0)),
        ))
        .node(CanvasNode::new(
            "outside",
            point(px(260.0), px(20.0)),
            size(px(40.0), px(40.0)),
        ))
        .build();
    document
        .apply_transaction(CanvasTransaction::single(
            DocumentCommand::SetRecordParent {
                child: CanvasRecordId::Node(NodeId::from("child")),
                parent: CanvasRecordId::Shape(ShapeId::from("frame")),
            },
        ))
        .unwrap();
    let mut editor = CanvasEditor::new(document);
    let mut selection = CanvasSelection::default();
    selection.insert_shape(ShapeId::from("frame"));
    selection.insert_node(NodeId::from("child"));
    selection.insert_node(NodeId::from("outside"));

    editor
        .apply_tool_effect(CanvasToolEffect::SetSelection(selection))
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
    assert_eq!(
        editor
            .session
            .selection
            .nodes
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![NodeId::from("outside")]
    );
}

#[test]
fn public_selection_intents_normalize_redundant_descendants() {
    let mut document = document_fixture()
        .shape(CanvasShape::new(
            "frame",
            Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(200.0))),
        ))
        .node(CanvasNode::new(
            "child",
            point(px(20.0), px(20.0)),
            size(px(40.0), px(40.0)),
        ))
        .build();
    document
        .apply_transaction(CanvasTransaction::single(
            DocumentCommand::SetRecordParent {
                child: CanvasRecordId::Node(NodeId::from("child")),
                parent: CanvasRecordId::Shape(ShapeId::from("frame")),
            },
        ))
        .unwrap();
    let mut editor = CanvasEditor::new(document);

    editor
        .apply_tool_intent(CanvasToolIntent::ReplaceSelection(HitTarget::Node(
            NodeId::from("child"),
        )))
        .unwrap();
    assert_eq!(
        editor
            .session
            .selection
            .nodes
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![NodeId::from("child")]
    );

    editor
        .apply_tool_intent(CanvasToolIntent::AddSelection(HitTarget::Shape(
            ShapeId::from("frame"),
        )))
        .unwrap();

    assert!(editor.session.selection.nodes.is_empty());
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
}

#[test]
fn selection_discards_removed_records_after_transaction() {
    let mut editor = CanvasEditor::default();
    editor
        .apply(DocumentCommand::InsertNode(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        )))
        .unwrap();
    editor.session.selection.nodes.insert(NodeId::from("a"));

    editor
        .apply(DocumentCommand::RemoveNode(NodeId::from("a")))
        .unwrap();

    assert!(editor.session.selection.is_empty());
}
