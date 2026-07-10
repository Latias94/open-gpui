use open_gpui_form::{
    FieldPath, FormStatus, FormStore, RedactionPolicy, SubmitCompletion, ValidationCompletion,
};

use super::SampleRuntimeLog;

/// Form lifecycle action executed by the deterministic Gallery runtime scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormSampleRuntimeAction {
    /// Start asynchronous validation for the current field value.
    BeginValidation,
    /// Apply the current asynchronous validation result.
    CompleteValidation,
    /// Start the first submission attempt.
    BeginSubmission,
    /// Complete the first submission with a failure.
    FailSubmission,
    /// Edit the form after a terminal failure.
    EditAfterFailure,
    /// Start a second submission attempt.
    RetrySubmission,
    /// Complete the retry successfully.
    CompleteSubmission,
    /// Reset values, metadata, tickets, and terminal state.
    Reset,
}

/// Typed completion attached to a Gallery form runtime event when applicable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormSampleRuntimeCompletion {
    /// Completion of asynchronous validation work.
    Validation(ValidationCompletion),
    /// Completion of submission work.
    Submission(SubmitCompletion),
}

/// Observable facts captured after one real `FormStore` lifecycle transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormSampleRuntimeEvent {
    /// Action executed before these facts were captured.
    pub action: FormSampleRuntimeAction,
    /// Effective status derived by the store.
    pub status: FormStatus,
    /// Typed completion returned by the action, when it completes asynchronous work.
    pub completion: Option<FormSampleRuntimeCompletion>,
    /// Number of fields with current validation work.
    pub validating_field_count: usize,
    /// Cumulative count of accepted submission starts.
    pub submit_count: u32,
    /// Whether a new submission can begin after this transition.
    pub can_submit: bool,
}

impl FormSampleRuntimeEvent {
    fn capture(
        action: FormSampleRuntimeAction,
        form: &FormStore,
        completion: Option<FormSampleRuntimeCompletion>,
    ) -> Self {
        let snapshot = form.snapshot(RedactionPolicy::RedactAll);
        Self {
            action,
            status: snapshot.status,
            completion,
            validating_field_count: snapshot.validating_field_count(),
            submit_count: snapshot.submit_count,
            can_submit: snapshot.can_submit(),
        }
    }
}

/// Runtime trace produced by a real form lifecycle scenario.
pub type FormSampleRuntimeLog = SampleRuntimeLog<FormSampleRuntimeEvent>;

/// Executes and returns the deterministic form lifecycle trace shown by the Gallery.
pub fn form_sample_runtime_log() -> FormSampleRuntimeLog {
    let mut form = FormStore::default();
    let email = FieldPath::new("account.email").expect("valid Gallery field path");
    form.register_field(email.clone(), serde_json::json!("team@example.com"))
        .expect("Gallery form field registers");

    let mut events = Vec::new();
    let validation = form
        .begin_async_validation(&email)
        .expect("Gallery validation starts");
    events.push(FormSampleRuntimeEvent::capture(
        FormSampleRuntimeAction::BeginValidation,
        &form,
        None,
    ));

    let completion = form.complete_async_validation(validation, Vec::new());
    events.push(FormSampleRuntimeEvent::capture(
        FormSampleRuntimeAction::CompleteValidation,
        &form,
        Some(FormSampleRuntimeCompletion::Validation(completion)),
    ));

    let submit = form.begin_submit().expect("Gallery submission starts");
    events.push(FormSampleRuntimeEvent::capture(
        FormSampleRuntimeAction::BeginSubmission,
        &form,
        None,
    ));

    let completion = form.finish_submit_error(submit, "Gallery sample failure");
    events.push(FormSampleRuntimeEvent::capture(
        FormSampleRuntimeAction::FailSubmission,
        &form,
        Some(FormSampleRuntimeCompletion::Submission(completion)),
    ));

    form.set_value(&email, serde_json::json!("retry@example.com"))
        .expect("Gallery form edit succeeds");
    events.push(FormSampleRuntimeEvent::capture(
        FormSampleRuntimeAction::EditAfterFailure,
        &form,
        None,
    ));

    let retry = form.begin_submit().expect("Gallery submission retries");
    events.push(FormSampleRuntimeEvent::capture(
        FormSampleRuntimeAction::RetrySubmission,
        &form,
        None,
    ));

    let completion = form.finish_submit_success(retry);
    events.push(FormSampleRuntimeEvent::capture(
        FormSampleRuntimeAction::CompleteSubmission,
        &form,
        Some(FormSampleRuntimeCompletion::Submission(completion)),
    ));

    form.reset();
    events.push(FormSampleRuntimeEvent::capture(
        FormSampleRuntimeAction::Reset,
        &form,
        None,
    ));

    SampleRuntimeLog::new("form-adapters", events)
}
