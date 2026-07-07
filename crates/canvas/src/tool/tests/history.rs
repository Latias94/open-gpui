use super::*;

#[test]
fn direct_transactions_clear_redo_history() {
    let mut editor = CanvasEditor::default();
    editor
        .apply(DocumentCommand::InsertNode(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        )))
        .unwrap();

    assert!(editor.undo().unwrap());
    assert_eq!(editor.history().redo_depth(), 1);

    editor
        .apply(DocumentCommand::InsertNode(CanvasNode::new(
            "b",
            point(px(100.0), px(0.0)),
            size(px(100.0), px(100.0)),
        )))
        .unwrap();

    assert_eq!(editor.history().undo_depth(), 1);
    assert_eq!(editor.history().redo_depth(), 0);
    assert!(editor.document().contains_node(&NodeId::from("b")));
    assert!(!editor.document().contains_node(&NodeId::from("a")));
}

#[test]
fn no_op_committed_transactions_do_not_push_history_or_clear_redo() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "child",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .shape(CanvasShape::new(
            "frame",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    let relation_transaction = CanvasTransaction::single(DocumentCommand::SetRecordParent {
        child: CanvasRecordId::Node(NodeId::from("child")),
        parent: CanvasRecordId::Shape(ShapeId::from("frame")),
    });

    let first_diff = editor
        .apply_transaction_with_diff(relation_transaction.clone())
        .unwrap();
    assert!(!first_diff.is_empty());
    assert_eq!(editor.history().undo_depth(), 1);

    assert!(editor.undo().unwrap());
    assert_eq!(editor.history().undo_depth(), 0);
    assert_eq!(editor.history().redo_depth(), 1);

    let second_diff = editor
        .apply_transaction_with_diff(CanvasTransaction::single(
            DocumentCommand::ClearRecordParent {
                child: CanvasRecordId::Node(NodeId::from("child")),
            },
        ))
        .unwrap();

    assert!(second_diff.is_empty());
    assert_eq!(editor.history().undo_depth(), 0);
    assert_eq!(editor.history().redo_depth(), 1);
}

#[test]
fn relation_order_only_transactions_do_not_push_history() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "member",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .build();
    let mut document = document;
    for id in ["group-a", "group-b"] {
        document
            .insert_shape(CanvasShape::new(
                id,
                Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
            ))
            .unwrap();
    }
    let member = CanvasRecordId::Node(NodeId::from("member"));
    let group_a = CanvasRecordId::Shape(ShapeId::from("group-a"));
    let group_b = CanvasRecordId::Shape(ShapeId::from("group-b"));
    document
        .apply_transaction(CanvasTransaction::new([
            DocumentCommand::AddRecordToGroup {
                group: group_a.clone(),
                member: member.clone(),
            },
            DocumentCommand::AddRecordToGroup {
                group: group_b,
                member: member.clone(),
            },
        ]))
        .unwrap();
    let mut editor = CanvasEditor::new(document);

    let diff = editor
        .apply_transaction_with_diff(CanvasTransaction::new([
            DocumentCommand::RemoveRecordFromGroup {
                group: group_a.clone(),
                member: member.clone(),
            },
            DocumentCommand::AddRecordToGroup {
                group: group_a,
                member,
            },
        ]))
        .unwrap();

    assert!(diff.is_empty());
    assert_eq!(editor.history().undo_depth(), 0);
}

#[test]
fn no_op_undo_and_redo_discard_stale_history_entries() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "child",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    let noop = CanvasTransaction::single(DocumentCommand::ClearRecordParent {
        child: CanvasRecordId::Node(NodeId::from("child")),
    });
    editor.history_mut_for_test().push_undo(noop.clone());
    editor.history_mut_for_test().push_redo(noop);

    assert!(!editor.undo().unwrap());
    assert_eq!(editor.history().undo_depth(), 0);
    assert_eq!(editor.history().redo_depth(), 1);

    assert!(!editor.redo().unwrap());
    assert_eq!(editor.history().undo_depth(), 0);
    assert_eq!(editor.history().redo_depth(), 0);
}

#[test]
fn editor_transactions_return_document_diff() {
    let mut editor = CanvasEditor::default();

    let diff = editor
        .apply_transaction_with_diff(CanvasTransaction::single(DocumentCommand::InsertNode(
            CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        )))
        .unwrap();

    assert_eq!(
        diff.inserted.iter().cloned().collect::<Vec<_>>(),
        vec![crate::CanvasRecordId::Node(NodeId::from("a"))]
    );
    assert!(editor.history().can_undo());
}

