use crate::{
    CanvasDocument, CanvasDocumentDiff, CanvasEditor, CanvasEvent, CanvasSnapshot,
    CanvasToolEffect, CanvasToolId, CanvasToolReducer, CanvasToolRegistry, CanvasToolRegistryError,
    CanvasTransaction, DocumentError,
};
use std::{convert::Infallible, error::Error, fmt};

pub const CANVAS_REDB_STORE_FEATURE: &str = "redb-store";
pub const CANVAS_LORO_CRDT_FEATURE: &str = "loro-crdt";
pub const CANVAS_RKYV_SNAPSHOT_FEATURE: &str = "rkyv-snapshot";
pub const CANVAS_PERSISTENCE_CODEC_VERSION: u32 = 1;

fn default_persistence_codec_version() -> u32 {
    CANVAS_PERSISTENCE_CODEC_VERSION
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasPersistenceRecordKind {
    Checkpoint,
    LogEntry,
}

impl CanvasPersistenceRecordKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Checkpoint => "checkpoint",
            Self::LogEntry => "log_entry",
        }
    }
}

impl fmt::Display for CanvasPersistenceRecordKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "record_kind", content = "payload", rename_all = "snake_case")]
pub enum CanvasPersistenceRecord {
    Checkpoint(CanvasCheckpoint),
    LogEntry(CanvasLogEntry),
}

impl CanvasPersistenceRecord {
    pub fn kind(&self) -> CanvasPersistenceRecordKind {
        match self {
            Self::Checkpoint(_) => CanvasPersistenceRecordKind::Checkpoint,
            Self::LogEntry(_) => CanvasPersistenceRecordKind::LogEntry,
        }
    }

    pub fn sequence(&self) -> u64 {
        match self {
            Self::Checkpoint(checkpoint) => checkpoint.sequence,
            Self::LogEntry(entry) => entry.sequence,
        }
    }

    pub fn document_format_version(&self) -> u32 {
        match self {
            Self::Checkpoint(checkpoint) => checkpoint.snapshot.format_version,
            Self::LogEntry(_) => crate::CANVAS_DOCUMENT_FORMAT_VERSION,
        }
    }
}

impl From<CanvasCheckpoint> for CanvasPersistenceRecord {
    fn from(value: CanvasCheckpoint) -> Self {
        Self::Checkpoint(value)
    }
}

impl From<CanvasLogEntry> for CanvasPersistenceRecord {
    fn from(value: CanvasLogEntry) -> Self {
        Self::LogEntry(value)
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CanvasPersistenceEnvelope {
    #[serde(default = "default_persistence_codec_version")]
    pub codec_version: u32,
    pub document_format_version: u32,
    pub record: CanvasPersistenceRecord,
}

impl CanvasPersistenceEnvelope {
    pub fn new(record: impl Into<CanvasPersistenceRecord>) -> Self {
        let record = record.into();
        Self {
            codec_version: CANVAS_PERSISTENCE_CODEC_VERSION,
            document_format_version: record.document_format_version(),
            record,
        }
    }

    pub fn kind(&self) -> CanvasPersistenceRecordKind {
        self.record.kind()
    }

    pub fn sequence(&self) -> u64 {
        self.record.sequence()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum CanvasPersistenceCodecError {
    UnsupportedCodecVersion {
        expected: u32,
        found: u32,
    },
    UnsupportedDocumentFormatVersion {
        expected: u32,
        found: u32,
    },
    RecordFormatVersionMismatch {
        envelope: u32,
        payload: u32,
    },
    UnexpectedRecordKind {
        expected: CanvasPersistenceRecordKind,
        found: CanvasPersistenceRecordKind,
    },
    Json(String),
}

impl CanvasPersistenceCodecError {
    fn json(error: serde_json::Error) -> Self {
        Self::Json(error.to_string())
    }
}

impl fmt::Display for CanvasPersistenceCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedCodecVersion { expected, found } => write!(
                f,
                "unsupported canvas persistence codec version `{found}`, expected `{expected}`"
            ),
            Self::UnsupportedDocumentFormatVersion { expected, found } => write!(
                f,
                "unsupported canvas persistence document format version `{found}`, expected `{expected}`"
            ),
            Self::RecordFormatVersionMismatch { envelope, payload } => write!(
                f,
                "canvas persistence envelope document format version `{envelope}` does not match payload version `{payload}`"
            ),
            Self::UnexpectedRecordKind { expected, found } => write!(
                f,
                "canvas persistence record kind `{found}` cannot be decoded as `{expected}`"
            ),
            Self::Json(error) => write!(f, "canvas persistence JSON codec error: {error}"),
        }
    }
}

impl Error for CanvasPersistenceCodecError {}

pub trait CanvasPersistenceCodec {
    type Error: fmt::Debug + fmt::Display;

