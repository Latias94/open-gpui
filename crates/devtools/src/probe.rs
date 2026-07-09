use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    DevtoolsCapture, SnapshotEnvelope, SnapshotKind, SnapshotRedactionSummary, SnapshotTree,
    adapters::sanitize_sensitive_text,
};

/// Stable id for a devtools probe.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProbeId(String);

impl ProbeId {
    /// Creates a non-empty probe id.
    pub fn new(id: impl Into<String>) -> Result<Self, ProbeSnapshotError> {
        let id = sanitize_sensitive_text(&id.into());
        if id.trim().is_empty() {
            return Err(ProbeSnapshotError::EmptyProbeId);
        }
        Ok(Self(id))
    }

    /// Returns the id as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for ProbeId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ProbeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let id = String::deserialize(deserializer)?;
        Self::new(id).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for ProbeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Read-only provider of devtools snapshots.
pub trait DevtoolsProbe: Send + Sync {
    /// Returns the stable probe id.
    fn id(&self) -> &ProbeId;

    /// Collects the current read-only snapshot.
    fn snapshot(&self) -> Result<SnapshotEnvelope, ProbeSnapshotError>;
}

/// Read-only provider of a rich target/domain/event DevTools capture.
pub trait DevtoolsCaptureProvider: Send + Sync {
    /// Returns the stable provider id.
    fn id(&self) -> &ProbeId;

    /// Collects the current read-only capture.
    fn capture(&self) -> Result<DevtoolsCapture, ProbeSnapshotError>;
}

/// Snapshot data returned by a lightweight probe adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotProbeSnapshot {
    tree: SnapshotTree,
    redaction: SnapshotRedactionSummary,
}

impl SnapshotProbeSnapshot {
    /// Creates snapshot data without redaction notes.
    pub fn new(tree: SnapshotTree) -> Self {
        Self {
            tree,
            redaction: SnapshotRedactionSummary::default(),
        }
    }

    /// Attaches a redaction summary.
    pub fn with_redaction(mut self, redaction: SnapshotRedactionSummary) -> Self {
        self.redaction = redaction.sanitized();
        self
    }

    /// Returns the snapshot tree.
    pub fn tree(&self) -> &SnapshotTree {
        &self.tree
    }

    /// Returns the redaction summary.
    pub fn redaction(&self) -> &SnapshotRedactionSummary {
        &self.redaction
    }
}

/// Closure-backed read-only probe adapter.
///
/// Use this for app-owned integrations that can project their runtime state into a devtools
/// snapshot without defining a custom probe type.
pub struct SnapshotProbe<F> {
    id: ProbeId,
    kind: SnapshotKind,
    snapshot: F,
}

impl<F> SnapshotProbe<F> {
    /// Creates a snapshot probe from a non-empty id string.
    pub fn new(
        id: impl Into<String>,
        kind: SnapshotKind,
        snapshot: F,
    ) -> Result<Self, ProbeSnapshotError> {
        Ok(Self {
            id: ProbeId::new(id)?,
            kind,
            snapshot,
        })
    }

    /// Creates a snapshot probe from an existing probe id.
    pub fn from_probe_id(id: ProbeId, kind: SnapshotKind, snapshot: F) -> Self {
        Self { id, kind, snapshot }
    }
}

impl<F> DevtoolsProbe for SnapshotProbe<F>
where
    F: Fn() -> Result<SnapshotProbeSnapshot, ProbeSnapshotError> + Send + Sync,
{
    fn id(&self) -> &ProbeId {
        &self.id
    }

    fn snapshot(&self) -> Result<SnapshotEnvelope, ProbeSnapshotError> {
        let snapshot = (self.snapshot)();
        snapshot.map(|snapshot| {
            SnapshotEnvelope::new(self.id.clone(), self.kind.clone(), snapshot.tree)
                .with_redaction(snapshot.redaction)
        })
    }
}

/// Closure-backed read-only capture provider adapter.
///
/// Use this for app-owned integrations that already produce a target/domain/event capture and
/// should participate in registry-level collection alongside legacy snapshot probes.
pub struct CaptureProvider<F> {
    id: ProbeId,
    capture: F,
}

impl<F> CaptureProvider<F> {
    /// Creates a capture provider from a non-empty id string.
    pub fn new(id: impl Into<String>, capture: F) -> Result<Self, ProbeSnapshotError> {
        Ok(Self {
            id: ProbeId::new(id)?,
            capture,
        })
    }

    /// Creates a capture provider from an existing probe id.
    pub fn from_probe_id(id: ProbeId, capture: F) -> Self {
        Self { id, capture }
    }
}

impl<F> DevtoolsCaptureProvider for CaptureProvider<F>
where
    F: Fn() -> Result<DevtoolsCapture, ProbeSnapshotError> + Send + Sync,
{
    fn id(&self) -> &ProbeId {
        &self.id
    }

    fn capture(&self) -> Result<DevtoolsCapture, ProbeSnapshotError> {
        (self.capture)().map(DevtoolsCapture::sanitized)
    }
}

/// Error returned while collecting probe snapshots.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProbeSnapshotError {
    /// Probe ids cannot be empty.
    #[error("probe id cannot be empty")]
    EmptyProbeId,
    /// Snapshot collection failed.
    #[error("snapshot collection failed: {0}")]
    CollectionFailed(String),
}
