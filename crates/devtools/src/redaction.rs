use serde::{Deserialize, Serialize};

/// Redaction summary attached to exported snapshots.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotRedactionSummary {
    /// Number of values hidden by redaction.
    pub redacted_values: usize,
    /// Human-readable redaction notes.
    pub notes: Vec<String>,
}
