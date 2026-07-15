//! Tag component.

use crate::a11y::UiA11yElementExt;
use crate::badge::{BadgeColors, BadgeMetrics, BadgeVariant};
use crate::color::ColorIntent;
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::geometry::gpui_px_from_ui;
use crate::theme::ThemeResolver;
use open_gpui::prelude::*;
use open_gpui::{
    App, ElementId, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{AccessibleAction, Role, SemanticDescriptor, Sizable, Size, ThemeTokens};
use std::rc::Rc;

use crate::activation::{Activation, ActivationBinding, ActivationHandle, ActivationKeyPolicy};

/// Visual intent for a [`Tag`].
pub type TagVariant = BadgeVariant;

/// Resolved tag color intents.
pub type TagColors = BadgeColors;

/// Resolved tag metrics.
pub type TagMetrics = BadgeMetrics;

/// Tag remove payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagRemove {
    value: String,
    label: String,
}

impl TagRemove {
    /// Creates a tag remove payload.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }

    /// Returns the stable tag value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible tag label.
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Resolved tag state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct TagState {
    value: String,
    label: String,
    variant: TagVariant,
    removable: bool,
    disabled: bool,
    size: Size,
    metrics: TagMetrics,
    colors: TagColors,
    remove_focus_ring: FocusRing,
}

impl TagState {
    /// Resolves the public state for a tag.
    pub fn resolve(
        value: impl Into<String>,
        label: impl Into<String>,
        variant: TagVariant,
        removable: bool,
        disabled: bool,
        size: Size,
        tokens: ThemeTokens,
    ) -> Self {
        let colors = ThemeResolver::badge_colors(tokens, variant);

        Self {
            value: value.into(),
            label: label.into(),
            variant,
            removable,
            disabled,
            size,
            metrics: TagMetrics::from_size(size),
            colors,
            remove_focus_ring: FocusRing::from_color(ColorIntent::new(tokens.focus_ring, 0x2f80ed)),
        }
    }

    /// Returns the stable tag value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the visual variant.
    pub const fn variant(&self) -> TagVariant {
        self.variant
    }

    /// Returns whether the tag shows a remove affordance.
    pub const fn removable(&self) -> bool {
        self.removable
    }

    /// Returns whether the tag is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the remove handler should run.
    pub const fn remove_enabled(&self) -> bool {
        self.removable && !self.disabled
    }

    /// Returns the accessibility role for the tag root.
    pub const fn role(&self) -> Role {
        Role::Label
    }

    /// Returns the accessibility role for the remove affordance.
    pub const fn remove_role(&self) -> Role {
        Role::Button
    }

    /// Returns remove payload for removable enabled tags.
    pub fn remove(&self) -> Option<TagRemove> {
        self.remove_enabled()
            .then(|| TagRemove::new(self.value.clone(), self.label.clone()))
    }

    fn remove_payload(&self) -> Option<TagRemove> {
        self.removable
            .then(|| TagRemove::new(self.value.clone(), self.label.clone()))
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> TagMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> TagColors {
        self.colors
    }

    /// Returns resolved remove focus ring metadata.
    pub const fn remove_focus_ring(&self) -> FocusRing {
        self.remove_focus_ring
    }
}

/// A concrete GPUI tag component.
#[derive(IntoElement)]
pub struct Tag {
    id: ElementId,
    value: SharedString,
    label: SharedString,
    variant: TagVariant,
    removable: bool,
    disabled: bool,
    size: Size,
    tokens: ThemeTokens,
    on_remove: Option<Rc<dyn Fn(TagRemove, Activation, &mut Window, &mut App)>>,
    activation_handle: Option<ActivationHandle>,
}

impl Tag {
    /// Creates a new tag.
    pub fn new(
        id: impl Into<ElementId>,
        value: impl Into<SharedString>,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            value: value.into(),
            label: label.into(),
            variant: TagVariant::Secondary,
            removable: false,
            disabled: false,
            size: Size::Medium,
            tokens: ThemeTokens::default(),
            on_remove: None,
            activation_handle: None,
        }
    }

