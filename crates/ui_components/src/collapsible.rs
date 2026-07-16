//! Collapsible disclosure component.

use crate::a11y::UiA11yElementExt;
use crate::activation::{ActivationBinding, ActivationHandle, ActivationKeyPolicy};
use crate::button::{ButtonColors, ButtonMetrics, ButtonVariant};
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::geometry::gpui_px_from_ui;
use crate::theme::ThemeResolver;
use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, ElementId, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{AccessibleAction, Role, SemanticDescriptor, Sizable, Size, ThemeTokens};
use std::rc::Rc;

/// Resolved collapsible color intents.
pub type CollapsibleColors = ButtonColors;

/// Resolved collapsible trigger metrics.
pub type CollapsibleMetrics = ButtonMetrics;

/// Resolved collapsible state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct CollapsibleState {
    label: String,
    open: bool,
    disabled: bool,
    size: Size,
    metrics: CollapsibleMetrics,
    colors: CollapsibleColors,
    focus_ring: FocusRing,
}

impl CollapsibleState {
    /// Resolves the public state for a collapsible disclosure.
    pub fn resolve(
        label: impl Into<String>,
        open: bool,
        disabled: bool,
        size: Size,
        tokens: ThemeTokens,
    ) -> Self {
        let colors = ThemeResolver::button_colors(tokens, ButtonVariant::Outline, open);

        Self {
            label: label.into(),
            open,
            disabled,
            size,
            metrics: CollapsibleMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
        }
    }

    /// Returns the visible and accessible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns whether the disclosure content is open.
    pub const fn open(&self) -> bool {
        self.open
    }

    /// Returns whether the trigger is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns the next open state produced by trigger activation.
    pub const fn next_open(&self) -> bool {
        !self.open
    }

    /// Returns whether activation handlers should run.
    pub const fn activation_enabled(&self) -> bool {
        !self.disabled
    }

    /// Returns the accessibility role for the trigger.
    pub const fn trigger_role(&self) -> Role {
        Role::Button
    }

    /// Returns the accessibility role for the content region.
    pub const fn content_role(&self) -> Role {
        Role::Group
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns resolved trigger metrics.
    pub const fn metrics(&self) -> CollapsibleMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> CollapsibleColors {
        self.colors
    }

    /// Returns resolved focus ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum CollapsibleOpenControl {
    #[default]
    Uncontrolled,
    Controlled(bool),
}

impl CollapsibleOpenControl {
    const fn controlled(self) -> bool {
        matches!(self, Self::Controlled(_))
    }

    const fn value(self, default_open: bool) -> bool {
        match self {
            Self::Uncontrolled => default_open,
            Self::Controlled(open) => open,
        }
    }
}

/// A concrete GPUI collapsible disclosure.
#[derive(IntoElement)]
pub struct Collapsible {
    id: ElementId,
    label: SharedString,
    open_control: CollapsibleOpenControl,
    default_open: bool,
    disabled: bool,
    size: Size,
    tokens: ThemeTokens,
    content: Option<AnyElement>,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    activation_handle: Option<ActivationHandle>,
}

impl Collapsible {
    /// Creates a new collapsible disclosure.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            open_control: CollapsibleOpenControl::default(),
            default_open: false,
            disabled: false,
            size: Size::Medium,
            tokens: ThemeTokens::default(),
            content: None,
            on_open_change: None,
            activation_handle: None,
        }
    }

    /// Sets the controlled open state.
    pub fn open(mut self, open: bool) -> Self {
        self.open_control = CollapsibleOpenControl::Controlled(open);
        self
    }

    /// Seeds the initial open state for uncontrolled callers.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Marks the trigger as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets the disclosure content.
    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers an open-state change handler.
    pub fn on_open_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Rc::new(handler));
        self
    }

    /// Binds an application-owned programmatic activation handle.
    pub fn activation_handle(mut self, handle: &ActivationHandle) -> Self {
        self.activation_handle = Some(handle.clone());
        self
    }

    /// Returns the resolved collapsible state.
    pub fn state(&self) -> CollapsibleState {
        CollapsibleState::resolve(
            self.label.to_string(),
            self.open_control.value(self.default_open),
            self.disabled,
            self.size,
            self.tokens,
        )
    }
}

impl Sizable for Collapsible {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Collapsible {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Collapsible {
            id,
            label,
            open_control,
            default_open,
            disabled,
            size,
            tokens,
            content,
            on_open_change,
            activation_handle,
        } = self;
        let controlled = open_control.controlled();
        let initial_open = open_control.value(default_open);
        let runtime = window.use_keyed_state((id.clone(), "open-runtime"), cx, move |_, _| {
            CollapsibleRuntime { open: initial_open }
        });
        let open = if controlled {
            open_control.value(default_open)
        } else {
            runtime.read(cx).open
        };
        let state = CollapsibleState::resolve(label.to_string(), open, disabled, size, tokens);
        runtime.update(cx, |runtime, _| runtime.sync(&state));

