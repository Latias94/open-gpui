//! Accordion component.

use crate::a11y::UiA11yElementExt;
use crate::button::{ButtonColors, ButtonMetrics, ButtonVariant};
use crate::focus::{FocusRing, focus_ring_shadow};
use crate::geometry::gpui_px_from_ui;
use crate::theme::ThemeResolver;
use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, ElementId, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{Role, Sizable, Size, ThemeTokens};
use std::collections::BTreeSet;
use std::rc::Rc;

/// Accordion selection behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccordionMode {
    /// At most one item may be open.
    #[default]
    Single,
    /// Multiple items may be open.
    Multiple,
}

impl AccordionMode {
    /// Returns the stable mode label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Multiple => "multiple",
        }
    }
}

/// Pure descriptor for one accordion item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccordionItemDescriptor {
    value: String,
    label: String,
    disabled: bool,
}

impl AccordionItemDescriptor {
    /// Creates an accordion item descriptor.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
        }
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

    /// Returns the visible item label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns whether the item is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }
}

/// Concrete accordion item builder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccordionItem {
    descriptor: AccordionItemDescriptor,
    content: SharedString,
}

impl AccordionItem {
    /// Creates an accordion item with text content.
    pub fn new(
        value: impl Into<String>,
        label: impl Into<String>,
        content: impl Into<SharedString>,
    ) -> Self {
        Self {
            descriptor: AccordionItemDescriptor::new(value, label),
            content: content.into(),
        }
    }

    /// Marks the item as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.descriptor = self.descriptor.disabled(disabled);
        self
    }

    /// Returns the pure descriptor.
    pub fn descriptor(&self) -> &AccordionItemDescriptor {
        &self.descriptor
    }

    /// Returns the text content.
    pub fn content(&self) -> &str {
        &self.content
    }
}

impl From<AccordionItem> for AccordionItemDescriptor {
    fn from(item: AccordionItem) -> Self {
        item.descriptor
    }
}

/// Resolved accordion color intents.
pub type AccordionColors = ButtonColors;

/// Resolved accordion item metrics.
pub type AccordionMetrics = ButtonMetrics;

/// Resolved accordion item state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccordionItemState {
    index: usize,
    value: String,
    label: String,
    open: bool,
    disabled: bool,
}

impl AccordionItemState {
    /// Returns the zero-based item index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the stable item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible item label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns whether the item is open.
    pub const fn open(&self) -> bool {
        self.open
    }

    /// Returns whether the item is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether activation handlers should run for this item.
    pub const fn activation_enabled(&self) -> bool {
        !self.disabled
    }

    /// Returns the accessibility role for the item trigger.
    pub const fn trigger_role(&self) -> Role {
        Role::Button
    }

    /// Returns the accessibility role for the item content region.
    pub const fn content_role(&self) -> Role {
        Role::Group
    }
}

/// Accordion open-change payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccordionOpenChange {
    item: AccordionItemState,
    open_values: Vec<String>,
}

impl AccordionOpenChange {
    /// Resolves the open-change payload for a requested item toggle.
    pub fn resolve(
        mode: AccordionMode,
        collapsible: bool,
        current_open_values: impl IntoIterator<Item = impl AsRef<str>>,
        item: &AccordionItemState,
    ) -> Self {
        let mut open_values = normalize_accordion_open_values(
            mode,
            current_open_values
                .into_iter()
                .map(|value| value.as_ref().to_owned()),
        );

        if item.disabled() {
            return Self {
                item: item.clone(),
                open_values,
            };
        }

        match mode {
            AccordionMode::Single => {
                if item.open() && collapsible {
                    open_values.clear();
                } else {
                    open_values.clear();
                    open_values.push(item.value().to_owned());
                }
            }
            AccordionMode::Multiple => {
                if item.open() {
                    open_values.retain(|value| value != item.value());
                } else if !open_values.iter().any(|value| value == item.value()) {
                    open_values.push(item.value().to_owned());
                }
            }
        }

        Self {
            item: item.clone(),
            open_values,
        }
    }

    /// Returns the item that requested the change.
    pub const fn item(&self) -> &AccordionItemState {
        &self.item
    }

    /// Returns the next open values.
    pub fn open_values(&self) -> &[String] {
        &self.open_values
    }
}

/// Resolved accordion state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct AccordionState {
    mode: AccordionMode,
    collapsible: bool,
    size: Size,
    items: Vec<AccordionItemState>,
    open_values: Vec<String>,
    metrics: AccordionMetrics,
    colors: AccordionColors,
    focus_ring: FocusRing,
}