#[test]
fn editor_kind_registry_normalizes_and_validates_transactions() {
    let mut registry = CanvasKindRegistry::open();
    registry.register_node_kind("note", required_title_node_kind());
    let mut editor =
        CanvasEditor::try_new_with_kind_registry(document_fixture().build(), registry).unwrap();

    let mut note = CanvasNode::new("note", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    note.kind = "note".to_string();
    note.data.insert("label".to_string(), json!("Migrated"));

    editor.apply(DocumentCommand::InsertNode(note)).unwrap();

    assert_eq!(
        editor
            .document()
            .node(&NodeId::from("note"))
            .unwrap()
            .data
            .get("title"),
        Some(&json!("Migrated"))
    );
    assert_eq!(editor.history().undo_depth(), 1);

    let mut invalid = CanvasNode::new(
        "invalid",
        point(px(0.0), px(0.0)),
        size(px(100.0), px(100.0)),
    );
    invalid.kind = "note".to_string();
    invalid.data.insert("title".to_string(), json!(false));
    let err = editor
        .apply(DocumentCommand::InsertNode(invalid))
        .unwrap_err();

    assert!(matches!(
        err,
        DocumentError::Schema(CanvasSchemaError::InvalidData {
            record_kind: CanvasRecordKind::Node,
            record_id: crate::CanvasRecordId::Node(id),
            kind,
            ..
        }) if id == NodeId::from("invalid") && kind == "note"
    ));
    assert!(!editor.document().contains_node(&NodeId::from("invalid")));
    assert_eq!(editor.history().undo_depth(), 1);
}

#[test]
fn editor_set_kind_registry_normalizes_document_and_clears_stale_history() {
    let mut note = CanvasNode::new("note", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    note.kind = "note".to_string();
    note.data.insert("label".to_string(), json!("Migrated"));
    let mut editor = CanvasEditor::default();
    editor.apply(DocumentCommand::InsertNode(note)).unwrap();
    assert_eq!(editor.history().undo_depth(), 1);

    let mut registry = CanvasKindRegistry::open();
    registry.register_node_kind("note", required_title_node_kind());
    editor.set_kind_registry(registry).unwrap();

    assert_eq!(
        editor
            .document()
            .node(&NodeId::from("note"))
            .unwrap()
            .data
            .get("title"),
        Some(&json!("Migrated"))
    );
    assert_eq!(editor.history().undo_depth(), 0);
    assert!(
        editor
            .runtime()
            .hit_test(point(px(10.0), px(10.0)), HitOptions::default())
            .next()
            .is_some()
    );
}

#[test]
fn editor_set_kind_registry_rejects_invalid_existing_document_atomically() {
    let mut note = CanvasNode::new("note", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    note.kind = "note".to_string();
    note.data.insert("title".to_string(), json!(false));
    let document = document_fixture().node(note).build();
    let mut editor = CanvasEditor::new(document);

    let mut registry = CanvasKindRegistry::open();
    registry.register_node_kind("note", required_title_node_kind());
    let err = editor.set_kind_registry(registry).unwrap_err();

    assert!(matches!(
        err,
        DocumentError::Schema(CanvasSchemaError::InvalidData {
            record_id: crate::CanvasRecordId::Node(id),
            ..
        }) if id == NodeId::from("note")
    ));
    assert_eq!(
        editor
            .document()
            .node(&NodeId::from("note"))
            .unwrap()
            .data
            .get("title"),
        Some(&json!(false))
    );
    assert!(editor.kind_registry().node_kind("note").is_none());
}

#[test]
fn tool_effect_applies_recorded_transaction() {
    let mut editor = CanvasEditor::default();

    editor
        .apply_tool_effect(CanvasToolEffect::ApplyTransaction(
            CanvasTransaction::single(DocumentCommand::InsertNode(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))),
        ))
        .unwrap();

    assert!(editor.document().contains_node(&NodeId::from("a")));
    assert_eq!(editor.history().undo_depth(), 1);
    assert!(
        editor
            .runtime()
            .hit_test(point(px(10.0), px(10.0)), HitOptions::default())
            .next()
            .is_some()
    );
}

#[test]
fn tool_effect_updates_gesture_without_history() {
    let mut editor = CanvasEditor::default();

    editor
        .apply_tool_effect(CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
            DocumentCommand::InsertNode(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            )),
        )))
        .unwrap();

    assert!(editor.document().contains_node(&NodeId::from("a")));
    assert_eq!(editor.history().undo_depth(), 0);
    assert!(
        editor
            .runtime()
            .hit_test(point(px(10.0), px(10.0)), HitOptions::default())
            .next()
            .is_some()
    );
}