    fn encode_checkpoint(&self, checkpoint: &CanvasCheckpoint) -> Result<Vec<u8>, Self::Error>;

    fn decode_checkpoint(&self, bytes: &[u8]) -> Result<CanvasCheckpoint, Self::Error>;

    fn encode_log_entry(&self, entry: &CanvasLogEntry) -> Result<Vec<u8>, Self::Error>;

    fn decode_log_entry(&self, bytes: &[u8]) -> Result<CanvasLogEntry, Self::Error>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CanvasJsonPersistenceCodec;

impl CanvasJsonPersistenceCodec {
    pub fn encode_record(
        &self,
        record: impl Into<CanvasPersistenceRecord>,
    ) -> Result<Vec<u8>, CanvasPersistenceCodecError> {
        serde_json::to_vec(&CanvasPersistenceEnvelope::new(record))
            .map_err(CanvasPersistenceCodecError::json)
    }

    pub fn decode_record(
        &self,
        bytes: &[u8],
    ) -> Result<CanvasPersistenceRecord, CanvasPersistenceCodecError> {
        Ok(self.decode_envelope(bytes)?.record)
    }

    pub fn decode_envelope(
        &self,
        bytes: &[u8],
    ) -> Result<CanvasPersistenceEnvelope, CanvasPersistenceCodecError> {
        let envelope: CanvasPersistenceEnvelope =
            serde_json::from_slice(bytes).map_err(CanvasPersistenceCodecError::json)?;
        validate_persistence_envelope(&envelope)?;
        Ok(envelope)
    }
}

impl CanvasPersistenceCodec for CanvasJsonPersistenceCodec {
    type Error = CanvasPersistenceCodecError;

    fn encode_checkpoint(&self, checkpoint: &CanvasCheckpoint) -> Result<Vec<u8>, Self::Error> {
        self.encode_record(checkpoint.clone())
    }

    fn decode_checkpoint(&self, bytes: &[u8]) -> Result<CanvasCheckpoint, Self::Error> {
        let envelope = self.decode_envelope(bytes)?;
        let found = envelope.kind();
        let CanvasPersistenceRecord::Checkpoint(checkpoint) = envelope.record else {
            return Err(CanvasPersistenceCodecError::UnexpectedRecordKind {
                expected: CanvasPersistenceRecordKind::Checkpoint,
                found,
            });
        };
        if checkpoint.snapshot.format_version != envelope.document_format_version {
            return Err(CanvasPersistenceCodecError::RecordFormatVersionMismatch {
                envelope: envelope.document_format_version,
                payload: checkpoint.snapshot.format_version,
            });
        }
        Ok(checkpoint)
    }

    fn encode_log_entry(&self, entry: &CanvasLogEntry) -> Result<Vec<u8>, Self::Error> {
        self.encode_record(entry.clone())
    }

    fn decode_log_entry(&self, bytes: &[u8]) -> Result<CanvasLogEntry, Self::Error> {
        let envelope = self.decode_envelope(bytes)?;
        let found = envelope.kind();
        let CanvasPersistenceRecord::LogEntry(entry) = envelope.record else {
            return Err(CanvasPersistenceCodecError::UnexpectedRecordKind {
                expected: CanvasPersistenceRecordKind::LogEntry,
                found,
            });
        };
        Ok(entry)
    }
}

fn validate_persistence_envelope(
    envelope: &CanvasPersistenceEnvelope,
) -> Result<(), CanvasPersistenceCodecError> {
    if envelope.codec_version != CANVAS_PERSISTENCE_CODEC_VERSION {
        return Err(CanvasPersistenceCodecError::UnsupportedCodecVersion {
            expected: CANVAS_PERSISTENCE_CODEC_VERSION,
            found: envelope.codec_version,
        });
    }

    if envelope.document_format_version < crate::CANVAS_DOCUMENT_MIN_SUPPORTED_FORMAT_VERSION
        || envelope.document_format_version > crate::CANVAS_DOCUMENT_FORMAT_VERSION
    {
        return Err(
            CanvasPersistenceCodecError::UnsupportedDocumentFormatVersion {
                expected: crate::CANVAS_DOCUMENT_FORMAT_VERSION,
                found: envelope.document_format_version,
            },
        );
    }

    let payload_format_version = envelope.record.document_format_version();
    if payload_format_version != envelope.document_format_version {
        return Err(CanvasPersistenceCodecError::RecordFormatVersionMismatch {
            envelope: envelope.document_format_version,
            payload: payload_format_version,
        });
    }

    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasEncodedLogEntry {
    pub sequence: u64,
    pub bytes: Vec<u8>,
}

impl CanvasEncodedLogEntry {
    pub fn new(sequence: u64, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            sequence,
            bytes: bytes.into(),
        }
    }
}

pub trait CanvasPersistenceByteStore {
    type Error: fmt::Debug + fmt::Display;

