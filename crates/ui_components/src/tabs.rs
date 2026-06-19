//! Tabs component.

use crate::geometry::gpui_px_from_ui;
use std::collections::BTreeMap;
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, ClickEvent, Context, ElementId, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled,
    Window, div,
};
use open_gpui_ui_core::{Orientation, Role, Sizable, Size, ThemeTokens, UiPx, ui_px};

use crate::a11y::UiA11yElementExt;
use crate::color::{ColorIntent, ColorState};
use crate::focus::{FocusRing, focus_ring_shadow};
pub use crate::roving_focus::{
    active_index_from_str_keys, first_enabled, last_enabled, next_enabled,
};
use crate::roving_focus::{roving_navigation_target, selection_index_from_str_keys};
use crate::theme::ThemeResolver;

const DEFAULT_SURFACE: u32 = 0xffffff;
const DEFAULT_GHOST_SURFACE: u32 = 0xf1f5ee;
const DEFAULT_BORDER: u32 = 0xcfd5cc;
const DEFAULT_TEXT: u32 = 0x18202a;
const DEFAULT_TEXT_MUTED: u32 = 0x5a6472;
const DEFAULT_ACCENT: u32 = 0x1f7a66;
const DEFAULT_FOCUS_RING: u32 = 0x2f80ed;

/// Tabs activation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabsActivationMode {
    /// Arrow navigation moves focus and selection together.
    #[default]
    Automatic,
    /// Arrow navigation only moves focus; Enter or Space activates the focused tab.
    Manual,
}

impl TabsActivationMode {
    /// Returns the stable activation mode label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Manual => "manual",
        }
    }
}

/// Pure descriptor for one tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabsItemDescriptor {
    value: String,
    label: String,
    disabled: bool,
}

impl TabsItemDescriptor {
    /// Creates a new descriptor.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
        }
    }

    /// Marks the tab as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Returns the stable tab value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns whether the tab is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }
}

/// Resolved tab colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabsColors {
    shell_background: ColorIntent,
    shell_border: ColorIntent,
    tab_background: ColorIntent,
    tab_background_selected: ColorIntent,
    tab_hover_background: ColorIntent,
    tab_text: ColorIntent,
    tab_text_muted: ColorIntent,
    tab_border: ColorIntent,
    tab_border_selected: ColorIntent,
    panel_background: ColorIntent,
    focus_ring: ColorIntent,
}

impl TabsColors {
    /// Resolves colors from the shared token bundle.
    pub const fn from_tokens(tokens: ThemeTokens) -> Self {
        Self {
            shell_background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            shell_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            tab_background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            tab_background_selected: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Selected,
                DEFAULT_GHOST_SURFACE,
            ),
            tab_hover_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Hover,
                DEFAULT_GHOST_SURFACE,
            ),
            tab_text: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            tab_text_muted: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            tab_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            tab_border_selected: ColorIntent::with_state(
                tokens.accent,
                ColorState::Selected,
                DEFAULT_ACCENT,
            ),
            panel_background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    /// Returns the shell background color intent.
    pub const fn shell_background(self) -> ColorIntent {
        self.shell_background
    }

    /// Returns the shell border color intent.
    pub const fn shell_border(self) -> ColorIntent {
        self.shell_border
    }

    /// Returns the unselected tab background color intent.
    pub const fn tab_background(self) -> ColorIntent {
        self.tab_background
    }

    /// Returns the selected tab background color intent.
    pub const fn tab_background_selected(self) -> ColorIntent {
        self.tab_background_selected
    }

    /// Returns the hover background color intent.
    pub const fn tab_hover_background(self) -> ColorIntent {
        self.tab_hover_background
    }

    /// Returns the default tab text color intent.
    pub const fn tab_text(self) -> ColorIntent {
        self.tab_text
    }

    /// Returns the muted tab text color intent.
    pub const fn tab_text_muted(self) -> ColorIntent {
        self.tab_text_muted
    }

