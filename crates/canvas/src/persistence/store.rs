use crate::{
    CanvasDocument, CanvasDocumentDiff, CanvasEditor, CanvasEvent, CanvasSnapshot,
    CanvasToolEffect, CanvasToolId, CanvasToolReducer, CanvasToolRegistry, CanvasToolRegistryError,
    CanvasTransaction, DocumentError,
};
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

#[derive(Debug, Eq, PartialEq)]
pub enum CanvasPersistentToolRegistryError<E = Infallible> {
    MissingTool(CanvasToolId),
    Persistence(CanvasPersistenceError<E>),
}

impl<E> CanvasPersistentToolRegistryError<E> {
    fn from_tool_registry_error(error: CanvasToolRegistryError) -> Self {
        match error {
            CanvasToolRegistryError::MissingTool(id) => Self::MissingTool(id),
            CanvasToolRegistryError::Document(error) => {
                Self::Persistence(CanvasPersistenceError::Document(error))
            }
        }
    }
}

impl<E> From<CanvasPersistenceError<E>> for CanvasPersistentToolRegistryError<E> {
    fn from(value: CanvasPersistenceError<E>) -> Self {
        Self::Persistence(value)
    }
}

impl<E> fmt::Display for CanvasPersistentToolRegistryError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTool(id) => write!(f, "canvas custom tool `{id}` is not registered"),
            Self::Persistence(error) => fmt::Display::fmt(error, f),
        }
    }
}

impl<E> Error for CanvasPersistentToolRegistryError<E>
where
    E: fmt::Debug + fmt::Display + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MissingTool(_) => None,
            Self::Persistence(error) => Some(error),
        }
    }
}

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

pub fn undo_persistent_transaction<S>(
    editor: &mut CanvasEditor,
    store: &mut S,
    cursor: &mut CanvasPersistenceCursor,
) -> Result<bool, CanvasPersistenceError<S::Error>>
where
    S: CanvasPersistenceStore,
{
    let Some(transaction) = editor.history.next_undo_transaction() else {
        return Ok(false);
    };
    let transaction = transaction.clone();
    editor.document.invert_transaction(&transaction)?;
    store
        .append_log_entry(CanvasLogEntry::new(cursor.next_sequence(), transaction))
        .map_err(CanvasPersistenceError::Store)?;
    editor.undo()?;
    cursor.advance();
    Ok(true)
}

pub fn redo_persistent_transaction<S>(
    editor: &mut CanvasEditor,
    store: &mut S,
    cursor: &mut CanvasPersistenceCursor,
) -> Result<bool, CanvasPersistenceError<S::Error>>
where
    S: CanvasPersistenceStore,
{
    let Some(transaction) = editor.history.next_redo_transaction() else {
        return Ok(false);
    };
    let transaction = transaction.clone();
    editor.document.invert_transaction(&transaction)?;
    store
        .append_log_entry(CanvasLogEntry::new(cursor.next_sequence(), transaction))
        .map_err(CanvasPersistenceError::Store)?;
    editor.redo()?;
    cursor.advance();
    Ok(true)
}

pub fn apply_persistent_tool_effect<S>(
    editor: &mut CanvasEditor,
    store: &mut S,
    cursor: &mut CanvasPersistenceCursor,
    effect: CanvasToolEffect,
) -> Result<(), CanvasPersistenceError<S::Error>>
where
    S: CanvasPersistenceStore,
{
    match effect {
        CanvasToolEffect::ApplyTransaction(transaction) => {
            apply_persistent_transaction(editor, store, cursor, transaction)?;
        }
        CanvasToolEffect::PushUndo(inverse) => {
            apply_persistent_undo_commit(editor, store, cursor, inverse)?;
        }
        effect => {
            editor.apply_tool_effect(effect)?;
        }
    }

    Ok(())
}

pub fn apply_persistent_tool_effects<S>(
    editor: &mut CanvasEditor,
    store: &mut S,
    cursor: &mut CanvasPersistenceCursor,
    effects: impl IntoIterator<Item = CanvasToolEffect>,
) -> Result<(), CanvasPersistenceError<S::Error>>
where
    S: CanvasPersistenceStore,
{
    for effect in effects {
        apply_persistent_tool_effect(editor, store, cursor, effect)?;
    }

    Ok(())
}

pub fn handle_persistent_event<S>(
    editor: &mut CanvasEditor,
    store: &mut S,
    cursor: &mut CanvasPersistenceCursor,
    event: CanvasEvent,
) -> Result<(), CanvasPersistenceError<S::Error>>
where
    S: CanvasPersistenceStore,
{
    let effects = editor.event_effects(event)?;
    apply_persistent_tool_effects(editor, store, cursor, effects)
}

pub fn handle_persistent_event_with_custom_tool<S, T>(
    editor: &mut CanvasEditor,
    store: &mut S,
    cursor: &mut CanvasPersistenceCursor,
    event: CanvasEvent,
    custom_tool: &mut T,
) -> Result<(), CanvasPersistenceError<S::Error>>
where
    S: CanvasPersistenceStore,
    T: CanvasToolReducer + ?Sized,
{
    let effects = editor.event_effects_with_custom_tool(event, custom_tool)?;
    apply_persistent_tool_effects(editor, store, cursor, effects)
}

pub fn handle_persistent_event_with_tool_registry<S>(
    editor: &mut CanvasEditor,
    store: &mut S,
    cursor: &mut CanvasPersistenceCursor,
    event: CanvasEvent,
    registry: &mut CanvasToolRegistry,
) -> Result<(), CanvasPersistentToolRegistryError<S::Error>>
where
    S: CanvasPersistenceStore,
{
    let effects = editor
        .event_effects_with_tool_registry(event, registry)
        .map_err(CanvasPersistentToolRegistryError::from_tool_registry_error)?;
    apply_persistent_tool_effects(editor, store, cursor, effects)?;
    Ok(())
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

fn apply_persistent_undo_commit<S>(
    editor: &mut CanvasEditor,
    store: &mut S,
    cursor: &mut CanvasPersistenceCursor,
    inverse: CanvasTransaction,
) -> Result<(), CanvasPersistenceError<S::Error>>
where
    S: CanvasPersistenceStore,
{
    if !inverse.is_empty() {
        let committed = editor.document.invert_transaction(&inverse)?;
        store
            .append_log_entry(CanvasLogEntry::new(cursor.next_sequence(), committed))
            .map_err(CanvasPersistenceError::Store)?;
        cursor.advance();
    }

    editor.apply_tool_effect(CanvasToolEffect::PushUndo(inverse))?;
    Ok(())
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
