//! Breadcrumb navigation component.

use crate::a11y::UiA11yElementExt;
use crate::color::{ColorIntent, ColorState};
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::geometry::gpui_px_from_ui;
use crate::theme::ThemeResolver;
use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, ElementId, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{Role, Sizable, Size, ThemeTokens, UiPx, ui_px};
use std::rc::Rc;

/// Pure descriptor for one breadcrumb item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbItemDescriptor {
    value: String,
    label: String,
    href: Option<String>,
    current: bool,
    disabled: bool,
}

impl BreadcrumbItemDescriptor {
    /// Creates a breadcrumb item descriptor.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            href: None,
            current: false,
            disabled: false,
        }
    }

    /// Assigns a navigation href.
    pub fn href(mut self, href: impl Into<String>) -> Self {
        self.href = Some(href.into());
        self
    }

    /// Marks the item as the current page.
    pub fn current(mut self, current: bool) -> Self {
        self.current = current;
        self
    }

    /// Marks the item as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Returns the stable item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the navigation href.
    pub fn href_value(&self) -> Option<&str> {
        self.href.as_deref()
    }

    /// Returns whether the item is current.
    pub const fn current_state(&self) -> bool {
        self.current
    }

    /// Returns whether the item is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }
}

/// Resolved breadcrumb color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreadcrumbColors {
    text: ColorIntent,
    current_text: ColorIntent,
    separator: ColorIntent,
    focus_ring: ColorIntent,
}

impl BreadcrumbColors {
    /// Resolves colors from tokens.
    pub const fn from_tokens(tokens: ThemeTokens) -> Self {
        Self {
            text: ColorIntent::new(tokens.text_muted, 0x667085),
            current_text: ColorIntent::new(tokens.text, 0x1d2939),
            separator: ColorIntent::new(tokens.border, 0xd0d5dd),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                0x2f80ed,
            ),
        }
    }

    /// Returns default breadcrumb text color.
    pub const fn text(self) -> ColorIntent {
        self.text
    }

    /// Returns current item text color.
    pub const fn current_text(self) -> ColorIntent {
        self.current_text
    }

    /// Returns separator color.
    pub const fn separator(self) -> ColorIntent {
        self.separator
    }

    /// Returns focus ring color.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved breadcrumb metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BreadcrumbMetrics {
    text_size: UiPx,
    gap: UiPx,
    radius: UiPx,
    padding_x: UiPx,
    padding_y: UiPx,
}

impl BreadcrumbMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            text_size: size.control_text_px(),
            gap: ui_px(6.0),
            radius: size.control_radius(),
            padding_x: ui_px(2.0),
            padding_y: ui_px(1.0),
        }
    }

    /// Returns text size.
    pub const fn text_size(self) -> UiPx {
        self.text_size
    }

    /// Returns item gap.
    pub const fn gap(self) -> UiPx {
        self.gap
    }

    /// Returns item radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns item horizontal padding.
    pub const fn padding_x(self) -> UiPx {
        self.padding_x
    }

    /// Returns item vertical padding.
    pub const fn padding_y(self) -> UiPx {
        self.padding_y
    }
}

/// Breadcrumb activation payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbActivation {
    index: usize,
    value: String,
    label: String,
    href: Option<String>,
}

impl BreadcrumbActivation {
    /// Creates a breadcrumb activation payload.
    pub fn new(
        index: usize,
        value: impl Into<String>,
        label: impl Into<String>,
        href: Option<String>,
    ) -> Self {
        Self {
            index,
            value: value.into(),
            label: label.into(),
            href,
        }
    }

    /// Returns activated item index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns activated item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns activated item label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns activated item href.
    pub fn href(&self) -> Option<&str> {
        self.href.as_deref()
    }
}

/// Resolved breadcrumb item state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbItemState {
    index: usize,
    value: String,
    label: String,
    href: Option<String>,
    current: bool,
    disabled: bool,
}