    /// Returns the default tab border color intent.
    pub const fn tab_border(self) -> ColorIntent {
        self.tab_border
    }

    /// Returns the selected tab border color intent.
    pub const fn tab_border_selected(self) -> ColorIntent {
        self.tab_border_selected
    }

    /// Returns the panel background color intent.
    pub const fn panel_background(self) -> ColorIntent {
        self.panel_background
    }

    /// Returns the focus ring color intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved tab metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TabsMetrics {
    tab_min_height: UiPx,
    tab_padding_x: UiPx,
    tab_padding_y: UiPx,
    tab_gap: UiPx,
    panel_padding: UiPx,
    radius: UiPx,
    text_size: UiPx,
}

impl TabsMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            tab_min_height: size.button_h(),
            tab_padding_x: size.button_px(),
            tab_padding_y: size.button_py(),
            tab_gap: ui_px(4.0),
            panel_padding: ui_px(12.0),
            radius: size.control_radius(),
            text_size: size.control_text_px(),
        }
    }

    /// Returns the minimum tab height.
    pub const fn tab_min_height(self) -> UiPx {
        self.tab_min_height
    }

    /// Returns the horizontal tab padding.
    pub const fn tab_padding_x(self) -> UiPx {
        self.tab_padding_x
    }

    /// Returns the vertical tab padding.
    pub const fn tab_padding_y(self) -> UiPx {
        self.tab_padding_y
    }

    /// Returns the gap between tabs.
    pub const fn tab_gap(self) -> UiPx {
        self.tab_gap
    }

    /// Returns the panel padding.
    pub const fn panel_padding(self) -> UiPx {
        self.panel_padding
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

/// Resolved tab item state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabsItemState {
    index: usize,
    value: String,
    label: String,
    disabled: bool,
    selected: bool,
    focused: bool,
}

impl TabsItemState {
    /// Returns the zero-based index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the stable tab value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns whether the tab is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the tab is selected.
    pub const fn selected(&self) -> bool {
        self.selected
    }

    /// Returns whether the tab currently has roving focus.
    pub const fn focused(&self) -> bool {
        self.focused
    }
}

/// Resolved selection change payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabsSelection {
    index: usize,
    value: String,
    label: String,
}

impl TabsSelection {
    /// Creates a selection payload from a descriptor.
    fn from_descriptor(index: usize, descriptor: &TabsItemDescriptor) -> Self {
        Self {
            index,
            value: descriptor.value.clone(),
            label: descriptor.label.clone(),
        }
    }

    /// Returns the zero-based index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the selected tab value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the selected tab label.
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Resolved tabs state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct TabsState {
    orientation: Orientation,
    activation_mode: TabsActivationMode,
    size: Size,
    metrics: TabsMetrics,
    colors: TabsColors,
    items: Vec<TabsItemState>,
    selected_index: Option<usize>,
    focused_index: Option<usize>,
}

impl TabsState {
    /// Resolves the public state for tabs.
    pub fn resolve(
        orientation: Orientation,
        activation_mode: TabsActivationMode,
        size: Size,
        selected_value: Option<&str>,
        focused_value: Option<&str>,
        items: impl IntoIterator<Item = TabsItemDescriptor>,
        tokens: ThemeTokens,
    ) -> Self {
        let descriptors: Vec<TabsItemDescriptor> = items.into_iter().collect();
        let values: Vec<String> = descriptors.iter().map(|item| item.value.clone()).collect();
        let disabled: Vec<bool> = descriptors.iter().map(|item| item.disabled).collect();
        let selected_index =
            selection_index_from_str_keys(&values, &disabled, selected_value, focused_value);
        let selected_seed = selected_index
            .and_then(|index| values.get(index))
            .map(String::as_str);
        let focused_index =
            selection_index_from_str_keys(&values, &disabled, focused_value, selected_seed);
        let metrics = TabsMetrics::from_size(size);
        let colors = TabsColors::from_tokens(tokens);

        let items = descriptors
            .into_iter()
            .enumerate()
            .map(|(index, descriptor)| {
                let selected = Some(index) == selected_index;
                let focused = Some(index) == focused_index;

                TabsItemState {
                    index,
                    value: descriptor.value,
                    label: descriptor.label,
                    disabled: descriptor.disabled,
                    selected,
                    focused,
                }
            })
            .collect();

        Self {
            orientation,
            activation_mode,
            size,
            metrics,
            colors,
            items,
            selected_index,
            focused_index,
        }
    }

