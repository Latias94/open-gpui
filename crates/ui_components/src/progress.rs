//! Progress component.

use crate::a11y::UiA11yElementExt;
use crate::color::ColorIntent;
use crate::geometry::gpui_px_from_ui;
use crate::theme::ThemeResolver;
use open_gpui::prelude::*;
use open_gpui::{
    ElementId, IntoElement, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement,
    Styled, div, relative,
};
use open_gpui_ui_core::{Role, Sizable, Size, ThemeTokens, UiPx, ui_px};

/// Resolved progress color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressColors {
    pub(crate) track: ColorIntent,
    pub(crate) indicator: ColorIntent,
}

impl ProgressColors {
    /// Returns the track color intent.
    pub const fn track(self) -> ColorIntent {
        self.track
    }

    /// Returns the indicator color intent.
    pub const fn indicator(self) -> ColorIntent {
        self.indicator
    }
}

/// Resolved progress metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProgressMetrics {
    height: UiPx,
    radius: UiPx,
}

impl ProgressMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        let height = match size {
            Size::XSmall => ui_px(4.0),
            Size::Small => ui_px(6.0),
            Size::Medium => ui_px(8.0),
            Size::Large => ui_px(10.0),
        };

        Self {
            height,
            radius: ui_px(height.as_f32() / 2.0),
        }
    }

    /// Returns progress height.
    pub const fn height(self) -> UiPx {
        self.height
    }

    /// Returns corner radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }
}

/// Resolved progress state used by tests, demos, and rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProgressState {
    value_percent: Option<f32>,
    normalized_value: Option<f32>,
    size: Size,
    metrics: ProgressMetrics,
    colors: ProgressColors,
}

impl ProgressState {
    /// Resolves the public state for a progress bar.
    pub fn resolve(value_percent: Option<f32>, size: Size, tokens: ThemeTokens) -> Self {
        let value_percent = value_percent.map(normalize_percent);
        let normalized_value = value_percent.map(|value| value / 100.0);

        Self {
            value_percent,
            normalized_value,
            size,
            metrics: ProgressMetrics::from_size(size),
            colors: ThemeResolver::progress_colors(tokens),
        }
    }

    /// Returns clamped determinate progress in the 0..100 range.
    pub const fn value_percent(self) -> Option<f32> {
        self.value_percent
    }

    /// Returns clamped determinate progress in the 0..1 range.
    pub const fn normalized_value(self) -> Option<f32> {
        self.normalized_value
    }

    /// Returns whether progress is indeterminate.
    pub const fn indeterminate(self) -> bool {
        self.value_percent.is_none()
    }

    /// Returns the foundation size.
    pub const fn size(self) -> Size {
        self.size
    }

    /// Returns the accessibility role.
    pub const fn role(self) -> Role {
        Role::ProgressIndicator
    }

    /// Returns resolved metrics.
    pub const fn metrics(self) -> ProgressMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(self) -> ProgressColors {
        self.colors
    }
}

/// A concrete GPUI progress component.
#[derive(IntoElement)]
pub struct Progress {
    id: ElementId,
    label: SharedString,
    value_percent: Option<f32>,
    size: Size,
    tokens: ThemeTokens,
}

impl Progress {
    /// Creates a determinate progress bar initialized at zero percent.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            value_percent: Some(0.0),
            size: Size::Medium,
            tokens: ThemeTokens::default(),
        }
    }

    /// Applies a determinate progress percentage.
    pub fn value(mut self, value_percent: f32) -> Self {
        self.value_percent = Some(value_percent);
        self
    }

    /// Marks progress as indeterminate.
    pub fn indeterminate(mut self) -> Self {
        self.value_percent = None;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Returns the resolved progress state.
    pub fn state(&self) -> ProgressState {
        ProgressState::resolve(self.value_percent, self.size, self.tokens)
    }
}

impl Sizable for Progress {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Progress {
    fn render(self, _window: &mut open_gpui::Window, _cx: &mut open_gpui::App) -> impl IntoElement {
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let debug_id = self.id.to_string();
        let fill_width = state.normalized_value().unwrap_or(0.33);
        let label = self.label.clone();

        div()
            .id(self.id)
            .debug_selector(move || format!("progress:{debug_id}:root"))
            .relative()
            .w_full()
            .h(gpui_px_from_ui(metrics.height()))
            .rounded(gpui_px_from_ui(metrics.radius()))
            .overflow_hidden()
            .bg(ThemeResolver::resolve(colors.track()))
            .ui_role(state.role())
            .aria_label(label)
            .aria_min_numeric_value(0.0)
            .aria_max_numeric_value(100.0)
            .when_some(state.value_percent(), |this, value| {
                this.aria_numeric_value(value as f64)
            })
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .h_full()
                    .w(relative(fill_width))
                    .rounded(gpui_px_from_ui(metrics.radius()))
                    .bg(ThemeResolver::resolve(colors.indicator())),
            )
    }
}

fn normalize_percent(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 100.0)
    } else {
        0.0
    }
}
