use crate::{
    CanvasDocument, CanvasDocumentDiff, CanvasEditor, CanvasSnapshot, CanvasTransaction,
    DocumentError,
};
use std::{convert::Infallible, error::Error, fmt};

pub const CANVAS_REDB_STORE_FEATURE: &str = "redb-store";
pub const CANVAS_LORO_CRDT_FEATURE: &str = "loro-crdt";
pub const CANVAS_RKYV_SNAPSHOT_FEATURE: &str = "rkyv-snapshot";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasPersistenceAdapter {
    MemoryStore,
    RedbStore,
    LoroCrdt,
    RkyvSnapshot,
}

impl CanvasPersistenceAdapter {
    pub fn feature_name(self) -> Option<&'static str> {
        match self {
            Self::MemoryStore => None,
            Self::RedbStore => Some(CANVAS_REDB_STORE_FEATURE),
            Self::LoroCrdt => Some(CANVAS_LORO_CRDT_FEATURE),
            Self::RkyvSnapshot => Some(CANVAS_RKYV_SNAPSHOT_FEATURE),
        }
    }

    pub fn feature_enabled(self) -> bool {
        match self {
            Self::MemoryStore => true,
            Self::RedbStore => cfg!(feature = "redb-store"),
            Self::LoroCrdt => cfg!(feature = "loro-crdt"),
            Self::RkyvSnapshot => cfg!(feature = "rkyv-snapshot"),
        }
    }

    pub fn implemented(self) -> bool {
        matches!(self, Self::MemoryStore)
    }

    pub fn status(self) -> CanvasPersistenceAdapterStatus {
        CanvasPersistenceAdapterStatus {
            adapter: self,
            feature_name: self.feature_name(),
            feature_enabled: self.feature_enabled(),
            implemented: self.implemented(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanvasPersistenceAdapterStatus {
    pub adapter: CanvasPersistenceAdapter,
    pub feature_name: Option<&'static str>,
    pub feature_enabled: bool,
    pub implemented: bool,
}

pub const CANVAS_PERSISTENCE_ADAPTERS: &[CanvasPersistenceAdapter] = &[
    CanvasPersistenceAdapter::MemoryStore,
    CanvasPersistenceAdapter::RedbStore,
    CanvasPersistenceAdapter::LoroCrdt,
    CanvasPersistenceAdapter::RkyvSnapshot,
];

pub fn canvas_persistence_adapter_statuses() -> [CanvasPersistenceAdapterStatus; 4] {
    [
        CanvasPersistenceAdapter::MemoryStore.status(),
        CanvasPersistenceAdapter::RedbStore.status(),
        CanvasPersistenceAdapter::LoroCrdt.status(),
        CanvasPersistenceAdapter::RkyvSnapshot.status(),
    ]
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CanvasCheckpoint {
    pub sequence: u64,
    pub snapshot: CanvasSnapshot,
}

impl CanvasCheckpoint {
    pub fn new(sequence: u64, document: &CanvasDocument) -> Self {
        Self {
            sequence,
            snapshot: document.to_snapshot(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CanvasLogEntry {
    pub sequence: u64,
    pub transaction: CanvasTransaction,
}

impl CanvasLogEntry {
    pub fn new(sequence: u64, transaction: impl Into<CanvasTransaction>) -> Self {
        Self {
            sequence,
            transaction: transaction.into(),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum CanvasPersistenceError<E = Infallible> {
    Store(E),
    Document(DocumentError),
    NonMonotonicLogSequence { previous: u64, found: u64 },
}

impl<E> fmt::Display for CanvasPersistenceError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(f, "canvas persistence store error: {error}"),
            Self::Document(error) => fmt::Display::fmt(error, f),
            Self::NonMonotonicLogSequence { previous, found } => write!(
                f,
                "canvas transaction log sequence `{found}` is not greater than previous sequence `{previous}`"
            ),
        }
    }
}

impl<E> Error for CanvasPersistenceError<E> where E: fmt::Debug + fmt::Display {}

impl<E> From<DocumentError> for CanvasPersistenceError<E> {
    fn from(value: DocumentError) -> Self {
        Self::Document(value)
    }
}

pub type CanvasReplayError = CanvasPersistenceError<Infallible>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CanvasPersistenceCursor {
    sequence: u64,
}

impl CanvasPersistenceCursor {
    pub fn new(sequence: u64) -> Self {
        Self { sequence }
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn next_sequence(&self) -> u64 {
        self.sequence + 1
    }

    pub fn advance(&mut self) -> u64 {
        self.sequence = self.next_sequence();
        self.sequence
    }
}

pub trait CanvasPersistenceStore {
    type Error: fmt::Debug + fmt::Display;

    fn load_checkpoint(&self) -> Result<Option<CanvasCheckpoint>, Self::Error>;

    fn save_checkpoint(&mut self, checkpoint: CanvasCheckpoint) -> Result<(), Self::Error>;

    fn append_log_entry(&mut self, entry: CanvasLogEntry) -> Result<(), Self::Error>;

    fn load_log_entries(&self, after_sequence: u64) -> Result<Vec<CanvasLogEntry>, Self::Error>;

    fn compact_log_entries(&mut self, through_sequence: u64) -> Result<(), Self::Error>;
}

pub fn replay_canvas_log(
    checkpoint: Option<CanvasCheckpoint>,
    log_entries: impl IntoIterator<Item = CanvasLogEntry>,
) -> Result<CanvasDocument, CanvasReplayError> {
    replay_checkpoint_and_log(checkpoint, log_entries)
}

pub fn load_canvas_document<S>(
    store: &S,
) -> Result<CanvasDocument, CanvasPersistenceError<S::Error>>
where
    S: CanvasPersistenceStore,
{
    let checkpoint = store
        .load_checkpoint()
        .map_err(CanvasPersistenceError::Store)?;
    let after_sequence = checkpoint
        .as_ref()
        .map_or(0, |checkpoint| checkpoint.sequence);
    let log_entries = store
        .load_log_entries(after_sequence)
        .map_err(CanvasPersistenceError::Store)?;

    replay_checkpoint_and_log(checkpoint, log_entries)
}

pub fn load_canvas_persistence_cursor<S>(
    store: &S,
) -> Result<CanvasPersistenceCursor, CanvasPersistenceError<S::Error>>
where
    S: CanvasPersistenceStore,
{
    let checkpoint = store
        .load_checkpoint()
        .map_err(CanvasPersistenceError::Store)?;
    let mut previous_sequence = checkpoint
        .as_ref()
        .map_or(0, |checkpoint| checkpoint.sequence);
    let log_entries = store
        .load_log_entries(previous_sequence)
        .map_err(CanvasPersistenceError::Store)?;

    for entry in log_entries {
        if entry.sequence <= previous_sequence {
            return Err(CanvasPersistenceError::NonMonotonicLogSequence {
                previous: previous_sequence,
                found: entry.sequence,
            });
        }

        previous_sequence = entry.sequence;
    }

    Ok(CanvasPersistenceCursor::new(previous_sequence))
}

pub fn apply_persistent_transaction<S>(
    editor: &mut CanvasEditor,
    store: &mut S,
    cursor: &mut CanvasPersistenceCursor,
    transaction: CanvasTransaction,
) -> Result<CanvasDocumentDiff, CanvasPersistenceError<S::Error>>
where
    S: CanvasPersistenceStore,
{
    if transaction.is_empty() {
        return Ok(CanvasDocumentDiff::default());
    }

    editor.document.invert_transaction(&transaction)?;
    let log_transaction = transaction.clone();
    store
        .append_log_entry(CanvasLogEntry::new(cursor.next_sequence(), log_transaction))
        .map_err(CanvasPersistenceError::Store)?;
    let diff = editor.apply_transaction_with_diff(transaction)?;
    cursor.advance();
    Ok(diff)
}

pub fn save_canvas_checkpoint<S>(
    editor: &CanvasEditor,
    store: &mut S,
    cursor: &CanvasPersistenceCursor,
) -> Result<CanvasCheckpoint, CanvasPersistenceError<S::Error>>
where
    S: CanvasPersistenceStore,
{
    let checkpoint = CanvasCheckpoint::new(cursor.sequence(), &editor.document);
    store
        .save_checkpoint(checkpoint.clone())
        .map_err(CanvasPersistenceError::Store)?;
    store
        .compact_log_entries(checkpoint.sequence)
        .map_err(CanvasPersistenceError::Store)?;
    Ok(checkpoint)
}

fn replay_checkpoint_and_log<E>(
    checkpoint: Option<CanvasCheckpoint>,
    log_entries: impl IntoIterator<Item = CanvasLogEntry>,
) -> Result<CanvasDocument, CanvasPersistenceError<E>> {
    let mut previous_sequence = checkpoint
        .as_ref()
        .map_or(0, |checkpoint| checkpoint.sequence);
    let mut document = match checkpoint {
        Some(checkpoint) => CanvasDocument::from_snapshot(checkpoint.snapshot)?,
        None => CanvasDocument::default(),
    };

    for entry in log_entries {
        if entry.sequence <= previous_sequence {
            return Err(CanvasPersistenceError::NonMonotonicLogSequence {
                previous: previous_sequence,
                found: entry.sequence,
            });
        }

        document.apply_transaction(entry.transaction)?;
        previous_sequence = entry.sequence;
    }

    Ok(document)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MemoryCanvasPersistenceStore {
    checkpoint: Option<CanvasCheckpoint>,
    log_entries: Vec<CanvasLogEntry>,
}

impl MemoryCanvasPersistenceStore {
    pub fn checkpoint(&self) -> Option<&CanvasCheckpoint> {
        self.checkpoint.as_ref()
    }

    pub fn log_entries(&self) -> &[CanvasLogEntry] {
        &self.log_entries
    }
}

impl CanvasPersistenceStore for MemoryCanvasPersistenceStore {
    type Error = Infallible;

    fn load_checkpoint(&self) -> Result<Option<CanvasCheckpoint>, Self::Error> {
        Ok(self.checkpoint.clone())
    }

    fn save_checkpoint(&mut self, checkpoint: CanvasCheckpoint) -> Result<(), Self::Error> {
        self.checkpoint = Some(checkpoint);
        Ok(())
    }

    fn append_log_entry(&mut self, entry: CanvasLogEntry) -> Result<(), Self::Error> {
        self.log_entries.push(entry);
        Ok(())
    }

    fn load_log_entries(&self, after_sequence: u64) -> Result<Vec<CanvasLogEntry>, Self::Error> {
        Ok(self
            .log_entries
            .iter()
            .filter(|entry| entry.sequence > after_sequence)
            .cloned()
            .collect())
    }

    fn compact_log_entries(&mut self, through_sequence: u64) -> Result<(), Self::Error> {
        self.log_entries
            .retain(|entry| entry.sequence > through_sequence);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanvasEditor, CanvasNode, CanvasRecordId, DocumentCommand, NodeId};
    use open_gpui::{point, px, size};

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

        assert!(restored.nodes.contains_key(&NodeId::from("a")));
        assert!(restored.nodes.contains_key(&NodeId::from("b")));
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

        assert!(restored.nodes.contains_key(&NodeId::from("a")));
        assert!(!restored.nodes.contains_key(&NodeId::from("stale")));
        assert!(restored.nodes.contains_key(&NodeId::from("b")));
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

        assert!(editor.document.nodes.contains_key(&NodeId::from("a")));
        assert_eq!(
            diff.inserted.iter().cloned().collect::<Vec<_>>(),
            vec![CanvasRecordId::Node(NodeId::from("a"))]
        );
        assert_eq!(cursor.sequence(), 1);
        assert_eq!(store.log_entries().len(), 1);
        assert_eq!(store.log_entries()[0], CanvasLogEntry::new(1, transaction));

        let restored = load_canvas_document(&store).unwrap();
        assert!(restored.nodes.contains_key(&NodeId::from("a")));
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
        assert!(editor.document.nodes.is_empty());
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
        assert!(editor.document.nodes.is_empty());
        assert_eq!(editor.history.undo_depth(), 0);
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
        assert!(restored.nodes.contains_key(&NodeId::from("a")));
        assert!(restored.nodes.contains_key(&NodeId::from("b")));
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
}
