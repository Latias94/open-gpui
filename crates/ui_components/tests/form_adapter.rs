use open_gpui_form::{FieldPath, FormStatus, FormStore, RedactionPolicy};
use open_gpui_ui_components::{
    FormFieldConfig, FormFieldProjection, form_checkbox_value, form_number_value,
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
