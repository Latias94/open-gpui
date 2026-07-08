use serde::{Deserialize, Serialize};

/// Redaction summary attached to exported snapshots.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotRedactionSummary {
    /// Number of values hidden by redaction.
    pub redacted_values: usize,
    /// Human-readable redaction notes.
    pub notes: Vec<String>,
}

impl SnapshotRedactionSummary {
    /// Records one redacted value with a short note.
    pub fn record_redacted(&mut self, note: impl Into<String>) {
        self.redacted_values += 1;
        self.notes.push(note.into());
    }

    /// Returns true when the snapshot contains redacted values.
    pub fn has_redactions(&self) -> bool {
        self.redacted_values > 0
    }
}
