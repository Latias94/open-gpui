//! Button component.

use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, ElementId, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, rgb,
};
use open_gpui_ui_core::{Role, Sizable, Size, ThemeTokens};

use crate::color::ColorIntent;

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
    background: ColorIntent,
    foreground: ColorIntent,
    border: ColorIntent,
    hover_background: ColorIntent,
    focus_ring: ColorIntent,
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
    height: open_gpui::Pixels,
    padding_x: open_gpui::Pixels,
    padding_y: open_gpui::Pixels,
    radius: open_gpui::Pixels,
    text_size: open_gpui::Pixels,
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

/// Resolved button state used by tests, demos, and rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ButtonState {
    variant: ButtonVariant,
    size: Size,
    disabled: bool,
    selected: bool,
    metrics: ButtonMetrics,
    colors: ButtonColors,
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
        let colors = button_colors(variant, selected, tokens);

        Self {
            variant,
            size,
            disabled,
            selected,
            metrics: ButtonMetrics::from_size(size),
            colors,
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
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let label = self.label.clone();
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let disabled = state.disabled();

        div()
            .id(self.id)
            .min_h(metrics.height())
            .px(metrics.padding_x())
            .py(metrics.padding_y())
            .flex()
            .items_center()
            .justify_center()
            .gap_2()
            .rounded(metrics.radius())
            .border_1()
            .border_color(rgb(colors.border().fallback_rgb()))
            .bg(rgb(colors.background().fallback_rgb()))
            .text_color(rgb(colors.foreground().fallback_rgb()))
            .text_size(metrics.text_size())
            .line_height(metrics.text_size())
            .focusable()
            .tab_stop(!disabled)
            .role(state.role())
            .aria_label(label.clone())
            .aria_selected(state.selected())
            .focus_visible(|style| {
                style
                    .border_2()
                    .border_color(rgb(colors.focus_ring().fallback_rgb()))
            })
            .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
            .when(!disabled, |this| {
                this.cursor_pointer()
                    .hover(move |style| style.bg(rgb(colors.hover_background().fallback_rgb())))
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

fn button_colors(variant: ButtonVariant, selected: bool, tokens: ThemeTokens) -> ButtonColors {
    if selected {
        return accent_button_colors(tokens);
    }

    match variant {
        ButtonVariant::Default => accent_button_colors(tokens),
        ButtonVariant::Secondary => ButtonColors {
            background: ColorIntent::new(tokens.surface_muted, 0xe8ede6),
            foreground: ColorIntent::new(tokens.text, 0x18202a),
            border: ColorIntent::new(tokens.border, 0xd6d8ce),
            hover_background: ColorIntent::new(tokens.surface_muted, 0xdfe6dc),
            focus_ring: ColorIntent::new(tokens.focus_ring, 0x2f80ed),
        },
        ButtonVariant::Outline => ButtonColors {
            background: ColorIntent::new(tokens.surface, 0xffffff),
            foreground: ColorIntent::new(tokens.text, 0x18202a),
            border: ColorIntent::new(tokens.border, 0xcfd5cc),
            hover_background: ColorIntent::new(tokens.surface_muted, 0xf1f5ee),
            focus_ring: ColorIntent::new(tokens.focus_ring, 0x2f80ed),
        },
        ButtonVariant::Ghost => ButtonColors {
            background: ColorIntent::new(tokens.surface, 0xf6f7f2),
            foreground: ColorIntent::new(tokens.text, 0x18202a),
            border: ColorIntent::new(tokens.surface, 0xf6f7f2),
            hover_background: ColorIntent::new(tokens.surface_muted, 0xe8ede6),
            focus_ring: ColorIntent::new(tokens.focus_ring, 0x2f80ed),
        },
        ButtonVariant::Destructive => ButtonColors {
            background: ColorIntent::new(tokens.destructive, 0xb42318),
            foreground: ColorIntent::new(tokens.destructive_foreground, 0xffffff),
            border: ColorIntent::new(tokens.destructive, 0xb42318),
            hover_background: ColorIntent::new(tokens.destructive, 0x971b12),
            focus_ring: ColorIntent::new(tokens.focus_ring, 0x2f80ed),
        },
    }
}

fn accent_button_colors(tokens: ThemeTokens) -> ButtonColors {
    ButtonColors {
        background: ColorIntent::new(tokens.accent, 0x1f7a66),
        foreground: ColorIntent::new(tokens.accent_foreground, 0xffffff),
        border: ColorIntent::new(tokens.accent, 0x1f7a66),
        hover_background: ColorIntent::new(tokens.accent, 0x176656),
        focus_ring: ColorIntent::new(tokens.focus_ring, 0x2f80ed),
    }
}
