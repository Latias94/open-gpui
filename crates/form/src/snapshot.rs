use serde::{Deserialize, Serialize};

use crate::{FieldId, FieldPath, RedactedValue};

/// Current lifecycle status for a form.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
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