impl BreadcrumbItemState {
    /// Returns the zero-based item index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the stable item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the href.
    pub fn href(&self) -> Option<&str> {
        self.href.as_deref()
    }

    /// Returns whether this item is the current page.
    pub const fn current(&self) -> bool {
        self.current
    }

    /// Returns whether this item is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether activation handlers should run.
    pub const fn activation_enabled(&self) -> bool {
        !self.current && !self.disabled
    }

    /// Returns the accessibility role.
    pub const fn role(&self) -> Role {
        if self.activation_enabled() {
            Role::Link
        } else {
            Role::Label
        }
    }

    /// Returns an activation payload for enabled items.
    pub fn activation(&self) -> Option<BreadcrumbActivation> {
        self.activation_enabled().then(|| {
            BreadcrumbActivation::new(
                self.index,
                self.value.clone(),
                self.label.clone(),
                self.href.clone(),
            )
        })
    }
}

/// Resolved breadcrumb state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct BreadcrumbState {
    label: String,
    disabled: bool,
    size: Size,
    items: Vec<BreadcrumbItemState>,
    current_index: Option<usize>,
    metrics: BreadcrumbMetrics,
    colors: BreadcrumbColors,
    focus_ring: FocusRing,
}

impl BreadcrumbState {
    /// Resolves public state for a breadcrumb.
    pub fn resolve(
        label: impl Into<String>,
        disabled: bool,
        size: Size,
        items: impl IntoIterator<Item = BreadcrumbItemDescriptor>,
        tokens: ThemeTokens,
    ) -> Self {
        let descriptors: Vec<BreadcrumbItemDescriptor> = items.into_iter().collect();
        let explicit_current = descriptors.iter().position(|item| item.current);
        let fallback_current = descriptors.len().checked_sub(1);
        let current_index = explicit_current.or(fallback_current);
        let colors = BreadcrumbColors::from_tokens(tokens);

        let items = descriptors
            .into_iter()
            .enumerate()
            .map(|(index, descriptor)| BreadcrumbItemState {
                index,
                value: descriptor.value,
                label: descriptor.label,
                href: descriptor.href,
                current: Some(index) == current_index,
                disabled: disabled || descriptor.disabled,
            })
            .collect();

        Self {
            label: label.into(),
            disabled,
            size,
            items,
            current_index,
            metrics: BreadcrumbMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
        }
    }

    /// Returns the navigation label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns whether the whole breadcrumb is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns the navigation landmark role.
    pub const fn role(&self) -> Role {
        Role::Navigation
    }

    /// Returns resolved item states.
    pub fn items(&self) -> &[BreadcrumbItemState] {
        &self.items
    }

    /// Returns the current item index.
    pub const fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> BreadcrumbMetrics {
        self.metrics
    }

    /// Returns resolved colors.
    pub const fn colors(&self) -> BreadcrumbColors {
        self.colors
    }

    /// Returns focus ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }
}

/// A concrete GPUI breadcrumb component.
#[derive(IntoElement)]
pub struct Breadcrumb {
    id: ElementId,
    label: SharedString,
    disabled: bool,
    size: Size,
    tokens: ThemeTokens,
    items: Vec<BreadcrumbItemDescriptor>,
    on_activate: Option<Rc<dyn Fn(BreadcrumbActivation, &ClickEvent, &mut Window, &mut App)>>,
}

