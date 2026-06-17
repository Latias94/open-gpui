//! Label component.

use open_gpui::prelude::*;
use open_gpui::{
    ElementId, IntoElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window,
    div,
};
use open_gpui_ui_core::{Role, Sizable, Size, ThemeTokens, UiPx, ui_px};

use crate::a11y::UiA11yElementExt;
use crate::color::ColorIntent;
use crate::theme::ThemeResolver;

/// Resolved label color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LabelColors {
    pub(crate) text: ColorIntent,
    pub(crate) required_marker: ColorIntent,
}

impl LabelColors {
    /// Returns the label text color intent.
    pub const fn text(self) -> ColorIntent {
        self.text
    }

    /// Returns the required marker color intent.
    pub const fn required_marker(self) -> ColorIntent {
        self.required_marker
    }
}

/// Resolved label metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelMetrics {
    text_size: UiPx,
    gap: UiPx,
    marker_size: UiPx,
}

impl LabelMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            text_size: size.control_text_px(),
            gap: ui_px(4.0),
            marker_size: ui_px(10.0),
        }
    }

    /// Returns the label text size.
    pub const fn text_size(self) -> UiPx {
        self.text_size
    }

    /// Returns the gap between the text and required marker.
    pub const fn gap(self) -> UiPx {
        self.gap
    }

    /// Returns the required marker size.
    pub const fn marker_size(self) -> UiPx {
        self.marker_size
    }
}

/// Resolved label state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelState {
    text: String,
    control_id: Option<String>,
    size: Size,
    required: bool,
    disabled: bool,
    metrics: LabelMetrics,
    colors: LabelColors,
}

impl LabelState {
    /// Resolves the public state for a label.
    pub fn resolve(
        text: impl Into<String>,
        control_id: Option<String>,
        size: Size,
        required: bool,
        disabled: bool,
        tokens: ThemeTokens,
    ) -> Self {
        Self {
            text: text.into(),
            control_id,
            size,
            required,
            disabled,
            metrics: LabelMetrics::from_size(size),
            colors: ThemeResolver::label_colors(tokens, disabled),
        }
    }

    /// Returns the label text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the associated control id, when present.
    pub fn control_id(&self) -> Option<&str> {
        self.control_id.as_deref()
    }

    /// Returns whether the label is associated with a control.
    pub fn associated(&self) -> bool {
        self.control_id.is_some()
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether the label marks a required control.
    pub const fn required(&self) -> bool {
        self.required
    }

    /// Returns whether the label is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns the accessibility role.
    pub const fn role(&self) -> Role {
        Role::Label
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> LabelMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> LabelColors {
        self.colors
    }
}

/// A concrete GPUI label component.
#[derive(IntoElement)]
pub struct Label {
    id: ElementId,
    text: SharedString,
    control_id: Option<SharedString>,
    required: bool,
    disabled: bool,
    size: Size,
    tokens: ThemeTokens,
}

impl Label {
    /// Creates a new label with an id and visible text.
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            control_id: None,
            required: false,
            disabled: false,
            size: Size::Medium,
            tokens: ThemeTokens::default(),
        }
    }

    /// Associates this label with a logical control id.
    pub fn for_control(mut self, control_id: impl Into<SharedString>) -> Self {
        self.control_id = Some(control_id.into());
        self
    }

    /// Marks the label as required.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Marks the label as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Returns the resolved label state.
    pub fn state(&self) -> LabelState {
        LabelState::resolve(
            self.text.to_string(),
            self.control_id.as_ref().map(ToString::to_string),
            self.size,
            self.required,
            self.disabled,
            self.tokens,
        )
    }
}

impl Sizable for Label {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Label {
    fn render(self, _window: &mut Window, _cx: &mut open_gpui::App) -> impl IntoElement {
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let text = self.text.clone();

        div()
            .id(self.id)
            .flex()
            .items_center()
            .gap_1()
            .ui_role(state.role())
            .aria_label(text.clone())
            .text_size(metrics.text_size())
            .line_height(metrics.text_size())
            .text_color(ThemeResolver::resolve(colors.text()))
            .when(state.disabled(), |this| this.opacity(0.56))
            .child(text)
            .when(state.required(), |this| {
                this.child(
                    div()
                        .text_color(ThemeResolver::resolve(colors.required_marker()))
                        .text_size(metrics.marker_size())
                        .line_height(metrics.text_size())
                        .child("*"),
                )
            })
    }
}