impl AccordionState {
    /// Resolves the public state for an accordion.
    pub fn resolve(
        mode: AccordionMode,
        collapsible: bool,
        open_values: impl IntoIterator<Item = impl AsRef<str>>,
        items: impl IntoIterator<Item = AccordionItemDescriptor>,
        size: Size,
        tokens: ThemeTokens,
    ) -> Self {
        let descriptors = items.into_iter().collect::<Vec<_>>();
        let allowed_values = descriptors
            .iter()
            .map(|item| item.value.clone())
            .collect::<BTreeSet<_>>();
        let open_values = normalize_accordion_open_values(
            mode,
            open_values.into_iter().filter_map(|value| {
                let value = value.as_ref();
                allowed_values.contains(value).then(|| value.to_owned())
            }),
        );
        let open_set = open_values.iter().cloned().collect::<BTreeSet<_>>();
        let items = descriptors
            .into_iter()
            .enumerate()
            .map(|(index, descriptor)| AccordionItemState {
                index,
                open: open_set.contains(&descriptor.value),
                value: descriptor.value,
                label: descriptor.label,
                disabled: descriptor.disabled,
            })
            .collect::<Vec<_>>();
        let colors = ThemeResolver::button_colors(tokens, ButtonVariant::Outline, false);

        Self {
            mode,
            collapsible,
            size,
            items,
            open_values,
            metrics: AccordionMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
        }
    }

    /// Returns the selection behavior.
    pub const fn mode(&self) -> AccordionMode {
        self.mode
    }

    /// Returns whether an open item may close itself in single mode.
    pub const fn collapsible(&self) -> bool {
        self.collapsible
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns the resolved item states.
    pub fn items(&self) -> &[AccordionItemState] {
        &self.items
    }

    /// Returns normalized open item values.
    pub fn open_values(&self) -> &[String] {
        &self.open_values
    }

    /// Returns the accessibility role for the root.
    pub const fn role(&self) -> Role {
        Role::Group
    }

    /// Returns resolved item metrics.
    pub const fn metrics(&self) -> AccordionMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> AccordionColors {
        self.colors
    }

    /// Returns resolved focus ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }

    /// Resolves the change payload for toggling an item.
    pub fn toggle_change(&self, value: &str) -> Option<AccordionOpenChange> {
        self.items
            .iter()
            .find(|item| item.value() == value)
            .map(|item| {
                AccordionOpenChange::resolve(
                    self.mode,
                    self.collapsible,
                    self.open_values.iter().map(String::as_str),
                    item,
                )
            })
    }
}

/// A concrete GPUI accordion component.
#[derive(IntoElement)]
pub struct Accordion {
    id: ElementId,
    mode: AccordionMode,
    collapsible: bool,
    open_values: Vec<String>,
    size: Size,
    tokens: ThemeTokens,
    items: Vec<AccordionItem>,
    on_open_change: Option<Rc<dyn Fn(AccordionOpenChange, &ClickEvent, &mut Window, &mut App)>>,
}

impl Accordion {
    /// Creates a new accordion.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            mode: AccordionMode::Single,
            collapsible: false,
            open_values: Vec::new(),
            size: Size::Medium,
            tokens: ThemeTokens::default(),
            items: Vec::new(),
            on_open_change: None,
        }
    }

    /// Applies accordion selection behavior.
    pub fn mode(mut self, mode: AccordionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Allows an open item to close itself in single mode.
    pub fn collapsible(mut self, collapsible: bool) -> Self {
        self.collapsible = collapsible;
        self
    }

    /// Sets controlled open values.
    pub fn open_values(mut self, values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.open_values = values.into_iter().map(Into::into).collect();
        self
    }

    /// Seeds initial open values for uncontrolled callers.
    pub fn default_open_values(
        mut self,
        values: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.open_values = values.into_iter().map(Into::into).collect();
        self
    }

    /// Appends an item.
    pub fn item(mut self, item: AccordionItem) -> Self {
        self.items.push(item);
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers an open-values change handler.
    pub fn on_open_change(
        mut self,
        handler: impl Fn(AccordionOpenChange, &ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Rc::new(handler));
        self
    }

    /// Returns the resolved accordion state.
    pub fn state(&self) -> AccordionState {
        AccordionState::resolve(
            self.mode,
            self.collapsible,
            self.open_values.iter().map(String::as_str),
            self.items.iter().map(|item| item.descriptor.clone()),
            self.size,
            self.tokens,
        )
    }
}

impl Sizable for Accordion {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Accordion {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let open_values = state.open_values().to_vec();
        let on_open_change = self.on_open_change;
        let id_prefix = self.id.to_string();

        div()
            .id(self.id)
            .debug_selector(move || format!("accordion:{id_prefix}:root"))
            .ui_role(state.role())
            .flex()
            .flex_col()
            .gap_2()
            .children(
                self.items
                    .into_iter()
                    .enumerate()
                    .map(move |(index, item)| {
                        let Some(item_state) = state.items().get(index).cloned() else {
                            return div().into_any_element();
                        };
                        let label = item_state.label().to_owned();
                        let content = item.content;
                        let disabled = item_state.disabled();
                        let open = item_state.open();
                        let value = item_state.value().to_owned();
                        let change = AccordionOpenChange::resolve(
                            state.mode(),
                            state.collapsible(),
                            open_values.iter().map(String::as_str),
                            &item_state,
                        );
                        let item_focus_ring = focus_ring;
                        let item_colors = colors;
                        let item_on_change = on_open_change.clone();

                        div()
                            .id(format!("accordion:{value}"))
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .id(format!("accordion:{value}:trigger"))
                                    .min_h(gpui_px_from_ui(metrics.height()))
                                    .px(gpui_px_from_ui(metrics.padding_x()))
                                    .py(gpui_px_from_ui(metrics.padding_y()))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .rounded(gpui_px_from_ui(metrics.radius()))
                                    .border_1()
                                    .border_color(ThemeResolver::resolve(item_colors.border()))
                                    .bg(ThemeResolver::resolve(if open {
                                        item_colors.hover_background()
                                    } else {
                                        item_colors.background()
                                    }))
                                    .text_color(ThemeResolver::resolve(item_colors.foreground()))
                                    .text_size(gpui_px_from_ui(metrics.text_size()))
                                    .line_height(gpui_px_from_ui(metrics.text_size()))
                                    .focusable()
                                    .tab_stop(!disabled)
                                    .ui_role(item_state.trigger_role())
                                    .aria_label(label.clone())
                                    .aria_expanded(open)
                                    .focus_visible(move |style| {
                                        style.shadow(focus_ring_shadow(item_focus_ring))
                                    })
                                    .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                                    .when(!disabled, |this| {
                                        this.cursor_pointer().hover(move |style| {
                                            style.bg(ThemeResolver::resolve(
                                                item_colors.hover_background(),
                                            ))
                                        })
                                    })
                                    .when_some(item_on_change.filter(|_| !disabled), {
                                        let change = change.clone();
                                        move |this, on_open_change| {
                                            this.on_click(move |event, window, cx| {
                                                cx.stop_propagation();
                                                on_open_change(change.clone(), event, window, cx);
                                            })
                                        }
                                    })
                                    .child(div().flex_1().child(label))
                                    .child(if open { "v" } else { ">" }),
                            )
                            .when(open, |this| {
                                this.child(
                                    div()
                                        .id(format!("accordion:{value}:content"))
                                        .ui_role(item_state.content_role())
                                        .rounded(gpui_px_from_ui(metrics.radius()))
                                        .border_1()
                                        .border_color(ThemeResolver::resolve(item_colors.border()))
                                        .bg(ThemeResolver::resolve(item_colors.background()))
                                        .p_3()
                                        .child(content),
                                )
                            })
                            .into_any_element()
                    }),
            )
    }
}

