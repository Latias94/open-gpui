use serde::{Deserialize, Serialize};

use crate::SnapshotEnvelope;

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
pub trait DevtoolsProbe {
    /// Returns the stable probe id.
    fn id(&self) -> &ProbeId;

    /// Collects the current read-only snapshot.
    fn snapshot(&self) -> Result<SnapshotEnvelope, ProbeSnapshotError>;
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
