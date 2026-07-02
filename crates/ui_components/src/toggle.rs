//! Toggle component.

use crate::geometry::gpui_px_from_ui;
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, ElementId, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{Role, Sizable, Size, ThemeTokens, Toggled};

use crate::a11y::UiA11yElementExt;
use crate::button::{ButtonColors, ButtonMetrics, ButtonVariant};
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::theme::ThemeResolver;

/// Resolved toggle color intents.
pub type ToggleColors = ButtonColors;

/// Resolved toggle metrics.
pub type ToggleMetrics = ButtonMetrics;

/// Visual intent for a [`Toggle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToggleVariant {
    /// Low-emphasis toggle.
    #[default]
    Ghost,
    /// Outline toggle with a visible border.
    Outline,
}

impl ToggleVariant {
    /// Returns the stable variant label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ghost => "ghost",
            Self::Outline => "outline",
        }
    }

    const fn button_variant(self) -> ButtonVariant {
        match self {
            Self::Ghost => ButtonVariant::Ghost,
            Self::Outline => ButtonVariant::Outline,
        }
    }
}

/// Resolved toggle state used by tests, demos, and rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToggleState {
    pressed: bool,
    disabled: bool,
    variant: ToggleVariant,
    size: Size,
    metrics: ToggleMetrics,
    colors: ToggleColors,
    focus_ring: FocusRing,
}

impl ToggleState {
    /// Resolves the public state for a toggle.
    pub fn resolve(
        pressed: bool,
        disabled: bool,
        variant: ToggleVariant,
        size: Size,
        tokens: ThemeTokens,
    ) -> Self {
        let colors = ThemeResolver::button_colors(tokens, variant.button_variant(), pressed);

        Self {
            pressed,
            disabled,
            variant,
            size,
            metrics: ToggleMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
        }
    }

    /// Returns whether the toggle is pressed.
    pub const fn pressed(self) -> bool {
        self.pressed
    }

    /// Returns whether the toggle is disabled.
    pub const fn disabled(self) -> bool {
        self.disabled
    }

    /// Returns the visual variant.
    pub const fn variant(self) -> ToggleVariant {
        self.variant
    }

    /// Returns the foundation size.
    pub const fn size(self) -> Size {
        self.size
    }

    /// Returns whether activation handlers should run.
    pub const fn activation_enabled(self) -> bool {
        !self.disabled
    }

    /// Returns the accessibility role.
    pub const fn role(self) -> Role {
        Role::Button
    }

    /// Returns the toggled accessibility state.
    pub const fn toggled(self) -> Toggled {
        if self.pressed {
            Toggled::True
        } else {
            Toggled::False
        }
    }

    /// Returns resolved metrics.
    pub const fn metrics(self) -> ToggleMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(self) -> ToggleColors {
        self.colors
    }

    /// Returns resolved focus ring metadata.
    pub const fn focus_ring(self) -> FocusRing {
        self.focus_ring
    }
}

/// A concrete GPUI toggle component.
#[derive(IntoElement)]
pub struct Toggle {
    id: ElementId,
    label: SharedString,
    pressed: bool,
    disabled: bool,
    variant: ToggleVariant,
    size: Size,
    tokens: ThemeTokens,
    on_change: Option<Rc<dyn Fn(bool, &ClickEvent, &mut Window, &mut App)>>,
}

impl Toggle {
    /// Creates a new toggle with an id and visible label.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            pressed: false,
            disabled: false,
            variant: ToggleVariant::Ghost,
            size: Size::Medium,
            tokens: ThemeTokens::default(),
            on_change: None,
        }
    }

    /// Applies a visual variant.
    pub fn variant(mut self, variant: ToggleVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Marks the toggle as pressed.
    pub fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = pressed;
        self
    }

    /// Marks the toggle as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers a change handler with the next pressed value.
    pub fn on_change(
        mut self,
        handler: impl Fn(bool, &ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Returns the resolved toggle state.
    pub fn state(&self) -> ToggleState {
        ToggleState::resolve(
            self.pressed,
            self.disabled,
            self.variant,
            self.size,
            self.tokens,
        )
    }
}

impl Sizable for Toggle {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Toggle {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let label = self.label.clone();
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let disabled = state.disabled();
        let next_pressed = !state.pressed();
        let theme_context = ThemeResolver::current(cx);
        let theme = &theme_context;
        let border_color = theme.resolve(colors.border());
        let background = theme.resolve(colors.background());
        let foreground = theme.resolve(colors.foreground());
        let hover_background = theme.resolve(colors.hover_background());
        let focus_shadow = focus_ring_shadow_with_theme(focus_ring, theme);

        div()
            .id(self.id)
            .min_h(gpui_px_from_ui(metrics.height()))
            .px(gpui_px_from_ui(metrics.padding_x()))
            .py(gpui_px_from_ui(metrics.padding_y()))
            .flex()
            .items_center()
            .justify_center()
            .gap_2()
            .rounded(gpui_px_from_ui(metrics.radius()))
            .border_1()
            .border_color(border_color)
            .bg(background)
            .text_color(foreground)
            .text_size(gpui_px_from_ui(metrics.text_size()))
            .line_height(gpui_px_from_ui(metrics.text_size()))
            .focusable()
            .tab_stop(!disabled)
            .ui_role(state.role())
            .aria_label(label.clone())
            .ui_aria_toggled(state.toggled())
            .focus_visible(move |style| style.shadow(focus_shadow.clone()))
            .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
            .when(!disabled, |this| {
                this.cursor_pointer()
                    .hover(move |style| style.bg(hover_background))
            })
            .when_some(self.on_change.filter(|_| !disabled), |this, on_change| {
                this.on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    on_change(next_pressed, event, window, cx);
                })
            })
            .child(label)
    }
}
