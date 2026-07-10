use serde::{Deserialize, Serialize};

use crate::{FieldId, FieldPath, RedactedValue};

/// Current lifecycle status for a form.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum FormStatus {
    /// No submission or validation work is currently running.
    #[default]
    Idle,
    /// One or more fields are validating.
    Validating,
    /// The form is submitting.
    Submitting,
    /// The latest submit completed.
    Submitted,
    /// The latest submit failed.
    SubmitFailed,
}

/// Renderer-neutral field meta state.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FieldMetaSnapshot {
    /// Whether the current value differs from the initial value.
    pub dirty: bool,
    /// Whether the user has focused or interacted with the field.
    pub touched: bool,
    /// Whether the field has been visited.
    pub visited: bool,
    /// Whether validation is currently pending.
    pub validating: bool,
    /// Current validation errors.
    pub errors: Vec<String>,
}

/// Redaction-aware field snapshot for tests, adapters, and devtools.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FieldSnapshot {
    /// Stable field id.
    pub id: FieldId,
    /// Stable field path.
    pub path: FieldPath,
    /// Redacted field value.
    pub value: RedactedValue,
    /// Field meta state.
    pub meta: FieldMetaSnapshot,
}

/// Redaction-aware form snapshot for tests, adapters, and devtools.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormSnapshot {
    /// Current form status.
    pub status: FormStatus,
    /// Field snapshots in deterministic display order.
    pub fields: Vec<FieldSnapshot>,
    /// Form-level errors.
    pub errors: Vec<String>,
    /// Number of submit attempts.
    pub submit_count: u32,
}

impl FormSnapshot {
    /// Returns a field snapshot by path.
    pub fn field(&self, path: &FieldPath) -> Option<&FieldSnapshot> {
        self.fields.iter().find(|field| &field.path == path)
    }

    /// Returns the number of fields with current validation work.
    pub fn validating_field_count(&self) -> usize {
        self.fields
            .iter()
            .filter(|field| field.meta.validating)
            .count()
    }

    /// Returns the number of fields with validation errors.
    pub fn invalid_field_count(&self) -> usize {
        self.fields
            .iter()
            .filter(|field| !field.meta.errors.is_empty())
            .count()
    }

    /// Returns whether validation or submission work is active.
    pub fn is_busy(&self) -> bool {
        matches!(self.status, FormStatus::Validating | FormStatus::Submitting)
    }

    /// Returns whether the current snapshot is eligible to begin submission.
    pub fn can_submit(&self) -> bool {
        !self.is_busy() && self.invalid_field_count() == 0
    }
}