    /// Returns the tab orientation.
    pub const fn orientation(&self) -> Orientation {
        self.orientation
    }

    /// Returns the activation mode.
    pub const fn activation_mode(&self) -> TabsActivationMode {
        self.activation_mode
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns the resolved metrics.
    pub const fn metrics(&self) -> TabsMetrics {
        self.metrics
    }

    /// Returns the resolved color intents.
    pub const fn colors(&self) -> TabsColors {
        self.colors
    }

    /// Returns all resolved tab items.
    pub fn items(&self) -> &[TabsItemState] {
        &self.items
    }

    /// Returns the selected tab index.
    pub const fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Returns the selected tab value.
    pub fn selected_value(&self) -> Option<&str> {
        self.selected_index
            .and_then(|index| self.items.get(index))
            .map(|item| item.value())
    }

    /// Returns the selected tab item.
    pub fn selected_item(&self) -> Option<&TabsItemState> {
        self.selected_index.and_then(|index| self.items.get(index))
    }

    /// Returns the focused tab index.
    pub const fn focused_index(&self) -> Option<usize> {
        self.focused_index
    }

    /// Returns the focused tab value.
    pub fn focused_value(&self) -> Option<&str> {
        self.focused_index
            .and_then(|index| self.items.get(index))
            .map(|item| item.value())
    }

    /// Returns the focused tab item.
    pub fn focused_item(&self) -> Option<&TabsItemState> {
        self.focused_index.and_then(|index| self.items.get(index))
    }

    /// Returns the current tab stop index.
    pub const fn tab_stop_index(&self) -> Option<usize> {
        if self.focused_index.is_some() {
            self.focused_index
        } else {
            self.selected_index
        }
    }

    /// Returns the current tab stop value.
    pub fn tab_stop_value(&self) -> Option<&str> {
        self.tab_stop_index()
            .and_then(|index| self.items.get(index))
            .map(|item| item.value())
    }

    /// Returns the current tab stop item.
    pub fn tab_stop_item(&self) -> Option<&TabsItemState> {
        self.tab_stop_index()
            .and_then(|index| self.items.get(index))
    }

    /// Returns the item at the given index.
    pub fn item(&self, index: usize) -> Option<&TabsItemState> {
        self.items.get(index)
    }

    /// Returns whether the state has no tabs.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// A concrete GPUI tab item.
pub struct TabsItem {
    value: String,
    label: SharedString,
    disabled: bool,
    panel: AnyElement,
}

impl TabsItem {
    /// Creates a new tab item with panel content.
    pub fn new(
        value: impl Into<String>,
        label: impl Into<SharedString>,
        panel: impl IntoElement,
    ) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
            panel: panel.into_any_element(),
        }
    }

    /// Marks the tab item as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    fn descriptor(&self) -> TabsItemDescriptor {
        TabsItemDescriptor {
            value: self.value.clone(),
            label: self.label.to_string(),
            disabled: self.disabled,
        }
    }
}

/// A concrete GPUI tabs component.
#[derive(IntoElement)]
pub struct Tabs {
    id: ElementId,
    orientation: Orientation,
    activation_mode: TabsActivationMode,
    selected_value: Option<String>,
    size: Size,
    tokens: ThemeTokens,
    items: Vec<TabsItem>,
    on_selection_change: Option<Rc<dyn Fn(TabsSelection, &mut Window, &mut App)>>,
}

