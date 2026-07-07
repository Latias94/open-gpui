use super::*;

#[test]
fn from_snapshot_rejects_duplicate_parent_relations_for_one_child() {
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
    let snapshot = serde_json::from_value(value).unwrap();

    assert_eq!(
        CanvasDocument::from_snapshot(snapshot).unwrap_err(),
        DocumentError::DuplicateParentRelation(CanvasRecordId::Node(NodeId::from("child")))
    );
}

#[test]
fn from_snapshot_rejects_duplicate_group_relations() {
    let mut document = document_fixture()
        .node(CanvasNode::new(
            "child",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .shape(CanvasShape::new(
            "group",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        ))
        .build();
    let group = CanvasRecordId::Shape(ShapeId::from("group"));
    let member = CanvasRecordId::Node(NodeId::from("child"));
    document
        .apply_transaction(CanvasTransaction::single(
            DocumentCommand::AddRecordToGroup {
                group: group.clone(),
                member: member.clone(),
            },
        ))
        .unwrap();
    let mut value = serde_json::to_value(document.to_snapshot()).unwrap();
    let groups = value["relations"]["groups"].as_array_mut().unwrap();
    groups.push(groups[0].clone());
    let snapshot = serde_json::from_value(value).unwrap();

    assert_eq!(
        CanvasDocument::from_snapshot(snapshot).unwrap_err(),
        DocumentError::DuplicateGroupRelation { group, member }
    );
}

#[test]
fn relation_commands_reject_parent_cycles() {
    let mut document = document_fixture()
        .shape(CanvasShape::new(
            "frame-a",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        ))
        .shape(CanvasShape::new(
            "frame-b",
            Bounds::new(point(px(20.0), px(20.0)), size(px(40.0), px(40.0))),
        ))
        .build();
    document
        .apply_transaction(CanvasTransaction::single(
            DocumentCommand::SetRecordParent {
                child: CanvasRecordId::Shape(ShapeId::from("frame-b")),
                parent: CanvasRecordId::Shape(ShapeId::from("frame-a")),
            },
        ))
        .unwrap();

    let err = document
        .apply_transaction(CanvasTransaction::single(
            DocumentCommand::SetRecordParent {
                child: CanvasRecordId::Shape(ShapeId::from("frame-a")),
                parent: CanvasRecordId::Shape(ShapeId::from("frame-b")),
            },
        ))
        .unwrap_err();

    assert_eq!(
        err,
        DocumentError::CyclicRecordRelation(CanvasRecordId::Shape(ShapeId::from("frame-a")))
    );
}

#[test]
fn relation_commands_reject_group_cycles() {
    let mut document = document_fixture()
        .shape(CanvasShape::new(
            "group-a",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        ))
        .shape(CanvasShape::new(
            "group-b",
            Bounds::new(point(px(20.0), px(20.0)), size(px(40.0), px(40.0))),
        ))
        .build();
    document
        .apply_transaction(CanvasTransaction::single(
            DocumentCommand::AddRecordToGroup {
                group: CanvasRecordId::Shape(ShapeId::from("group-a")),
                member: CanvasRecordId::Shape(ShapeId::from("group-b")),
            },
        ))
        .unwrap();

    let err = document
        .apply_transaction(CanvasTransaction::single(
            DocumentCommand::AddRecordToGroup {
                group: CanvasRecordId::Shape(ShapeId::from("group-b")),
                member: CanvasRecordId::Shape(ShapeId::from("group-a")),
            },
        ))
        .unwrap_err();

    assert_eq!(
        err,
        DocumentError::CyclicRecordRelation(CanvasRecordId::Shape(ShapeId::from("group-a")))
    );
}

#[test]
fn from_snapshot_rejects_relation_cycles() {
    let mut document = document_fixture()
        .shape(CanvasShape::new(
            "frame-a",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        ))
        .shape(CanvasShape::new(
            "frame-b",
            Bounds::new(point(px(20.0), px(20.0)), size(px(40.0), px(40.0))),
        ))
        .build();
    document
        .apply_transaction(CanvasTransaction::single(
            DocumentCommand::SetRecordParent {
                child: CanvasRecordId::Shape(ShapeId::from("frame-b")),
                parent: CanvasRecordId::Shape(ShapeId::from("frame-a")),
            },
        ))
        .unwrap();
    let mut value = serde_json::to_value(document.to_snapshot()).unwrap();
    let parents = value["relations"]["parents"].as_array_mut().unwrap();
    parents.push(
        serde_json::to_value(CanvasRecordParentRelation::new(
            CanvasRecordId::Shape(ShapeId::from("frame-a")),
            CanvasRecordId::Shape(ShapeId::from("frame-b")),
        ))
        .unwrap(),
    );
    let snapshot = serde_json::from_value(value).unwrap();

    assert_eq!(
        CanvasDocument::from_snapshot(snapshot).unwrap_err(),
        DocumentError::CyclicRecordRelation(CanvasRecordId::Shape(ShapeId::from("frame-a")))
    );
}

#[test]
fn from_snapshot_rejects_duplicate_binding_relations() {
    let mut document = document_fixture()
        .node(CanvasNode::new(
            "source",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .shape(CanvasShape::new(
            "target",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        ))
        .build();
    let binding = CanvasRecordBindingRelation::new(
        "binding",
        CanvasRecordId::Node(NodeId::from("source")),
        CanvasRecordId::Shape(ShapeId::from("target")),
    );
    document
        .apply_transaction(CanvasTransaction::single(
            DocumentCommand::SetRecordBinding(binding),
        ))
        .unwrap();
    let mut value = serde_json::to_value(document.to_snapshot()).unwrap();
    let bindings = value["relations"]["bindings"].as_array_mut().unwrap();
    bindings.push(bindings[0].clone());
    let snapshot = serde_json::from_value(value).unwrap();

    assert_eq!(
        CanvasDocument::from_snapshot(snapshot).unwrap_err(),
        DocumentError::DuplicateBindingRelation(BindingId::from("binding"))
    );
}

#[test]
fn relation_commands_reject_self_binding() {
    let mut document = document_fixture()
        .node(CanvasNode::new(
            "source",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .build();
    let source = CanvasRecordId::Node(NodeId::from("source"));

    let err = document
        .apply_transaction(CanvasTransaction::single(
            DocumentCommand::SetRecordBinding(CanvasRecordBindingRelation::new(
                "binding",
                source.clone(),
                source.clone(),
            )),
        ))
        .unwrap_err();

    assert_eq!(err, DocumentError::SelfBindingRelation(source));
}

#[test]
fn relation_commands_reject_dangling_records() {
    let mut document = document_fixture()
        .node(CanvasNode::new(
            "child",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .build();

    let err = document
        .apply_transaction(CanvasTransaction::single(
            DocumentCommand::SetRecordParent {
                child: CanvasRecordId::Node(NodeId::from("child")),
                parent: CanvasRecordId::Shape(ShapeId::from("missing")),
            },
        ))
        .unwrap_err();

    assert_eq!(
        err,
        DocumentError::MissingRelationRecord(CanvasRecordId::Shape(ShapeId::from("missing")))
    );
    assert!(document.relations().is_empty());

    let err = document
        .apply_transaction(CanvasTransaction::single(
            DocumentCommand::SetRecordBinding(CanvasRecordBindingRelation::new(
                "binding",
                CanvasRecordId::Node(NodeId::from("child")),
                CanvasRecordId::Shape(ShapeId::from("missing")),
            )),
        ))
        .unwrap_err();

    assert_eq!(
        err,
        DocumentError::MissingRelationRecord(CanvasRecordId::Shape(ShapeId::from("missing")))
    );
}

#[test]
fn relation_inverse_restores_previous_relations() {
    let mut document = document_fixture()
        .node(CanvasNode::new(
            "child",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .shape(CanvasShape::new(
            "group",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        ))
        .build();
    let before = document.clone();
    let transaction = CanvasTransaction::new([
        DocumentCommand::SetRecordParent {
            child: CanvasRecordId::Node(NodeId::from("child")),
            parent: CanvasRecordId::Shape(ShapeId::from("group")),
        },
        DocumentCommand::AddRecordToGroup {
            group: CanvasRecordId::Shape(ShapeId::from("group")),
            member: CanvasRecordId::Node(NodeId::from("child")),
        },
        DocumentCommand::SetRecordBinding(CanvasRecordBindingRelation::new(
            "binding",
            CanvasRecordId::Node(NodeId::from("child")),
            CanvasRecordId::Shape(ShapeId::from("group")),
        )),
    ]);
    let inverse = document.invert_transaction(&transaction).unwrap();

    document.apply_transaction(transaction).unwrap();
    assert!(!document.relations().is_empty());

    document.apply_transaction(inverse).unwrap();
    assert_eq!(document, before);
}