    /// Applies a visual variant.
    pub fn variant(mut self, variant: TagVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Shows or hides the remove affordance.
    pub fn removable(mut self, removable: bool) -> Self {
        self.removable = removable;
        self
    }

    /// Marks the tag as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers a remove handler.
    pub fn on_remove(
        mut self,
        handler: impl Fn(TagRemove, Activation, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_remove = Some(Rc::new(handler));
        self.removable = true;
        self
    }

    /// Binds an application-owned programmatic remove activation handle.
    pub fn activation_handle(mut self, handle: &ActivationHandle) -> Self {
        self.activation_handle = Some(handle.clone());
        self
    }

    /// Returns the resolved tag state.
    pub fn state(&self) -> TagState {
        TagState::resolve(
            self.value.to_string(),
            self.label.to_string(),
            self.variant,
            self.removable,
            self.disabled,
            self.size,
            self.tokens,
        )
    }
}

impl Sizable for Tag {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Tag {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(cx);
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let remove_focus_ring = state.remove_focus_ring();
        let remove_focus_shadow = focus_ring_shadow_with_theme(remove_focus_ring, &theme);
        let disabled = state.disabled();
        let remove_enabled = state.remove_enabled();
        let label = self.label.clone();
        let remove_payload = state.remove_payload();
        let remove_state_key = (self.id.clone(), "remove-activation");
        let remove_debug_id = self.id.to_string();
        let activation_handle = self.activation_handle;
        let semantics = SemanticDescriptor::new(state.role())
            .with_label(label.as_ref())
            .with_disabled(disabled);

        div()
            .id(self.id)
            .min_h(gpui_px_from_ui(metrics.min_height()))
            .px(gpui_px_from_ui(metrics.padding_x()))
            .py(gpui_px_from_ui(metrics.padding_y()))
            .flex()
            .items_center()
            .gap_1()
            .rounded(gpui_px_from_ui(metrics.radius()))
            .border_1()
            .border_color(theme.resolve(colors.border()))
            .bg(theme.resolve(colors.background()))
            .text_color(theme.resolve(colors.foreground()))
            .text_size(gpui_px_from_ui(metrics.text_size()))
            .line_height(gpui_px_from_ui(metrics.text_size()))
            .ui_semantics(&semantics)
            .when(disabled, |this| this.opacity(0.56))
            .child(label)
            .when_some(
                self.on_remove.zip(remove_payload),
                |this, (on_remove, remove_payload)| {
                    let remove_label = format!("Remove {}", remove_payload.label());
                    let remove_actions: &[AccessibleAction] = if remove_enabled {
                        &[AccessibleAction::Click, AccessibleAction::Focus]
                    } else {
                        &[AccessibleAction::Focus]
                    };
                    let remove_semantics = SemanticDescriptor::new(state.remove_role())
                        .with_label(&remove_label)
                        .with_disabled(disabled)
                        .with_actions(remove_actions);
                    let activation_payload = remove_payload.clone();
                    let remove_debug_id = remove_debug_id.clone();
                    this.child(
                        ActivationBinding::new(
                            window,
                            cx,
                            remove_state_key.clone(),
                            remove_enabled,
                            ActivationKeyPolicy::EnterOrSpace,
                            move |activation, window, cx| {
                                on_remove(activation_payload.clone(), activation, window, cx);
                            },
                        )
                        .with_programmatic_handle(activation_handle.clone())
                        .bind(
                            div()
                                .id(format!("tag-remove:{}", remove_payload.value()))
                                .debug_selector(move || format!("tag:{remove_debug_id}:remove"))
                                .size(gpui_px_from_ui(metrics.min_height()))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(gpui_px_from_ui(metrics.radius()))
                                .focusable()
                                .tab_stop(remove_enabled)
                                .ui_semantics(&remove_semantics)
                                .focus_visible(move |style| {
                                    style.shadow(remove_focus_shadow.clone())
                                })
                                .when(remove_enabled, |this| this.cursor_pointer())
                                .when(disabled, |this| this.cursor_not_allowed())
                                .child("x"),
                        ),
                    )
                },
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui_ui_core::semantic;

    #[test]
    fn tag_supports_removable_payload() {
        let state = Tag::new("tag-ready", "ready", "Ready")
            .variant(TagVariant::Outline)
            .removable(true)
            .state();

        assert_eq!(state.role(), Role::Label);
        assert_eq!(state.remove_role(), Role::Button);
        assert_eq!(state.value(), "ready");
        assert_eq!(state.label(), "Ready");
        assert_eq!(state.variant(), TagVariant::Outline);
        assert_eq!(state.colors().border().token(), semantic::BORDER);
        let remove = state.remove().expect("removable tag should emit payload");
        assert_eq!(remove.value(), "ready");
        assert_eq!(remove.label(), "Ready");
    }

    #[test]
    fn disabled_tag_blocks_remove() {
        let state = Tag::new("tag-ready", "ready", "Ready")
            .removable(true)
            .disabled(true)
            .state();

        assert!(state.disabled());
        assert!(!state.remove_enabled());
        assert_eq!(state.remove(), None);
    }
}
