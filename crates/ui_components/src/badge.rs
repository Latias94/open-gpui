//! Badge component.

use open_gpui::{
    ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString, Styled,
    div,
};
use open_gpui_ui_core::{Sizable, Size, ThemeTokens, UiPx, ui_px};

use crate::color::ColorIntent;
use crate::theme::ThemeResolver;

/// Visual intent for a [`Badge`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BadgeVariant {
    /// High-emphasis badge using the accent token.
    #[default]
    Default,
    /// Lower-emphasis badge using muted surface tokens.
    Secondary,
    /// Badge for destructive or error-adjacent metadata.
    Destructive,
    /// Badge with visible border and neutral fill.
    Outline,
}

impl BadgeVariant {
    /// Returns the stable variant label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Secondary => "secondary",
            Self::Destructive => "destructive",
            Self::Outline => "outline",
        }
    }
}

/// Resolved badge color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BadgeColors {
    pub(crate) background: ColorIntent,
    pub(crate) foreground: ColorIntent,
    pub(crate) border: ColorIntent,
}

impl BadgeColors {
    /// Returns the background color intent.
    pub const fn background(self) -> ColorIntent {
        self.background
    }

    /// Returns the foreground color intent.
    pub const fn foreground(self) -> ColorIntent {
        self.foreground
    }

    /// Returns the border color intent.
    pub const fn border(self) -> ColorIntent {
        self.border
    }
}

/// Resolved badge metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BadgeMetrics {
    min_height: UiPx,
    padding_x: UiPx,
    padding_y: UiPx,
    radius: UiPx,
    text_size: UiPx,
}

impl BadgeMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            min_height: match size {
                Size::XSmall => ui_px(18.0),
                Size::Small => ui_px(20.0),
                Size::Medium => ui_px(22.0),
                Size::Large => ui_px(24.0),
            },
            padding_x: match size {
                Size::XSmall => ui_px(6.0),
                Size::Small => ui_px(8.0),
                Size::Medium => ui_px(9.0),
                Size::Large => ui_px(10.0),
            },
            padding_y: ui_px(2.0),
            radius: ui_px(999.0),
            text_size: size.control_text_px(),
        }
    }

    /// Returns the minimum badge height.
    pub const fn min_height(self) -> UiPx {
        self.min_height
    }

    /// Returns horizontal padding.
    pub const fn padding_x(self) -> UiPx {
        self.padding_x
    }

    /// Returns vertical padding.
    pub const fn padding_y(self) -> UiPx {
        self.padding_y
    }

    /// Returns the pill radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns text size.
    pub const fn text_size(self) -> UiPx {
        self.text_size
    }
}

/// Resolved badge state used by tests, demos, and rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BadgeState {
    variant: BadgeVariant,
    size: Size,
    metrics: BadgeMetrics,
    colors: BadgeColors,
}

impl BadgeState {
    /// Resolves the public state for a badge.
    pub fn resolve(variant: BadgeVariant, size: Size, tokens: ThemeTokens) -> Self {
        Self {
            variant,
            size,
            metrics: BadgeMetrics::from_size(size),
            colors: ThemeResolver::badge_colors(tokens, variant),
        }
    }

    /// Returns the visual variant.
    pub const fn variant(self) -> BadgeVariant {
        self.variant
    }

    /// Returns the foundation size.
    pub const fn size(self) -> Size {
        self.size
    }

    /// Returns whether this badge is display-only.
    pub const fn display_only(self) -> bool {
        true
    }

    /// Returns the optional accessibility role.
    pub const fn role(self) -> Option<open_gpui_ui_core::Role> {
        None
    }

    /// Returns resolved metrics.
    pub const fn metrics(self) -> BadgeMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(self) -> BadgeColors {
        self.colors
    }
}

/// A concrete GPUI badge component.
#[derive(IntoElement)]
pub struct Badge {
    id: ElementId,
    label: SharedString,
    variant: BadgeVariant,
    size: Size,
    tokens: ThemeTokens,
}

impl Badge {
    /// Creates a new display-only badge.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            variant: BadgeVariant::Default,
            size: Size::Medium,
            tokens: ThemeTokens::default(),
        }
    }

    /// Applies a visual variant.
    pub fn variant(mut self, variant: BadgeVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Returns the resolved badge state.
    pub fn state(&self) -> BadgeState {
        BadgeState::resolve(self.variant, self.size, self.tokens)
    }
}

impl Sizable for Badge {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Badge {
    fn render(self, _window: &mut open_gpui::Window, _cx: &mut open_gpui::App) -> impl IntoElement {
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();

        div()
            .id(self.id)
            .min_h(metrics.min_height())
            .px(metrics.padding_x())
            .py(metrics.padding_y())
            .flex()
            .items_center()
            .justify_center()
            .rounded(metrics.radius())
            .border_1()
            .border_color(ThemeResolver::resolve(colors.border()))
            .bg(ThemeResolver::resolve(colors.background()))
            .text_color(ThemeResolver::resolve(colors.foreground()))
            .text_size(metrics.text_size())
            .line_height(metrics.text_size())
            .font_weight(open_gpui::FontWeight::MEDIUM)
            .child(self.label)
    }
}
