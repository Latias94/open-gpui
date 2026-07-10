use open_gpui_form::{FieldPath, FormStatus, FormStore, RedactionPolicy};
use open_gpui_ui_components::{
    FormFieldConfig, FormFieldProjection, FormProjection, form_checkbox_value, form_number_value,
    form_select_value, form_text_value,
};
use open_gpui_ui_core::ThemeTokens;

#[test]
fn form_field_projection_maps_errors_and_submit_disabled_state() {
    let mut store = FormStore::default();
    let email = FieldPath::new("account.email").unwrap();
    store
        .register_field(email.clone(), serde_json::json!("bad"))
        .unwrap();
    store
        .validate_field(&email, |_| vec!["Email must contain @".to_owned()])
        .unwrap();

    let snapshot = store.snapshot(RedactionPolicy::Expose);
    let field = snapshot.field(&email).unwrap();
    let projection = FormFieldProjection::resolve(
        FormStatus::Submitting,
        field,
        FormFieldConfig::new("Email")
            .required(true)
            .help("Use a work email"),
    );

    assert!(projection.invalid());
    assert_eq!(projection.error(), Some("Email must contain @"));
    assert!(projection.disabled());

    let field_state = projection.field_state(ThemeTokens::default());
    assert!(field_state.invalid());
    assert!(field_state.disabled());
    assert_eq!(
        field_state.message().unwrap().text(),
        "Email must contain @"
    );
}

#[test]
fn form_projection_resolves_text_and_textarea_states() {
    let projection = FormFieldProjection::resolve(
        Default::default(),
        &field_snapshot("profile.bio", serde_json::json!("hello\nworld")),
        FormFieldConfig::new("Bio").read_only(true),
    );

    let text = projection.text_input_state(
        form_text_value(&serde_json::json!("hello\nworld")),
        Some("Single line"),
        ThemeTokens::default(),
    );
    let textarea = projection.textarea_state(
        form_text_value(&serde_json::json!("hello\nworld")),
        Some("Multiline"),
        3,
        ThemeTokens::default(),
    );

    assert_eq!(text.value(), "hello world");
    assert!(text.read_only());
    assert_eq!(textarea.value(), "hello\nworld");
    assert!(textarea.read_only());
}

#[test]
fn form_projection_resolves_number_checkbox_and_select_values() {
    let projection = FormFieldProjection::resolve(
        Default::default(),
        &field_snapshot("settings.enabled", serde_json::json!(true)),
        FormFieldConfig::new("Enabled").required(true),
    );

    let number = projection.number_input_state(
        form_number_value(&serde_json::json!(42.0)).unwrap(),
        0.0,
        100.0,
        1.0,
        ThemeTokens::default(),
    );
    let checkbox = projection.checkbox_state(
        form_checkbox_value(&serde_json::json!(true)).unwrap(),
        ThemeTokens::default(),
    );

    assert_eq!(number.value(), 42.0);
    assert!(checkbox.checked());
    assert_eq!(
        form_select_value(&serde_json::json!("option-a")).as_deref(),
        Some("option-a")
    );
}

#[test]
fn form_projection_preserves_editability_while_validation_is_busy() {
    let mut store = FormStore::default();
    let email = FieldPath::new("account.email").unwrap();
    store
        .register_field(email.clone(), serde_json::json!("team@example.com"))
        .unwrap();
    let _ticket = store.begin_async_validation(&email).unwrap();
    let snapshot = store.snapshot(RedactionPolicy::Expose);
    let form = FormProjection::resolve(&snapshot, false);
    let field = FormFieldProjection::resolve(
        snapshot.status,
        snapshot.field(&email).unwrap(),
        FormFieldConfig::new("Email").required(true),
    );

    assert_eq!(snapshot.status, FormStatus::Validating);
    assert!(form.busy());
    assert!(form.validating());
    assert!(!form.submit_enabled());
    assert!(field.validating());
    assert!(field.busy());
    assert!(!field.disabled());

    let field_state = field.field_state(ThemeTokens::default());
    let text_state = field.text_input_state(
        "team@example.com",
        Some("you@example.com"),
        ThemeTokens::default(),
    );
    let textarea_state = field.textarea_state("notes", Some("notes"), 3, ThemeTokens::default());
    let number_state = field.number_input_state(3.0, 1.0, 5.0, 1.0, ThemeTokens::default());
    let checkbox_state = field.checkbox_state(true, ThemeTokens::default());

    assert!(field_state.busy());
    assert!(text_state.busy());
    assert!(text_state.control_state().input_enabled());
    assert!(textarea_state.busy());
    assert!(number_state.busy());
    assert!(checkbox_state.busy());
}

fn field_snapshot(path: &str, value: serde_json::Value) -> open_gpui_form::FieldSnapshot {
    let mut store = FormStore::default();
    let path = FieldPath::new(path).unwrap();
    store.register_field(path.clone(), value).unwrap();
    store
        .snapshot(RedactionPolicy::Expose)
        .field(&path)
        .unwrap()
        .clone()
}
