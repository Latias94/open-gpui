use super::*;
use crate::{
    CanvasDocument, CanvasEdge, CanvasEditor, CanvasEndpoint, CanvasEvent, CanvasKeyModifiers,
    CanvasKindRegistry, CanvasNode, CanvasNodeKind, CanvasRecordChange, CanvasRecordId,
    CanvasRecordKind, CanvasSchemaError, CanvasSelection, CanvasTool, CanvasToolContext,
    CanvasToolEffect, CanvasToolId, CanvasToolReducer, CanvasToolRegistry, CanvasTransaction,
    DocumentCommand, DocumentError, EdgeId, NodeId, PointerButton, ToolState,
};
use open_gpui::{point, px, size};
use serde_json::json;
use std::fmt;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Default)]
struct PersistentStampTool {
    calls: usize,
}

struct CountedNodeKind {
    calls: Arc<AtomicUsize>,
    max_successful_validations: usize,
}

impl CanvasNodeKind for CountedNodeKind {
    fn validate_node(&self, node: &CanvasNode) -> Result<(), CanvasSchemaError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if call <= self.max_successful_validations {
            Ok(())
        } else {
            Err(CanvasSchemaError::invalid_data(
                CanvasRecordKind::Node,
                node.id.clone(),
                &node.kind,
                "validation limit exceeded",
            ))
        }
    }
}

impl CanvasToolReducer for PersistentStampTool {
    fn handle_event(
        &mut self,
        context: CanvasToolContext<'_>,
        event: CanvasEvent,
    ) -> Result<Vec<CanvasToolEffect>, DocumentError> {
        self.calls += 1;

        let CanvasEvent::PointerDown {
            position,
            button: PointerButton::Primary,
            ..
        } = event
        else {
            return Ok(Vec::new());
        };

        let node_id = NodeId::new(format!(
            "persistent-stamp-{}",
            context.document.node_count()
        ));
        Ok(vec![
            CanvasToolEffect::ApplyTransaction(CanvasTransaction::single(
                DocumentCommand::InsertNode(CanvasNode::new(
                    node_id,
                    context.document_position(position),
                    size(px(24.0), px(24.0)),
                )),
            )),
            CanvasToolEffect::SetState(ToolState::Idle),
        ])
    }
}

#[test]
fn persistence_adapter_statuses_describe_default_and_future_adapters() {
    assert_eq!(
        CANVAS_PERSISTENCE_ADAPTERS,
        [
            CanvasPersistenceAdapter::MemoryStore,
            CanvasPersistenceAdapter::RedbStore,
            CanvasPersistenceAdapter::LoroCrdt,
            CanvasPersistenceAdapter::RkyvSnapshot,
        ]
    );

    let statuses = canvas_persistence_adapter_statuses();
    assert_eq!(
        statuses[0],
        CanvasPersistenceAdapterStatus {
            adapter: CanvasPersistenceAdapter::MemoryStore,
            feature_name: None,
            feature_enabled: true,
            implemented: true,
        }
    );
    assert_eq!(
        statuses[1],
        CanvasPersistenceAdapterStatus {
            adapter: CanvasPersistenceAdapter::RedbStore,
            feature_name: Some(CANVAS_REDB_STORE_FEATURE),
            feature_enabled: cfg!(feature = "redb-store"),
            implemented: false,
        }
    );
    assert_eq!(
        statuses[2],
        CanvasPersistenceAdapterStatus {
            adapter: CanvasPersistenceAdapter::LoroCrdt,
            feature_name: Some(CANVAS_LORO_CRDT_FEATURE),
            feature_enabled: cfg!(feature = "loro-crdt"),
            implemented: false,
        }
    );
    assert_eq!(
        statuses[3],
        CanvasPersistenceAdapterStatus {
            adapter: CanvasPersistenceAdapter::RkyvSnapshot,
            feature_name: Some(CANVAS_RKYV_SNAPSHOT_FEATURE),
            feature_enabled: cfg!(feature = "rkyv-snapshot"),
            implemented: false,
        }
    );
}