impl Breadcrumb {
    /// Creates a new breadcrumb navigation component.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
            size: Size::Medium,
            tokens: ThemeTokens::default(),
            items: Vec::new(),
            on_activate: None,
        }
    }

    /// Marks the whole breadcrumb as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Adds one breadcrumb item.
    pub fn item(mut self, item: BreadcrumbItemDescriptor) -> Self {
        self.items.push(item);
        self
    }

    /// Adds many breadcrumb items.
    pub fn items(mut self, items: impl IntoIterator<Item = BreadcrumbItemDescriptor>) -> Self {
        self.items.extend(items);
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
        handler: impl Fn(BreadcrumbActivation, &ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    /// Returns the resolved breadcrumb state.
    pub fn state(&self) -> BreadcrumbState {
        BreadcrumbState::resolve(
            self.label.to_string(),
            self.disabled,
            self.size,
            self.items.clone(),
            self.tokens,
        )
    }
}

impl Sizable for Breadcrumb {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Breadcrumb {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(cx);
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let on_activate = self.on_activate.clone();

        div()
            .id(self.id)
            .ui_role(state.role())
            .aria_label(self.label)
            .aria_disabled(state.disabled())
            .flex()
            .items_center()
            .gap(gpui_px_from_ui(metrics.gap()))
            .text_size(gpui_px_from_ui(metrics.text_size()))
            .line_height(gpui_px_from_ui(metrics.text_size()))
            .children(state.items().iter().enumerate().flat_map(|(index, item)| {
                let mut elements = Vec::new();
                if index > 0 {
                    elements.push(
                        div()
                            .text_color(theme.resolve(colors.separator()))
                            .child("/")
                            .into_any_element(),
                    );
                }

                let activation = item.activation();
                let disabled = item.disabled();
                let current = item.current();
                let label = item.label().to_owned();
                let item_role = item.role();
                let on_activate = on_activate.clone();
                let item_text = theme.resolve(if current {
                    colors.current_text()
                } else {
                    colors.text()
                });
                let item_hover_text = theme.resolve(colors.current_text());
                let item_focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);

                elements.push(
                    div()
                        .id(format!("breadcrumb-item:{}", item.value()))
                        .px(gpui_px_from_ui(metrics.padding_x()))
                        .py(gpui_px_from_ui(metrics.padding_y()))
                        .rounded(gpui_px_from_ui(metrics.radius()))
                        .ui_role(item_role)
                        .aria_label(label.clone())
                        .aria_disabled(disabled)
                        .tab_stop(!current && !disabled)
                        .focusable()
                        .text_color(item_text)
                        .focus_visible(move |style| style.shadow(item_focus_shadow.clone()))
                        .when(current, |this| {
                            this.font_weight(open_gpui::FontWeight::BOLD)
                        })
                        .when(!current && !disabled, |this| {
                            this.cursor_pointer()
                                .hover(move |style| style.text_color(item_hover_text))
                        })
                        .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                        .when_some(
                            on_activate.zip(activation),
                            |this, (on_activate, activation)| {
                                this.on_click(move |event, window, cx| {
                                    cx.stop_propagation();
                                    on_activate(activation.clone(), event, window, cx);
                                })
                            },
                        )
                        .child(label)
                        .into_any_element(),
                );
                elements
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui_ui_core::semantic;

    #[test]
    fn breadcrumb_marks_last_item_current_by_default() {
        let state = Breadcrumb::new("crumbs", "Project path")
            .item(BreadcrumbItemDescriptor::new("home", "Home").href("/"))
            .item(BreadcrumbItemDescriptor::new("docs", "Docs").href("/docs"))
            .state();

        assert_eq!(state.role(), Role::Navigation);
        assert_eq!(state.current_index(), Some(1));
        assert_eq!(state.items()[0].role(), Role::Link);
        assert_eq!(state.items()[1].role(), Role::Label);
        assert_eq!(state.items()[0].activation().unwrap().href(), Some("/"));
        assert_eq!(state.colors().current_text().token(), semantic::TEXT);
    }

    #[test]
    fn breadcrumb_disabled_items_do_not_activate() {
        let state = Breadcrumb::new("crumbs", "Project path")
            .item(
                BreadcrumbItemDescriptor::new("home", "Home")
                    .href("/")
                    .disabled(true),
            )
            .item(BreadcrumbItemDescriptor::new("docs", "Docs").href("/docs"))
            .state();

        assert!(state.items()[0].disabled());
        assert!(!state.items()[0].activation_enabled());
        assert_eq!(state.items()[0].activation(), None);
    }
}