impl Tabs {
    /// Creates a new tabs component.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            orientation: Orientation::Horizontal,
            activation_mode: TabsActivationMode::Automatic,
            selected_value: None,
            size: Size::Medium,
            tokens: ThemeTokens::default(),
            items: Vec::new(),
            on_selection_change: None,
        }
    }

    /// Sets the tab orientation.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Sets the activation mode.
    pub fn activation_mode(mut self, activation_mode: TabsActivationMode) -> Self {
        self.activation_mode = activation_mode;
        self
    }

    /// Seeds the selected tab value.
    pub fn selected(mut self, value: impl Into<String>) -> Self {
        self.selected_value = Some(value.into());
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Adds a tab item.
    pub fn item(mut self, item: TabsItem) -> Self {
        self.items.push(item);
        self
    }

    /// Registers a selection change handler.
    pub fn on_selection_change(
        mut self,
        handler: impl Fn(TabsSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(Rc::new(handler));
        self
    }

    /// Returns the resolved state.
    pub fn state(&self) -> TabsState {
        TabsState::resolve(
            self.orientation,
            self.activation_mode,
            self.size,
            self.selected_value.as_deref(),
            None,
            self.items.iter().map(TabsItem::descriptor),
            self.tokens,
        )
    }
}

impl Sizable for Tabs {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Tabs {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Tabs {
            id,
            orientation,
            activation_mode,
            selected_value,
            size,
            tokens,
            items,
            on_selection_change,
        } = self;
        let tabs_id = id.to_string();
        let panel_id = tabs_panel_id();

        window.with_id(id.clone(), |window| {
            let descriptors: Vec<TabsItemDescriptor> =
                items.iter().map(TabsItem::descriptor).collect();
            let selected_seed = selected_value.clone();
            let runtime = window.use_keyed_state("runtime", cx, |_, _| TabsRuntime {
                selected_value: selected_seed.clone(),
                focused_value: selected_seed,
                focus_handles: BTreeMap::new(),
            });
            let runtime_snapshot = {
                let runtime = runtime.read(cx);
                (
                    runtime.selected_value.clone(),
                    runtime.focused_value.clone(),
                )
            };
            let state = TabsState::resolve(
                orientation,
                activation_mode,
                size,
                runtime_snapshot.0.as_deref(),
                runtime_snapshot.1.as_deref(),
                descriptors.clone(),
                tokens,
            );
            runtime.update(cx, |runtime, cx| runtime.sync(&state, &descriptors, cx));

            let panel_node_id = window.with_global_id(panel_id.clone(), |global_id, _| {
                global_id.accesskit_node_id()
            });
            let tab_node_ids: Vec<_> = state
                .items()
                .iter()
                .map(|item| {
                    window.with_global_id(tabs_trigger_id(item.value()), |global_id, _| {
                        global_id.accesskit_node_id()
                    })
                })
                .collect();

            let selected_panel = if let Some(selected_index) = state.selected_index() {
                items
                    .into_iter()
                    .enumerate()
                    .find_map(|(index, item)| (index == selected_index).then_some(item.panel))
                    .unwrap_or_else(|| div().into_any_element())
            } else {
                div().into_any_element()
            };

            let item_descriptors = Rc::new(descriptors);
            let disabled = Rc::new(
                state
                    .items()
                    .iter()
                    .map(TabsItemState::disabled)
                    .collect::<Vec<_>>(),
            );
            let selected_value = state.selected_value().map(str::to_owned);
            let selected_index = state.selected_index();
            let selected_tab_node_id = selected_index.map(|index| tab_node_ids[index]);
            let colors = state.colors();
            let metrics = state.metrics();
            let focus_ring = FocusRing::from_color(colors.focus_ring());
            let is_vertical = matches!(orientation, Orientation::Vertical);
            let focus_handles = {
                let runtime = runtime.read(cx);
                state
                    .items()
                    .iter()
                    .map(|item| runtime.focus_handles.get(item.value()).cloned())
                    .collect::<Vec<_>>()
            };
            let tab_stop_index = state.tab_stop_index();

            div()
                .id(id.clone())
                .w_full()
                .flex()
                .rounded(gpui_px_from_ui(metrics.radius()))
                .border_1()
                .border_color(ThemeResolver::resolve(colors.shell_border()))
                .bg(ThemeResolver::resolve(colors.shell_background()))
                .overflow_hidden()
                .when(is_vertical, |this| this.flex_row().h_full())
                .when(!is_vertical, |this| this.flex_col())
                .child(
                    div()
                        .id("tablist")
                        .debug_selector({
                            let tabs_id = tabs_id.clone();
                            move || format!("tabs:{tabs_id}:tablist")
                        })
                        .ui_role(Role::TabList)
                        .ui_aria_orientation(orientation)
                        .flex()
                        .flex_none()
                        .gap(gpui_px_from_ui(metrics.tab_gap()))
                        .p_1()
                        .border_color(ThemeResolver::resolve(colors.shell_border()))
                        .when(is_vertical, |this| {
                            this.flex_col().border_r_1().h_full().overflow_y_scroll()
                        })
                        .when(!is_vertical, |this| {
                            this.flex_row().flex_wrap().border_b_1()
                        })
                        .children(state.items().iter().enumerate().map(|(index, item)| {
                            let descriptor = item_descriptors[index].clone();
                            let disabled = disabled.clone();
                            let click_runtime = runtime.clone();
                            let click_on_selection_change = on_selection_change.clone();
                            let click_selected_value = selected_value.clone();
                            let key_runtime = runtime.clone();
                            let key_on_selection_change = on_selection_change.clone();
                            let key_selected_value = selected_value.clone();
                            let key_item_descriptors = item_descriptors.clone();
                            let item_index = index;
                            let is_selected = item.selected();
                            let is_tab_stop = Some(index) == tab_stop_index;
                            let focus_handle = focus_handles[index].clone();

                            div()
                                .id(tabs_trigger_id(item.value()))
                                .debug_selector({
                                    let tabs_id = tabs_id.clone();
                                    let value = descriptor.value().to_owned();
                                    move || format!("tabs:{tabs_id}:trigger:{value}")
                                })
                                .focusable()
                                .tab_stop(is_tab_stop)
                                .when_some(focus_handle, |this, focus_handle| {
                                    this.track_focus(&focus_handle)
                                })
                                .ui_role(Role::Tab)
                                .aria_label(descriptor.label())
                                .aria_selected(is_selected)
                                .aria_controls(std::iter::once(panel_node_id))
                                .aria_position_in_set(item_index + 1)
                                .aria_size_of_set(state.items().len())
                                .flex_none()
                                .min_h(gpui_px_from_ui(metrics.tab_min_height()))
                                .px(gpui_px_from_ui(metrics.tab_padding_x()))
                                .py(gpui_px_from_ui(metrics.tab_padding_y()))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(gpui_px_from_ui(metrics.radius()))
                                .border_1()
                                .border_color(ThemeResolver::resolve(if is_selected {
                                    colors.tab_border_selected()
                                } else {
                                    colors.tab_border()
                                }))
                                .bg(ThemeResolver::resolve(if is_selected {
                                    colors.tab_background_selected()
                                } else {
                                    colors.tab_background()
                                }))
                                .text_size(gpui_px_from_ui(metrics.text_size()))
                                .line_height(gpui_px_from_ui(metrics.text_size()))
                                .text_color(ThemeResolver::resolve(if is_selected {
                                    colors.tab_text()
                                } else {
                                    colors.tab_text_muted()
                                }))
                                .font_weight(if is_selected {
                                    open_gpui::FontWeight::BOLD
                                } else {
                                    open_gpui::FontWeight::NORMAL
                                })
                                .focus_visible(move |style| {
                                    style.shadow(focus_ring_shadow(focus_ring))
                                })
                                .when(!item.disabled(), |this| {
                                    this.cursor_pointer().hover(move |style| {
                                        style.bg(ThemeResolver::resolve(
                                            colors.tab_hover_background(),
                                        ))
                                    })
                                })
                                .when(item.disabled(), |this| {
                                    this.opacity(0.56).cursor_not_allowed()
                                })
                                .on_click({
                                    let descriptor = descriptor.clone();
                                    move |_event: &ClickEvent, window, cx| {
                                        if descriptor.disabled_state() {
                                            return;
                                        }

                                        cx.stop_propagation();
                                        let changed = click_selected_value.as_deref()
                                            != Some(descriptor.value());
                                        let focus_handle =
                                            click_runtime.update(cx, |runtime, cx| {
                                                runtime.set_active(descriptor.value(), cx)
                                            });

                                        if changed
                                            && let Some(handler) = click_on_selection_change.clone()
                                        {
                                            handler(
                                                TabsSelection::from_descriptor(
                                                    item_index,
                                                    &descriptor,
                                                ),
                                                window,
                                                cx,
                                            );
                                        }

                                        if let Some(focus_handle) = focus_handle {
                                            focus_handle.focus(window, cx);
                                        }
                                    }
                                })
                                .on_key_down({
                                    let descriptor = descriptor.clone();
                                    let disabled = disabled.clone();
                                    move |event: &KeyDownEvent, window, cx| {
                                        if descriptor.disabled_state() {
                                            return;
                                        }
                                        if event.keystroke.modifiers.modified() {
                                            return;
                                        }

                                        let key = event.keystroke.key.as_str();
                                        let Some(target_index) = roving_navigation_target(
                                            orientation,
                                            key,
                                            item_index,
                                            &disabled,
                                        ) else {
                                            if !matches!(key, "space" | "enter") {
                                                return;
                                            }

                                            let changed = key_selected_value.as_deref()
                                                != Some(descriptor.value());
                                            let focus_handle =
                                                key_runtime.update(cx, |runtime, cx| {
                                                    runtime.set_active(descriptor.value(), cx)
                                                });

                                            if changed
                                                && let Some(handler) =
                                                    key_on_selection_change.clone()
                                            {
                                                handler(
                                                    TabsSelection::from_descriptor(
                                                        item_index,
                                                        &descriptor,
                                                    ),
                                                    window,
                                                    cx,
                                                );
                                            }

                                            if let Some(focus_handle) = focus_handle {
                                                focus_handle.focus(window, cx);
                                            }
                                            cx.stop_propagation();
                                            return;
                                        };

                                        let target = &key_item_descriptors[target_index];
                                        let target_value = target.value().to_owned();
                                        let target_selection =
                                            TabsSelection::from_descriptor(target_index, target);
                                        let activate =
                                            activation_mode == TabsActivationMode::Automatic;
                                        let changed = if activate {
                                            key_selected_value.as_deref() != Some(target.value())
                                        } else {
                                            false
                                        };
                                        let focus_handle = key_runtime.update(cx, |runtime, cx| {
                                            if activate {
                                                runtime.set_active(&target_value, cx)
                                            } else {
                                                runtime.set_focused_only(&target_value, cx)
                                            }
                                        });

                                        if changed
                                            && let Some(handler) = key_on_selection_change.clone()
                                        {
                                            handler(target_selection, window, cx);
                                        }

                                        if let Some(focus_handle) = focus_handle {
                                            focus_handle.focus(window, cx);
                                        }

                                        cx.stop_propagation();
                                    }
                                })
                                .child(descriptor.label().to_string())
                        })),
                )
                .child(
                    div()
                        .id(panel_id)
                        .ui_role(Role::TabPanel)
                        .flex()
                        .flex_1()
                        .min_w(open_gpui::px(0.0))
                        .border_color(ThemeResolver::resolve(colors.shell_border()))
                        .bg(ThemeResolver::resolve(colors.panel_background()))
                        .px(gpui_px_from_ui(metrics.panel_padding()))
                        .py(gpui_px_from_ui(metrics.panel_padding()))
                        .when(is_vertical, |this| this.min_w(open_gpui::px(0.0)))
                        .when(!is_vertical, |this| this.border_t_1())
                        .when_some(selected_tab_node_id, |this, tab_node_id| {
                            this.aria_labelled_by(std::iter::once(tab_node_id))
                        })
                        .child(selected_panel),
                )
        })
    }
}

#[derive(Debug, Default)]
struct TabsRuntime {
    selected_value: Option<String>,
    focused_value: Option<String>,
    focus_handles: BTreeMap<String, FocusHandle>,
}

impl TabsRuntime {
    fn sync(&mut self, state: &TabsState, items: &[TabsItemDescriptor], cx: &mut Context<Self>) {
        self.focus_handles
            .retain(|value, _| items.iter().any(|item| item.value() == value));

        for item in items {
            self.focus_handles
                .entry(item.value().to_owned())
                .or_insert_with(|| cx.focus_handle());
        }

        self.selected_value = state.selected_value().map(str::to_owned);
        self.focused_value = state.focused_value().map(str::to_owned);
    }

    fn set_active(&mut self, value: &str, cx: &mut Context<Self>) -> Option<FocusHandle> {
        let value = value.to_owned();
        let changed = self.selected_value.as_deref() != Some(value.as_str())
            || self.focused_value.as_deref() != Some(value.as_str());
        self.selected_value = Some(value.clone());
        self.focused_value = Some(value.clone());
        if changed {
            cx.notify();
        }
        self.focus_handles.get(&value).cloned()
    }

    fn set_focused_only(&mut self, value: &str, cx: &mut Context<Self>) -> Option<FocusHandle> {
        let value = value.to_owned();
        let changed = self.focused_value.as_deref() != Some(value.as_str());
        self.focused_value = Some(value.clone());
        if changed {
            cx.notify();
        }
        self.focus_handles.get(&value).cloned()
    }
}

fn tabs_panel_id() -> ElementId {
    "panel".into()
}

fn tabs_trigger_id(value: &str) -> ElementId {
    format!("tab-{value}").into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roving_focus_helpers_skip_disabled_items_and_wrap() {
        let disabled = [false, true, false];

        assert_eq!(first_enabled(&disabled), Some(0));
        assert_eq!(last_enabled(&disabled), Some(2));
        assert_eq!(next_enabled(&disabled, 0, true, true), Some(2));
        assert_eq!(next_enabled(&disabled, 2, true, true), Some(0));
        assert_eq!(next_enabled(&disabled, 2, false, true), Some(0));
    }

    #[test]
    fn tabs_state_prefers_selected_value_and_tracks_focus() {
        let state = TabsState::resolve(
            Orientation::Horizontal,
            TabsActivationMode::Manual,
            Size::Medium,
            Some("details"),
            Some("history"),
            [
                TabsItemDescriptor::new("overview", "Overview"),
                TabsItemDescriptor::new("details", "Details"),
                TabsItemDescriptor::new("history", "History").disabled(true),
            ],
            ThemeTokens::default(),
        );

        assert_eq!(state.orientation(), Orientation::Horizontal);
        assert_eq!(state.activation_mode(), TabsActivationMode::Manual);
        assert_eq!(state.selected_value(), Some("details"));
        assert_eq!(state.focused_value(), Some("details"));
        assert_eq!(state.tab_stop_value(), Some("details"));
        assert!(state.items()[1].selected());
        assert!(state.items()[1].focused());
        assert_eq!(state.tab_stop_index(), Some(1));
    }
}
