//! Icon button component.

use crate::geometry::gpui_px_from_ui;
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    AnyView, App, ClickEvent, ElementId, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{Role, Sizable, Size, ThemeTokens, UiPx};

use crate::a11y::UiA11yElementExt;
use crate::button::{ButtonColors, ButtonVariant};
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::theme::ThemeResolver;

/// Resolved icon button color intents.
pub type IconButtonColors = ButtonColors;

/// Resolved icon button metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IconButtonMetrics {
    size: UiPx,
    radius: UiPx,
    icon_size: UiPx,
}

impl IconButtonMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            size: size.icon_button_size(),
            radius: size.control_radius(),
            icon_size: size.icon_size(),
        }
    }

    /// Returns square control size.
    pub const fn size(self) -> UiPx {
        self.size
    }

    /// Returns corner radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns icon glyph size.
    pub const fn icon_size(self) -> UiPx {
        self.icon_size
    }
}

/// Resolved icon button state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct IconButtonState {
    variant: ButtonVariant,
    size: Size,
    disabled: bool,
    selected: bool,
    accessible_label: SharedString,
    metrics: IconButtonMetrics,
    colors: IconButtonColors,
    focus_ring: FocusRing,
}

impl IconButtonState {
    /// Resolves the public state for an icon button.
    pub fn resolve(
        variant: ButtonVariant,
        size: Size,
        disabled: bool,
        selected: bool,
        accessible_label: impl Into<SharedString>,
        tokens: ThemeTokens,
    ) -> Self {
        let colors = ThemeResolver::button_colors(tokens, variant, selected);

        Self {
            variant,
            size,
            disabled,
            selected,
            accessible_label: accessible_label.into(),
            metrics: IconButtonMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
        }
    }

    /// Returns the visual variant.
    pub const fn variant(&self) -> ButtonVariant {
        self.variant
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether the button is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the icon button is selected.
    pub const fn selected(&self) -> bool {
        self.selected
    }

    /// Returns whether activation handlers should run.
    pub const fn activation_enabled(&self) -> bool {
        !self.disabled
    }

    /// Returns the accessibility role.
    pub const fn role(&self) -> Role {
        Role::Button
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> IconButtonMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> IconButtonColors {
        self.colors
    }

    /// Returns resolved focus ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }

    /// Returns the accessible label used by the icon button.
    pub fn accessible_label(&self) -> &str {
        &self.accessible_label
    }
}

/// A concrete GPUI icon-only button component.
#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon: SharedString,
    accessible_label: SharedString,
    variant: ButtonVariant,
    size: Size,
    disabled: bool,
    selected: bool,
    tokens: ThemeTokens,
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    tooltip: Option<Rc<dyn Fn(&mut Window, &mut App) -> AnyView>>,
}

impl IconButton {
    /// Creates a new icon-only button with an explicit accessible label.
    pub fn new(
        id: impl Into<ElementId>,
        icon: impl Into<SharedString>,
        accessible_label: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            icon: icon.into(),
            accessible_label: accessible_label.into(),
            variant: ButtonVariant::Ghost,
            size: Size::Medium,
            disabled: false,
            selected: false,
            tokens: ThemeTokens::default(),
            on_click: None,
            tooltip: None,
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

    /// Marks the icon button as selected.
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

    /// Adds a hover/focus tooltip to the icon button.
    pub fn tooltip(mut self, tooltip: impl Fn(&mut Window, &mut App) -> AnyView + 'static) -> Self {
        self.tooltip = Some(Rc::new(tooltip));
        self
    }

    /// Returns the accessible label.
    pub fn accessible_label(&self) -> &str {
        &self.accessible_label
    }

    /// Returns the resolved icon button state.
    pub fn state(&self) -> IconButtonState {
        IconButtonState::resolve(
            self.variant,
            self.size,
            self.disabled,
            self.selected,
            self.accessible_label.clone(),
            self.tokens,
        )
    }
}

impl Sizable for IconButton {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let label = self.accessible_label.clone();
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
            .w(gpui_px_from_ui(metrics.size()))
            .h(gpui_px_from_ui(metrics.size()))
            .min_w(gpui_px_from_ui(metrics.size()))
            .min_h(gpui_px_from_ui(metrics.size()))
            .flex()
            .items_center()
            .justify_center()
            .rounded(gpui_px_from_ui(metrics.radius()))
            .border_1()
            .border_color(border_color)
            .bg(background)
            .text_color(foreground)
            .text_size(gpui_px_from_ui(metrics.icon_size()))
            .line_height(gpui_px_from_ui(metrics.icon_size()))
            .focusable()
            .tab_stop(!disabled)
            .ui_role(state.role())
            .aria_label(label)
            .aria_selected(state.selected())
            .aria_disabled(disabled)
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
            .when_some(self.tooltip, |this, tooltip| {
                this.tooltip(move |window, cx| tooltip(window, cx))
            })
            .child(self.icon)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_button_state_tracks_selected_builder_value() {
        let inactive = IconButton::new("reader-select", "S", "Select text").state();
        assert!(!inactive.selected());

        let active = IconButton::new("reader-select", "S", "Select text")
            .selected(true)
            .state();
        assert!(active.selected());
        assert_eq!(active.accessible_label(), "Select text");
    }
}