fn normalize_accordion_open_values(
    mode: AccordionMode,
    values: impl IntoIterator<Item = String>,
) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    for value in values {
        if seen.insert(value.clone()) {
            normalized.push(value);
        }
        if mode == AccordionMode::Single && !normalized.is_empty() {
            break;
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_mode_keeps_one_open_value() {
        let state = AccordionState::resolve(
            AccordionMode::Single,
            false,
            ["billing", "security"],
            [
                AccordionItemDescriptor::new("billing", "Billing"),
                AccordionItemDescriptor::new("security", "Security"),
            ],
            Size::Medium,
            ThemeTokens::default(),
        );

        assert_eq!(state.mode(), AccordionMode::Single);
        assert_eq!(state.role(), Role::Group);
        assert_eq!(state.open_values(), &["billing".to_string()]);
        assert!(state.items()[0].open());
        assert!(!state.items()[1].open());
    }

    #[test]
    fn multiple_mode_preserves_unique_open_values() {
        let state = Accordion::new("settings")
            .mode(AccordionMode::Multiple)
            .default_open_values(["billing", "billing", "security"])
            .item(AccordionItem::new("billing", "Billing", "Invoice settings"))
            .item(AccordionItem::new(
                "security",
                "Security",
                "Session settings",
            ))
            .state();

        assert_eq!(
            state.open_values(),
            &["billing".to_string(), "security".to_string()]
        );
        assert!(state.items()[0].open());
        assert!(state.items()[1].open());
    }

    #[test]
    fn single_mode_change_respects_collapsible_policy() {
        let locked_open = Accordion::new("settings")
            .default_open_values(["billing"])
            .item(AccordionItem::new("billing", "Billing", "Invoice settings"))
            .state();
        let locked_change = locked_open
            .toggle_change("billing")
            .expect("item should produce change");
        assert_eq!(locked_change.open_values(), &["billing".to_string()]);

        let collapsible_open = Accordion::new("settings")
            .collapsible(true)
            .default_open_values(["billing"])
            .item(AccordionItem::new("billing", "Billing", "Invoice settings"))
            .state();
        let collapsible_change = collapsible_open
            .toggle_change("billing")
            .expect("item should produce change");
        assert!(collapsible_change.open_values().is_empty());
    }

    #[test]
    fn disabled_item_keeps_current_open_values() {
        let state = Accordion::new("settings")
            .mode(AccordionMode::Multiple)
            .default_open_values(["billing"])
            .item(AccordionItem::new("billing", "Billing", "Invoice settings"))
            .item(AccordionItem::new("security", "Security", "Session settings").disabled(true))
            .state();
        let change = state
            .toggle_change("security")
            .expect("disabled item should still resolve no-op change");

        assert!(change.item().disabled());
        assert_eq!(change.open_values(), &["billing".to_string()]);
    }
}
