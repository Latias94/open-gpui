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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormSampleRuntimeLog {
    /// Stable sample id.
    pub sample_id: &'static str,
    /// Deterministic actions represented by the sample set.
    pub actions: Vec<FormSampleRuntimeAction>,
}

/// Returns the deterministic form sample runtime log.
pub fn form_sample_runtime_log() -> FormSampleRuntimeLog {
    FormSampleRuntimeLog {
        sample_id: "form-adapters",
        actions: vec![
            FormSampleRuntimeAction::Validate,
            FormSampleRuntimeAction::Submit,
            FormSampleRuntimeAction::Reset,
        ],
    }
}
