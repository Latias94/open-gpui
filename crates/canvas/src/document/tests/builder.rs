use super::*;

#[test]
fn builder_rejects_duplicate_records_during_construction() {
    let mut builder = CanvasDocument::builder();
    builder
        .add_node(CanvasNode::new(
            "duplicate",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .unwrap();

    let error = builder
        .add_node(CanvasNode::new(
            "duplicate",
            point(px(20.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .unwrap_err();

    assert_eq!(
        error,
        DocumentError::DuplicateNode(NodeId::from("duplicate"))
    );
}

#[test]
fn builder_rejects_invalid_edge_endpoints_during_construction() {
    let mut builder = CanvasDocument::builder();
    builder
        .add_node(CanvasNode::new(
            "source",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .unwrap();

    let error = builder
        .add_edge(CanvasEdge::new(
            "edge",
            CanvasEndpoint::new("source", None::<&str>),
            CanvasEndpoint::new("missing", None::<&str>),
        ))
        .unwrap_err();

    assert_eq!(error, DocumentError::MissingNode(NodeId::from("missing")));
}

#[test]
fn builder_prunes_dangling_relations_at_build_time() {
    let child = CanvasRecordId::Node(NodeId::from("child"));
    let existing_group = CanvasRecordId::Shape(ShapeId::from("group"));
    let missing_group = CanvasRecordId::Shape(ShapeId::from("missing"));

    let mut relations = CanvasRecordRelations::default();
    relations.set_parent(child.clone(), missing_group.clone());
    relations.add_to_group(existing_group.clone(), child.clone());
    relations.add_to_group(missing_group.clone(), child.clone());
    relations.set_binding(CanvasRecordBindingRelation::new(
        "binding",
        child.clone(),
        missing_group,
    ));

    let mut builder = CanvasDocument::builder().with_relations(relations);
    builder
        .add_node(CanvasNode::new(
            "child",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .unwrap();
    builder
        .add_shape(CanvasShape::new(
            "group",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        ))
        .unwrap();

    let document = builder.build().unwrap();

    assert_eq!(document.relations().parent_of(&child), None);
    assert_eq!(
        document
            .relations()
            .members_of(&existing_group)
            .cloned()
            .collect::<Vec<_>>(),
        vec![child]
    );
    assert!(document.relations().bindings().next().is_none());
}

#[test]
fn builder_rejects_duplicate_parent_relations_at_build_time() {
    let mut document = document_fixture()
        .node(CanvasNode::new(
            "child",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .build();
    for id in ["frame-a", "frame-b"] {
        document
            .insert_shape(CanvasShape::new(
                id,
                Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
            ))
            .unwrap();
    }
    document
        .apply_transaction(CanvasTransaction::single(
            DocumentCommand::SetRecordParent {
                child: CanvasRecordId::Node(NodeId::from("child")),
                parent: CanvasRecordId::Shape(ShapeId::from("frame-a")),
            },
        ))
        .unwrap();
    let mut value = serde_json::to_value(document.to_snapshot()).unwrap();
    let parents = value["relations"]["parents"].as_array_mut().unwrap();
    let mut duplicate = parents[0].clone();
    duplicate["parent"] =
        serde_json::to_value(CanvasRecordId::Shape(ShapeId::from("frame-b"))).unwrap();
    parents.push(duplicate);
    let snapshot: CanvasSnapshot = serde_json::from_value(value).unwrap();

    let mut builder = CanvasDocument::builder().with_relations(snapshot.relations);
    for node in snapshot.nodes {
        builder.add_node(node).unwrap();
    }
    for shape in snapshot.shapes {
        builder.add_shape(shape).unwrap();
    }

    assert_eq!(
        builder.build().unwrap_err(),
        DocumentError::DuplicateParentRelation(CanvasRecordId::Node(NodeId::from("child")))
    );
}