        let theme = ThemeResolver::current(cx);
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let disabled = state.disabled();
        let open = state.open();
        let next_open = state.next_open();
        let content_id = format!("{id}:content");
        let debug_id = id.to_string();
        let root_debug_id = debug_id.clone();
        let border_color = theme.resolve(colors.border());
        let background = theme.resolve(colors.background());
        let foreground = theme.resolve(colors.foreground());
        let hover_background = theme.resolve(colors.hover_background());
        let focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);
        let activation = (!controlled || on_open_change.is_some()).then(|| {
            let activation_runtime = runtime.clone();
            let activation_handler = on_open_change.clone();
            ActivationBinding::new(
                window,
                cx,
                (id.clone(), "activation"),
                !disabled,
                ActivationKeyPolicy::EnterOrSpace,
                move |_, window, cx| {
                    let requested_open = if controlled {
                        next_open
                    } else {
                        activation_runtime.update(cx, CollapsibleRuntime::toggle)
                    };
                    if let Some(handler) = activation_handler.clone() {
                        handler(requested_open, window, cx);
                    }
                },
            )
            .with_programmatic_handle(activation_handle)
        });
        let trigger_actions: &[AccessibleAction] = if activation.is_some() && !disabled {
            &[AccessibleAction::Click, AccessibleAction::Focus]
        } else {
            &[AccessibleAction::Focus]
        };
        let trigger_semantics = SemanticDescriptor::new(state.trigger_role())
            .with_label(state.label())
            .with_disabled(disabled)
            .with_expanded(open)
            .with_actions(trigger_actions);
        let content_semantics = SemanticDescriptor::new(state.content_role());

        div()
            .id(id)
            .debug_selector(move || format!("collapsible:{root_debug_id}:root"))
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .id(format!("{content_id}:trigger"))
                    .debug_selector({
                        let debug_id = debug_id.clone();
                        move || format!("collapsible:{debug_id}:trigger")
                    })
                    .min_h(gpui_px_from_ui(metrics.height()))
                    .px(gpui_px_from_ui(metrics.padding_x()))
                    .py(gpui_px_from_ui(metrics.padding_y()))
                    .flex()
                    .items_center()
                    .justify_between()
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
                    .ui_semantics(&trigger_semantics)
                    .focus_visible(move |style| style.shadow(focus_shadow.clone()))
                    .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                    .when(!disabled, |this| {
                        this.cursor_pointer()
                            .hover(move |style| style.bg(hover_background))
                    })
                    .when_some(activation, |this, activation| activation.bind(this))
                    .child(div().flex_1().child(label))
                    .child(if open { "v" } else { ">" }),
            )
            .when_some(content.filter(|_| open), move |this, content| {
                this.child(
                    div()
                        .id(content_id)
                        .ui_semantics(&content_semantics)
                        .rounded(gpui_px_from_ui(metrics.radius()))
                        .border_1()
                        .border_color(border_color)
                        .bg(background)
                        .p_3()
                        .child(content),
                )
            })
    }
}

#[derive(Debug)]
struct CollapsibleRuntime {
    open: bool,
}

impl CollapsibleRuntime {
    fn sync(&mut self, state: &CollapsibleState) {
        self.open = state.open();
    }

    fn commit(&mut self, open: bool, cx: &mut open_gpui::Context<Self>) {
        if self.open != open {
            self.open = open;
            cx.notify();
        }
    }

    fn toggle(&mut self, cx: &mut open_gpui::Context<Self>) -> bool {
        let open = !self.open;
        self.commit(open, cx);
        open
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui_ui_core::{semantic, ui_px};

    #[test]
    fn state_clamps_to_controlled_open_value() {
        let state = Collapsible::new("details", "Details")
            .open(true)
            .small()
            .state();

        assert!(state.open());
        assert_eq!(state.next_open(), false);
        assert_eq!(state.trigger_role(), Role::Button);
        assert_eq!(state.content_role(), Role::Group);
        assert_eq!(state.size(), Size::Small);
        assert_eq!(state.metrics().height(), Size::Small.button_h());
        assert_eq!(state.colors().background().token(), semantic::ACCENT);
    }

    #[test]
    fn disabled_state_blocks_activation() {
        let state = Collapsible::new("details", "Details")
            .default_open(false)
            .disabled(true)
            .state();

        assert!(!state.open());
        assert_eq!(state.next_open(), true);
        assert!(state.disabled());
        assert!(!state.activation_enabled());
        assert_eq!(state.metrics().padding_x(), ui_px(12.0));
    }
}
