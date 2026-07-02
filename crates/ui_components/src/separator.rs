//! Separator component.

use crate::a11y::UiA11yElementExt;
use crate::color::ColorIntent;
use crate::geometry::gpui_px_from_ui;
use crate::theme::ThemeResolver;
use open_gpui::prelude::*;
use open_gpui::{ElementId, InteractiveElement, IntoElement, RenderOnce, Styled, div};
use open_gpui_ui_core::{Orientation, Role, Sizable, Size, ThemeTokens, UiPx, ui_px};

/// Resolved separator color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeparatorColors {
    pub(crate) line: ColorIntent,
}

impl SeparatorColors {
    /// Returns the separator line color intent.
    pub const fn line(self) -> ColorIntent {
        self.line
    }
}

/// Resolved separator metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeparatorMetrics {
    thickness: UiPx,
}

impl SeparatorMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            thickness: match size {
                Size::XSmall | Size::Small | Size::Medium => ui_px(1.0),
                Size::Large => ui_px(2.0),
            },
        }
    }

    /// Returns line thickness.
    pub const fn thickness(self) -> UiPx {
        self.thickness
    }
}

/// Resolved separator state used by tests, demos, and rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeparatorState {
    orientation: Orientation,
    decorative: bool,
    size: Size,
    metrics: SeparatorMetrics,
    colors: SeparatorColors,
}

impl SeparatorState {
    /// Resolves the public state for a separator.
    pub fn resolve(
        orientation: Orientation,
        decorative: bool,
        size: Size,
        tokens: ThemeTokens,
    ) -> Self {
        Self {
            orientation,
            decorative,
            size,
            metrics: SeparatorMetrics::from_size(size),
            colors: ThemeResolver::separator_colors(tokens),
        }
    }

    /// Returns semantic orientation.
    pub const fn orientation(self) -> Orientation {
        self.orientation
    }

    /// Returns whether the separator is hidden from accessibility semantics.
    pub const fn decorative(self) -> bool {
        self.decorative
    }

    /// Returns the foundation size.
    pub const fn size(self) -> Size {
        self.size
    }

    /// Returns the optional accessibility role.
    pub const fn role(self) -> Option<Role> {
        if self.decorative {
            None
        } else {
            Some(Role::Separator)
        }
    }

    /// Returns resolved metrics.
    pub const fn metrics(self) -> SeparatorMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(self) -> SeparatorColors {
        self.colors
    }
}

/// A concrete GPUI separator component.
#[derive(IntoElement)]
pub struct Separator {
    id: ElementId,
    orientation: Orientation,
    decorative: bool,
    size: Size,
    tokens: ThemeTokens,
}

impl Separator {
    /// Creates a horizontal semantic separator.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            orientation: Orientation::Horizontal,
            decorative: false,
            size: Size::Medium,
            tokens: ThemeTokens::default(),
        }
    }

    /// Applies semantic orientation.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Makes the separator vertical.
    pub fn vertical(mut self) -> Self {
        self.orientation = Orientation::Vertical;
        self
    }

    /// Marks the separator as decorative.
    pub fn decorative(mut self, decorative: bool) -> Self {
        self.decorative = decorative;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Returns the resolved separator state.
    pub fn state(&self) -> SeparatorState {
        SeparatorState::resolve(self.orientation, self.decorative, self.size, self.tokens)
    }
}

impl Sizable for Separator {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Separator {
    fn render(self, _window: &mut open_gpui::Window, cx: &mut open_gpui::App) -> impl IntoElement {
        let theme = ThemeResolver::current(cx);
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let debug_id = self.id.to_string();
        let thickness = gpui_px_from_ui(metrics.thickness());

        div()
            .id(self.id)
            .debug_selector(move || format!("separator:{debug_id}:root"))
            .flex_none()
            .bg(theme.resolve(colors.line()))
            .when(state.orientation() == Orientation::Horizontal, |this| {
                this.w_full().h(thickness).min_h(thickness)
            })
            .when(state.orientation() == Orientation::Vertical, |this| {
                this.w(thickness).min_w(thickness).h_full()
            })
            .when_some(state.role(), |this, role| {
                this.ui_role(role).ui_aria_orientation(state.orientation())
            })
    }
}
