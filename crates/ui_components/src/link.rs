//! Link component.

use crate::a11y::UiA11yElementExt;
use crate::color::{ColorIntent, ColorState};
use crate::focus::{FocusRing, focus_ring_shadow};
use crate::geometry::gpui_px_from_ui;
use crate::theme::ThemeResolver;
use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, ElementId, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{Role, Sizable, Size, ThemeTokens, UiPx, ui_px};
use std::rc::Rc;

/// Resolved link color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkColors {
    text: ColorIntent,
    hover_text: ColorIntent,
    focus_ring: ColorIntent,
}

impl LinkColors {
    /// Resolves link colors from tokens.
    pub const fn from_tokens(tokens: ThemeTokens) -> Self {
        Self {
            text: ColorIntent::new(tokens.accent, 0x1f7a66),
            hover_text: ColorIntent::with_state(tokens.accent, ColorState::Hover, 0x176b5a),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                0x2f80ed,
            ),
        }
    }

    /// Returns link text color.
    pub const fn text(self) -> ColorIntent {
        self.text
    }

    /// Returns hovered text color.
    pub const fn hover_text(self) -> ColorIntent {
        self.hover_text
    }

    /// Returns focus ring color.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved link metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinkMetrics {
    text_size: UiPx,
    radius: UiPx,
    padding_x: UiPx,
    padding_y: UiPx,
}

impl LinkMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            text_size: size.control_text_px(),
            radius: size.control_radius(),
            padding_x: ui_px(2.0),
            padding_y: ui_px(1.0),
        }
    }

    /// Returns text size.
    pub const fn text_size(self) -> UiPx {
        self.text_size
    }

    /// Returns focus radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns horizontal padding.
    pub const fn padding_x(self) -> UiPx {
        self.padding_x
    }

    /// Returns vertical padding.
    pub const fn padding_y(self) -> UiPx {
        self.padding_y
    }
}

/// Link activation payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkActivation {
    href: String,
    label: String,
}

impl LinkActivation {
    /// Creates a link activation payload.
    pub fn new(href: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            href: href.into(),
            label: label.into(),
        }
    }

    /// Returns the target href.
    pub fn href(&self) -> &str {
        &self.href
    }

    /// Returns the activated label.
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Resolved link state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkState {
    label: String,
    href: String,
    disabled: bool,
    external: bool,
    size: Size,
    metrics: LinkMetrics,
    colors: LinkColors,
    focus_ring: FocusRing,
}

impl LinkState {
    /// Resolves the public state for a link.
    pub fn resolve(
        label: impl Into<String>,
        href: impl Into<String>,
        disabled: bool,
        external: bool,
        size: Size,
        tokens: ThemeTokens,
    ) -> Self {
        let colors = LinkColors::from_tokens(tokens);

        Self {
            label: label.into(),
            href: href.into(),
            disabled,
            external,
            size,
            metrics: LinkMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
        }
    }

    /// Returns the visible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the target href.
    pub fn href(&self) -> &str {
        &self.href
    }

    /// Returns whether the link is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the link targets an external destination.
    pub const fn external(&self) -> bool {
        self.external
    }

    /// Returns whether activation handlers should run.
    pub const fn activation_enabled(&self) -> bool {
        !self.disabled
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns the accessibility role.
    pub const fn role(&self) -> Role {
        Role::Link
    }

    /// Returns the activation payload for enabled links.
    pub fn activation(&self) -> Option<LinkActivation> {
        self.activation_enabled()
            .then(|| LinkActivation::new(self.href.clone(), self.label.clone()))
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> LinkMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> LinkColors {
        self.colors
    }

    /// Returns resolved focus ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }
}

/// A concrete GPUI link component.
#[derive(IntoElement)]
pub struct Link {
    id: ElementId,
    label: SharedString,
    href: SharedString,
    disabled: bool,
    external: bool,
    size: Size,
    tokens: ThemeTokens,
    on_activate: Option<Rc<dyn Fn(LinkActivation, &ClickEvent, &mut Window, &mut App)>>,
}

impl Link {
    /// Creates a new link.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        href: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            href: href.into(),
            disabled: false,
            external: false,
            size: Size::Medium,
            tokens: ThemeTokens::default(),
            on_activate: None,
        }
    }

    /// Marks the link as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks the link as external.
    pub fn external(mut self, external: bool) -> Self {
        self.external = external;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers an activation handler.
    pub fn on_activate(
        mut self,
        handler: impl Fn(LinkActivation, &ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    /// Returns the resolved link state.
    pub fn state(&self) -> LinkState {
        LinkState::resolve(
            self.label.to_string(),
            self.href.to_string(),
            self.disabled,
            self.external,
            self.size,
            self.tokens,
        )
    }
}

impl Sizable for Link {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Link {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let disabled = state.disabled();
        let label = self.label.clone();
        let visible_label = label.to_string();

        div()
            .id(self.id)
            .px(gpui_px_from_ui(metrics.padding_x()))
            .py(gpui_px_from_ui(metrics.padding_y()))
            .rounded(gpui_px_from_ui(metrics.radius()))
            .text_size(gpui_px_from_ui(metrics.text_size()))
            .line_height(gpui_px_from_ui(metrics.text_size()))
            .text_color(ThemeResolver::resolve(colors.text()))
            .underline()
            .focusable()
            .tab_stop(!disabled)
            .ui_role(state.role())
            .aria_label(label)
            .focus_visible(move |style| style.shadow(focus_ring_shadow(focus_ring)))
            .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
            .when(!disabled, |this| {
                this.cursor_pointer().hover(move |style| {
                    style.text_color(ThemeResolver::resolve(colors.hover_text()))
                })
            })
            .when_some(
                self.on_activate
                    .filter(|_| !disabled)
                    .zip(state.activation()),
                |this, (on_activate, activation)| {
                    this.on_click(move |event, window, cx| {
                        cx.stop_propagation();
                        on_activate(activation.clone(), event, window, cx);
                    })
                },
            )
            .child(visible_label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui_ui_core::semantic;

    #[test]
    fn link_state_exposes_accessible_activation_payload() {
        let state = Link::new("docs", "Docs", "/docs").external(true).state();

        assert_eq!(state.role(), Role::Link);
        assert_eq!(state.label(), "Docs");
        assert_eq!(state.href(), "/docs");
        assert!(state.external());
        assert_eq!(state.colors().text().token(), semantic::ACCENT);
        let activation = state.activation().expect("enabled link should activate");
        assert_eq!(activation.href(), "/docs");
        assert_eq!(activation.label(), "Docs");
    }

    #[test]
    fn disabled_link_blocks_activation() {
        let state = Link::new("docs", "Docs", "/docs").disabled(true).state();

        assert!(state.disabled());
        assert!(!state.activation_enabled());
        assert_eq!(state.activation(), None);
    }
}