    fn load_checkpoint_bytes(&self) -> Result<Option<Vec<u8>>, Self::Error>;

    fn save_checkpoint_bytes(&mut self, bytes: Vec<u8>) -> Result<(), Self::Error>;

    fn append_log_entry_bytes(&mut self, sequence: u64, bytes: Vec<u8>) -> Result<(), Self::Error>;

    fn load_log_entry_bytes(
        &self,
        after_sequence: u64,
    ) -> Result<Vec<CanvasEncodedLogEntry>, Self::Error>;

    fn compact_log_entry_bytes(&mut self, through_sequence: u64) -> Result<(), Self::Error>;
}

#[derive(Debug, Eq, PartialEq)]
pub enum CanvasPersistenceByteStoreError<StoreError, CodecError> {
    Store(StoreError),
    Codec(CodecError),
    LogSequenceMismatch { key: u64, payload: u64 },
}

impl<StoreError, CodecError> fmt::Display
    for CanvasPersistenceByteStoreError<StoreError, CodecError>
where
    StoreError: fmt::Display,
    CodecError: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(f, "canvas persistence byte store error: {error}"),
            Self::Codec(error) => fmt::Display::fmt(error, f),
            Self::LogSequenceMismatch { key, payload } => write!(
                f,
                "canvas persistence log key sequence `{key}` does not match payload sequence `{payload}`"
            ),
        }
    }
}

impl<StoreError, CodecError> Error for CanvasPersistenceByteStoreError<StoreError, CodecError>
where
    StoreError: fmt::Debug + fmt::Display,
    CodecError: fmt::Debug + fmt::Display,
{
}

#[derive(Clone, Debug)]
pub struct CanvasPersistenceByteStoreAdapter<S, C = CanvasJsonPersistenceCodec> {
    store: S,
    codec: C,
}

impl<S> CanvasPersistenceByteStoreAdapter<S, CanvasJsonPersistenceCodec> {
    pub fn new(store: S) -> Self {
        Self::with_codec(store, CanvasJsonPersistenceCodec)
    }
}

impl<S, C> CanvasPersistenceByteStoreAdapter<S, C> {
    pub fn with_codec(store: S, codec: C) -> Self {
        Self { store, codec }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }

    pub fn into_store(self) -> S {
        self.store
    }

    pub fn codec(&self) -> &C {
        &self.codec
    }
}

