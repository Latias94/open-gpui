use super::*;

#[test]
fn applies_transaction_atomically() {
    let mut document = document_fixture().build();
    let transaction = CanvasTransaction::new([
        DocumentCommand::InsertNode(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        )),
        DocumentCommand::InsertEdge(CanvasEdge::new(
            "bad",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("missing", None::<&str>),
        )),
    ]);

    let err = document.apply_transaction(transaction).unwrap_err();

    assert_eq!(err, DocumentError::MissingNode(NodeId::from("missing")));
    assert!(document.nodes.is_empty());
    assert!(document.edges.is_empty());
}

#[test]
fn transaction_inverse_restores_document() {
    let mut document = document_fixture()
        .node(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .build();

    let before = document.clone();
    let transaction = CanvasTransaction::new([
        DocumentCommand::InsertNode(CanvasNode::new(
            "b",
            point(px(20.0), px(0.0)),
            size(px(10.0), px(10.0)),
        )),
        DocumentCommand::InsertEdge(CanvasEdge::new(
            "a-b",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        )),
    ]);
    let inverse = document.invert_transaction(&transaction).unwrap();

    document.apply_transaction(transaction).unwrap();
    assert_ne!(document, before);

    document.apply_transaction(inverse).unwrap();
    assert_eq!(document, before);
}

#[test]
fn transaction_diff_tracks_record_changes() {
    let mut document = document_fixture()
        .node(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .build();

    let moved_a = CanvasNode::new("a", point(px(5.0), px(0.0)), size(px(10.0), px(10.0)));
    let transaction = CanvasTransaction::new([
        DocumentCommand::UpdateNode(moved_a),
        DocumentCommand::InsertNode(CanvasNode::new(
            "b",
            point(px(20.0), px(0.0)),
            size(px(10.0), px(10.0)),
        )),
        DocumentCommand::InsertEdge(CanvasEdge::new(
            "a-b",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        )),
    ]);

    let diff = document.apply_transaction_with_diff(transaction).unwrap();

    assert_eq!(
        diff.updated.iter().cloned().collect::<Vec<_>>(),
        vec![CanvasRecordId::Node(NodeId::from("a"))]
    );
    assert_eq!(
        diff.inserted.iter().cloned().collect::<Vec<_>>(),
        vec![
            CanvasRecordId::Node(NodeId::from("b")),
            CanvasRecordId::Edge(EdgeId::from("a-b")),
        ]
    );
    assert!(diff.removed.is_empty());
}

#[test]
fn transaction_diff_tracks_relation_changes() {
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

    let diff = document
        .apply_transaction_with_diff(CanvasTransaction::single(
            DocumentCommand::SetRecordParent {
                child: CanvasRecordId::Node(NodeId::from("child")),
                parent: CanvasRecordId::Shape(ShapeId::from("group")),
            },
        ))
        .unwrap();

    assert!(diff.relations_changed);
    assert!(!diff.is_empty());
    assert!(diff.inserted.is_empty());
    assert!(diff.updated.is_empty());
    assert!(diff.removed.is_empty());
}

#[test]
fn deleting_records_prunes_relations() {
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
    document
        .apply_transaction(CanvasTransaction::new([
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
        ]))
        .unwrap();

    let diff = document
        .apply_transaction_with_diff(CanvasTransaction::single(DocumentCommand::RemoveNode(
            NodeId::from("child"),
        )))
        .unwrap();

    assert!(diff.relations_changed);
    assert!(document.relations().is_empty());
}

#[test]
fn transaction_diff_compacts_insert_then_remove() {
    let mut document = document_fixture().build();
    let transaction = CanvasTransaction::new([
        DocumentCommand::InsertNode(CanvasNode::new(
            "temp",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        )),
        DocumentCommand::RemoveNode(NodeId::from("temp")),
    ]);

    let diff = document.apply_transaction_with_diff(transaction).unwrap();

    assert!(diff.is_empty());
    assert!(document.nodes.is_empty());
}

#[test]
fn transaction_diff_includes_edges_removed_with_node() {
    let mut document = connected_pair_fixture().build();

    let diff = document
        .apply_transaction_with_diff(CanvasTransaction::single(DocumentCommand::RemoveNode(
            NodeId::from("a"),
        )))
        .unwrap();

    assert_eq!(
        diff.removed.iter().cloned().collect::<Vec<_>>(),
        vec![
            CanvasRecordId::Node(NodeId::from("a")),
            CanvasRecordId::Edge(EdgeId::from("a-b")),
        ]
    );
    assert!(document.edges.is_empty());
}

#[test]
fn document_diff_tracks_metadata_changes() {
    let previous = document_fixture().build();
    let mut document = previous.clone();
    document
        .metadata
        .insert("title".to_string(), serde_json::json!("Canvas"));

    let diff = document.diff_against(&previous);

    assert!(diff.metadata_changed);
    assert!(!diff.is_empty());
}

#[test]
fn randomized_transaction_batches_match_final_diff_and_inverse() {
    let mut rng = TestRng::new(0x7a99_21c8_5f01_4d3b);
    let mut generator = CanvasCommandGenerator::default();
    let mut document = document_fixture().build();

    for _ in 0..96 {
        let before = document.clone();
        let mut draft = before.clone();
        let mut commands = Vec::new();

        for _ in 0..(1 + rng.usize(6)) {
            let command = generator.next_command(&draft, &mut rng);
            draft.apply(command.clone()).unwrap();
            commands.push(command);
        }

        let transaction = CanvasTransaction::new(commands);
        let inverse = before.invert_transaction(&transaction).unwrap();
        let diff = document
            .apply_transaction_with_diff(transaction.clone())
            .unwrap();

        assert_eq!(document, draft);
        assert_eq!(diff, document.diff_against(&before));
        document.validate_integrity().unwrap();

        document.apply_transaction(inverse).unwrap();
        assert_eq!(document, before);

        document.apply_transaction(transaction).unwrap();
        assert_eq!(document, draft);
    }
}
