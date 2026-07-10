use super::*;
use open_gpui_form::{FieldPath, FormSnapshot, FormStore, RedactedValue, RedactionPolicy};
use open_gpui_ui_components::{
    FormFieldConfig, FormFieldProjection, FormProjection, form_checkbox_value, form_number_value,
    form_select_value, form_text_value,
};

const SUBMIT_ERROR_CANARY: &str = "violet meadow 744";
const SUBMIT_VALUE_CANARY: &str = "silver harbor 319";
const SUBMIT_NOTES_CANARY: &str = "indigo summit 638";
const VALIDATION_VALUE_CANARY: &str = "azure ridge 581";
const VALIDATION_ERROR_CANARY: &str = "amber canyon 882";

/// Unique source strings that must never survive the Gallery DevTools adapter boundary.
pub const FORM_DEVTOOLS_DOGFOOD_CANARIES: &[&str] = &[
    SUBMIT_ERROR_CANARY,
    SUBMIT_VALUE_CANARY,
    SUBMIT_NOTES_CANARY,
    VALIDATION_VALUE_CANARY,
    VALIDATION_ERROR_CANARY,
];

/// One form-adapter integration sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct FormAdapterSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible sample title.
    pub title: &'static str,
    /// Short explanation of the adapter slice.
    pub summary: &'static str,
    /// Form-level lifecycle and submit projection.
    pub form: FormProjection,
    /// Resolved email field wrapper state.
    pub email_field: FieldState,
    /// Resolved email text input state.
    pub email_input: TextInputState,
    /// Resolved notes field wrapper state.
    pub notes_field: FieldState,
    /// Resolved notes textarea state.
    pub notes_textarea: TextareaState,
    /// Resolved seats number input state.
    pub seats_input: NumberInputState,
    /// Resolved alerts checkbox state.
    pub alerts_checkbox: CheckboxState,
    /// Select value extracted from form data.
    pub region_value: String,
    /// Number of fields in the exposed sample snapshot.
    pub field_count: usize,
    /// Number of fields with validation errors.
    pub invalid_field_count: usize,
    /// Number of fields with pending validation work.
    pub validating_field_count: usize,
    /// Number of fields marked dirty.
    pub dirty_field_count: usize,
    /// Number of fields redacted by the default diagnostic snapshot.
    pub redacted_field_count: usize,
    /// Redacted snapshot shared with the DevTools dogfood page.
    pub redacted_snapshot: FormSnapshot,
}

/// Returns form-adapter samples backed by the headless form store.
pub fn form_adapter_samples(tokens: ThemeTokens) -> Vec<FormAdapterSample> {
    vec![
        invalid_form_sample(tokens),
        validating_form_sample(tokens),
        submitting_form_sample(tokens),
        reset_form_sample(tokens),
    ]
}

/// Returns the deterministic form snapshot consumed by the DevTools dogfood page.
pub fn form_devtools_dogfood_snapshot() -> FormSnapshot {
    let mut store = base_form_store();
    let email = path("account.email");
    let notes = path("profile.notes");
    store
        .set_value(&email, serde_json::json!(SUBMIT_VALUE_CANARY))
        .unwrap();
    store
        .set_value(&notes, serde_json::json!(SUBMIT_NOTES_CANARY))
        .unwrap();
    store.validate_field(&email, |_| Vec::new()).unwrap();
    let submit = store.begin_submit().unwrap();
    let _completion = store.finish_submit_error(submit, SUBMIT_ERROR_CANARY);
    store.snapshot(RedactionPolicy::Expose)
}

/// Returns the first-frame validation snapshot used by the DevTools dogfood session.
pub fn form_devtools_validation_dogfood_snapshot() -> FormSnapshot {
    let mut store = base_form_store();
    let email = path("account.email");
    store
        .set_value(&email, serde_json::json!(VALIDATION_VALUE_CANARY))
        .unwrap();
    store
        .validate_field(&email, |_| vec![VALIDATION_ERROR_CANARY.to_owned()])
        .unwrap();
    store.snapshot(RedactionPolicy::Expose)
}

fn invalid_form_sample(tokens: ThemeTokens) -> FormAdapterSample {
    let mut store = base_form_store();
    let email = path("account.email");
    store.touch(&email).unwrap();
    store.visit(&email).unwrap();
    store
        .set_value(&email, serde_json::json!("not-an-email"))
        .unwrap();
    store
        .validate_field(&email, |_| vec!["Enter a valid work email.".to_owned()])
        .unwrap();

    build_form_adapter_sample(
        "validation",
        "Validated profile",
        "Validation errors from FormStore resolve through Field and TextInput state.",
        store,
        tokens,
    )
}

fn submitting_form_sample(tokens: ThemeTokens) -> FormAdapterSample {
    let mut store = base_form_store();
    let notes = path("profile.notes");
    store
        .set_value(&notes, serde_json::json!("Ship adapter samples."))
        .unwrap();
    let _submit = store.begin_submit().unwrap();

    build_form_adapter_sample(
        "submitting",
        "Submitting profile",
        "Submitting status disables projected controls while app-owned values stay outside components.",
        store,
        tokens,
    )
}

fn validating_form_sample(tokens: ThemeTokens) -> FormAdapterSample {
    let mut store = base_form_store();
    let email = path("account.email");
    store
        .set_value(&email, serde_json::json!("pending@example.com"))
        .unwrap();
    let _validation = store.begin_async_validation(&email).unwrap();

    build_form_adapter_sample(
        "validating",
        "Validating profile",
        "A real pending validation projects busy state while fields remain editable.",
        store,
        tokens,
    )
}