impl<S, C> CanvasPersistenceStore for CanvasPersistenceByteStoreAdapter<S, C>
where
    S: CanvasPersistenceByteStore,
    C: CanvasPersistenceCodec,
{
    type Error = CanvasPersistenceByteStoreError<S::Error, C::Error>;

    fn load_checkpoint(&self) -> Result<Option<CanvasCheckpoint>, Self::Error> {
        self.store
            .load_checkpoint_bytes()
            .map_err(CanvasPersistenceByteStoreError::Store)?
            .map(|bytes| {
                self.codec
                    .decode_checkpoint(&bytes)
                    .map_err(CanvasPersistenceByteStoreError::Codec)
            })
            .transpose()
    }

    fn save_checkpoint(&mut self, checkpoint: CanvasCheckpoint) -> Result<(), Self::Error> {
        let bytes = self
            .codec
            .encode_checkpoint(&checkpoint)
            .map_err(CanvasPersistenceByteStoreError::Codec)?;
        self.store
            .save_checkpoint_bytes(bytes)
            .map_err(CanvasPersistenceByteStoreError::Store)
    }

    fn append_log_entry(&mut self, entry: CanvasLogEntry) -> Result<(), Self::Error> {
        let sequence = entry.sequence;
        let bytes = self
            .codec
            .encode_log_entry(&entry)
            .map_err(CanvasPersistenceByteStoreError::Codec)?;
        self.store
            .append_log_entry_bytes(sequence, bytes)
            .map_err(CanvasPersistenceByteStoreError::Store)
    }

    fn load_log_entries(&self, after_sequence: u64) -> Result<Vec<CanvasLogEntry>, Self::Error> {
        self.store
            .load_log_entry_bytes(after_sequence)
            .map_err(CanvasPersistenceByteStoreError::Store)?
            .into_iter()
            .map(|encoded| {
                let entry = self
                    .codec
                    .decode_log_entry(&encoded.bytes)
                    .map_err(CanvasPersistenceByteStoreError::Codec)?;
                if entry.sequence != encoded.sequence {
                    return Err(CanvasPersistenceByteStoreError::LogSequenceMismatch {
                        key: encoded.sequence,
                        payload: entry.sequence,
                    });
                }
                Ok(entry)
            })
            .collect()
    }

    fn compact_log_entries(&mut self, through_sequence: u64) -> Result<(), Self::Error> {
        self.store
            .compact_log_entry_bytes(through_sequence)
            .map_err(CanvasPersistenceByteStoreError::Store)
    }
}

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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MemoryCanvasPersistenceByteStore {
    checkpoint: Option<Vec<u8>>,
    log_entries: Vec<CanvasEncodedLogEntry>,
}

impl MemoryCanvasPersistenceByteStore {
    pub fn checkpoint_bytes(&self) -> Option<&[u8]> {
        self.checkpoint.as_deref()
    }

    pub fn encoded_log_entries(&self) -> &[CanvasEncodedLogEntry] {
        &self.log_entries
    }
}

impl CanvasPersistenceByteStore for MemoryCanvasPersistenceByteStore {
    type Error = Infallible;

