use crate::FieldPath;

/// Error returned by renderer-neutral form operations.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum FormError {
    /// A field with this path is already registered.
    #[error("field is already registered: {0}")]
    DuplicateField(FieldPath),
    /// No field exists for this path.
    #[error("unknown field: {0}")]
    UnknownField(FieldPath),
    /// The form cannot submit while invalid or validating.
    #[error("form cannot submit: {reason}")]
    CannotSubmit {
        /// Human-readable reason.
        reason: String,
    },
    /// A typed field lens failed.
    #[error("field lens failed: {0}")]
    Lens(String),
}
