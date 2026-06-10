use super::{CanvasCheckpoint, CanvasLogEntry};
use crate::{DocumentError, validate_canvas_document_format_version};
use std::{error::Error, fmt};

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
            Self::LogEntry(entry) => entry.sequence(),
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

    if let Err(DocumentError::UnsupportedFormatVersion { expected, found }) =
        validate_canvas_document_format_version(envelope.document_format_version)
    {
        return Err(
            CanvasPersistenceCodecError::UnsupportedDocumentFormatVersion { expected, found },
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
