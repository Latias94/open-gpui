//! Button component.

use crate::geometry::gpui_px_from_ui;
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    AnyView, App, ElementId, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{
    AccessibleAction, Role, SemanticDescriptor, Sizable, Size, ThemeDesignScales, ThemeTokens, UiPx,
};

use crate::a11y::UiA11yElementExt;
use crate::action::ResolvedActionState;
use crate::activation::{Activation, ActivationBinding, ActivationHandle, ActivationKeyPolicy};
use crate::color::ColorIntent;
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::theme::ThemeResolver;
use crate::tooltip::Tooltip;

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
    size: Size,
    height: UiPx,
    padding_x: UiPx,
    padding_y: UiPx,
    radius: UiPx,
    text_size: UiPx,
    line_height: UiPx,
}

impl ButtonMetrics {
    /// Resolves the built-in Theme v1 metric baseline for a size.
    pub const fn from_size(size: Size) -> Self {
        let design = ThemeDesignScales::baseline();
        Self::from_theme_values(
            size,
            design.spacing().control_inline().resolve(size),
            design.spacing().control_block().resolve(size),
            design.radius().control().resolve(size),
            design.typography().control_text().resolve(size),
            design.typography().control_line_height().resolve(size),
        )
    }

    pub(crate) const fn from_theme_values(
        size: Size,
        padding_x: UiPx,
        padding_y: UiPx,
        radius: UiPx,
        text_size: UiPx,
        line_height: UiPx,
    ) -> Self {
        Self {
            size,
            height: size.button_h(),
            padding_x,
            padding_y,
            radius,
            text_size,
            line_height,
        }
    }

    /// Returns the resolved component size.
    pub const fn size(self) -> Size {
        self.size
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

    /// Returns the text line height.
    pub const fn line_height(self) -> UiPx {
        self.line_height
    }
}

/// Renderer-neutral button state used by tests, demos, and rendering.
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
        Self::resolve_with_metrics(
            variant,
            ButtonMetrics::from_size(size),
            disabled,
            selected,
            tokens,
        )
    }

    fn resolve_with_metrics(
        variant: ButtonVariant,
        metrics: ButtonMetrics,
        disabled: bool,
        selected: bool,
        tokens: ThemeTokens,
    ) -> Self {
        let colors = ThemeResolver::button_colors(tokens, variant, selected);

        Self {
            variant,
            size: metrics.size(),
            disabled,
            selected,
            metrics,
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
    icon: Option<SharedString>,
    variant: ButtonVariant,
    size: Option<Size>,
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

impl Button {
    /// Creates a new button with an id and visible label.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            variant: ButtonVariant::Default,
            size: None,
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

    /// Creates a button from resolved action metadata.
    pub fn from_resolved_action(id: impl Into<ElementId>, action: &ResolvedActionState) -> Self {
        Self {
            id: id.into(),
            label: action.label().into(),
            icon: action.icon_label().map(SharedString::from),
            variant: ButtonVariant::Default,
            size: None,
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

    /// Applies an icon label resolved by the app.
    pub fn icon_label(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
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

    /// Adds a hover/focus tooltip to the button.
    pub fn tooltip(mut self, tooltip: impl Fn(&mut Window, &mut App) -> AnyView + 'static) -> Self {
        self.tooltip = Some(Rc::new(tooltip));
        self
    }

    /// Adds a hover/focus text tooltip to the button.
    pub fn tooltip_text(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip_text = Some(tooltip.into());
        self
    }

    /// Applies an accessibility description in addition to the visible label.
    pub fn accessibility_description(mut self, description: impl Into<SharedString>) -> Self {
        self.accessibility_description = Some(description.into());
        self
    }

    /// Returns resolved action metadata used to create this button, if any.
    pub const fn resolved_action(&self) -> Option<&ResolvedActionState> {
        self.resolved_action.as_ref()
    }

    /// Returns renderer-neutral state using the built-in Theme v1 metric baseline.
    pub fn state(&self) -> ButtonState {
        self.state_for_size(self.size.unwrap_or_default())
    }

    fn state_for_size(&self, size: Size) -> ButtonState {
        self.state_for_metrics(ButtonMetrics::from_size(size))
    }

    fn state_for_metrics(&self, metrics: ButtonMetrics) -> ButtonState {
        ButtonState::resolve_with_metrics(
            self.variant,
            metrics,
            self.disabled,
            self.selected,
            self.tokens,
        )
    }
}

impl Sizable for Button {
    fn with_size(mut self, size: Size) -> Self {
        self.size = Some(size);
        self
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let label = self.label.clone();
        let icon = self.icon.clone();
        let disabled_reason = self.disabled_reason.clone();
        let accessibility_description = self.accessibility_description.clone();
        let custom_tooltip = self.tooltip.clone();
        let tooltip_text = self
            .tooltip_text
            .clone()
            .filter(|_| custom_tooltip.is_none());
        let theme_context = ThemeResolver::current(window, cx);
        let metrics = ThemeResolver::button_metrics(&theme_context, self.size);
        let state = self.state_for_metrics(metrics);
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let disabled = state.disabled();
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
            .with_label(label.as_ref())
            .with_selected(state.selected())
            .with_disabled(disabled)
            .with_actions(actions);
        if let Some(description) = description {
            semantics = semantics.with_description(description);
        }

        let debug_id = self.id.to_string();
        let activation_state_key: ElementId = (self.id.clone(), "button-activation").into();
        let activation_handle = self.activation_handle;
        let custom_tooltip_theme = theme_context.clone();
        let text_tooltip_theme = theme_context.clone();

        div()
            .id(self.id)
            .debug_selector(move || format!("button:{debug_id}:root"))
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
            .line_height(gpui_px_from_ui(metrics.line_height()))
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
            .when_some(icon, |this, icon| this.child(icon))
            .child(label)
    }
}
