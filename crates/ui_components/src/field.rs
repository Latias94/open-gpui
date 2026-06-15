//! Field component.

use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, ElementId, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    div, rgb,
};
use open_gpui_ui_core::{Sizable, Size, ThemeTokens};

use crate::color::ColorIntent;

/// The resolved field message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldMessage {
    /// Help text shown below a valid field.
    Help(String),
    /// Error text shown below an invalid field.
    Error(String),
}

impl FieldMessage {
    /// Returns the message text.
    pub fn text(&self) -> &str {
        match self {
            Self::Help(text) | Self::Error(text) => text,
        }
    }

    /// Returns whether this is an error message.
    pub const fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }
}

/// Resolved field color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldColors {
    label: ColorIntent,
    message: ColorIntent,
    required_marker: ColorIntent,
}

impl FieldColors {
    /// Returns the label color intent.
    pub const fn label(self) -> ColorIntent {
        self.label
    }

    /// Returns the message color intent.
    pub const fn message(self) -> ColorIntent {
        self.message
    }

    /// Returns the required marker color intent.
    pub const fn required_marker(self) -> ColorIntent {
        self.required_marker
    }
}

/// Resolved field metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldMetrics {
    label_text_size: open_gpui::Pixels,
    message_text_size: open_gpui::Pixels,
    gap: open_gpui::Pixels,
}

impl FieldMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            label_text_size: size.control_text_px(),
            message_text_size: open_gpui::px(12.0),
            gap: open_gpui::px(6.0),
        }
    }

    /// Returns the label text size.
    pub const fn label_text_size(self) -> open_gpui::Pixels {
        self.label_text_size
    }

    /// Returns the message text size.
    pub const fn message_text_size(self) -> open_gpui::Pixels {
        self.message_text_size
    }

    /// Returns the vertical gap.
    pub const fn gap(self) -> open_gpui::Pixels {
        self.gap
    }
}

/// Resolved field state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldState {
    label: String,
    control_id: String,
    help_text: Option<String>,
    error_text: Option<String>,
    size: Size,
    required: bool,
    disabled: bool,
    invalid: bool,
    metrics: FieldMetrics,
    colors: FieldColors,
}

impl FieldState {
    /// Resolves the public state for a field.
    pub fn resolve(
        label: impl Into<String>,
        control_id: impl Into<String>,
        help_text: Option<impl Into<String>>,
        error_text: Option<impl Into<String>>,
        size: Size,
        required: bool,
        disabled: bool,
        invalid: bool,
        tokens: ThemeTokens,
    ) -> Self {
        Self {
            label: label.into(),
            control_id: control_id.into(),
            help_text: help_text.map(Into::into),
            error_text: error_text.map(Into::into),
            size,
            required,
            disabled,
            invalid,
            metrics: FieldMetrics::from_size(size),
            colors: field_colors(disabled, invalid, tokens),
        }
    }

    /// Returns the visible label text.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the logical control id associated with the field.
    pub fn control_id(&self) -> &str {
        &self.control_id
    }

    /// Returns the configured help text.
    pub fn help_text(&self) -> Option<&str> {
        self.help_text.as_deref()
    }

    /// Returns the configured help text.
    pub fn help(&self) -> Option<&str> {
        self.help_text()
    }

    /// Returns the configured error text.
    pub fn error_text(&self) -> Option<&str> {
        self.error_text.as_deref()
    }

    /// Returns the configured error text.
    pub fn error(&self) -> Option<&str> {
        self.error_text()
    }

    /// Returns the message that should be rendered.
    pub fn message(&self) -> Option<FieldMessage> {
        if self.invalid {
            if let Some(error) = &self.error_text {
                return Some(FieldMessage::Error(error.clone()));
            }
        }

        self.help_text.clone().map(FieldMessage::Help)
    }

    /// Returns the support text that should be rendered.
    pub fn support_text(&self) -> Option<&str> {
        if self.invalid {
            self.error_text().or(self.help_text())
        } else {
            self.help_text()
        }
    }

