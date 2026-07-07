use super::*;

#[test]
fn editor_reorders_selected_records_and_supports_undo() {
    let mut back = CanvasNode::new("back", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    back.z_index = 1;
    let mut front = CanvasNode::new(
        "front",
        point(px(10.0), px(10.0)),
        size(px(100.0), px(100.0)),
    );
    front.z_index = 2;
    let document = document_fixture().node(back).node(front).build();
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("back"));

    assert!(
        editor
            .reorder_selection(CanvasZOrderCommand::BringToFront)
            .unwrap()
    );
    assert_eq!(
        editor
            .document()
            .node(&NodeId::from("back"))
            .unwrap()
            .z_index,
        2
    );
    assert_eq!(editor.history().undo_depth(), 1);
    assert_eq!(
        editor
            .runtime()
            .hit_test(point(px(20.0), px(20.0)), HitOptions::default())
            .next()
            .map(|record| record.target.clone()),
        Some(HitTarget::Node(NodeId::from("back")))
    );

    assert!(editor.undo().unwrap());
    assert_eq!(
        editor
            .document()
            .node(&NodeId::from("back"))
            .unwrap()
            .z_index,
        1
    );
}

#[test]
fn bring_forward_crosses_sparse_adjacent_layer() {
    let mut back = CanvasNode::new("back", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    back.z_index = 1;
    let mut front = CanvasShape::new(
        "front",
        Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
    );
    front.z_index = 10;
    let document = document_fixture().node(back).shape(front).build();
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("back"));

    assert!(
        editor
            .reorder_selection(CanvasZOrderCommand::BringForward)
            .unwrap()
    );

    assert_eq!(
        editor
            .runtime()
            .hit_test(point(px(10.0), px(10.0)), HitOptions::default())
            .next()
            .map(|record| record.target.clone()),
        Some(HitTarget::Node(NodeId::from("back")))
    );
    assert!(
        editor
            .document()
            .node(&NodeId::from("back"))
            .unwrap()
            .z_index
            > editor
                .document()
                .shape(&ShapeId::from("front"))
                .unwrap()
                .z_index
    );
}

#[test]
fn send_backward_crosses_duplicate_adjacent_layer() {
    let mut back = CanvasNode::new("back", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    back.z_index = 0;
    let mut front = CanvasShape::new(
        "front",
        Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
    );
    front.z_index = 0;
    let document = document_fixture().node(back).shape(front).build();
    let mut editor = CanvasEditor::new(document);
    editor
        .session
        .selection
        .shapes
        .insert(ShapeId::from("front"));

    assert!(
        editor
            .reorder_selection(CanvasZOrderCommand::SendBackward)
            .unwrap()
    );

    assert_eq!(
        editor
            .runtime()
            .hit_test(point(px(10.0), px(10.0)), HitOptions::default())
            .next()
            .map(|record| record.target.clone()),
        Some(HitTarget::Node(NodeId::from("back")))
    );
    assert!(
        editor
            .document()
            .shape(&ShapeId::from("front"))
            .unwrap()
            .z_index
            < editor
                .document()
                .node(&NodeId::from("back"))
                .unwrap()
                .z_index
    );
}

#[test]
fn z_order_multi_select_preserves_relative_order_across_record_kinds() {
    let mut node = CanvasNode::new("node", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    node.z_index = 0;
    let mut shape = CanvasShape::new(
        "shape",
        Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
    );
    shape.z_index = 1;
    let mut edge = CanvasEdge::new(
        "edge",
        CanvasEndpoint::new("node", None::<&str>),
        CanvasEndpoint::new("node", None::<&str>),
    );
    edge.z_index = 2;
    let mut top = CanvasShape::new(
        "top",
        Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
    );
    top.z_index = 3;
    let document = document_fixture()
        .node(node)
        .shape(shape)
        .shape(top)
        .edge(edge)
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.session.selection.nodes.insert(NodeId::from("node"));
    editor.session.selection.edges.insert(EdgeId::from("edge"));

    assert!(
        editor
            .reorder_selection(CanvasZOrderCommand::BringToFront)
            .unwrap()
    );

    let hits = editor
        .runtime()
        .hit_test(
            point(px(50.0), px(50.0)),
            HitOptions {
                include_handles: false,
                margin: px(24.0),
                ..HitOptions::default()
            },
        )
        .map(|record| record.target.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        hits,
        vec![
            HitTarget::Edge(EdgeId::from("edge")),
            HitTarget::Node(NodeId::from("node")),
            HitTarget::Shape(ShapeId::from("top")),
            HitTarget::Shape(ShapeId::from("shape")),
        ]
    );
}

#[test]
fn z_order_selected_parent_reorders_related_descendants() {
    let mut child = CanvasNode::new("child", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    child.z_index = 0;
    let mut peer = CanvasNode::new("peer", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    peer.z_index = 1;
    let mut edge = CanvasEdge::new(
        "child-peer",
        CanvasEndpoint::new("child", None::<&str>),
        CanvasEndpoint::new("peer", None::<&str>),
    );
    edge.z_index = 2;
    let mut frame = CanvasShape::new(
        "frame",
        Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
    );
    frame.z_index = 3;
    let mut top = CanvasShape::new(
        "top",
        Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
    );
    top.z_index = 4;
    let mut document = document_fixture()
        .node(child)
        .node(peer)
        .edge(edge)
        .shape(frame)
        .shape(top)
        .build();
    document
        .apply_transaction(CanvasTransaction::new([
            DocumentCommand::SetRecordParent {
                child: CanvasRecordId::Node(NodeId::from("child")),
                parent: CanvasRecordId::Shape(ShapeId::from("frame")),
            },
            DocumentCommand::AddRecordToGroup {
                group: CanvasRecordId::Shape(ShapeId::from("frame")),
                member: CanvasRecordId::Node(NodeId::from("peer")),
            },
        ]))
        .unwrap();
    let mut editor = CanvasEditor::new(document);
    editor
        .session
        .selection
        .shapes
        .insert(ShapeId::from("frame"));

    assert!(
        editor
            .reorder_selection(CanvasZOrderCommand::BringToFront)
            .unwrap()
    );

    let hits = editor
        .runtime()
        .hit_test(
            point(px(50.0), px(50.0)),
            HitOptions {
                include_handles: false,
                margin: px(24.0),
                ..HitOptions::default()
            },
        )
        .map(|record| record.target.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        hits,
        vec![
            HitTarget::Shape(ShapeId::from("frame")),
            HitTarget::Edge(EdgeId::from("child-peer")),
            HitTarget::Node(NodeId::from("peer")),
            HitTarget::Node(NodeId::from("child")),
            HitTarget::Shape(ShapeId::from("top")),
        ]
    );

    assert_eq!(editor.history().undo_depth(), 1);
    assert!(editor.undo().unwrap());
    assert_eq!(
        editor
            .runtime()
            .hit_test(
                point(px(50.0), px(50.0)),
                HitOptions {
                    include_handles: false,
                    margin: px(24.0),
                    ..HitOptions::default()
                },
            )
            .next()
            .map(|record| record.target.clone()),
        Some(HitTarget::Shape(ShapeId::from("top")))
    );
}