#[test]
fn gesture_update_uses_kind_registry_validation() {
    let mut registry = CanvasKindRegistry::open();
    registry.register_node_kind("note", required_title_node_kind());
    let mut editor =
        CanvasEditor::try_new_with_kind_registry(document_fixture().build(), registry).unwrap();
    let mut note = CanvasNode::new("note", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    note.kind = "note".to_string();
    note.data.insert("title".to_string(), json!("Valid"));
    editor
        .apply(DocumentCommand::InsertNode(note.clone()))
        .unwrap();

    let mut invalid = note.clone();
    invalid.data.insert("title".to_string(), json!(false));
    let err = editor
        .apply_tool_effects([
            CanvasToolEffect::BeginGesture,
            CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                DocumentCommand::UpdateNode(invalid),
            )),
        ])
        .unwrap_err();

    assert!(matches!(
        err,
        DocumentError::Schema(CanvasSchemaError::InvalidData {
            record_id: crate::CanvasRecordId::Node(id),
            ..
        }) if id == NodeId::from("note")
    ));
    assert_eq!(
        editor.document().node(&NodeId::from("note")).unwrap(),
        &note
    );
    assert_eq!(editor.history().undo_depth(), 1);
}

#[test]
fn gesture_commit_pushes_one_undo_entry() {
    let mut editor = CanvasEditor::default();
    let original = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    let first = CanvasNode::new("a", point(px(12.0), px(0.0)), size(px(100.0), px(100.0)));
    let second = CanvasNode::new("a", point(px(40.0), px(0.0)), size(px(100.0), px(100.0)));
    editor
        .apply(DocumentCommand::InsertNode(original.clone()))
        .unwrap();

    editor
        .apply_tool_effects([
            CanvasToolEffect::BeginGesture,
            CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                DocumentCommand::UpdateNode(first),
            )),
            CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                DocumentCommand::UpdateNode(second.clone()),
            )),
            CanvasToolEffect::CommitGesture,
        ])
        .unwrap();

    assert_eq!(editor.document().node(&NodeId::from("a")).unwrap(), &second);
    assert_eq!(editor.history().undo_depth(), 2);
    assert!(editor.undo().unwrap());
    assert_eq!(
        editor.document().node(&NodeId::from("a")).unwrap(),
        &original
    );
}

#[test]
fn gesture_updates_notify_listeners_only_on_commit() {
    let mut editor = CanvasEditor::default();
    let original = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    let moved = CanvasNode::new("a", point(px(40.0), px(0.0)), size(px(100.0), px(100.0)));
    editor
        .apply(DocumentCommand::InsertNode(original.clone()))
        .unwrap();
    let baseline_depth = editor.history().undo_depth();
    let changes = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&changes);
    editor.listen(move |change| observed.lock().unwrap().push(change.clone()));

    editor
        .apply_tool_effects([
            CanvasToolEffect::BeginGesture,
            CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                DocumentCommand::UpdateNode(moved.clone()),
            )),
        ])
        .unwrap();

    assert!(changes.lock().unwrap().is_empty());
    assert_eq!(editor.history().undo_depth(), baseline_depth);

    editor
        .apply_tool_effect(CanvasToolEffect::CommitGesture)
        .unwrap();

    let changes = changes.lock().unwrap();
    assert_eq!(changes.len(), 1);
    let change = &changes[0];
    assert_eq!(change.source(), crate::CanvasStoreMutationSource::Gesture);
    assert_eq!(
        change.history_effect(),
        crate::CanvasStoreHistoryEffect::PushUndo
    );
    assert_eq!(change.document().node(&NodeId::from("a")).unwrap(), &moved);
    assert_eq!(editor.history().undo_depth(), baseline_depth + 1);
}

#[test]
fn gesture_commit_records_relation_updates() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "child",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .shape(CanvasShape::new(
            "frame",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        ))
        .build();
    let child = CanvasRecordId::Node(NodeId::from("child"));
    let frame = CanvasRecordId::Shape(ShapeId::from("frame"));
    let mut editor = CanvasEditor::new(document);

    editor
        .apply_tool_effects([
            CanvasToolEffect::BeginGesture,
            CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                DocumentCommand::SetRecordParent {
                    child: child.clone(),
                    parent: frame.clone(),
                },
            )),
            CanvasToolEffect::CommitGesture,
        ])
        .unwrap();

    assert_eq!(
        editor.document().relations().parent_of(&child),
        Some(&frame)
    );
    assert_eq!(editor.history().undo_depth(), 1);

    assert!(editor.undo().unwrap());
    assert_eq!(editor.document().relations().parent_of(&child), None);
}

