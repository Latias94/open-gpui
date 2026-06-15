//! Text input component.

use open_gpui::prelude::*;
use open_gpui::{
    CursorStyle, ElementId, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{Role, Sizable, Size, ThemeTokens};

use crate::color::ColorIntent;
use crate::theme::ThemeResolver;

/// Resolved text input color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextInputColors {
    pub(crate) background: ColorIntent,
    pub(crate) foreground: ColorIntent,
    pub(crate) placeholder: ColorIntent,
    pub(crate) border: ColorIntent,
    pub(crate) focus_ring: ColorIntent,
}

impl TextInputColors {
    /// Returns the background color intent.
    pub const fn background(self) -> ColorIntent {
        self.background
    }

    /// Returns the foreground color intent.
    pub const fn foreground(self) -> ColorIntent {
        self.foreground
    }

    /// Returns the placeholder color intent.
    pub const fn placeholder(self) -> ColorIntent {
        self.placeholder
    }

    /// Returns the border color intent.
    pub const fn border(self) -> ColorIntent {
        self.border
    }

    /// Returns the focus ring color intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved text input metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextInputMetrics {
    height: open_gpui::Pixels,
    padding_x: open_gpui::Pixels,
    padding_y: open_gpui::Pixels,
    radius: open_gpui::Pixels,
    text_size: open_gpui::Pixels,
}

impl TextInputMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            height: size.input_h(),
            padding_x: size.input_px(),
            padding_y: size.input_py(),
            radius: size.control_radius(),
            text_size: size.control_text_px(),
        }
    }

    /// Returns the input height.
    pub const fn height(self) -> open_gpui::Pixels {
        self.height
    }

    /// Returns horizontal padding.
    pub const fn padding_x(self) -> open_gpui::Pixels {
        self.padding_x
    }

    /// Returns vertical padding.
    pub const fn padding_y(self) -> open_gpui::Pixels {
        self.padding_y
    }

    /// Returns the corner radius.
    pub const fn radius(self) -> open_gpui::Pixels {
        self.radius
    }

    /// Returns the text size.
    pub const fn text_size(self) -> open_gpui::Pixels {
        self.text_size
    }
}

/// Resolved text input state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct TextInputState {
    value: String,
    placeholder: Option<String>,
    size: Size,
    disabled: bool,
    read_only: bool,
    invalid: bool,
    required: bool,
    metrics: TextInputMetrics,
    colors: TextInputColors,
}

impl TextInputState {
    /// Resolves the public state for a text input.
    pub fn resolve(
        value: impl Into<String>,
        placeholder: Option<impl Into<String>>,
        size: Size,
        disabled: bool,
        read_only: bool,
        invalid: bool,
        required: bool,
        tokens: ThemeTokens,
    ) -> Self {
        Self {
            value: value.into(),
            placeholder: placeholder.map(Into::into),
            size,
            disabled,
            read_only,
            invalid,
            required,
            metrics: TextInputMetrics::from_size(size),
            colors: ThemeResolver::text_input_colors(tokens, disabled, read_only, invalid),
        }
    }

    /// Returns the current value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the placeholder text.
    pub fn placeholder(&self) -> Option<&str> {
        self.placeholder.as_deref()
    }

    /// Returns whether the value is empty.
    pub fn value_is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// Returns whether the input has a non-empty value.
    pub fn has_value(&self) -> bool {
        !self.value_is_empty()
    }

    /// Returns whether placeholder text should be visible.
    pub fn placeholder_visible(&self) -> bool {
        self.value.is_empty() && self.placeholder.is_some()
    }

    /// Returns whether display text comes from the placeholder.
    pub fn displaying_placeholder(&self) -> bool {
        self.placeholder_visible()
    }

