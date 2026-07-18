//! Icon button component.

use crate::geometry::gpui_px_from_ui;
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    AnyView, App, ElementId, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{
    AccessibleAction, Role, SemanticDescriptor, Sizable, Size, ThemeTokens, UiPx,
};

use crate::a11y::UiA11yElementExt;
use crate::action::ResolvedActionState;
use crate::activation::{Activation, ActivationBinding, ActivationHandle, ActivationKeyPolicy};
use crate::button::{ButtonColors, ButtonVariant};
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::theme::ThemeResolver;
use crate::tooltip::Tooltip;

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
    disabled_reason: Option<SharedString>,
    selected: bool,
    tokens: ThemeTokens,
    tooltip_text: Option<SharedString>,
    accessibility_description: Option<SharedString>,
    resolved_action: Option<ResolvedActionState>,
    on_activate: Option<Rc<dyn Fn(Activation, &mut Window, &mut App)>>,
    activation_handle: Option<ActivationHandle>,
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
            disabled_reason: None,
            selected: false,
            tokens: ThemeTokens::default(),
            tooltip_text: None,
            accessibility_description: None,
            resolved_action: None,
            on_activate: None,
            activation_handle: None,
            tooltip: None,
        }
    }

    /// Creates an icon-only button from resolved action metadata.
    pub fn from_resolved_action(id: impl Into<ElementId>, action: &ResolvedActionState) -> Self {
        Self {
            id: id.into(),
            icon: action
                .icon_label()
                .map(SharedString::from)
                .unwrap_or_else(|| action.label().into()),
            accessible_label: action.label().into(),
            variant: ButtonVariant::Ghost,
            size: Size::Medium,
            disabled: action.disabled(),
            disabled_reason: action.disabled_reason().map(SharedString::from),
            selected: false,
            tokens: ThemeTokens::default(),
            tooltip_text: action.tooltip().map(SharedString::from),
            accessibility_description: action.accessibility_description().map(SharedString::from),
            resolved_action: Some(action.clone()),
            on_activate: None,
            activation_handle: None,
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
        if !disabled {
            self.disabled_reason = None;
        }
        self
    }

    /// Marks the button as disabled with a user-displayable reason.
    pub fn disabled_reason(mut self, reason: impl Into<SharedString>) -> Self {
        let reason = reason.into();
        if !reason.is_empty() {
            self.disabled = true;
            self.disabled_reason = Some(reason);
        }
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

    /// Registers a semantic activation handler.
    pub fn on_activate(
        mut self,
        handler: impl Fn(Activation, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    /// Binds an application-owned programmatic activation handle.
    pub fn activation_handle(mut self, handle: &ActivationHandle) -> Self {
        self.activation_handle = Some(handle.clone());
        self
    }

    /// Adds a hover/focus tooltip to the icon button.
    pub fn tooltip(mut self, tooltip: impl Fn(&mut Window, &mut App) -> AnyView + 'static) -> Self {
        self.tooltip = Some(Rc::new(tooltip));
        self
    }

    /// Adds a hover/focus text tooltip to the icon button.
    pub fn tooltip_text(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip_text = Some(tooltip.into());
        self
    }

    /// Applies an accessibility description in addition to the visible label.
    pub fn accessibility_description(mut self, description: impl Into<SharedString>) -> Self {
        self.accessibility_description = Some(description.into());
        self
    }

    /// Returns the accessible label.
    pub fn accessible_label(&self) -> &str {
        &self.accessible_label
    }

    /// Returns resolved action metadata used to create this icon button, if any.
    pub const fn resolved_action(&self) -> Option<&ResolvedActionState> {
        self.resolved_action.as_ref()
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
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let disabled_reason = self.disabled_reason.clone();
        let accessibility_description = self.accessibility_description.clone();
        let custom_tooltip = self.tooltip.clone();
        let tooltip_text = self
            .tooltip_text
            .clone()
            .filter(|_| custom_tooltip.is_none());
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let disabled = state.disabled();
        let theme_context = ThemeResolver::current(window, cx);
        let theme = &theme_context;
        let border_color = theme.resolve(colors.border());
        let background = theme.resolve(colors.background());
        let foreground = theme.resolve(colors.foreground());
        let hover_background = theme.resolve(colors.hover_background());
        let focus_shadow = focus_ring_shadow_with_theme(focus_ring, theme);

        let description = accessibility_description
            .as_deref()
            .or(disabled_reason.as_deref());
        let actions: &[AccessibleAction] = if self.on_activate.is_some() {
            &[AccessibleAction::Click, AccessibleAction::Focus]
        } else {
            &[AccessibleAction::Focus]
        };
        let mut semantics = SemanticDescriptor::new(state.role())
            .with_label(state.accessible_label())
            .with_selected(state.selected())
            .with_disabled(disabled)
            .with_actions(actions);
        if let Some(description) = description {
            semantics = semantics.with_description(description);
        }
        let activation_handle = self.activation_handle;
        let activation_state_key: ElementId = (self.id.clone(), "icon-button-activation").into();
        let custom_tooltip_theme = theme_context.clone();
        let text_tooltip_theme = theme_context.clone();
        let debug_id = self.id.to_string();

        div()
            .id(self.id)
            .debug_selector(move || format!("icon-button:{debug_id}:root"))
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
            .ui_semantics(&semantics)
            .focus_visible(move |style| style.shadow(focus_shadow.clone()))
            .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
            .when(!disabled, |this| {
                this.cursor_pointer()
                    .hover(move |style| style.bg(hover_background))
            })
            .when_some(self.on_activate, |this, on_activate| {
                ActivationBinding::new(
                    window,
                    cx,
                    activation_state_key,
                    !disabled,
                    ActivationKeyPolicy::EnterOrSpace,
                    move |activation, window, cx| on_activate(activation, window, cx),
                )
                .with_programmatic_handle(activation_handle)
                .bind(this)
            })
            .when_some(custom_tooltip, |this, tooltip| {
                this.tooltip(Tooltip::scoped(custom_tooltip_theme, move |window, cx| {
                    tooltip(window, cx)
                }))
            })
            .when_some(tooltip_text, |this, tooltip| {
                this.tooltip(Tooltip::scoped(text_tooltip_theme, Tooltip::text(tooltip)))
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
