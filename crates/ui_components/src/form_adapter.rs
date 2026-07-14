//! Form-state projection helpers for concrete UI components.

use open_gpui_form::{FieldSnapshot, FormSnapshot, FormStatus};
use open_gpui_ui_core::{Size, ThemeTokens};

use crate::{
    checkbox::CheckboxState, field::FieldState, number_input::NumberInputState,
    text_input::TextInputState, textarea::TextareaState,
};

/// Projection of form-level lifecycle facts into concrete UI decisions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormProjection {
    status: FormStatus,
    busy: bool,
    validating: bool,
    submitting: bool,
    submit_enabled: bool,
}

impl FormProjection {
    /// Resolves form-level UI state from one diagnostic snapshot.
    pub fn resolve(snapshot: &FormSnapshot, disabled: bool) -> Self {
        let validating = matches!(snapshot.status, FormStatus::Validating);
        let submitting = matches!(snapshot.status, FormStatus::Submitting);
        Self {
            status: snapshot.status,
            busy: validating || submitting,
            validating,
            submitting,
            submit_enabled: !disabled && snapshot.can_submit(),
        }
    }

    /// Returns the effective form status.
    pub fn status(&self) -> &FormStatus {
        &self.status
    }

    /// Returns whether validation or submission work is active.
    pub const fn busy(&self) -> bool {
        self.busy
    }

    /// Returns whether validation work is active.
    pub const fn validating(&self) -> bool {
        self.validating
    }

    /// Returns whether submission work is active.
    pub const fn submitting(&self) -> bool {
        self.submitting
    }

    /// Returns whether the submit command should be enabled.
    pub const fn submit_enabled(&self) -> bool {
        self.submit_enabled
    }
}

/// Configuration supplied by the application for one form field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormFieldConfig {
    label: String,
    control_id: Option<String>,
    help: Option<String>,
    required: bool,
    disabled: bool,
    read_only: bool,
    size: Size,
}

impl FormFieldConfig {
    /// Creates a field config with a visible label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            control_id: None,
            help: None,
            required: false,
            disabled: false,
            read_only: false,
            size: Size::Medium,
        }
    }

    /// Sets the logical control id.
    pub fn control_id(mut self, control_id: impl Into<String>) -> Self {
        self.control_id = Some(control_id.into());
        self
    }

    /// Sets help text.
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Sets whether the field is required.
    pub const fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Sets whether the field is disabled.
    pub const fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets whether the field is read-only.
    pub const fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Sets the component size.
    pub const fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

/// Projection from headless form state into concrete component state inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormFieldProjection {
    label: String,
    control_id: String,
    help: Option<String>,
    error: Option<String>,
    required: bool,
    disabled: bool,
    read_only: bool,
    invalid: bool,
    validating: bool,
    busy: bool,
    size: Size,
}

impl FormFieldProjection {
    /// Resolves projection state for one field snapshot.
    pub fn resolve(status: FormStatus, field: &FieldSnapshot, config: FormFieldConfig) -> Self {
        let invalid = !field.meta.errors.is_empty();
        let validating = field.meta.validating;
        let submitting = matches!(status, FormStatus::Submitting);
        Self {
            label: config.label,
            control_id: config
                .control_id
                .unwrap_or_else(|| field.path.as_str().replace('.', "-")),
            help: config.help,
            error: field.meta.errors.first().cloned(),
            required: config.required,
            disabled: config.disabled || submitting,
            read_only: config.read_only,
            invalid,
            validating,
            busy: validating || submitting,
            size: config.size,
        }
    }

    /// Returns the visible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the logical control id.
    pub fn control_id(&self) -> &str {
        &self.control_id
    }

    /// Returns the first validation error, if any.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Returns whether the field is invalid.
    pub const fn invalid(&self) -> bool {
        self.invalid
    }

    /// Returns whether this field has pending validation work.
    pub const fn validating(&self) -> bool {
        self.validating
    }

    /// Returns whether validation or submission work affects this field.
    pub const fn busy(&self) -> bool {
        self.busy
    }

    /// Returns whether the field is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the field is read-only.
    pub const fn read_only(&self) -> bool {
        self.read_only
    }

    /// Returns whether the field is required.
    pub const fn required(&self) -> bool {
        self.required
    }

    /// Resolves a concrete field wrapper state.
    pub fn field_state(&self, tokens: ThemeTokens) -> FieldState {
        FieldState::resolve(
            self.label.clone(),
            self.help.clone(),
            self.error.clone(),
            self.size,
            self.required,
            self.disabled,
            self.invalid,
            tokens,
        )
        .with_busy(self.busy)
    }

    /// Resolves text input state for this field.
    pub fn text_input_state(
        &self,
        value: impl Into<String>,
        placeholder: Option<impl Into<String>>,
        tokens: ThemeTokens,
    ) -> TextInputState {
        TextInputState::resolve(
            value,
            placeholder,
            self.size,
            self.disabled,
            self.read_only,
            self.invalid,
            self.required,
            false,
            tokens,
        )
        .with_busy(self.busy)
    }

    /// Resolves textarea state for this field.
    pub fn textarea_state(
        &self,
        value: impl Into<String>,
        placeholder: Option<impl Into<String>>,
        rows: usize,
        tokens: ThemeTokens,
    ) -> TextareaState {
        TextareaState::resolve(
            value,
            placeholder,
            self.size,
            rows,
            self.disabled,
            self.read_only,
            self.invalid,
            self.required,
            false,
            tokens,
        )
        .with_busy(self.busy)
    }

    /// Resolves number input state for this field.
    pub fn number_input_state(
        &self,
        value: f32,
        min: f32,
        max: f32,
        step: f32,
        tokens: ThemeTokens,
    ) -> NumberInputState {
        NumberInputState::resolve(
            self.label.clone(),
            value,
            min,
            max,
            step,
            self.disabled,
            self.read_only,
            self.invalid,
            self.required,
            self.size,
            tokens,
        )
        .with_busy(self.busy)
    }

    /// Resolves checkbox state for this field.
    pub fn checkbox_state(&self, checked: bool, tokens: ThemeTokens) -> CheckboxState {
        CheckboxState::resolve(
            checked,
            false,
            self.size,
            self.disabled,
            self.required,
            self.invalid,
            tokens,
        )
        .with_busy(self.busy)
    }
}

/// Extracts a text control value from JSON form data.
pub fn form_text_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => value.to_string(),
    }
}

/// Extracts a number control value from JSON form data.
pub fn form_number_value(value: &serde_json::Value) -> Option<f32> {
    match value {
        serde_json::Value::Number(value) => value.as_f64().map(|value| value as f32),
        serde_json::Value::String(value) => value.parse::<f32>().ok(),
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Array(_)
        | serde_json::Value::Object(_) => None,
    }
}

/// Extracts a checkbox value from JSON form data.
pub fn form_checkbox_value(value: &serde_json::Value) -> Option<bool> {
    match value {
        serde_json::Value::Bool(value) => Some(*value),
        serde_json::Value::String(value) if value == "true" => Some(true),
        serde_json::Value::String(value) if value == "false" => Some(false),
        serde_json::Value::Null
        | serde_json::Value::String(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::Array(_)
        | serde_json::Value::Object(_) => None,
    }
}

/// Extracts a select option value from JSON form data.
pub fn form_select_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        serde_json::Value::Null | serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            None
        }
    }
}