    /// Returns whether the rendered support text is an error.
    pub fn support_is_error(&self) -> bool {
        self.invalid && self.error_text.is_some()
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether the field is required.
    pub const fn required(&self) -> bool {
        self.required
    }

    /// Returns whether the field is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the field is invalid.
    pub const fn invalid(&self) -> bool {
        self.invalid
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> FieldMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> FieldColors {
        self.colors
    }
}

/// A concrete GPUI field composition component.
#[derive(IntoElement)]
pub struct Field {
    id: ElementId,
    label: SharedString,
    control_id: SharedString,
    help_text: Option<SharedString>,
    error_text: Option<SharedString>,
    size: Size,
    required: bool,
    disabled: bool,
    invalid: bool,
    tokens: ThemeTokens,
    control: Option<AnyElement>,
}

impl Field {
    /// Creates a new field with an id, control id, and visible label.
    pub fn new(
        id: impl Into<ElementId>,
        control_id: impl Into<SharedString>,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            control_id: control_id.into(),
            help_text: None,
            error_text: None,
            size: Size::Medium,
            required: false,
            disabled: false,
            invalid: false,
            tokens: ThemeTokens::default(),
            control: None,
        }
    }

    /// Sets help text.
    pub fn help_text(mut self, help_text: impl Into<SharedString>) -> Self {
        self.help_text = Some(help_text.into());
        self
    }

    /// Sets help text.
    pub fn help(self, help_text: impl Into<SharedString>) -> Self {
        self.help_text(help_text)
    }

    /// Sets error text.
    pub fn error_text(mut self, error_text: impl Into<SharedString>) -> Self {
        self.error_text = Some(error_text.into());
        self
    }

    /// Sets error text.
    pub fn error(self, error_text: impl Into<SharedString>) -> Self {
        self.error_text(error_text)
    }

    /// Marks the field as required.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Marks the field as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks the field as invalid.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Sets the field control child.
    pub fn control(mut self, control: impl IntoElement) -> Self {
        self.control = Some(control.into_any_element());
        self
    }

    /// Returns the resolved field state.
    pub fn state(&self) -> FieldState {
        FieldState::resolve(
            self.label.to_string(),
            self.control_id.to_string(),
            self.help_text.as_ref().map(ToString::to_string),
            self.error_text.as_ref().map(ToString::to_string),
            self.size,
            self.required,
            self.disabled,
            self.invalid,
            self.tokens,
        )
    }
}

impl Sizable for Field {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Field {
    fn render(self, _window: &mut Window, _cx: &mut open_gpui::App) -> impl IntoElement {
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let message = state.message();

        div()
            .id(self.id)
            .flex()
            .flex_col()
            .gap(metrics.gap())
            .when(state.disabled(), |this| this.opacity(0.64))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_size(metrics.label_text_size())
                    .line_height(metrics.label_text_size())
                    .text_color(rgb(colors.label().fallback_rgb()))
                    .child(self.label)
                    .when(state.required(), |this| {
                        this.child(
                            div()
                                .text_color(rgb(colors.required_marker().fallback_rgb()))
                                .child("*"),
                        )
                    }),
            )
            .when_some(self.control, |this, control| this.child(control))
            .when_some(message, |this, message| {
                this.child(
                    div()
                        .id(format!("{}:message", state.control_id()))
                        .text_size(metrics.message_text_size())
                        .line_height(open_gpui::px(18.0))
                        .text_color(rgb(colors.message().fallback_rgb()))
                        .child(message.text().to_string()),
                )
            })
    }
}

fn field_colors(disabled: bool, invalid: bool, tokens: ThemeTokens) -> FieldColors {
    let message = if invalid {
        ColorIntent::new(tokens.destructive, 0xb42318)
    } else {
        ColorIntent::new(tokens.text_muted, 0x5a6472)
    };
    let label = if disabled {
        ColorIntent::new(tokens.text_muted, 0x7a8491)
    } else {
        ColorIntent::new(tokens.text, 0x18202a)
    };

    FieldColors {
        label,
        message,
        required_marker: ColorIntent::new(tokens.destructive, 0xb42318),
    }
}
