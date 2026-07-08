use super::SampleRuntimeLog;

/// Deterministic form runtime action shown by the gallery integration sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormSampleRuntimeAction {
    /// Validate visible fields.
    Validate,
    /// Submit the form.
    Submit,
    /// Reset field values and metadata.
    Reset,
}

/// Read-only runtime log for form adapter samples.
pub type FormSampleRuntimeLog = SampleRuntimeLog<FormSampleRuntimeAction>;

/// Returns the deterministic form sample runtime log.
pub fn form_sample_runtime_log() -> FormSampleRuntimeLog {
    SampleRuntimeLog::new(
        "form-adapters",
        [
            FormSampleRuntimeAction::Validate,
            FormSampleRuntimeAction::Submit,
            FormSampleRuntimeAction::Reset,
        ],
    )
}