fn reset_form_sample(tokens: ThemeTokens) -> FormAdapterSample {
    let mut store = base_form_store();
    let seats = path("workspace.seats");
    store.set_value(&seats, serde_json::json!(12)).unwrap();
    store.reset();

    build_form_adapter_sample(
        "reset",
        "Reset profile",
        "Reset restores initial values and clears dirty metadata before projection.",
        store,
        tokens,
    )
}

fn build_form_adapter_sample(
    id: &'static str,
    title: &'static str,
    summary: &'static str,
    store: FormStore,
    tokens: ThemeTokens,
) -> FormAdapterSample {
    let exposed = store.snapshot(RedactionPolicy::Expose);
    let redacted = store.snapshot(RedactionPolicy::RedactAll);
    let form = FormProjection::resolve(&exposed, false);
    let email = path("account.email");
    let notes = path("profile.notes");
    let seats = path("workspace.seats");
    let alerts = path("workspace.alerts");
    let region = path("workspace.region");

    let email_projection = projection(
        &exposed,
        &email,
        FormFieldConfig::new("Email")
            .required(true)
            .help("Use a work email."),
    );
    let notes_projection = projection(
        &exposed,
        &notes,
        FormFieldConfig::new("Release notes").help("Summarize the workflow change."),
    );
    let seats_projection = projection(
        &exposed,
        &seats,
        FormFieldConfig::new("Seats").required(true),
    );
    let alerts_projection = projection(
        &exposed,
        &alerts,
        FormFieldConfig::new("Alerts").help("Notify the workspace owner."),
    );

    let email_value = form_text_value(store.value(&email).unwrap());
    let notes_value = form_text_value(store.value(&notes).unwrap());
    let seats_value = form_number_value(store.value(&seats).unwrap()).unwrap_or_default();
    let alerts_value = form_checkbox_value(store.value(&alerts).unwrap()).unwrap_or(false);
    let region_value = form_select_value(store.value(&region).unwrap()).unwrap_or_default();

    FormAdapterSample {
        id,
        title,
        summary,
        form,
        email_field: email_projection.field_state(tokens),
        email_input: email_projection.text_input_state(
            email_value,
            Some("you@example.com"),
            tokens,
        ),
        notes_field: notes_projection.field_state(tokens),
        notes_textarea: notes_projection.textarea_state(
            notes_value,
            Some("Release note"),
            3,
            tokens,
        ),
        seats_input: seats_projection.number_input_state(seats_value, 1.0, 24.0, 1.0, tokens),
        alerts_checkbox: alerts_projection.checkbox_state(alerts_value, tokens),
        region_value,
        field_count: exposed.fields.len(),
        invalid_field_count: exposed
            .fields
            .iter()
            .filter(|field| !field.meta.errors.is_empty())
            .count(),
        validating_field_count: exposed.validating_field_count(),
        dirty_field_count: exposed
            .fields
            .iter()
            .filter(|field| field.meta.dirty)
            .count(),
        redacted_field_count: redacted
            .fields
            .iter()
            .filter(|field| matches!(field.value, RedactedValue::Redacted))
            .count(),
        redacted_snapshot: redacted,
    }
}

fn projection(
    snapshot: &FormSnapshot,
    path: &FieldPath,
    config: FormFieldConfig,
) -> FormFieldProjection {
    FormFieldProjection::resolve(snapshot.status, snapshot.field(path).unwrap(), config)
}

fn base_form_store() -> FormStore {
    let mut store = FormStore::default();
    store
        .register_field(path("account.email"), serde_json::json!("team@example.com"))
        .unwrap();
    store
        .register_field(path("profile.notes"), serde_json::json!("Initial rollout."))
        .unwrap();
    store
        .register_field(path("workspace.seats"), serde_json::json!(8))
        .unwrap();
    store
        .register_field(path("workspace.alerts"), serde_json::json!(true))
        .unwrap();
    store
        .register_field(path("workspace.region"), serde_json::json!("us-east"))
        .unwrap();
    store
}

fn path(value: &str) -> FieldPath {
    FieldPath::new(value).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui_form::FormStatus;

    #[test]
    fn form_adapter_samples_cover_validation_submit_and_reset() {
        let samples = form_adapter_samples(ThemeTokens::default());
        let validation = samples
            .iter()
            .find(|sample| sample.id == "validation")
            .unwrap();
        let submitting = samples
            .iter()
            .find(|sample| sample.id == "submitting")
            .unwrap();
        let validating = samples
            .iter()
            .find(|sample| sample.id == "validating")
            .unwrap();
        let reset = samples.iter().find(|sample| sample.id == "reset").unwrap();

        assert_eq!(validation.form.status(), &FormStatus::Idle);
        assert!(validation.email_field.invalid());
        assert!(validation.email_input.invalid());
        assert_eq!(validation.invalid_field_count, 1);
        assert_eq!(validation.redacted_field_count, validation.field_count);
        assert_eq!(validation.region_value, "us-east");

        assert_eq!(submitting.form.status(), &FormStatus::Submitting);
        assert!(submitting.email_input.disabled());
        assert!(submitting.alerts_checkbox.disabled());

        assert_eq!(validating.form.status(), &FormStatus::Validating);
        assert_eq!(validating.validating_field_count, 1);
        assert!(validating.email_field.busy());
        assert!(validating.email_input.busy());
        assert!(!validating.email_input.disabled());
        assert!(validating.email_input.control_state().input_enabled());

        assert_eq!(reset.dirty_field_count, 0);
        assert_eq!(reset.seats_input.value(), 8.0);
    }
}
