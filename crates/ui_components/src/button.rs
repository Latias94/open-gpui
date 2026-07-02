//! Button component.

use crate::geometry::gpui_px_from_ui;
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, ElementId, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{Role, Sizable, Size, ThemeTokens, UiPx};

use crate::a11y::UiA11yElementExt;
use crate::color::ColorIntent;
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::theme::ThemeResolver;

/// Visual intent for a [`Button`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// Primary action using the accent token.
    #[default]
    Default,
    /// Secondary action using muted surface tokens.
    Secondary,
    /// Outline action with a visible border.
    Outline,
    /// Low-emphasis action.
    Ghost,
    /// Destructive action using destructive tokens.
    Destructive,
}

impl ButtonVariant {
    /// Returns the stable label for this variant.
    pub const fn as_str(self) -> &'static str {
        match self {
            ButtonVariant::Default => "default",
            ButtonVariant::Secondary => "secondary",
            ButtonVariant::Outline => "outline",
            ButtonVariant::Ghost => "ghost",
            ButtonVariant::Destructive => "destructive",
        }
    }
}

/// Resolved button color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonColors {
    pub(crate) background: ColorIntent,
    pub(crate) foreground: ColorIntent,
    pub(crate) border: ColorIntent,
    pub(crate) hover_background: ColorIntent,
    pub(crate) focus_ring: ColorIntent,
}

impl ButtonColors {
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

    /// Returns the hover background color intent.
    pub const fn hover_background(self) -> ColorIntent {
        self.hover_background
    }

    /// Returns the focus ring color intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved button metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ButtonMetrics {
    height: UiPx,
    padding_x: UiPx,
    padding_y: UiPx,
    radius: UiPx,
    text_size: UiPx,
}

impl ButtonMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            height: size.button_h(),
            padding_x: size.button_px(),
            padding_y: size.button_py(),
            radius: size.control_radius(),
            text_size: size.control_text_px(),
        }
    }

    /// Returns the button height.
    pub const fn height(self) -> UiPx {
        self.height
    }

    /// Returns horizontal padding.
    pub const fn padding_x(self) -> UiPx {
        self.padding_x
    }

    /// Returns vertical padding.
    pub const fn padding_y(self) -> UiPx {
        self.padding_y
    }

    /// Returns the corner radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns the text size.
    pub const fn text_size(self) -> UiPx {
        self.text_size
    }
}

/// Resolved button state used by tests, demos, and rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ButtonState {
    variant: ButtonVariant,
    size: Size,
    disabled: bool,
    selected: bool,
    metrics: ButtonMetrics,
    colors: ButtonColors,
    focus_ring: FocusRing,
}

impl ButtonState {
    /// Resolves the public state for a button.
    pub fn resolve(
        variant: ButtonVariant,
        size: Size,
        disabled: bool,
        selected: bool,
        tokens: ThemeTokens,
    ) -> Self {
        let colors = ThemeResolver::button_colors(tokens, variant, selected);

        Self {
            variant,
            size,
            disabled,
            selected,
            metrics: ButtonMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
        }
    }

    /// Returns the visual variant.
    pub const fn variant(self) -> ButtonVariant {
        self.variant
    }

    /// Returns the foundation size.
    pub const fn size(self) -> Size {
        self.size
    }

    /// Returns whether the button is disabled.
    pub const fn disabled(self) -> bool {
        self.disabled
    }

    /// Returns whether the button is selected.
    pub const fn selected(self) -> bool {
        self.selected
    }

    /// Returns whether activation handlers should run.
    pub const fn activation_enabled(self) -> bool {
        !self.disabled
    }

    /// Returns the accessibility role.
    pub const fn role(self) -> Role {
        Role::Button
    }

    /// Returns resolved metrics.
    pub const fn metrics(self) -> ButtonMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(self) -> ButtonColors {
        self.colors
    }

    /// Returns resolved focus ring metadata.
    pub const fn focus_ring(self) -> FocusRing {
        self.focus_ring
    }
}

/// A concrete GPUI button component.
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    variant: ButtonVariant,
    size: Size,
    disabled: bool,
    selected: bool,
    tokens: ThemeTokens,
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
}

impl Button {
    /// Creates a new button with an id and visible label.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            variant: ButtonVariant::Default,
            size: Size::Medium,
            disabled: false,
            selected: false,
            tokens: ThemeTokens::default(),
            on_click: None,
        }
    }

    /// Applies a visual variant.
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Marks the button as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks the button as selected.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers a click handler.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// Returns the resolved button state.
    pub fn state(&self) -> ButtonState {
        ButtonState::resolve(
            self.variant,
            self.size,
            self.disabled,
            self.selected,
            self.tokens,
        )
    }
}

impl Sizable for Button {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let label = self.label.clone();
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let disabled = state.disabled();
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
            .aria_selected(state.selected())
            .focus_visible(move |style| style.shadow(focus_shadow.clone()))
            .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
            .when(!disabled, |this| {
                this.cursor_pointer()
                    .hover(move |style| style.bg(hover_background))
            })
            .when_some(self.on_click.filter(|_| !disabled), |this, on_click| {
                this.on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    on_click(event, window, cx);
                })
            })
            .child(label)
    }
}
