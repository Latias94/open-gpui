mod support;

use open_gpui_ui_components::{Field, FormControlState, NumberInput, TextInput, Textarea};
use open_gpui_ui_core::{Size, semantic};

use support::tokens::custom_tokens;

#[test]
fn default_field_state_exposes_label_help_and_metrics() {
    let state = Field::new("email-field", "email", "Email")
        .help("Use a work address.")
        .state();

    assert_eq!(state.label(), "Email");
    assert_eq!(state.help().unwrap(), "Use a work address.");
    assert_eq!(state.support_text().unwrap(), "Use a work address.");
    assert!(!state.support_is_error());
    assert_eq!(state.size(), Size::Medium);
    assert_eq!(
        state.metrics().label_text_size(),
        Size::Medium.control_text_px()
    );
}

#[test]
fn required_field_exposes_required_metadata() {
    let state = Field::new("email-field", "email", "Email")
        .required(true)
        .state();

    assert!(state.required());
    assert_eq!(
        state.colors().required_marker().token(),
        semantic::DESTRUCTIVE
    );
}

#[test]
fn invalid_field_prefers_error_support_text() {
    let tokens = custom_tokens();
    let state = Field::new("email-field", "email", "Email")
        .help("Use a work address.")
        .error("Enter a valid email.")
        .invalid(true)
        .tokens(tokens)
        .state();

    assert!(state.invalid());
    assert_eq!(state.support_text().unwrap(), "Enter a valid email.");
    assert!(state.support_is_error());
    assert_eq!(state.colors().message().token(), tokens.destructive);
}

#[test]
fn disabled_field_uses_muted_label_intent() {
    let tokens = custom_tokens();
    let state = Field::new("email-field", "email", "Email")
        .disabled(true)
        .tokens(tokens)
        .state();

    assert!(state.disabled());
    assert_eq!(state.colors().message().token(), tokens.text_muted);
}

#[test]
fn form_control_state_centralizes_editing_and_focus_rules() {
    let disabled = FormControlState::new(Size::Small)
        .with_disabled(true)
        .with_required(true);
    let read_only = FormControlState::new(Size::Large).with_read_only(true);

    assert_eq!(disabled.size(), Size::Small);
    assert!(disabled.required());
    assert!(!disabled.input_enabled());
    assert!(!disabled.editable());
    assert!(!disabled.activation_enabled());
    assert!(!disabled.tab_stop_enabled());
    assert!(!read_only.input_enabled());
    assert!(read_only.tab_stop_enabled());
}

#[test]
fn field_and_inputs_expose_shared_form_control_state() {
    let field = Field::new("email-field", "email", "Email")
        .required(true)
        .invalid(true)
        .state();
    let text_input = TextInput::new("email", "Email")
        .read_only(true)
        .on_change(|_, _, _| {})
        .state();
    let textarea = Textarea::new("notes", "Notes").disabled(true).state();
    let number_input = NumberInput::new("quantity", "Quantity")
        .required(true)
        .state();

    assert!(field.control_state().required());
    assert!(field.control_state().invalid());
    assert!(text_input.control_state().read_only());
    assert!(text_input.control_state().controller_driven());
    assert!(!text_input.control_state().input_enabled());
    assert!(textarea.control_state().disabled());
    assert!(!textarea.control_state().tab_stop_enabled());
    assert!(number_input.control_state().required());
    assert!(number_input.control_state().input_enabled());
}