    fn load_checkpoint_bytes(&self) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self.checkpoint.clone())
    }

    fn save_checkpoint_bytes(&mut self, bytes: Vec<u8>) -> Result<(), Self::Error> {
        self.checkpoint = Some(bytes);
        Ok(())
    }

    fn append_log_entry_bytes(&mut self, sequence: u64, bytes: Vec<u8>) -> Result<(), Self::Error> {
        self.log_entries
            .push(CanvasEncodedLogEntry::new(sequence, bytes));
        Ok(())
    }

    fn load_log_entry_bytes(
        &self,
        after_sequence: u64,
    ) -> Result<Vec<CanvasEncodedLogEntry>, Self::Error> {
        Ok(self
            .log_entries
            .iter()
            .filter(|entry| entry.sequence > after_sequence)
            .cloned()
            .collect())
    }

    fn compact_log_entry_bytes(&mut self, through_sequence: u64) -> Result<(), Self::Error> {
        self.log_entries
            .retain(|entry| entry.sequence > through_sequence);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CanvasEditor, CanvasNode, CanvasRecordId, CanvasSelection, CanvasTool, CanvasToolContext,
        CanvasToolRegistry, DocumentCommand, NodeId, PointerButton, ToolState,
    };
    use open_gpui::{point, px, size};

    #[derive(Default)]
    struct PersistentStampTool {
        calls: usize,
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
            } = event
            else {
                return Ok(Vec::new());
            };

            let node_id = NodeId::new(format!("persistent-stamp-{}", context.document.nodes.len()));
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

        assert!(restored.nodes.contains_key(&NodeId::from("a")));
        assert!(restored.nodes.contains_key(&NodeId::from("b")));
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
        assert!(!editor.document.nodes.contains_key(&NodeId::from("a")));
        assert_eq!(editor.history.undo_depth(), 0);
        assert_eq!(editor.history.redo_depth(), 1);
        assert_eq!(store.log_entries().len(), 2);
        assert!(matches!(
            store.log_entries()[1].transaction.commands.as_slice(),
            [DocumentCommand::RemoveNode(id)] if id == &NodeId::from("a")
        ));

        let restored = load_canvas_document(&store).unwrap();
        assert!(!restored.nodes.contains_key(&NodeId::from("a")));
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
        assert!(editor.document.nodes.contains_key(&NodeId::from("a")));
        assert_eq!(editor.history.undo_depth(), 1);
        assert_eq!(editor.history.redo_depth(), 0);
        assert_eq!(store.log_entries().len(), 3);
        assert!(matches!(
            store.log_entries()[2].transaction.commands.as_slice(),
            [DocumentCommand::InsertNode(inserted)] if inserted == &node
        ));

        let restored = load_canvas_document(&store).unwrap();
        assert!(restored.nodes.contains_key(&NodeId::from("a")));
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
        assert!(editor.document.nodes.is_empty());
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
        assert!(editor.document.nodes.contains_key(&NodeId::from("a")));
        assert_eq!(editor.history.undo_depth(), 1);
        assert_eq!(editor.history.redo_depth(), 0);
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
        assert!(editor.selection.nodes.contains(&NodeId::from("a")));
        let restored = load_canvas_document(&store).unwrap();
        assert!(restored.nodes.contains_key(&NodeId::from("a")));
    }

    #[test]
    fn persistent_tool_effects_commit_unrecorded_gesture_on_push_undo() {
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
        let inverse = CanvasTransaction::single(DocumentCommand::UpdateNode(original));

        apply_persistent_tool_effects(
            &mut editor,
            &mut store,
            &mut cursor,
            [
                CanvasToolEffect::ApplyUnrecorded(CanvasTransaction::single(
                    DocumentCommand::UpdateNode(moved.clone()),
                )),
                CanvasToolEffect::PushUndo(inverse),
            ],
        )
        .unwrap();

        assert_eq!(cursor.sequence(), 1);
        assert_eq!(store.log_entries().len(), 1);
        assert_eq!(editor.history.undo_depth(), 1);
        assert_eq!(editor.document.nodes[&NodeId::from("a")], moved);

        let restored = load_canvas_document(&store).unwrap();
        assert_eq!(
            restored.nodes[&NodeId::from("a")].position,
            point(px(40.0), px(0.0))
        );
    }

    #[test]
    fn persistent_tool_effects_keep_unrecorded_effects_out_of_log() {
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
            CanvasToolEffect::ApplyUnrecorded(CanvasTransaction::single(
                DocumentCommand::UpdateNode(moved),
            )),
        )
        .unwrap();

        assert_eq!(cursor.sequence(), 9);
        assert!(store.log_entries().is_empty());
        assert_eq!(
            editor.document.nodes[&NodeId::from("a")].position,
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
            .save_checkpoint(CanvasCheckpoint::new(0, &editor.document))
            .unwrap();
        let mut cursor = CanvasPersistenceCursor::default();

        handle_persistent_event(
            &mut editor,
            &mut store,
            &mut cursor,
            CanvasEvent::PointerDown {
                position: point(px(10.0), px(10.0)),
                button: PointerButton::Primary,
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
            },
        )
        .unwrap();

        assert_eq!(editor.document.edges.len(), 1);
        assert_eq!(editor.history.undo_depth(), 1);
        assert_eq!(cursor.sequence(), 1);
        assert_eq!(store.log_entries().len(), 1);
        assert!(matches!(
            store.log_entries()[0].transaction.commands.as_slice(),
            [DocumentCommand::InsertEdge(edge)]
                if edge.source.node_id == NodeId::from("a")
                    && edge.target.node_id == NodeId::from("b")
        ));

        let restored = load_canvas_document(&store).unwrap();
        assert_eq!(restored.edges.len(), 1);
    }

    #[test]
    fn persistent_event_dispatch_logs_custom_tool_transaction() {
        let mut editor = CanvasEditor::default();
        editor.viewport = crate::CanvasViewport::new(point(px(100.0), px(50.0)), 2.0).unwrap();
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
            },
            &mut tool,
        )
        .unwrap();

        assert_eq!(tool.calls, 1);
        assert_eq!(cursor.sequence(), 1);
        assert_eq!(store.log_entries().len(), 1);
        assert_eq!(
            editor.document.nodes[&NodeId::from("persistent-stamp-0")].position,
            point(px(110.0), px(55.0))
        );

        let restored = load_canvas_document(&store).unwrap();
        assert!(
            restored
                .nodes
                .contains_key(&NodeId::from("persistent-stamp-0"))
        );
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
            },
            &mut registry,
        )
        .unwrap();

        assert_eq!(cursor.sequence(), 1);
        assert_eq!(store.log_entries().len(), 1);
        assert!(
            editor
                .document
                .nodes
                .contains_key(&NodeId::from("persistent-stamp-0"))
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
        assert!(editor.document.nodes.is_empty());
        assert_eq!(editor.history.undo_depth(), 0);
    }
}
