//! Headless DevTools artifact records and sinks.

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    DevtoolsCapture, DevtoolsReport, DevtoolsSessionExport, adapters::sanitize_sensitive_text,
};

/// Schema version used by serialized DevTools artifact records.
pub const DEVTOOLS_ARTIFACT_RECORD_SCHEMA_VERSION: &str = "open-gpui-devtools-artifact-record/v1";

/// One supported artifact payload kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DevtoolsArtifactKind {
    /// A single sanitized capture.
    Capture,
    /// A bounded sanitized session export.
    SessionExport,
    /// A derived diagnostics report.
    Report,
}

impl DevtoolsArtifactKind {
    /// Returns the stable label for this artifact kind.
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Capture => "capture",
            Self::SessionExport => "session-export",
            Self::Report => "report",
        }
    }
}

/// Payload carried by one DevTools artifact record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "kebab-case")]
pub enum DevtoolsArtifact {
    /// A single sanitized capture.
    Capture(DevtoolsCapture),
    /// A bounded sanitized session export.
    SessionExport(DevtoolsSessionExport),
    /// A derived diagnostics report.
    Report(DevtoolsReport),
}

impl DevtoolsArtifact {
    /// Builds a capture artifact from a sanitized capture clone.
    pub fn capture(capture: &DevtoolsCapture) -> Self {
        Self::Capture(capture.clone().sanitized())
    }

    /// Builds a session-export artifact.
    pub fn session_export(export: &DevtoolsSessionExport) -> Self {
        Self::SessionExport(export.clone())
    }

    /// Builds a report artifact.
    pub fn report(report: &DevtoolsReport) -> Self {
        Self::Report(report.clone())
    }

    /// Returns the artifact kind.
    pub const fn kind(&self) -> DevtoolsArtifactKind {
        match self {
            Self::Capture(_) => DevtoolsArtifactKind::Capture,
            Self::SessionExport(_) => DevtoolsArtifactKind::SessionExport,
            Self::Report(_) => DevtoolsArtifactKind::Report,
        }
    }

    fn redacted_value_count(&self) -> usize {
        match self {
            Self::Capture(capture) => redacted_values_for_capture(capture),
            Self::SessionExport(export) => export
                .frames
                .iter()
                .map(|frame| redacted_values_for_capture(&frame.capture))
                .sum(),
            Self::Report(report) => report.summary.redacted_value_count,
        }
    }

    fn session_id(&self) -> Option<&str> {
        match self {
            Self::Capture(_) => None,
            Self::SessionExport(export) => Some(export.session_id.as_str()),
            Self::Report(report) => report.source.session_id.as_deref(),
        }
    }

    fn generation(&self) -> Option<u64> {
        match self {
            Self::Capture(_) => None,
            Self::SessionExport(export) => export.current_generation,
            Self::Report(report) => report.source.generation,
        }
    }
}

/// Metadata attached to a DevTools artifact record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DevtoolsArtifactMetadata {
    /// Sanitized producer id.
    pub producer_id: String,
    /// Optional sanitized scenario id.
    pub scenario_id: Option<String>,
    /// Sanitized session id when available.
    pub session_id: Option<String>,
    /// Current generation when available.
    pub generation: Option<u64>,
    /// Producer-assigned monotonic artifact sequence.
    pub sequence: u64,
    /// Sanitized reason this record was flushed.
    pub flush_reason: String,
    /// Optional producer timestamp in milliseconds.
    pub timestamp_ms: Option<u64>,
    /// Redacted value count derived from the payload.
    pub redacted_value_count: usize,
}

impl DevtoolsArtifactMetadata {
    /// Creates artifact metadata with a sanitized producer id.
    pub fn new(producer_id: impl Into<String>) -> Self {
        Self {
            producer_id: sanitize_or_default(producer_id.into(), "devtools.producer"),
            scenario_id: None,
            session_id: None,
            generation: None,
            sequence: 0,
            flush_reason: "manual".to_owned(),
            timestamp_ms: None,
            redacted_value_count: 0,
        }
    }

    /// Attaches an optional scenario id.
    pub fn scenario_id(mut self, scenario_id: impl Into<String>) -> Self {
        let scenario_id = sanitize_sensitive_text(&scenario_id.into());
        self.scenario_id = (!scenario_id.trim().is_empty()).then_some(scenario_id);
        self
    }

    /// Sets the artifact sequence.
    pub const fn sequence(mut self, sequence: u64) -> Self {
        self.sequence = sequence;
        self
    }

