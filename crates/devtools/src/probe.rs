use serde::{Deserialize, Serialize};

use crate::{SnapshotEnvelope, SnapshotKind, SnapshotRedactionSummary, SnapshotTree};

/// Stable id for a devtools probe.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ProbeId(String);

impl ProbeId {
    /// Creates a non-empty probe id.
    pub fn new(id: impl Into<String>) -> Result<Self, ProbeSnapshotError> {
        let id = id.into();
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
        self.redaction = redaction;
        self
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
