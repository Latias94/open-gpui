//! Skeleton loading placeholder component.

use crate::color::ColorIntent;
use crate::geometry::gpui_px_from_ui;
use crate::theme::ThemeResolver;
use open_gpui::prelude::*;
use open_gpui::{ElementId, InteractiveElement, IntoElement, RenderOnce, Styled, div};
use open_gpui_ui_core::{Sizable, Size, ThemeTokens, UiPx, ui_px};

/// Resolved skeleton color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkeletonColors {
    pub(crate) background: ColorIntent,
}

impl SkeletonColors {
    /// Returns the placeholder background color intent.
    pub const fn background(self) -> ColorIntent {
        self.background
    }
}

/// Resolved skeleton metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkeletonMetrics {
    width: UiPx,
    height: UiPx,
    radius: UiPx,
}

impl SkeletonMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            width: match size {
                Size::XSmall => ui_px(96.0),
                Size::Small => ui_px(128.0),
                Size::Medium => ui_px(160.0),
                Size::Large => ui_px(224.0),
            },
            height: match size {
                Size::XSmall => ui_px(12.0),
                Size::Small => ui_px(14.0),
                Size::Medium => ui_px(16.0),
                Size::Large => ui_px(20.0),
            },
            radius: ui_px(4.0),
        }
    }

    /// Returns placeholder width.
    pub const fn width(self) -> UiPx {
        self.width
    }

    /// Returns placeholder height.
    pub const fn height(self) -> UiPx {
        self.height
    }

    /// Returns corner radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }
}

/// Resolved skeleton state used by tests, demos, and rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkeletonState {
    size: Size,
    subtle: bool,
    metrics: SkeletonMetrics,
    colors: SkeletonColors,
}

impl SkeletonState {
    /// Resolves the public state for a skeleton placeholder.
    pub fn resolve(size: Size, subtle: bool, tokens: ThemeTokens) -> Self {
        Self {
            size,
            subtle,
            metrics: SkeletonMetrics::from_size(size),
            colors: ThemeResolver::skeleton_colors(tokens),
        }
    }

    /// Returns the foundation size.
    pub const fn size(self) -> Size {
        self.size
    }

    /// Returns whether the placeholder uses lower visual emphasis.
    pub const fn subtle(self) -> bool {
        self.subtle
    }

    /// Returns whether this primitive is display-only.
    pub const fn display_only(self) -> bool {
        true
    }

    /// Returns resolved metrics.
    pub const fn metrics(self) -> SkeletonMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(self) -> SkeletonColors {
        self.colors
    }
}

/// A concrete GPUI skeleton placeholder component.
#[derive(IntoElement)]
pub struct Skeleton {
    id: ElementId,
    size: Size,
    subtle: bool,
    tokens: ThemeTokens,
}

impl Skeleton {
    /// Creates a static skeleton placeholder.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            size: Size::Medium,
            subtle: false,
            tokens: ThemeTokens::default(),
        }
    }

    /// Applies lower visual emphasis.
    pub fn subtle(mut self, subtle: bool) -> Self {
        self.subtle = subtle;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Returns the resolved skeleton state.
    pub fn state(&self) -> SkeletonState {
        SkeletonState::resolve(self.size, self.subtle, self.tokens)
    }
}

impl Sizable for Skeleton {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Skeleton {
    fn render(self, _window: &mut open_gpui::Window, _cx: &mut open_gpui::App) -> impl IntoElement {
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let debug_id = self.id.to_string();

        div()
            .id(self.id)
            .debug_selector(move || format!("skeleton:{debug_id}:root"))
            .w(gpui_px_from_ui(metrics.width()))
            .h(gpui_px_from_ui(metrics.height()))
            .rounded(gpui_px_from_ui(metrics.radius()))
            .bg(ThemeResolver::resolve(colors.background()))
            .when(state.subtle(), |this| this.opacity(0.56))
    }
}
