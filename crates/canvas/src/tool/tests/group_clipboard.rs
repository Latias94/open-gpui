use super::*;

#[test]
fn editor_duplicate_selection_remaps_internal_edges_and_selects_paste() {
    let mut document = document_fixture()
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
        .node(CanvasNode::new(
            "outside",
            point(px(400.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .edge(CanvasEdge::new(
            "a-b",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        ))
        .edge(CanvasEdge::new(
            "a-outside",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("outside", None::<&str>),
        ))
        .shape(CanvasShape::new(
            "frame",
            Bounds::new(point(px(-20.0), px(-20.0)), size(px(360.0), px(160.0))),
        ))
        .build();
    document
        .apply_transaction(CanvasTransaction::new([
            DocumentCommand::SetRecordParent {
                child: CanvasRecordId::Node(NodeId::from("a")),
                parent: CanvasRecordId::Shape(ShapeId::from("frame")),
            },
            DocumentCommand::AddRecordToGroup {
                group: CanvasRecordId::Shape(ShapeId::from("frame")),
                member: CanvasRecordId::Node(NodeId::from("a")),
            },
        ]))
        .unwrap();
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("a"));
    editor.session.selection.nodes.insert(NodeId::from("b"));
    editor
        .session
        .selection
        .shapes
        .insert(ShapeId::from("frame"));

    assert!(
        editor
            .duplicate_selection(point(px(20.0), px(30.0)))
            .unwrap()
    );

    assert!(editor.document().contains_node(&NodeId::from("a-copy")));
    assert!(editor.document().contains_node(&NodeId::from("b-copy")));
    assert!(editor.document().contains_edge(&EdgeId::from("a-b-copy")));
    assert!(
        editor
            .document()
            .contains_shape(&ShapeId::from("frame-copy"))
    );
    assert!(
        !editor
            .document()
            .contains_edge(&EdgeId::from("a-outside-copy"))
    );
    assert_eq!(
        editor
            .document()
            .node(&NodeId::from("a-copy"))
            .unwrap()
            .position,
        point(px(20.0), px(30.0))
    );
    assert_eq!(
        editor
            .session
            .selection
            .nodes
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![NodeId::from("b-copy")]
    );
    assert!(editor.session.selection.edges.is_empty());
    assert_eq!(
        editor
            .session
            .selection
            .shapes
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![ShapeId::from("frame-copy")]
    );
    let copied_node = CanvasRecordId::Node(NodeId::from("a-copy"));
    let copied_frame = CanvasRecordId::Shape(ShapeId::from("frame-copy"));
    assert_eq!(
        editor.document().relations().parent_of(&copied_node),
        Some(&copied_frame)
    );
    assert_eq!(
        editor
            .document()
            .relations()
            .members_of(&copied_frame)
            .cloned()
            .collect::<Vec<_>>(),
        vec![copied_node]
    );
    assert_eq!(editor.history().undo_depth(), 1);
    assert!(
        editor
            .runtime()
            .hit_test(point(px(25.0), px(35.0)), HitOptions::default())
            .any(|record| record.target == HitTarget::Node(NodeId::from("a-copy")))
    );

    assert!(editor.undo().unwrap());
    assert!(!editor.document().contains_node(&NodeId::from("a-copy")));
    assert!(!editor.document().contains_edge(&EdgeId::from("a-b-copy")));
    assert!(
        !editor
            .document()
            .contains_shape(&ShapeId::from("frame-copy"))
    );
}

#[test]
fn editor_cut_and_paste_selection_use_command_path() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .shape(CanvasShape::new(
            "shape",
            Bounds::new(point(px(0.0), px(200.0)), size(px(40.0), px(40.0))),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("a"));
    editor
        .session
        .selection
        .shapes
        .insert(ShapeId::from("shape"));

    let payload = editor.cut_selection().unwrap().unwrap();
    assert!(!editor.document().contains_node(&NodeId::from("a")));
    assert!(!editor.document().contains_shape(&ShapeId::from("shape")));
    assert!(editor.session.selection.is_empty());
    assert_eq!(editor.history().undo_depth(), 1);

    assert!(
        editor
            .paste_clipboard(&payload, point(px(10.0), px(20.0)))
            .unwrap()
    );
    assert!(editor.document().contains_node(&NodeId::from("a-copy")));
    assert!(
        editor
            .document()
            .contains_shape(&ShapeId::from("shape-copy"))
    );
    assert_eq!(
        editor
            .session
            .selection
            .nodes
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![NodeId::from("a-copy")]
    );
    assert_eq!(
        editor
            .session
            .selection
            .shapes
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![ShapeId::from("shape-copy")]
    );
    assert_eq!(editor.history().undo_depth(), 2);
}

#[test]
fn editor_groups_selection_with_internal_edges_and_selects_group() {
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
        .node(CanvasNode::new(
            "outside",
            point(px(400.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .edge(CanvasEdge::new(
            "a-b",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        ))
        .edge(CanvasEdge::new(
            "a-outside",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("outside", None::<&str>),
        ))
        .shape(CanvasShape::new(
            "shape",
            Bounds::new(point(px(50.0), px(160.0)), size(px(80.0), px(40.0))),
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

    assert!(editor.group_selection("group").unwrap());

    let group = CanvasRecordId::Shape(ShapeId::from("group"));
    let member_a = CanvasRecordId::Node(NodeId::from("a"));
    let member_b = CanvasRecordId::Node(NodeId::from("b"));
    let member_shape = CanvasRecordId::Shape(ShapeId::from("shape"));
    let internal_edge = CanvasRecordId::Edge(EdgeId::from("a-b"));
    let external_edge = CanvasRecordId::Edge(EdgeId::from("a-outside"));
    assert_eq!(
        editor
            .document()
            .shape(&ShapeId::from("group"))
            .unwrap()
            .kind,
        "group"
    );
    for member in [&member_a, &member_b, &member_shape, &internal_edge] {
        assert_eq!(
            editor.document().relations().parent_of(member),
            Some(&group)
        );
        assert!(
            editor
                .document()
                .relations()
                .members_of(&group)
                .any(|candidate| candidate == member)
        );
    }
    assert_eq!(
        editor.document().relations().parent_of(&external_edge),
        None
    );
    assert!(
        !editor
            .document()
            .relations()
            .members_of(&group)
            .any(|candidate| candidate == &external_edge)
    );
    assert_eq!(
        editor
            .session
            .selection
            .shapes
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![ShapeId::from("group")]
    );
    assert!(editor.session.selection.nodes.is_empty());
    assert!(editor.session.selection.edges.is_empty());
    assert_eq!(editor.history().undo_depth(), 1);

    assert!(editor.undo().unwrap());
    assert!(!editor.document().contains_shape(&ShapeId::from("group")));
    assert!(editor.document().relations().is_empty());
    assert!(editor.document().contains_edge(&EdgeId::from("a-b")));
}

#[test]
fn editor_ungroups_selected_groups_and_selects_members() {
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
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("a"));
    editor.session.selection.nodes.insert(NodeId::from("b"));
    assert!(editor.group_selection("group").unwrap());

    assert!(editor.ungroup_selection().unwrap());

    assert!(!editor.document().contains_shape(&ShapeId::from("group")));
    assert!(editor.document().relations().is_empty());
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
    assert_eq!(
        editor
            .session
            .selection
            .edges
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        Vec::<EdgeId>::new()
    );
    assert!(editor.session.selection.shapes.is_empty());
    assert!(editor.document().contains_edge(&EdgeId::from("a-b")));

    assert!(editor.undo().unwrap());
    let group = CanvasRecordId::Shape(ShapeId::from("group"));
    assert!(editor.document().contains_shape(&ShapeId::from("group")));
    let expected_members: IndexSet<CanvasRecordId> = IndexSet::from_iter([
        CanvasRecordId::Node(NodeId::from("a")),
        CanvasRecordId::Node(NodeId::from("b")),
        CanvasRecordId::Edge(EdgeId::from("a-b")),
    ]);
    assert_eq!(
        editor
            .document()
            .relations()
            .members_of(&group)
            .cloned()
            .collect::<IndexSet<_>>(),
        expected_members
    );
}

#[test]
fn editor_group_selection_skips_locked_and_hidden_records() {
    let mut locked = CanvasNode::new(
        "locked",
        point(px(400.0), px(0.0)),
        size(px(100.0), px(100.0)),
    );
    locked.locked = true;
    let mut hidden = CanvasShape::new(
        "hidden",
        Bounds::new(point(px(0.0), px(200.0)), size(px(40.0), px(40.0))),
    );
    hidden.hidden = true;
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
        .node(locked)
        .edge(CanvasEdge::new(
            "a-b",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        ))
        .shape(hidden)
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("a"));
    editor.session.selection.nodes.insert(NodeId::from("b"));
    editor
        .session
        .selection
        .nodes
        .insert(NodeId::from("locked"));
    editor
        .session
        .selection
        .shapes
        .insert(ShapeId::from("hidden"));

    assert!(editor.group_selection("group").unwrap());

    let group = CanvasRecordId::Shape(ShapeId::from("group"));
    let expected_members: IndexSet<CanvasRecordId> = IndexSet::from_iter([
        CanvasRecordId::Node(NodeId::from("a")),
        CanvasRecordId::Node(NodeId::from("b")),
        CanvasRecordId::Edge(EdgeId::from("a-b")),
    ]);
    assert_eq!(
        editor
            .document()
            .relations()
            .members_of(&group)
            .cloned()
            .collect::<IndexSet<_>>(),
        expected_members
    );
    assert_eq!(
        editor
            .document()
            .relations()
            .parent_of(&CanvasRecordId::Node(NodeId::from("locked"))),
        None
    );
    assert_eq!(
        editor
            .document()
            .relations()
            .parent_of(&CanvasRecordId::Shape(ShapeId::from("hidden"))),
        None
    );
}

#[test]
fn editor_groups_existing_group_as_atomic_member() {
    let mut inner_group = CanvasShape::new(
        "inner-group",
        Bounds::new(point(px(0.0), px(0.0)), size(px(120.0), px(120.0))),
    );
    inner_group.kind = "group".to_string();
    let mut document = document_fixture()
        .shape(inner_group)
        .node(CanvasNode::new(
            "leaf",
            point(px(10.0), px(10.0)),
            size(px(20.0), px(20.0)),
        ))
        .node(CanvasNode::new(
            "peer",
            point(px(200.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .build();
    document
        .apply_transaction(CanvasTransaction::new([
            DocumentCommand::SetRecordParent {
                child: CanvasRecordId::Node(NodeId::from("leaf")),
                parent: CanvasRecordId::Shape(ShapeId::from("inner-group")),
            },
            DocumentCommand::AddRecordToGroup {
                group: CanvasRecordId::Shape(ShapeId::from("inner-group")),
                member: CanvasRecordId::Node(NodeId::from("leaf")),
            },
        ]))
        .unwrap();
    let mut editor = CanvasEditor::new(document);
    editor
        .session
        .selection
        .shapes
        .insert(ShapeId::from("inner-group"));
    editor.session.selection.nodes.insert(NodeId::from("peer"));

    assert!(editor.group_selection("outer-group").unwrap());

    let outer_group = CanvasRecordId::Shape(ShapeId::from("outer-group"));
    let expected_members: IndexSet<CanvasRecordId> = IndexSet::from_iter([
        CanvasRecordId::Node(NodeId::from("peer")),
        CanvasRecordId::Shape(ShapeId::from("inner-group")),
    ]);
    assert_eq!(
        editor
            .document()
            .relations()
            .members_of(&outer_group)
            .cloned()
            .collect::<IndexSet<_>>(),
        expected_members
    );
    assert_eq!(
        editor
            .document()
            .relations()
            .parent_of(&CanvasRecordId::Node(NodeId::from("leaf"))),
        Some(&CanvasRecordId::Shape(ShapeId::from("inner-group")))
    );
}

#[test]
fn editor_group_selection_preserves_common_parent_membership() {
    let mut frame = CanvasShape::new(
        "frame",
        Bounds::new(point(px(-20.0), px(-20.0)), size(px(360.0), px(180.0))),
    );
    frame.kind = "group".to_string();
    let mut document = document_fixture()
        .shape(frame)
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
        .build();
    let frame = CanvasRecordId::Shape(ShapeId::from("frame"));
    for member in [
        CanvasRecordId::Node(NodeId::from("a")),
        CanvasRecordId::Node(NodeId::from("b")),
    ] {
        document
            .apply_transaction(CanvasTransaction::new([
                DocumentCommand::SetRecordParent {
                    child: member.clone(),
                    parent: frame.clone(),
                },
                DocumentCommand::AddRecordToGroup {
                    group: frame.clone(),
                    member,
                },
            ]))
            .unwrap();
    }
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("a"));
    editor.session.selection.nodes.insert(NodeId::from("b"));

    assert!(editor.group_selection("group").unwrap());

    let group = CanvasRecordId::Shape(ShapeId::from("group"));
    assert_eq!(
        editor.document().relations().parent_of(&group),
        Some(&frame)
    );
    assert!(
        editor
            .document()
            .relations()
            .members_of(&frame)
            .any(|member| member == &group)
    );
    for member in [
        CanvasRecordId::Node(NodeId::from("a")),
        CanvasRecordId::Node(NodeId::from("b")),
        CanvasRecordId::Edge(EdgeId::from("a-b")),
    ] {
        assert_eq!(
            editor.document().relations().parent_of(&member),
            Some(&group)
        );
        assert!(
            !editor
                .document()
                .relations()
                .members_of(&frame)
                .any(|candidate| candidate == &member)
        );
    }

    assert!(editor.ungroup_selection().unwrap());

    assert!(!editor.document().contains_shape(&ShapeId::from("group")));
    for member in [
        CanvasRecordId::Node(NodeId::from("a")),
        CanvasRecordId::Node(NodeId::from("b")),
        CanvasRecordId::Edge(EdgeId::from("a-b")),
    ] {
        assert_eq!(
            editor.document().relations().parent_of(&member),
            Some(&frame)
        );
        assert!(
            editor
                .document()
                .relations()
                .members_of(&frame)
                .any(|candidate| candidate == &member)
        );
    }
}

#[test]
fn editor_group_selection_ignores_selected_descendant_of_selected_group() {
    let mut inner_group = CanvasShape::new(
        "inner-group",
        Bounds::new(point(px(0.0), px(0.0)), size(px(120.0), px(120.0))),
    );
    inner_group.kind = "group".to_string();
    let mut document = document_fixture()
        .shape(inner_group)
        .node(CanvasNode::new(
            "leaf",
            point(px(10.0), px(10.0)),
            size(px(20.0), px(20.0)),
        ))
        .node(CanvasNode::new(
            "peer",
            point(px(200.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .build();
    document
        .apply_transaction(CanvasTransaction::new([
            DocumentCommand::SetRecordParent {
                child: CanvasRecordId::Node(NodeId::from("leaf")),
                parent: CanvasRecordId::Shape(ShapeId::from("inner-group")),
            },
            DocumentCommand::AddRecordToGroup {
                group: CanvasRecordId::Shape(ShapeId::from("inner-group")),
                member: CanvasRecordId::Node(NodeId::from("leaf")),
            },
        ]))
        .unwrap();
    let mut editor = CanvasEditor::new(document);
    editor
        .session
        .selection
        .shapes
        .insert(ShapeId::from("inner-group"));
    editor.session.selection.nodes.insert(NodeId::from("leaf"));
    editor.session.selection.nodes.insert(NodeId::from("peer"));

    assert!(editor.group_selection("outer-group").unwrap());

    let outer_group = CanvasRecordId::Shape(ShapeId::from("outer-group"));
    let expected_members: IndexSet<CanvasRecordId> = IndexSet::from_iter([
        CanvasRecordId::Node(NodeId::from("peer")),
        CanvasRecordId::Shape(ShapeId::from("inner-group")),
    ]);
    assert_eq!(
        editor
            .document()
            .relations()
            .members_of(&outer_group)
            .cloned()
            .collect::<IndexSet<_>>(),
        expected_members
    );
    assert_eq!(
        editor
            .document()
            .relations()
            .parent_of(&CanvasRecordId::Node(NodeId::from("leaf"))),
        Some(&CanvasRecordId::Shape(ShapeId::from("inner-group")))
    );
}