    /// Sets the flush reason.
    pub fn flush_reason(mut self, flush_reason: impl Into<String>) -> Self {
        self.flush_reason = sanitize_or_default(flush_reason.into(), "manual");
        self
    }

    /// Sets the optional timestamp.
    pub const fn timestamp_ms(mut self, timestamp_ms: u64) -> Self {
        self.timestamp_ms = Some(timestamp_ms);
        self
    }

    fn for_artifact(mut self, artifact: &DevtoolsArtifact) -> Self {
        self.producer_id = sanitize_or_default(self.producer_id, "devtools.producer");
        self.scenario_id = self
            .scenario_id
            .map(|scenario_id| sanitize_sensitive_text(&scenario_id))
            .filter(|scenario_id| !scenario_id.trim().is_empty());
        self.flush_reason = sanitize_or_default(self.flush_reason, "manual");
        self.session_id = artifact
            .session_id()
            .map(|session_id| sanitize_sensitive_text(session_id))
            .filter(|session_id| !session_id.trim().is_empty())
            .or_else(|| {
                self.session_id
                    .map(|session_id| sanitize_sensitive_text(&session_id))
            });
        self.generation = artifact.generation().or(self.generation);
        self.redacted_value_count = artifact.redacted_value_count();
        self
    }
}

/// One schema-versioned artifact record suitable for files and JSONL streams.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DevtoolsArtifactRecord {
    /// Artifact record schema version.
    pub schema_version: String,
    /// Artifact payload kind.
    pub artifact_kind: DevtoolsArtifactKind,
    /// Artifact metadata.
    pub metadata: DevtoolsArtifactMetadata,
    /// Artifact payload.
    pub artifact: DevtoolsArtifact,
}

impl DevtoolsArtifactRecord {
    /// Creates a sanitized artifact record.
    pub fn new(metadata: DevtoolsArtifactMetadata, artifact: DevtoolsArtifact) -> Self {
        let artifact_kind = artifact.kind();
        Self {
            schema_version: DEVTOOLS_ARTIFACT_RECORD_SCHEMA_VERSION.to_owned(),
            artifact_kind,
            metadata: metadata.for_artifact(&artifact),
            artifact,
        }
    }

    /// Returns this record with a different sequence.
    pub fn with_sequence(mut self, sequence: u64) -> Self {
        self.metadata.sequence = sequence;
        self
    }

    /// Serializes this record as pretty JSON.
    pub fn to_pretty_json(&self) -> Result<String, DevtoolsArtifactWriteError> {
        serde_json::to_string_pretty(self).map_err(DevtoolsArtifactWriteError::Serialize)
    }

    /// Serializes this record as compact JSON.
    pub fn to_json_line(&self) -> Result<String, DevtoolsArtifactWriteError> {
        serde_json::to_string(self).map_err(DevtoolsArtifactWriteError::Serialize)
    }
}

/// Where a file sink writes artifact records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DevtoolsArtifactFileMode {
    /// Replace the file with a pretty JSON record.
    Replace,
    /// Write a pretty JSON record through a sibling temporary file and rename it into place.
    ReplaceAtomic,
    /// Append one compact JSON record and newline.
    AppendJsonl,
}

/// Error returned while writing DevTools artifacts.
#[derive(Debug, thiserror::Error)]
pub enum DevtoolsArtifactWriteError {
    /// Serialization failed.
    #[error("failed to serialize devtools artifact: {0}")]
    Serialize(serde_json::Error),
    /// Parent directory creation failed.
    #[error("failed to create devtools artifact parent `{path:?}`: {source}")]
    CreateParent {
        /// Parent path.
        path: PathBuf,
        /// Source error.
        source: io::Error,
    },
    /// File write failed.
    #[error("failed to write devtools artifact `{path:?}`: {source}")]
    Write {
        /// Target path.
        path: PathBuf,
        /// Source error.
        source: io::Error,
    },
    /// File append failed.
    #[error("failed to append devtools artifact `{path:?}`: {source}")]
    Append {
        /// Target path.
        path: PathBuf,
        /// Source error.
        source: io::Error,
    },
    /// Atomic rename failed.
    #[error("failed to replace devtools artifact `{to:?}` from `{from:?}`: {source}")]
    Replace {
        /// Temporary source path.
        from: PathBuf,
        /// Final destination path.
        to: PathBuf,
        /// Source error.
        source: io::Error,
    },
    /// Stream write failed.
    #[error("failed to write devtools artifact stream: {0}")]
    Stream(io::Error),
}

