use crate::{CanvasDocument, CanvasSnapshot, CanvasTransaction, DocumentError};
use std::{convert::Infallible, error::Error, fmt};

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
    use crate::{CanvasNode, DocumentCommand, NodeId};
    use open_gpui::{point, px, size};

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
}
