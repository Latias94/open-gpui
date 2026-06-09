use crate::{
    CanvasCommittedMutation, CanvasDocument, CanvasDocumentDiff, CanvasEditor, CanvasEvent,
    CanvasRecordOperationBatch, CanvasRelationOperationBatch, CanvasSnapshot, CanvasToolId,
    CanvasToolIntent, CanvasToolReducer, CanvasToolRegistry, CanvasTransaction, DocumentError,
    tool::CanvasToolEffect,
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
    sequence: u64,
    transaction: CanvasTransaction,
    #[serde(default, alias = "record_operation_batch")]
    committed_record_operation_batch: Option<CanvasRecordOperationBatch>,
    #[serde(default)]
    committed_relation_operation_batch: Option<CanvasRelationOperationBatch>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasLogEntryKind {
    CommittedMutation,
    PartialCommittedMutation,
    LegacyReplayTransaction,
}

impl CanvasLogEntry {
    pub fn from_replay_transaction(
        sequence: u64,
        transaction: impl Into<CanvasTransaction>,
    ) -> Self {
        Self {
            sequence,
            transaction: transaction.into(),
            committed_record_operation_batch: None,
            committed_relation_operation_batch: None,
        }
    }

    pub fn from_committed_mutation(sequence: u64, committed: &CanvasCommittedMutation) -> Self {
        Self {
            sequence,
            transaction: committed.transaction().clone(),
            committed_record_operation_batch: Some(committed.record_operation_batch(sequence)),
            committed_relation_operation_batch: Some(committed.relation_operation_batch(sequence)),
        }
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn transaction(&self) -> &CanvasTransaction {
        &self.transaction
    }

    pub fn kind(&self) -> CanvasLogEntryKind {
        match (
            self.committed_record_operation_batch.is_some(),
            self.committed_relation_operation_batch.is_some(),
        ) {
            (true, true) => CanvasLogEntryKind::CommittedMutation,
            (true, false) | (false, true) => CanvasLogEntryKind::PartialCommittedMutation,
            (false, false) => CanvasLogEntryKind::LegacyReplayTransaction,
        }
    }

    pub fn committed_record_operations(&self) -> Option<&CanvasRecordOperationBatch> {
        self.committed_record_operation_batch.as_ref()
    }

    pub fn committed_relation_operations(&self) -> Option<&CanvasRelationOperationBatch> {
        self.committed_relation_operation_batch.as_ref()
    }

    pub fn is_legacy_replay_entry(&self) -> bool {
        self.kind() == CanvasLogEntryKind::LegacyReplayTransaction
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
        if entry.sequence() <= previous_sequence {
            return Err(CanvasPersistenceError::NonMonotonicLogSequence {
                previous: previous_sequence,
                found: entry.sequence(),
            });
        }

        previous_sequence = entry.sequence();
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

    let prepared = editor.prepare_document_transaction(transaction)?;
    if prepared.committed().diff().is_empty() {
        let diff = editor.apply_prepared_document_mutation(prepared);
        return Ok(diff);
    }
    append_prepared_log_entry(store, cursor, prepared.committed())?;
    let diff = editor.apply_prepared_document_mutation(prepared);
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
    let Some(transaction) = editor.next_undo_transaction() else {
        return Ok(false);
    };
    let transaction = transaction.clone();
    let prepared = editor.prepare_document_transaction(transaction)?;
    if prepared.committed().diff().is_empty() {
        editor.apply_prepared_undo_mutation(prepared);
        return Ok(false);
    }
    append_prepared_log_entry(store, cursor, prepared.committed())?;
    editor.apply_prepared_undo_mutation(prepared);
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
    let Some(transaction) = editor.next_redo_transaction() else {
        return Ok(false);
    };
    let transaction = transaction.clone();
    let prepared = editor.prepare_document_transaction(transaction)?;
    if prepared.committed().diff().is_empty() {
        editor.apply_prepared_redo_mutation(prepared);
        return Ok(false);
    }
    append_prepared_log_entry(store, cursor, prepared.committed())?;
    editor.apply_prepared_redo_mutation(prepared);
    cursor.advance();
    Ok(true)
}

pub(crate) fn apply_persistent_tool_effect<S>(
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
        CanvasToolEffect::CommitGesture => {
            apply_persistent_gesture_commit(editor, store, cursor)?;
        }
        effect => {
            editor.apply_tool_effect(effect)?;
        }
    }

    Ok(())
}

pub(crate) fn apply_persistent_tool_effects<S>(
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

pub fn apply_persistent_tool_intent<S>(
    editor: &mut CanvasEditor,
    store: &mut S,
    cursor: &mut CanvasPersistenceCursor,
    intent: CanvasToolIntent,
) -> Result<(), CanvasPersistenceError<S::Error>>
where
    S: CanvasPersistenceStore,
{
    match intent {
        CanvasToolIntent::ApplyTransaction(transaction) => {
            apply_persistent_transaction(editor, store, cursor, transaction)?;
        }
        CanvasToolIntent::CommitTransientTransaction => {
            apply_persistent_gesture_commit(editor, store, cursor)?;
        }
        intent => {
            editor.apply_tool_intent(intent)?;
        }
    }

    Ok(())
}

pub fn apply_persistent_tool_intents<S>(
    editor: &mut CanvasEditor,
    store: &mut S,
    cursor: &mut CanvasPersistenceCursor,
    intents: impl IntoIterator<Item = CanvasToolIntent>,
) -> Result<(), CanvasPersistenceError<S::Error>>
where
    S: CanvasPersistenceStore,
{
    for intent in intents {
        apply_persistent_tool_intent(editor, store, cursor, intent)?;
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
    if editor.tool().custom_id().is_some() {
        let intents = custom_tool.handle_event(editor.tool_context(), event)?;
        apply_persistent_tool_intents(editor, store, cursor, intents)
    } else {
        let effects = editor.event_effects(event)?;
        apply_persistent_tool_effects(editor, store, cursor, effects)
    }
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
    if let Some(tool_id) = editor.tool().custom_id().cloned() {
        let reducer = registry
            .reducer_mut(&tool_id)
            .ok_or_else(|| CanvasPersistentToolRegistryError::MissingTool(tool_id.clone()))?;
        let intents = reducer
            .handle_event(editor.tool_context(), event)
            .map_err(|error| {
                CanvasPersistentToolRegistryError::Persistence(CanvasPersistenceError::Document(
                    error,
                ))
            })?;
        apply_persistent_tool_intents(editor, store, cursor, intents)
            .map_err(CanvasPersistentToolRegistryError::from)?;
    } else {
        let effects = editor.event_effects(event).map_err(|error| {
            CanvasPersistentToolRegistryError::Persistence(CanvasPersistenceError::Document(error))
        })?;
        apply_persistent_tool_effects(editor, store, cursor, effects)
            .map_err(CanvasPersistentToolRegistryError::from)?;
    }
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
    let checkpoint = CanvasCheckpoint::new(cursor.sequence(), editor.document());
    store
        .save_checkpoint(checkpoint.clone())
        .map_err(CanvasPersistenceError::Store)?;
    store
        .compact_log_entries(checkpoint.sequence)
        .map_err(CanvasPersistenceError::Store)?;
    Ok(checkpoint)
}

fn append_prepared_log_entry<S>(
    store: &mut S,
    cursor: &CanvasPersistenceCursor,
    committed: &CanvasCommittedMutation,
) -> Result<(), CanvasPersistenceError<S::Error>>
where
    S: CanvasPersistenceStore,
{
    store
        .append_log_entry(CanvasLogEntry::from_committed_mutation(
            cursor.next_sequence(),
            committed,
        ))
        .map_err(CanvasPersistenceError::Store)
}

fn apply_persistent_gesture_commit<S>(
    editor: &mut CanvasEditor,
    store: &mut S,
    cursor: &mut CanvasPersistenceCursor,
) -> Result<(), CanvasPersistenceError<S::Error>>
where
    S: CanvasPersistenceStore,
{
    match editor.prepare_gesture_commit()? {
        Some(prepared) => {
            if prepared.committed().diff().is_empty() {
                editor.apply_prepared_gesture_commit(prepared);
                return Ok(());
            }
            append_prepared_log_entry(store, cursor, prepared.committed())?;
            editor.apply_prepared_gesture_commit(prepared);
            cursor.advance();
        }
        None => {
            editor.apply_tool_effect(CanvasToolEffect::CommitGesture)?;
        }
    }

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
        if entry.sequence() <= previous_sequence {
            return Err(CanvasPersistenceError::NonMonotonicLogSequence {
                previous: previous_sequence,
                found: entry.sequence(),
            });
        }

        document.apply_transaction(entry.transaction().clone())?;
        previous_sequence = entry.sequence();
    }

    Ok(document)
}
