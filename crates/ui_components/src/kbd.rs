//! Keyboard shortcut display component.

use crate::color::ColorIntent;
use crate::geometry::gpui_px_from_ui;
use crate::theme::ThemeResolver;
use open_gpui::{
    ElementId, FontWeight, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, Styled, div,
};
use open_gpui_ui_core::{Sizable, Size, ThemeTokens, UiPx, ui_px};

/// Resolved keyboard shortcut color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KbdColors {
    pub(crate) background: ColorIntent,
    pub(crate) foreground: ColorIntent,
    pub(crate) border: ColorIntent,
}

impl KbdColors {
    /// Returns the key background color intent.
    pub const fn background(self) -> ColorIntent {
        self.background
    }

    /// Returns the key foreground color intent.
    pub const fn foreground(self) -> ColorIntent {
        self.foreground
    }

    /// Returns the key border color intent.
    pub const fn border(self) -> ColorIntent {
        self.border
    }
}

/// Resolved keyboard shortcut metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KbdMetrics {
    min_height: UiPx,
    min_width: UiPx,
    padding_x: UiPx,
    padding_y: UiPx,
    radius: UiPx,
    text_size: UiPx,
}

impl KbdMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            min_height: match size {
                Size::XSmall => ui_px(18.0),
                Size::Small => ui_px(20.0),
                Size::Medium => ui_px(22.0),
                Size::Large => ui_px(26.0),
            },
            min_width: match size {
                Size::XSmall => ui_px(18.0),
                Size::Small => ui_px(20.0),
                Size::Medium => ui_px(22.0),
                Size::Large => ui_px(26.0),
            },
            padding_x: match size {
                Size::XSmall => ui_px(4.0),
                Size::Small => ui_px(5.0),
                Size::Medium => ui_px(6.0),
                Size::Large => ui_px(8.0),
            },
            padding_y: ui_px(1.0),
            radius: ui_px(4.0),
            text_size: size.control_text_px(),
        }
    }

    /// Returns the minimum key height.
    pub const fn min_height(self) -> UiPx {
        self.min_height
    }

    /// Returns the minimum key width.
    pub const fn min_width(self) -> UiPx {
        self.min_width
    }

    /// Returns horizontal padding.
    pub const fn padding_x(self) -> UiPx {
        self.padding_x
    }

    /// Returns vertical padding.
    pub const fn padding_y(self) -> UiPx {
        self.padding_y
    }

    /// Returns corner radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns text size.
    pub const fn text_size(self) -> UiPx {
        self.text_size
    }
}

/// Resolved keyboard shortcut state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct KbdState {
    label: String,
    size: Size,
    metrics: KbdMetrics,
    colors: KbdColors,
}

impl KbdState {
    /// Resolves the public state for a keyboard shortcut.
    pub fn resolve(label: impl Into<String>, size: Size, tokens: ThemeTokens) -> Self {
        Self {
            label: label.into(),
            size,
            metrics: KbdMetrics::from_size(size),
            colors: ThemeResolver::kbd_colors(tokens),
        }
    }

    /// Returns the visible key label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether this primitive is display-only.
    pub const fn display_only(&self) -> bool {
        true
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> KbdMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> KbdColors {
        self.colors
    }
}

/// A concrete GPUI keyboard shortcut component.
#[derive(IntoElement)]
pub struct Kbd {
    id: ElementId,
    label: SharedString,
    size: Size,
    tokens: ThemeTokens,
}

impl Kbd {
    /// Creates a display-only keyboard shortcut.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            size: Size::Medium,
            tokens: ThemeTokens::default(),
        }
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Returns the resolved keyboard shortcut state.
    pub fn state(&self) -> KbdState {
        KbdState::resolve(self.label.to_string(), self.size, self.tokens)
    }
}

impl Sizable for Kbd {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Kbd {
    fn render(self, window: &mut open_gpui::Window, cx: &mut open_gpui::App) -> impl IntoElement {
        let theme = ThemeResolver::current(window, cx);
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let debug_id = self.id.to_string();

        div()
            .id(self.id)
            .debug_selector(move || format!("kbd:{debug_id}:root"))
            .min_h(gpui_px_from_ui(metrics.min_height()))
            .min_w(gpui_px_from_ui(metrics.min_width()))
            .px(gpui_px_from_ui(metrics.padding_x()))
            .py(gpui_px_from_ui(metrics.padding_y()))
            .flex()
            .items_center()
            .justify_center()
            .rounded(gpui_px_from_ui(metrics.radius()))
            .border_1()
            .border_color(theme.resolve(colors.border()))
            .bg(theme.resolve(colors.background()))
            .text_color(theme.resolve(colors.foreground()))
            .text_size(gpui_px_from_ui(metrics.text_size()))
            .line_height(gpui_px_from_ui(metrics.text_size()))
            .font_weight(FontWeight::MEDIUM)
            .child(self.label)
    }
}