/// Sink for writing artifact records.
pub trait DevtoolsArtifactSink {
    /// Writes one artifact record.
    fn write_record(
        &mut self,
        record: &DevtoolsArtifactRecord,
    ) -> Result<(), DevtoolsArtifactWriteError>;
}

/// JSONL artifact sink over any writer.
pub struct DevtoolsArtifactJsonlSink<W> {
    writer: W,
}

impl<W> DevtoolsArtifactJsonlSink<W> {
    /// Creates a JSONL artifact sink.
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Consumes the sink and returns the inner writer.
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W> DevtoolsArtifactSink for DevtoolsArtifactJsonlSink<W>
where
    W: Write,
{
    fn write_record(
        &mut self,
        record: &DevtoolsArtifactRecord,
    ) -> Result<(), DevtoolsArtifactWriteError> {
        let line = record.to_json_line()?;
        writeln!(self.writer, "{line}").map_err(DevtoolsArtifactWriteError::Stream)?;
        self.writer
            .flush()
            .map_err(DevtoolsArtifactWriteError::Stream)
    }
}

/// File-backed artifact sink.
pub struct DevtoolsArtifactFileSink {
    path: PathBuf,
    mode: DevtoolsArtifactFileMode,
}

impl DevtoolsArtifactFileSink {
    /// Creates a file sink.
    pub fn new(path: impl AsRef<Path>, mode: DevtoolsArtifactFileMode) -> Self {
        Self {
            path: path.as_ref().to_owned(),
            mode,
        }
    }
}

impl DevtoolsArtifactSink for DevtoolsArtifactFileSink {
    fn write_record(
        &mut self,
        record: &DevtoolsArtifactRecord,
    ) -> Result<(), DevtoolsArtifactWriteError> {
        ensure_parent(&self.path)?;
        match self.mode {
            DevtoolsArtifactFileMode::Replace => write_replace(&self.path, record),
            DevtoolsArtifactFileMode::ReplaceAtomic => write_replace_atomic(&self.path, record),
            DevtoolsArtifactFileMode::AppendJsonl => write_append_jsonl(&self.path, record),
        }
    }
}

fn redacted_values_for_capture(capture: &DevtoolsCapture) -> usize {
    capture
        .snapshots
        .iter()
        .map(|snapshot| snapshot.redaction.redacted_values)
        .sum()
}

fn sanitize_or_default(value: impl Into<String>, default: &str) -> String {
    let sanitized = sanitize_sensitive_text(&value.into());
    if sanitized.trim().is_empty() {
        default.to_owned()
    } else {
        sanitized
    }
}

fn ensure_parent(path: &Path) -> Result<(), DevtoolsArtifactWriteError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| DevtoolsArtifactWriteError::CreateParent {
            path: parent.to_owned(),
            source,
        })?;
    }
    Ok(())
}

fn write_replace(
    path: &Path,
    record: &DevtoolsArtifactRecord,
) -> Result<(), DevtoolsArtifactWriteError> {
    let json = record.to_pretty_json()?;
    fs::write(path, json).map_err(|source| DevtoolsArtifactWriteError::Write {
        path: path.to_owned(),
        source,
    })
}

fn write_replace_atomic(
    path: &Path,
    record: &DevtoolsArtifactRecord,
) -> Result<(), DevtoolsArtifactWriteError> {
    let json = record.to_pretty_json()?;
    let temp = atomic_temp_path(path);
    fs::write(&temp, json).map_err(|source| DevtoolsArtifactWriteError::Write {
        path: temp.clone(),
        source,
    })?;
    fs::rename(&temp, path).map_err(|source| DevtoolsArtifactWriteError::Replace {
        from: temp,
        to: path.to_owned(),
        source,
    })
}

fn write_append_jsonl(
    path: &Path,
    record: &DevtoolsArtifactRecord,
) -> Result<(), DevtoolsArtifactWriteError> {
    let line = record.to_json_line()?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| DevtoolsArtifactWriteError::Append {
            path: path.to_owned(),
            source,
        })?;
    writeln!(file, "{line}").map_err(|source| DevtoolsArtifactWriteError::Append {
        path: path.to_owned(),
        source,
    })?;
    file.flush()
        .map_err(|source| DevtoolsArtifactWriteError::Append {
            path: path.to_owned(),
            source,
        })
}

fn atomic_temp_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("artifact");
    path.with_file_name(format!(".{file_name}.tmp"))
}