#[test]
fn empty_gesture_commit_does_not_push_history() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "child",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);

    editor
        .apply_tool_effects([
            CanvasToolEffect::BeginGesture,
            CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                DocumentCommand::ClearRecordParent {
                    child: CanvasRecordId::Node(NodeId::from("child")),
                },
            )),
        ])
        .unwrap();

    editor
        .apply_tool_effect(CanvasToolEffect::CommitGesture)
        .unwrap();

    assert_eq!(editor.history().undo_depth(), 0);
    assert!(editor.is_tool_state_idle());
}

#[test]
fn set_tool_cancels_active_gesture_before_switching() {
    let mut editor = CanvasEditor::default();
    let original = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    let moved = CanvasNode::new("a", point(px(400.0), px(0.0)), size(px(100.0), px(100.0)));
    editor
        .apply(DocumentCommand::InsertNode(original.clone()))
        .unwrap();

    editor
        .apply_tool_effects([
            CanvasToolEffect::BeginGesture,
            CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                DocumentCommand::UpdateNode(moved),
            )),
        ])
        .unwrap();
    assert_eq!(
        editor.document().node(&NodeId::from("a")).unwrap().position,
        point(px(400.0), px(0.0))
    );

    editor.set_tool(CanvasTool::Pan).unwrap();

    assert_eq!(editor.tool(), &CanvasTool::Pan);
    assert_eq!(
        editor.document().node(&NodeId::from("a")).unwrap(),
        &original
    );
    assert_eq!(editor.history().undo_depth(), 1);
    assert!(
        editor
            .runtime()
            .hit_test(point(px(10.0), px(10.0)), HitOptions::default())
            .any(|record| record.target == HitTarget::Node(NodeId::from("a")))
    );
    assert!(
        editor
            .runtime()
            .hit_test(point(px(410.0), px(10.0)), HitOptions::default())
            .next()
            .is_none()
    );
}

#[test]
fn begin_gesture_preserves_existing_baseline() {
    let mut editor = CanvasEditor::default();
    let original = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    let first = CanvasNode::new("a", point(px(100.0), px(0.0)), size(px(100.0), px(100.0)));
    let second = CanvasNode::new("a", point(px(200.0), px(0.0)), size(px(100.0), px(100.0)));
    editor
        .apply(DocumentCommand::InsertNode(original.clone()))
        .unwrap();

    editor
        .apply_tool_effects([
            CanvasToolEffect::BeginGesture,
            CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                DocumentCommand::UpdateNode(first),
            )),
            CanvasToolEffect::BeginGesture,
            CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                DocumentCommand::UpdateNode(second),
            )),
            CanvasToolEffect::CancelGesture,
        ])
        .unwrap();

    assert_eq!(
        editor.document().node(&NodeId::from("a")).unwrap(),
        &original
    );
    assert_eq!(editor.history().undo_depth(), 1);
}

#[test]
fn public_tool_intents_commit_transaction_as_one_undo_entry() {
    let mut editor = CanvasEditor::default();
    let original = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    let first = CanvasNode::new("a", point(px(100.0), px(0.0)), size(px(100.0), px(100.0)));
    let second = CanvasNode::new("a", point(px(200.0), px(0.0)), size(px(100.0), px(100.0)));
    editor
        .apply(DocumentCommand::InsertNode(original.clone()))
        .unwrap();
    let baseline_depth = editor.history().undo_depth();

    for intent in [
        CanvasToolIntent::ApplyTransaction(CanvasTransaction::single(DocumentCommand::UpdateNode(
            first,
        ))),
        CanvasToolIntent::ApplyTransaction(CanvasTransaction::single(DocumentCommand::UpdateNode(
            second.clone(),
        ))),
        CanvasToolIntent::CommitTransaction,
    ] {
        editor.apply_custom_tool_intent(intent).unwrap();
    }

    assert_eq!(editor.document().node(&NodeId::from("a")).unwrap(), &second);
    assert_eq!(editor.history().undo_depth(), baseline_depth + 1);
    assert!(editor.undo().unwrap());
    assert_eq!(
        editor.document().node(&NodeId::from("a")).unwrap(),
        &original
    );
}

#[test]
fn public_tool_intents_cancel_transaction_without_history() {
    let mut editor = CanvasEditor::default();
    let original = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    let moved = CanvasNode::new("a", point(px(100.0), px(0.0)), size(px(100.0), px(100.0)));
    editor
        .apply(DocumentCommand::InsertNode(original.clone()))
        .unwrap();
    let baseline_depth = editor.history().undo_depth();

    for intent in [
        CanvasToolIntent::ApplyTransaction(CanvasTransaction::single(DocumentCommand::UpdateNode(
            moved,
        ))),
        CanvasToolIntent::CancelTransaction,
    ] {
        editor.apply_custom_tool_intent(intent).unwrap();
    }

    assert_eq!(
        editor.document().node(&NodeId::from("a")).unwrap(),
        &original
    );
    assert_eq!(editor.history().undo_depth(), baseline_depth);
}

