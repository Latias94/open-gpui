use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::SerializeStruct};

use crate::adapters::sanitize_sensitive_text;

/// Redaction summary attached to exported snapshots.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
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
        self.notes.push(sanitize_sensitive_text(&note.into()));
    }

    /// Merges another redaction summary into this summary.
    pub fn merge(&mut self, other: Self) {
        self.redacted_values += other.redacted_values;
        self.notes.extend(
            other
                .notes
                .into_iter()
                .map(|note| sanitize_sensitive_text(&note)),
        );
    }

    /// Returns a merged redaction summary.
    pub fn merged(mut self, other: Self) -> Self {
        self.merge(other);
        self
    }

    /// Returns this summary with all notes sanitized.
    pub fn sanitized(mut self) -> Self {
        self.notes = self
            .notes
            .into_iter()
            .map(|note| sanitize_sensitive_text(&note))
            .collect();
        self
    }

    /// Returns true when the snapshot contains redacted values.
    pub fn has_redactions(&self) -> bool {
        self.redacted_values > 0
    }
}

impl Serialize for SnapshotRedactionSummary {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let notes = self
            .notes
            .iter()
            .map(|note| sanitize_sensitive_text(note))
            .collect::<Vec<_>>();
        let mut state = serializer.serialize_struct("SnapshotRedactionSummary", 2)?;
        state.serialize_field("redacted_values", &self.redacted_values)?;
        state.serialize_field("notes", &notes)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for SnapshotRedactionSummary {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct SnapshotRedactionSummaryValue {
            redacted_values: usize,
            notes: Vec<String>,
        }

        let value = SnapshotRedactionSummaryValue::deserialize(deserializer)?;
        Ok(Self {
            redacted_values: value.redacted_values,
            notes: value
                .notes
                .into_iter()
                .map(|note| sanitize_sensitive_text(&note))
                .collect(),
        })
    }
}
