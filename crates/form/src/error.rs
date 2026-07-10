use crate::FieldPath;

/// Typed reason why a form cannot begin submission.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SubmitBlockReason {
    /// One or more fields contain validation errors.
    #[error("validation errors remain")]
    Invalid,
    /// One or more current field validations are still running.
    #[error("validation is still running")]
    Validating,
    /// A submission is already active.
    #[error("submission is already running")]
    AlreadySubmitting,
}

/// Error returned by renderer-neutral form operations.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum FormError {
    /// A field with this path is already registered.
    #[error("field is already registered: {0}")]
    DuplicateField(FieldPath),
    /// No field exists for this path.
    #[error("unknown field: {0}")]
    UnknownField(FieldPath),
    /// The form cannot submit in its current lifecycle state.
    #[error("form cannot submit: {reason}")]
    CannotSubmit {
        /// Stable reason applications can handle without parsing text.
        reason: SubmitBlockReason,
    },
    /// Validation cannot begin while submission owns the form lifecycle.
    #[error("form cannot validate while submitting")]
    CannotValidateWhileSubmitting,
    /// No further lifecycle ticket can be allocated without reusing an identity.
    #[error("form lifecycle generation exhausted")]
    LifecycleGenerationExhausted,
    /// A typed field lens failed.
    #[error("field lens failed: {0}")]
    Lens(String),
}