#[test]
fn gesture_cancel_restores_document_without_history() {
    let mut editor = CanvasEditor::default();
    let original = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    let moved = CanvasNode::new("a", point(px(40.0), px(0.0)), size(px(100.0), px(100.0)));
    editor
        .apply(DocumentCommand::InsertNode(original.clone()))
        .unwrap();
    let undo_depth = editor.history().undo_depth();

    editor
        .apply_tool_effects([
            CanvasToolEffect::BeginGesture,
            CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                DocumentCommand::UpdateNode(moved),
            )),
            CanvasToolEffect::CancelGesture,
        ])
        .unwrap();

    assert_eq!(
        editor.document().node(&NodeId::from("a")).unwrap(),
        &original
    );
    assert_eq!(editor.history().undo_depth(), undo_depth);
}

#[test]
fn tool_effects_update_transient_editor_state() {
    let mut editor = CanvasEditor::default();
    editor
        .apply(DocumentCommand::InsertNode(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        )))
        .unwrap();

    let mut selection = CanvasSelection::default();
    selection.nodes.insert(NodeId::from("a"));
    selection.nodes.insert(NodeId::from("missing"));

    editor
        .apply_tool_effects([
            CanvasToolEffect::SetSelection(selection),
            CanvasToolEffect::SetState(ToolState::Pointing {
                origin: point(px(10.0), px(20.0)),
                selection_mode: CanvasSelectionMode::Replace,
                base_selection: CanvasSelection::default(),
            }),
            CanvasToolEffect::PanViewport(point(px(5.0), px(-3.0))),
        ])
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
    assert_eq!(
        editor.session.state,
        ToolState::Pointing {
            origin: point(px(10.0), px(20.0)),
            selection_mode: CanvasSelectionMode::Replace,
            base_selection: CanvasSelection::default(),
        }
    );
    assert_eq!(editor.session.viewport.origin, point(px(5.0), px(-3.0)));
}

#[test]
fn tool_effects_update_selection_incrementally() {
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

    editor
        .apply_tool_effects([
            CanvasToolEffect::AddSelection(HitTarget::Node(NodeId::from("a"))),
            CanvasToolEffect::ToggleSelection(HitTarget::Shape(ShapeId::from("shape"))),
            CanvasToolEffect::ToggleSelection(HitTarget::Edge(EdgeId::from("a-b"))),
            CanvasToolEffect::RemoveSelection(HitTarget::Node(NodeId::from("a"))),
            CanvasToolEffect::ToggleSelection(HitTarget::Shape(ShapeId::from("shape"))),
            CanvasToolEffect::AddSelection(HitTarget::Node(NodeId::from("missing"))),
        ])
        .unwrap();

    assert!(editor.session.selection.nodes.is_empty());
    assert!(editor.session.selection.shapes.is_empty());
    assert_eq!(
        editor
            .session
            .selection
            .edges
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![EdgeId::from("a-b")]
    );
}

#[test]
fn editor_keeps_spatial_index_in_sync_with_transactions() {
    let mut editor = CanvasEditor::default();
    editor
        .apply(DocumentCommand::InsertNode(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        )))
        .unwrap();

    assert!(
        editor
            .runtime()
            .hit_test(point(px(10.0), px(10.0)), HitOptions::default())
            .next()
            .is_some()
    );

    assert!(editor.undo().unwrap());
    assert!(
        editor
            .runtime()
            .hit_test(point(px(10.0), px(10.0)), HitOptions::default())
            .next()
            .is_none()
    );
}

#[test]
fn editor_refreshes_runtime_geometry_with_installed_router() {
    let mut editor = CanvasEditor::new_with_router(connected_edge_document(), VerticalDetourRouter);

    assert_eq!(
        editor
            .runtime()
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

    let mut target = editor.document().node(&NodeId::from("b")).unwrap().clone();
    target.position = point(px(40.0), px(0.0));
    editor.apply(DocumentCommand::UpdateNode(target)).unwrap();

    assert_eq!(
        editor
            .runtime()
            .edge_geometry(&EdgeId::from("a-b"))
            .unwrap()
            .path
            .document_points(),
        vec![
            point(px(5.0), px(5.0)),
            point(px(5.0), px(80.0)),
            point(px(45.0), px(5.0)),
        ]
    );
}