    /// Returns the text that should be rendered by the display adapter.
    pub fn display_text(&self) -> &str {
        if self.placeholder_visible() {
            self.placeholder().unwrap_or("")
        } else {
            self.value()
        }
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether the input is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the input is read-only.
    pub const fn read_only(&self) -> bool {
        self.read_only
    }

    /// Returns whether the input is invalid.
    pub const fn invalid(&self) -> bool {
        self.invalid
    }

    /// Returns whether the input is required.
    pub const fn required(&self) -> bool {
        self.required
    }

    /// Returns whether text editing should be accepted.
    pub const fn input_enabled(&self) -> bool {
        !self.disabled && !self.read_only
    }

    /// Returns whether text editing should be accepted.
    pub const fn editable(&self) -> bool {
        self.input_enabled()
    }

    /// Returns whether activation/edit handlers should run.
    pub const fn activation_enabled(&self) -> bool {
        self.input_enabled()
    }

    /// Returns whether the element should be included in tab traversal.
    pub const fn tab_stop_enabled(&self) -> bool {
        !self.disabled
    }

    /// Returns the accessibility role.
    pub const fn role(&self) -> Role {
        Role::TextInput
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> TextInputMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> TextInputColors {
        self.colors
    }
}

/// A concrete GPUI text input component shell.
#[derive(IntoElement)]
pub struct TextInput {
    id: ElementId,
    label: SharedString,
    value: SharedString,
    placeholder: Option<SharedString>,
    size: Size,
    disabled: bool,
    read_only: bool,
    invalid: bool,
    required: bool,
    tokens: ThemeTokens,
}

impl TextInput {
    /// Creates a new text input with an id and accessible label.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            value: SharedString::default(),
            placeholder: None,
            size: Size::Medium,
            disabled: false,
            read_only: false,
            invalid: false,
            required: false,
            tokens: ThemeTokens::default(),
        }
    }

    /// Sets the displayed value.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = value.into();
        self
    }

    /// Sets the placeholder text.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Marks the input as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks the input as read-only.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Marks the input as invalid.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Marks the input as required.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Returns the resolved text input state.
    pub fn state(&self) -> TextInputState {
        TextInputState::resolve(
            self.value.to_string(),
            self.placeholder.as_ref().map(ToString::to_string),
            self.size,
            self.disabled,
            self.read_only,
            self.invalid,
            self.required,
            self.tokens,
        )
    }
}

impl Sizable for TextInput {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for TextInput {
    fn render(self, _window: &mut Window, _cx: &mut open_gpui::App) -> impl IntoElement {
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let show_placeholder = state.placeholder_visible();
        let display_text = if show_placeholder {
            self.placeholder.unwrap_or_default()
        } else {
            self.value.clone()
        };
        let text_color = if show_placeholder {
            colors.placeholder()
        } else {
            colors.foreground()
        };

        div()
            .id(self.id)
            .min_h(metrics.height())
            .w_full()
            .min_w(open_gpui::px(0.0))
            .flex()
            .items_center()
            .rounded(metrics.radius())
            .border_1()
            .border_color(ThemeResolver::resolve(colors.border()))
            .bg(ThemeResolver::resolve(colors.background()))
            .px(metrics.padding_x())
            .py(metrics.padding_y())
            .text_size(metrics.text_size())
            .line_height(metrics.text_size())
            .text_color(ThemeResolver::resolve(text_color))
            .focusable()
            .tab_stop(state.tab_stop_enabled())
            .role(state.role())
            .aria_label(self.label)
            .focus_visible(|style| {
                style
                    .border_2()
                    .border_color(ThemeResolver::resolve(colors.focus_ring()))
            })
            .when(state.disabled(), |this| {
                this.opacity(0.56).cursor_not_allowed()
            })
            .when(state.input_enabled(), |this| {
                this.cursor(CursorStyle::IBeam)
            })
            .when(state.read_only() && !state.disabled(), |this| {
                this.cursor_default()
            })
            .child(
                div()
                    .min_w(open_gpui::px(0.0))
                    .truncate()
                    .child(display_text),
            )
    }
}