#[test]
fn replays_checkpoint_and_transaction_log() {
    let mut document = CanvasDocument::default();
    document
        .insert_node(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .unwrap();
    let checkpoint = CanvasCheckpoint::new(1, &document);
    let log_entry = CanvasLogEntry::new(
        2,
        DocumentCommand::InsertNode(CanvasNode::new(
            "b",
            point(px(20.0), px(0.0)),
            size(px(10.0), px(10.0)),
        )),
    );

    let restored = replay_canvas_log(Some(checkpoint), [log_entry]).unwrap();

    assert!(restored.contains_node(&NodeId::from("a")));
    assert!(restored.contains_node(&NodeId::from("b")));
}

#[test]
fn log_entries_expose_record_operation_batches() {
    let entry = CanvasLogEntry::new(
        9,
        DocumentCommand::InsertNode(CanvasNode::new(
            "node",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        )),
    );

    let batch = entry.record_operation_batch();

    assert_eq!(batch.transaction_sequence, 9);
    assert_eq!(batch.operations.len(), 1);
    assert_eq!(batch.operations[0].transaction_sequence, 9);
    assert_eq!(batch.operations[0].operation_index, 0);
    assert!(matches!(
        &batch.operations[0].change,
        CanvasRecordChange::Upsert(record)
            if record.id() == CanvasRecordId::Node(NodeId::from("node"))
    ));
}

#[test]
fn committed_log_entries_expose_actual_record_operation_batches() {
    let mut document = CanvasDocument::default();
    document
        .insert_node(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .unwrap();
    document
        .insert_node(CanvasNode::new(
            "b",
            point(px(20.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .unwrap();
    document
        .insert_edge(CanvasEdge::new(
            "a-b",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        ))
        .unwrap();
    let mut transaction = CanvasTransaction::single(DocumentCommand::RemoveNode(NodeId::from("a")));
    transaction
        .metadata
        .insert("reason".into(), serde_json::json!("delete-node"));
    let committed = document.commit_transaction(transaction).unwrap();
    let entry = CanvasLogEntry::from_committed_mutation(11, &committed);

    let batch = entry.record_operation_batch();

    assert_eq!(batch.transaction_sequence, 11);
    assert_eq!(
        batch.transaction_metadata.get("reason"),
        Some(&serde_json::json!("delete-node"))
    );
    assert_eq!(
        batch
            .operations
            .iter()
            .map(|operation| (operation.operation_index, operation.id()))
            .collect::<Vec<_>>(),
        vec![
            (0, CanvasRecordId::Node(NodeId::from("a"))),
            (1, CanvasRecordId::Edge(EdgeId::from("a-b"))),
        ]
    );
}

#[test]
fn rejects_non_monotonic_log_sequences() {
    let mut document = CanvasDocument::default();
    document
        .insert_node(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .unwrap();
    let checkpoint = CanvasCheckpoint::new(3, &document);
    let log_entry = CanvasLogEntry::new(
        3,
        DocumentCommand::InsertNode(CanvasNode::new(
            "b",
            point(px(20.0), px(0.0)),
            size(px(10.0), px(10.0)),
        )),
    );

    let err = replay_canvas_log(Some(checkpoint), [log_entry]).unwrap_err();

    assert_eq!(
        err,
        CanvasPersistenceError::NonMonotonicLogSequence {
            previous: 3,
            found: 3,
        }
    );
}

#[test]
fn loads_document_from_store_after_checkpoint_sequence() {
    let mut store = MemoryCanvasPersistenceStore::default();
    let mut document = CanvasDocument::default();
    document
        .insert_node(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .unwrap();

    store
        .save_checkpoint(CanvasCheckpoint::new(1, &document))
        .unwrap();
    store
        .append_log_entry(CanvasLogEntry::new(
            1,
            DocumentCommand::InsertNode(CanvasNode::new(
                "stale",
                point(px(20.0), px(0.0)),
                size(px(10.0), px(10.0)),
            )),
        ))
        .unwrap();
    store
        .append_log_entry(CanvasLogEntry::new(
            2,
            DocumentCommand::InsertNode(CanvasNode::new(
                "b",
                point(px(40.0), px(0.0)),
                size(px(10.0), px(10.0)),
            )),
        ))
        .unwrap();

    let restored = load_canvas_document(&store).unwrap();

    assert!(restored.contains_node(&NodeId::from("a")));
    assert!(!restored.contains_node(&NodeId::from("stale")));
    assert!(restored.contains_node(&NodeId::from("b")));
}

#[test]
fn compacts_log_entries_through_checkpoint_sequence() {
    let mut store = MemoryCanvasPersistenceStore::default();
    store
        .append_log_entry(CanvasLogEntry::new(1, CanvasTransaction::default()))
        .unwrap();
    store
        .append_log_entry(CanvasLogEntry::new(2, CanvasTransaction::default()))
        .unwrap();
    store
        .append_log_entry(CanvasLogEntry::new(3, CanvasTransaction::default()))
        .unwrap();

    store.compact_log_entries(2).unwrap();

    assert_eq!(
        store
            .log_entries()
            .iter()
            .map(|entry| entry.sequence)
            .collect::<Vec<_>>(),
        vec![3]
    );
}

#[test]
fn json_persistence_codec_round_trips_checkpoint_envelope() {
    let codec = CanvasJsonPersistenceCodec;
    let mut document = CanvasDocument::default();
    document
        .insert_node(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .unwrap();
    let checkpoint = CanvasCheckpoint::new(7, &document);

    let bytes = codec.encode_checkpoint(&checkpoint).unwrap();
    let envelope = codec.decode_envelope(&bytes).unwrap();
    let decoded = codec.decode_checkpoint(&bytes).unwrap();

    assert_eq!(envelope.codec_version, CANVAS_PERSISTENCE_CODEC_VERSION);
    assert_eq!(
        envelope.document_format_version,
        crate::CANVAS_DOCUMENT_FORMAT_VERSION
    );
    assert_eq!(envelope.kind(), CanvasPersistenceRecordKind::Checkpoint);
    assert_eq!(envelope.sequence(), 7);
    assert_eq!(decoded, checkpoint);
}

#[test]
fn json_persistence_codec_round_trips_log_entry_envelope() {
    let codec = CanvasJsonPersistenceCodec;
    let entry = CanvasLogEntry::new(
        3,
        DocumentCommand::InsertNode(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        )),
    );

    let bytes = codec.encode_log_entry(&entry).unwrap();
    let envelope = codec.decode_envelope(&bytes).unwrap();
    let decoded = codec.decode_log_entry(&bytes).unwrap();

    assert_eq!(envelope.kind(), CanvasPersistenceRecordKind::LogEntry);
    assert_eq!(envelope.sequence(), 3);
    assert_eq!(decoded, entry);
}

#[test]
fn json_persistence_codec_rejects_unsupported_codec_version() {
    let codec = CanvasJsonPersistenceCodec;
    let bytes = br#"{
            "codec_version": 999,
            "document_format_version": 1,
            "record": {
                "record_kind": "log_entry",
                "payload": {
                    "sequence": 1,
                    "transaction": {
                        "commands": [],
                        "metadata": {}
                    }
                }
            }
        }"#;

    let err = codec.decode_log_entry(bytes).unwrap_err();

    assert_eq!(
        err,
        CanvasPersistenceCodecError::UnsupportedCodecVersion {
            expected: CANVAS_PERSISTENCE_CODEC_VERSION,
            found: 999,
        }
    );
}

#[test]
fn json_persistence_codec_rejects_unsupported_document_format_version() {
    let codec = CanvasJsonPersistenceCodec;
    let bytes = br#"{
            "codec_version": 1,
            "document_format_version": 999,
            "record": {
                "record_kind": "log_entry",
                "payload": {
                    "sequence": 1,
                    "transaction": {
                        "commands": [],
                        "metadata": {}
                    }
                }
            }
        }"#;

    let err = codec.decode_log_entry(bytes).unwrap_err();

    assert_eq!(
        err,
        CanvasPersistenceCodecError::UnsupportedDocumentFormatVersion {
            expected: crate::CANVAS_DOCUMENT_FORMAT_VERSION,
            found: 999,
        }
    );
}

#[test]
fn json_persistence_codec_rejects_checkpoint_as_log_entry() {
    let codec = CanvasJsonPersistenceCodec;
    let checkpoint = CanvasCheckpoint::new(1, &CanvasDocument::default());
    let bytes = codec.encode_checkpoint(&checkpoint).unwrap();

    let err = codec.decode_log_entry(&bytes).unwrap_err();

    assert_eq!(
        err,
        CanvasPersistenceCodecError::UnexpectedRecordKind {
            expected: CanvasPersistenceRecordKind::LogEntry,
            found: CanvasPersistenceRecordKind::Checkpoint,
        }
    );
}

#[test]
fn byte_store_adapter_replays_encoded_checkpoint_and_log() {
    let mut typed_store =
        CanvasPersistenceByteStoreAdapter::new(MemoryCanvasPersistenceByteStore::default());
    let mut document = CanvasDocument::default();
    document
        .insert_node(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .unwrap();
    typed_store
        .save_checkpoint(CanvasCheckpoint::new(1, &document))
        .unwrap();
    typed_store
        .append_log_entry(CanvasLogEntry::new(
            2,
            DocumentCommand::InsertNode(CanvasNode::new(
                "b",
                point(px(20.0), px(0.0)),
                size(px(10.0), px(10.0)),
            )),
        ))
        .unwrap();

    let restored = load_canvas_document(&typed_store).unwrap();
    let byte_store = typed_store.store();

    assert!(restored.contains_node(&NodeId::from("a")));
    assert!(restored.contains_node(&NodeId::from("b")));
    assert!(byte_store.checkpoint_bytes().is_some());
    assert_eq!(byte_store.encoded_log_entries().len(), 1);
}

#[test]
fn byte_store_adapter_rejects_log_key_sequence_mismatch() {
    let codec = CanvasJsonPersistenceCodec;
    let entry = CanvasLogEntry::new(2, CanvasTransaction::default());
    let mut byte_store = MemoryCanvasPersistenceByteStore::default();
    byte_store
        .append_log_entry_bytes(9, codec.encode_log_entry(&entry).unwrap())
        .unwrap();
    let typed_store = CanvasPersistenceByteStoreAdapter::new(byte_store);

    let err = typed_store.load_log_entries(0).unwrap_err();

    assert_eq!(
        err,
        CanvasPersistenceByteStoreError::LogSequenceMismatch { key: 9, payload: 2 }
    );
}

#[test]
fn loads_persistence_cursor_from_checkpoint_and_log_tail() {
    let mut store = MemoryCanvasPersistenceStore::default();
    let document = CanvasDocument::default();
    store
        .save_checkpoint(CanvasCheckpoint::new(3, &document))
        .unwrap();
    store
        .append_log_entry(CanvasLogEntry::new(4, CanvasTransaction::default()))
        .unwrap();
    store
        .append_log_entry(CanvasLogEntry::new(5, CanvasTransaction::default()))
        .unwrap();

    let cursor = load_canvas_persistence_cursor(&store).unwrap();

    assert_eq!(cursor.sequence(), 5);
    assert_eq!(cursor.next_sequence(), 6);
}

#[test]
fn persistent_transaction_appends_successful_editor_transaction() {
    let mut editor = CanvasEditor::default();
    let mut store = MemoryCanvasPersistenceStore::default();
    let mut cursor = CanvasPersistenceCursor::default();
    let transaction = CanvasTransaction::single(DocumentCommand::InsertNode(CanvasNode::new(
        "a",
        point(px(0.0), px(0.0)),
        size(px(10.0), px(10.0)),
    )));

    let diff =
        apply_persistent_transaction(&mut editor, &mut store, &mut cursor, transaction.clone())
            .unwrap();

    assert!(editor.document().contains_node(&NodeId::from("a")));
    assert_eq!(
        diff.inserted.iter().cloned().collect::<Vec<_>>(),
        vec![CanvasRecordId::Node(NodeId::from("a"))]
    );
    assert_eq!(cursor.sequence(), 1);
    assert_eq!(store.log_entries().len(), 1);
    assert_eq!(store.log_entries()[0].sequence, 1);
    assert_eq!(store.log_entries()[0].transaction, transaction);
    assert_eq!(
        store.log_entries()[0]
            .record_operation_batch()
            .operations
            .iter()
            .map(|operation| operation.id())
            .collect::<Vec<_>>(),
        vec![CanvasRecordId::Node(NodeId::from("a"))]
    );

    let restored = load_canvas_document(&store).unwrap();
    assert!(restored.contains_node(&NodeId::from("a")));
}

#[test]
fn persistent_transaction_skips_empty_transactions() {
    let mut editor = CanvasEditor::default();
    let mut store = MemoryCanvasPersistenceStore::default();
    let mut cursor = CanvasPersistenceCursor::new(7);

    let diff = apply_persistent_transaction(
        &mut editor,
        &mut store,
        &mut cursor,
        CanvasTransaction::default(),
    )
    .unwrap();

    assert!(diff.is_empty());
    assert_eq!(cursor.sequence(), 7);
    assert!(store.log_entries().is_empty());
}

#[test]
fn persistent_transaction_does_not_log_document_failure() {
    let mut editor = CanvasEditor::default();
    let mut store = MemoryCanvasPersistenceStore::default();
    let mut cursor = CanvasPersistenceCursor::default();
    let transaction =
        CanvasTransaction::single(DocumentCommand::RemoveNode(NodeId::from("missing")));

    let err = apply_persistent_transaction(&mut editor, &mut store, &mut cursor, transaction)
        .unwrap_err();

    assert_eq!(
        err,
        CanvasPersistenceError::Document(DocumentError::MissingNode(NodeId::from("missing")))
    );
    assert_eq!(cursor.sequence(), 0);
    assert!(store.log_entries().is_empty());
    assert!(editor.document().node_count() == 0);
}

#[test]
fn persistent_transaction_does_not_mutate_editor_when_store_fails() {
    #[derive(Debug, Eq, PartialEq)]
    struct StoreFailure;

    impl fmt::Display for StoreFailure {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("store failure")
        }
    }

    struct FailingStore;

    impl CanvasPersistenceStore for FailingStore {
        type Error = StoreFailure;

        fn load_checkpoint(&self) -> Result<Option<CanvasCheckpoint>, Self::Error> {
            Ok(None)
        }

        fn save_checkpoint(&mut self, _: CanvasCheckpoint) -> Result<(), Self::Error> {
            Err(StoreFailure)
        }

        fn append_log_entry(&mut self, _: CanvasLogEntry) -> Result<(), Self::Error> {
            Err(StoreFailure)
        }

        fn load_log_entries(&self, _: u64) -> Result<Vec<CanvasLogEntry>, Self::Error> {
            Ok(Vec::new())
        }

        fn compact_log_entries(&mut self, _: u64) -> Result<(), Self::Error> {
            Err(StoreFailure)
        }
    }

    let mut editor = CanvasEditor::default();
    let mut store = FailingStore;
    let mut cursor = CanvasPersistenceCursor::default();
    let transaction = CanvasTransaction::single(DocumentCommand::InsertNode(CanvasNode::new(
        "a",
        point(px(0.0), px(0.0)),
        size(px(10.0), px(10.0)),
    )));

    let err = apply_persistent_transaction(&mut editor, &mut store, &mut cursor, transaction)
        .unwrap_err();

    assert_eq!(err, CanvasPersistenceError::Store(StoreFailure));
    assert_eq!(cursor.sequence(), 0);
    assert!(editor.document().node_count() == 0);
    assert_eq!(editor.history().undo_depth(), 0);
}

#[test]
fn persistent_undo_logs_inverse_transaction_before_mutation() {
    let mut editor = CanvasEditor::default();
    let mut store = MemoryCanvasPersistenceStore::default();
    let mut cursor = CanvasPersistenceCursor::default();
    apply_persistent_transaction(
        &mut editor,
        &mut store,
        &mut cursor,
        CanvasTransaction::single(DocumentCommand::InsertNode(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))),
    )
    .unwrap();

    let changed = undo_persistent_transaction(&mut editor, &mut store, &mut cursor).unwrap();

    assert!(changed);
    assert_eq!(cursor.sequence(), 2);
    assert!(!editor.document().contains_node(&NodeId::from("a")));
    assert_eq!(editor.history().undo_depth(), 0);
    assert_eq!(editor.history().redo_depth(), 1);
    assert_eq!(store.log_entries().len(), 2);
    assert!(matches!(
        store.log_entries()[1].transaction.commands.as_slice(),
        [DocumentCommand::RemoveNode(id)] if id == &NodeId::from("a")
    ));

    let restored = load_canvas_document(&store).unwrap();
    assert!(!restored.contains_node(&NodeId::from("a")));
}

#[test]
fn persistent_redo_logs_redo_transaction_before_mutation() {
    let mut editor = CanvasEditor::default();
    let mut store = MemoryCanvasPersistenceStore::default();
    let mut cursor = CanvasPersistenceCursor::default();
    let node = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
    apply_persistent_transaction(
        &mut editor,
        &mut store,
        &mut cursor,
        CanvasTransaction::single(DocumentCommand::InsertNode(node.clone())),
    )
    .unwrap();
    undo_persistent_transaction(&mut editor, &mut store, &mut cursor).unwrap();

    let changed = redo_persistent_transaction(&mut editor, &mut store, &mut cursor).unwrap();

    assert!(changed);
    assert_eq!(cursor.sequence(), 3);
    assert!(editor.document().contains_node(&NodeId::from("a")));
    assert_eq!(editor.history().undo_depth(), 1);
    assert_eq!(editor.history().redo_depth(), 0);
    assert_eq!(store.log_entries().len(), 3);
    assert!(matches!(
        store.log_entries()[2].transaction.commands.as_slice(),
        [DocumentCommand::InsertNode(inserted)] if inserted == &node
    ));

    let restored = load_canvas_document(&store).unwrap();
    assert!(restored.contains_node(&NodeId::from("a")));
}

#[test]
fn persistent_redo_reuses_prepared_mutation_after_logging() {
    let mut editor = CanvasEditor::default();
    let mut store = MemoryCanvasPersistenceStore::default();
    let mut cursor = CanvasPersistenceCursor::default();
    let mut node = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
    node.kind = "counted".to_string();
    node.data.insert("marker".to_string(), json!(true));

    apply_persistent_transaction(
        &mut editor,
        &mut store,
        &mut cursor,
        CanvasTransaction::single(DocumentCommand::InsertNode(node)),
    )
    .unwrap();
    undo_persistent_transaction(&mut editor, &mut store, &mut cursor).unwrap();

    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = CanvasKindRegistry::open();
    registry.register_node_kind(
        "counted",
        CountedNodeKind {
            calls: calls.clone(),
            max_successful_validations: 2,
        },
    );
    editor.set_kind_registry(registry).unwrap();

    let changed = redo_persistent_transaction(&mut editor, &mut store, &mut cursor).unwrap();

    assert!(changed);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(cursor.sequence(), 3);
    assert_eq!(store.log_entries().len(), 3);
    assert!(editor.document().contains_node(&NodeId::from("a")));
    assert_eq!(editor.history().undo_depth(), 1);
    assert_eq!(editor.history().redo_depth(), 0);
}

#[test]
fn persistent_undo_and_redo_skip_empty_history() {
    let mut editor = CanvasEditor::default();
    let mut store = MemoryCanvasPersistenceStore::default();
    let mut cursor = CanvasPersistenceCursor::new(4);

    assert!(!undo_persistent_transaction(&mut editor, &mut store, &mut cursor).unwrap());
    assert!(!redo_persistent_transaction(&mut editor, &mut store, &mut cursor).unwrap());

    assert_eq!(cursor.sequence(), 4);
    assert!(store.log_entries().is_empty());
    assert!(editor.document().node_count() == 0);
}

#[test]
fn persistent_undo_does_not_mutate_editor_when_store_fails() {
    #[derive(Debug, Eq, PartialEq)]
    struct StoreFailure;

    impl fmt::Display for StoreFailure {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("store failure")
        }
    }

    struct FailingAppendStore;

    impl CanvasPersistenceStore for FailingAppendStore {
        type Error = StoreFailure;

        fn load_checkpoint(&self) -> Result<Option<CanvasCheckpoint>, Self::Error> {
            Ok(None)
        }

        fn save_checkpoint(&mut self, _: CanvasCheckpoint) -> Result<(), Self::Error> {
            Ok(())
        }

        fn append_log_entry(&mut self, _: CanvasLogEntry) -> Result<(), Self::Error> {
            Err(StoreFailure)
        }

        fn load_log_entries(&self, _: u64) -> Result<Vec<CanvasLogEntry>, Self::Error> {
            Ok(Vec::new())
        }

        fn compact_log_entries(&mut self, _: u64) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    let mut editor = CanvasEditor::default();
    editor
        .apply_transaction(CanvasTransaction::single(DocumentCommand::InsertNode(
            CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(10.0), px(10.0))),
        )))
        .unwrap();
    let mut store = FailingAppendStore;
    let mut cursor = CanvasPersistenceCursor::new(9);

    let err = undo_persistent_transaction(&mut editor, &mut store, &mut cursor).unwrap_err();

    assert_eq!(err, CanvasPersistenceError::Store(StoreFailure));
    assert_eq!(cursor.sequence(), 9);
    assert!(editor.document().contains_node(&NodeId::from("a")));
    assert_eq!(editor.history().undo_depth(), 1);
    assert_eq!(editor.history().redo_depth(), 0);
}

#[test]
fn save_checkpoint_persists_editor_snapshot_and_compacts_log() {
    let mut editor = CanvasEditor::default();
    let mut store = MemoryCanvasPersistenceStore::default();
    let mut cursor = CanvasPersistenceCursor::default();

    apply_persistent_transaction(
        &mut editor,
        &mut store,
        &mut cursor,
        CanvasTransaction::single(DocumentCommand::InsertNode(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))),
    )
    .unwrap();
    apply_persistent_transaction(
        &mut editor,
        &mut store,
        &mut cursor,
        CanvasTransaction::single(DocumentCommand::InsertNode(CanvasNode::new(
            "b",
            point(px(20.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))),
    )
    .unwrap();

    let checkpoint = save_canvas_checkpoint(&editor, &mut store, &cursor).unwrap();

    assert_eq!(checkpoint.sequence, 2);
    assert_eq!(cursor.sequence(), 2);
    assert!(store.log_entries().is_empty());
    assert_eq!(store.checkpoint().unwrap(), &checkpoint);

    let restored = load_canvas_document(&store).unwrap();
    assert!(restored.contains_node(&NodeId::from("a")));
    assert!(restored.contains_node(&NodeId::from("b")));
}

#[test]
fn save_checkpoint_reports_store_failure() {
    #[derive(Debug, Eq, PartialEq)]
    struct StoreFailure;

    impl fmt::Display for StoreFailure {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("store failure")
        }
    }

    struct FailingCheckpointStore;

    impl CanvasPersistenceStore for FailingCheckpointStore {
        type Error = StoreFailure;

        fn load_checkpoint(&self) -> Result<Option<CanvasCheckpoint>, Self::Error> {
            Ok(None)
        }

        fn save_checkpoint(&mut self, _: CanvasCheckpoint) -> Result<(), Self::Error> {
            Err(StoreFailure)
        }

        fn append_log_entry(&mut self, _: CanvasLogEntry) -> Result<(), Self::Error> {
            Ok(())
        }

        fn load_log_entries(&self, _: u64) -> Result<Vec<CanvasLogEntry>, Self::Error> {
            Ok(Vec::new())
        }

        fn compact_log_entries(&mut self, _: u64) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    let editor = CanvasEditor::default();
    let mut store = FailingCheckpointStore;
    let cursor = CanvasPersistenceCursor::new(3);

    let err = save_canvas_checkpoint(&editor, &mut store, &cursor).unwrap_err();

    assert_eq!(err, CanvasPersistenceError::Store(StoreFailure));
}

#[test]
fn persistent_tool_effects_log_recorded_transactions() {
    let mut editor = CanvasEditor::default();
    let mut store = MemoryCanvasPersistenceStore::default();
    let mut cursor = CanvasPersistenceCursor::default();

    apply_persistent_tool_effects(
        &mut editor,
        &mut store,
        &mut cursor,
        [
            CanvasToolEffect::ApplyTransaction(CanvasTransaction::single(
                DocumentCommand::InsertNode(CanvasNode::new(
                    "a",
                    point(px(0.0), px(0.0)),
                    size(px(10.0), px(10.0)),
                )),
            )),
            CanvasToolEffect::SetSelection({
                let mut selection = CanvasSelection::default();
                selection.nodes.insert(NodeId::from("a"));
                selection
            }),
        ],
    )
    .unwrap();

    assert_eq!(cursor.sequence(), 1);
    assert_eq!(store.log_entries().len(), 1);
    assert!(editor.selection().nodes.contains(&NodeId::from("a")));
    let restored = load_canvas_document(&store).unwrap();
    assert!(restored.contains_node(&NodeId::from("a")));
}

#[test]
fn persistent_tool_effects_commit_gesture_as_one_log_entry() {
    let mut document = CanvasDocument::default();
    let original = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
    document.insert_node(original.clone()).unwrap();

    let mut editor = CanvasEditor::new(document.clone());
    let mut store = MemoryCanvasPersistenceStore::default();
    store
        .save_checkpoint(CanvasCheckpoint::new(0, &document))
        .unwrap();
    let mut cursor = CanvasPersistenceCursor::default();

    let moved = CanvasNode::new("a", point(px(40.0), px(0.0)), size(px(10.0), px(10.0)));

    apply_persistent_tool_effects(
        &mut editor,
        &mut store,
        &mut cursor,
        [
            CanvasToolEffect::BeginGesture,
            CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                DocumentCommand::UpdateNode(moved.clone()),
            )),
            CanvasToolEffect::CommitGesture,
        ],
    )
    .unwrap();

    assert_eq!(cursor.sequence(), 1);
    assert_eq!(store.log_entries().len(), 1);
    assert_eq!(editor.history().undo_depth(), 1);
    assert_eq!(editor.document().node(&NodeId::from("a")).unwrap(), &moved);

    let restored = load_canvas_document(&store).unwrap();
    assert_eq!(
        restored.node(&NodeId::from("a")).unwrap().position,
        point(px(40.0), px(0.0))
    );
}

#[test]
fn persistent_gesture_commit_does_not_finish_when_store_fails() {
    #[derive(Debug, Eq, PartialEq)]
    struct StoreFailure;

    impl fmt::Display for StoreFailure {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("store failure")
        }
    }

    struct FailingAppendStore;

    impl CanvasPersistenceStore for FailingAppendStore {
        type Error = StoreFailure;

        fn load_checkpoint(&self) -> Result<Option<CanvasCheckpoint>, Self::Error> {
            Ok(None)
        }

        fn save_checkpoint(&mut self, _: CanvasCheckpoint) -> Result<(), Self::Error> {
            Ok(())
        }

        fn append_log_entry(&mut self, _: CanvasLogEntry) -> Result<(), Self::Error> {
            Err(StoreFailure)
        }

        fn load_log_entries(&self, _: u64) -> Result<Vec<CanvasLogEntry>, Self::Error> {
            Ok(Vec::new())
        }

        fn compact_log_entries(&mut self, _: u64) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    let mut document = CanvasDocument::default();
    document
        .insert_node(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .unwrap();
    let mut retry_store = MemoryCanvasPersistenceStore::default();
    retry_store
        .save_checkpoint(CanvasCheckpoint::new(4, &document))
        .unwrap();
    let mut editor = CanvasEditor::new(document);
    let moved = CanvasNode::new("a", point(px(40.0), px(0.0)), size(px(10.0), px(10.0)));
    editor
        .apply_tool_effects([
            CanvasToolEffect::BeginGesture,
            CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                DocumentCommand::UpdateNode(moved.clone()),
            )),
        ])
        .unwrap();

    let mut store = FailingAppendStore;
    let mut cursor = CanvasPersistenceCursor::new(4);

    let err = apply_persistent_tool_effect(
        &mut editor,
        &mut store,
        &mut cursor,
        CanvasToolEffect::CommitGesture,
    )
    .unwrap_err();

    assert_eq!(err, CanvasPersistenceError::Store(StoreFailure));
    assert_eq!(cursor.sequence(), 4);
    assert_eq!(editor.history().undo_depth(), 0);
    assert_eq!(editor.document().node(&NodeId::from("a")).unwrap(), &moved);

    apply_persistent_tool_effect(
        &mut editor,
        &mut retry_store,
        &mut cursor,
        CanvasToolEffect::CommitGesture,
    )
    .unwrap();

    assert_eq!(cursor.sequence(), 5);
    assert_eq!(editor.history().undo_depth(), 1);
    assert_eq!(
        load_canvas_document(&retry_store)
            .unwrap()
            .node(&NodeId::from("a"))
            .unwrap(),
        editor.document().node(&NodeId::from("a")).unwrap()
    );
}

#[test]
fn persistent_tool_effects_keep_gesture_updates_out_of_log_until_commit() {
    let mut document = CanvasDocument::default();
    document
        .insert_node(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .unwrap();

    let mut editor = CanvasEditor::new(document);
    let mut store = MemoryCanvasPersistenceStore::default();
    let mut cursor = CanvasPersistenceCursor::new(9);
    let moved = CanvasNode::new("a", point(px(12.0), px(0.0)), size(px(10.0), px(10.0)));

    apply_persistent_tool_effect(
        &mut editor,
        &mut store,
        &mut cursor,
        CanvasToolEffect::UpdateGesture(CanvasTransaction::single(DocumentCommand::UpdateNode(
            moved,
        ))),
    )
    .unwrap();

    assert_eq!(cursor.sequence(), 9);
    assert!(store.log_entries().is_empty());
    assert_eq!(
        editor.document().node(&NodeId::from("a")).unwrap().position,
        point(px(12.0), px(0.0))
    );
}

#[test]
fn persistent_event_dispatch_logs_builtin_connect_transaction() {
    let mut document = CanvasDocument::default();
    document
        .insert_node(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .unwrap();
    document
        .insert_node(CanvasNode::new(
            "b",
            point(px(200.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .unwrap();
    let mut editor = CanvasEditor::new(document);
    editor.set_tool(CanvasTool::Connect);
    let mut store = MemoryCanvasPersistenceStore::default();
    store
        .save_checkpoint(CanvasCheckpoint::new(0, editor.document()))
        .unwrap();
    let mut cursor = CanvasPersistenceCursor::default();

    handle_persistent_event(
        &mut editor,
        &mut store,
        &mut cursor,
        CanvasEvent::PointerDown {
            position: point(px(10.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        },
    )
    .unwrap();
    assert_eq!(cursor.sequence(), 0);
    assert!(store.log_entries().is_empty());

    handle_persistent_event(
        &mut editor,
        &mut store,
        &mut cursor,
        CanvasEvent::PointerUp {
            position: point(px(210.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        },
    )
    .unwrap();

    assert_eq!(editor.document().edge_count(), 1);
    assert_eq!(editor.history().undo_depth(), 1);
    assert_eq!(cursor.sequence(), 1);
    assert_eq!(store.log_entries().len(), 1);
    assert!(matches!(
        store.log_entries()[0].transaction.commands.as_slice(),
        [DocumentCommand::InsertEdge(edge)]
            if edge.source.node_id == NodeId::from("a")
                && edge.target.node_id == NodeId::from("b")
    ));

    let restored = load_canvas_document(&store).unwrap();
    assert_eq!(restored.edge_count(), 1);
}

#[test]
fn persistent_event_dispatch_logs_custom_tool_transaction() {
    let mut editor = CanvasEditor::default();
    editor.set_viewport(crate::CanvasViewport::new(point(px(100.0), px(50.0)), 2.0).unwrap());
    editor.set_tool(CanvasTool::custom("stamp"));
    let mut tool = PersistentStampTool::default();
    let mut store = MemoryCanvasPersistenceStore::default();
    let mut cursor = CanvasPersistenceCursor::default();

    handle_persistent_event_with_custom_tool(
        &mut editor,
        &mut store,
        &mut cursor,
        CanvasEvent::PointerDown {
            position: point(px(20.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        },
        &mut tool,
    )
    .unwrap();

    assert_eq!(tool.calls, 1);
    assert_eq!(cursor.sequence(), 1);
    assert_eq!(store.log_entries().len(), 1);
    assert_eq!(
        editor
            .document()
            .node(&NodeId::from("persistent-stamp-0"))
            .unwrap()
            .position,
        point(px(110.0), px(55.0))
    );

    let restored = load_canvas_document(&store).unwrap();
    assert!(restored.contains_node(&NodeId::from("persistent-stamp-0")));
}

#[test]
fn persistent_registry_event_dispatch_logs_registered_custom_tool_transaction() {
    let mut editor = CanvasEditor::default();
    editor.set_tool(CanvasTool::custom("stamp"));
    let mut registry = CanvasToolRegistry::new();
    registry.insert("stamp", PersistentStampTool::default());
    let mut store = MemoryCanvasPersistenceStore::default();
    let mut cursor = CanvasPersistenceCursor::default();

    handle_persistent_event_with_tool_registry(
        &mut editor,
        &mut store,
        &mut cursor,
        CanvasEvent::PointerDown {
            position: point(px(12.0), px(18.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        },
        &mut registry,
    )
    .unwrap();

    assert_eq!(cursor.sequence(), 1);
    assert_eq!(store.log_entries().len(), 1);
    assert!(
        editor
            .document()
            .contains_node(&NodeId::from("persistent-stamp-0"))
    );
}

#[test]
fn persistent_registry_event_dispatch_reports_missing_custom_tool_without_mutation() {
    let mut editor = CanvasEditor::default();
    editor.set_tool(CanvasTool::custom("missing"));
    let mut registry = CanvasToolRegistry::new();
    let mut store = MemoryCanvasPersistenceStore::default();
    let mut cursor = CanvasPersistenceCursor::default();

    let err = handle_persistent_event_with_tool_registry(
        &mut editor,
        &mut store,
        &mut cursor,
        CanvasEvent::PointerDown {
            position: point(px(0.0), px(0.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        },
        &mut registry,
    )
    .unwrap_err();

    assert_eq!(
        err,
        CanvasPersistentToolRegistryError::MissingTool(CanvasToolId::from("missing"))
    );
    assert_eq!(cursor.sequence(), 0);
    assert!(store.log_entries().is_empty());
    assert!(editor.document().node_count() == 0);
    assert_eq!(editor.history().undo_depth(), 0);
}
