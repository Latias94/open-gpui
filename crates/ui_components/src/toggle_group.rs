//! Toggle group component.

use crate::a11y::UiA11yElementExt;
use crate::button::{ButtonColors, ButtonMetrics, ButtonVariant};
use crate::choice::{
    self, ChoiceCollection, ChoiceInteractionPolicy, ChoiceItemProjection, ChoiceSelectionMode,
};
use crate::focus::{FocusRing, focus_ring_shadow};
use crate::geometry::gpui_px_from_ui;
use crate::theme::ThemeResolver;
use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, Context, ElementId, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled,
    Window, div,
};
use open_gpui_ui_core::{Orientation, Role, Sizable, Size, ThemeTokens, Toggled, UiPx, ui_px};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

/// Selection mode for a toggle group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToggleGroupSelectionMode {
    /// At most one item can be selected.
    #[default]
    Single,
    /// Multiple items can be selected.
    Multiple,
}

impl ToggleGroupSelectionMode {
    /// Returns the stable mode label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Multiple => "multiple",
        }
    }
}

/// Pure descriptor for one toggle group item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToggleGroupItemDescriptor {
    value: String,
    label: String,
    disabled: bool,
}

impl ToggleGroupItemDescriptor {
    /// Creates a toggle group item descriptor.
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

    /// Returns the visible or accessible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns whether the item is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }
}

/// Resolved toggle group colors.
pub type ToggleGroupColors = ButtonColors;

/// Resolved toggle group metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToggleGroupMetrics {
    item: ButtonMetrics,
    gap: UiPx,
    padding: UiPx,
    radius: UiPx,
}

impl ToggleGroupMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            item: ButtonMetrics::from_size(size),
            gap: ui_px(4.0),
            padding: ui_px(4.0),
            radius: size.control_radius(),
        }
    }

    /// Returns item metrics.
    pub const fn item(self) -> ButtonMetrics {
        self.item
    }

    /// Returns group gap.
    pub const fn gap(self) -> UiPx {
        self.gap
    }

    /// Returns group padding.
    pub const fn padding(self) -> UiPx {
        self.padding
    }

    /// Returns group radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }
}

/// Resolved toggle group item state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToggleGroupItemState {
    index: usize,
    value: String,
    label: String,
    selected: bool,
    disabled: bool,
    focused: bool,
}

impl ToggleGroupItemState {
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

    /// Returns whether the item is selected.
    pub const fn selected(&self) -> bool {
        self.selected
    }

    /// Returns whether the item is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the item can receive focus.
    pub const fn focusable(&self) -> bool {
        !self.disabled
    }

    /// Returns whether the item has roving focus.
    pub const fn focused(&self) -> bool {
        self.focused
    }

    /// Returns the accessibility role.
    pub const fn role(&self) -> Role {
        Role::Button
    }

    /// Returns the accessibility toggled state.
    pub const fn toggled(&self) -> Toggled {
        if self.selected {
            Toggled::True
        } else {
            Toggled::False
        }
    }
}

/// Toggle group selection change payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToggleGroupSelectionChange {
    item: ToggleGroupItemState,
    selected_values: Vec<String>,
}

impl ToggleGroupSelectionChange {
    /// Creates a selection change payload.
    pub fn new(item: ToggleGroupItemState, selected_values: Vec<String>) -> Self {
        Self {
            item,
            selected_values,
        }
    }

    /// Returns the activated item state.
    pub const fn item(&self) -> &ToggleGroupItemState {
        &self.item
    }

    /// Returns the next selected stable values.
    pub fn selected_values(&self) -> &[String] {
        &self.selected_values
    }
}

/// Resolved toggle group state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct ToggleGroupState {
    orientation: Orientation,
    mode: ToggleGroupSelectionMode,
    selection_required: bool,
    disabled: bool,
    label: String,
    size: Size,
    items: Vec<ToggleGroupItemState>,
    selected_values: Vec<String>,
    focused_index: Option<usize>,
    metrics: ToggleGroupMetrics,
    colors: ToggleGroupColors,
    selected_colors: ToggleGroupColors,
    focus_ring: FocusRing,
}

impl ToggleGroupState {
    /// Resolves public state for a toggle group.
    pub fn resolve(
        orientation: Orientation,
        mode: ToggleGroupSelectionMode,
        selection_required: bool,
        disabled: bool,
        label: impl Into<String>,
        selected_values: impl IntoIterator<Item = impl Into<String>>,
        focused_value: Option<&str>,
        items: impl IntoIterator<Item = ToggleGroupItemDescriptor>,
        size: Size,
        tokens: ThemeTokens,
    ) -> Self {
        let descriptors: Vec<ToggleGroupItemDescriptor> = items.into_iter().collect();
        let choice_items = toggle_group_choice_items(disabled, &descriptors);
        let selected_values = choice::resolve_projected_selected_values(
            toggle_group_choice_selection_mode(mode),
            &choice_items,
            None,
            selected_values,
        );
        let first_selected = selected_values.first().map(String::as_str);
        let collection = ChoiceCollection::resolve(
            disabled,
            choice_items,
            None,
            focused_value.or(first_selected),
            toggle_group_choice_policy(orientation, mode, selection_required),
        );
        let focused_index = collection.active_index();
        let colors = ThemeResolver::button_colors(tokens, ButtonVariant::Outline, false);
        let selected_colors = ThemeResolver::button_colors(tokens, ButtonVariant::Outline, true);
        let selected_set: BTreeSet<&str> = selected_values.iter().map(String::as_str).collect();

        let items = descriptors
            .into_iter()
            .enumerate()
            .map(|(index, descriptor)| ToggleGroupItemState {
                index,
                selected: selected_set.contains(descriptor.value.as_str()),
                disabled: disabled || descriptor.disabled,
                focused: Some(index) == focused_index,
                value: descriptor.value,
                label: descriptor.label,
            })
            .collect();

        Self {
            orientation,
            mode,
            selection_required,
            disabled,
            label: label.into(),
            size,
            items,
            selected_values,
            focused_index,
            metrics: ToggleGroupMetrics::from_size(size),
            colors,
            selected_colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
        }
    }

    /// Returns the group orientation.
    pub const fn orientation(&self) -> Orientation {
        self.orientation
    }

    /// Returns the selection mode.
    pub const fn mode(&self) -> ToggleGroupSelectionMode {
        self.mode
    }

    /// Returns whether the last selected item cannot be deselected.
    pub const fn selection_required(&self) -> bool {
        self.selection_required
    }

    /// Returns whether the group is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns the accessible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns the accessibility role.
    pub const fn role(&self) -> Role {
        Role::Group
    }

    /// Returns resolved items.
    pub fn items(&self) -> &[ToggleGroupItemState] {
        &self.items
    }

    /// Returns selected stable values.
    pub fn selected_values(&self) -> &[String] {
        &self.selected_values
    }

    /// Returns the focused item index.
    pub const fn focused_index(&self) -> Option<usize> {
        self.focused_index
    }

    /// Returns the focused item value.
    pub fn focused_value(&self) -> Option<&str> {
        self.focused_index
            .and_then(|index| self.items.get(index))
            .map(ToggleGroupItemState::value)
    }

    /// Returns the current tab-stop index.
    pub const fn tab_stop_index(&self) -> Option<usize> {
        self.focused_index
    }

    /// Resolves a focus target for an APG-style navigation key.
    pub fn navigation_target(&self, key: &str) -> Option<&ToggleGroupItemState> {
        let current = self.focused_index?;
        let disabled = self.disabled_map();
        toggle_group_navigation_target(self.orientation, key, current, &disabled)
            .and_then(|index| self.items.get(index))
    }

    /// Resolves a selection change for an activated item.
    pub fn selection_change_for_item(&self, value: &str) -> Option<ToggleGroupSelectionChange> {
        let item = self
            .items
            .iter()
            .find(|item| item.value() == value && item.focusable())?
            .clone();
        let next_selected = choice::next_selected_values(
            toggle_group_choice_selection_mode(self.mode),
            self.selection_required,
            &self.selected_values,
            item.value(),
        );

        Some(ToggleGroupSelectionChange::new(item, next_selected))
    }

    /// Resolves a selection change for the currently focused item.
    pub fn selection_change_for_key(&self, key: &str) -> Option<ToggleGroupSelectionChange> {
        if !matches!(key, "enter" | "space") {
            return None;
        }

        let item = self.focused_index.and_then(|index| self.items.get(index))?;
        self.selection_change_for_item(item.value())
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> ToggleGroupMetrics {
        self.metrics
    }

    /// Returns unselected item colors.
    pub const fn colors(&self) -> ToggleGroupColors {
        self.colors
    }

    /// Returns selected item colors.
    pub const fn selected_colors(&self) -> ToggleGroupColors {
        self.selected_colors
    }

    /// Returns focus ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }

    fn disabled_map(&self) -> Vec<bool> {
        self.items.iter().map(|item| !item.focusable()).collect()
    }
}

/// Resolves a toggle group roving-focus target from an APG-style key name.
pub fn toggle_group_navigation_target(
    orientation: Orientation,
    key: &str,
    current: usize,
    disabled: &[bool],
) -> Option<usize> {
    ChoiceInteractionPolicy::roving(orientation).navigation_target_index(key, current, disabled)
}

fn toggle_group_choice_selection_mode(mode: ToggleGroupSelectionMode) -> ChoiceSelectionMode {
    match mode {
        ToggleGroupSelectionMode::Single => ChoiceSelectionMode::Single,
        ToggleGroupSelectionMode::Multiple => ChoiceSelectionMode::Multiple,
    }
}

fn toggle_group_choice_policy(
    orientation: Orientation,
    mode: ToggleGroupSelectionMode,
    selection_required: bool,
) -> ChoiceInteractionPolicy {
    match mode {
        ToggleGroupSelectionMode::Single => ChoiceInteractionPolicy::single_optional(orientation)
            .with_selection_required(selection_required),
        ToggleGroupSelectionMode::Multiple => ChoiceInteractionPolicy::multiple(orientation)
            .with_selection_required(selection_required),
    }
}

fn toggle_group_choice_items(
    disabled: bool,
    items: &[ToggleGroupItemDescriptor],
) -> Vec<ChoiceItemProjection<()>> {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let label = item.label().to_owned();
            ChoiceItemProjection::new(
                index,
                None,
                item.value(),
                label.clone(),
                disabled || item.disabled_state(),
                (),
            )
            .text_value(label)
        })
        .collect()
}

/// A concrete GPUI toggle group item.
#[derive(Clone)]
pub struct ToggleGroupItem {
    descriptor: ToggleGroupItemDescriptor,
}

impl ToggleGroupItem {
    /// Creates a toggle group item.
    pub fn new(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: ToggleGroupItemDescriptor::new(value, label.to_string()),
        }
    }

    /// Marks the item as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.descriptor = self.descriptor.disabled(disabled);
        self
    }

    /// Returns a pure descriptor for this item.
    pub fn descriptor(&self) -> ToggleGroupItemDescriptor {
        self.descriptor.clone()
    }
}

/// A concrete GPUI toggle group component.
#[derive(IntoElement)]
pub struct ToggleGroup {
    id: ElementId,
    label: SharedString,
    orientation: Orientation,
    mode: ToggleGroupSelectionMode,
    selected_values: Option<Vec<String>>,
    default_selected_values: Vec<String>,
    focused_value: Option<String>,
    selection_required: bool,
    disabled: bool,
    size: Size,
    tokens: ThemeTokens,
    items: Vec<ToggleGroupItem>,
    on_change: Option<Rc<dyn Fn(ToggleGroupSelectionChange, &mut Window, &mut App)>>,
}

impl ToggleGroup {
    /// Creates a new toggle group with an accessible label.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            orientation: Orientation::Horizontal,
            mode: ToggleGroupSelectionMode::Single,
            selected_values: None,
            default_selected_values: Vec::new(),
            focused_value: None,
            selection_required: false,
            disabled: false,
            size: Size::Medium,
            tokens: ThemeTokens::default(),
            items: Vec::new(),
            on_change: None,
        }
    }

    /// Sets the group orientation.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Sets the selection mode.
    pub fn mode(mut self, mode: ToggleGroupSelectionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Sets the selected values.
    pub fn selected_values(
        mut self,
        selected_values: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.selected_values = Some(selected_values.into_iter().map(Into::into).collect());
        self
    }

    /// Seeds the adapter-owned selected values.
    pub fn default_selected_values(
        mut self,
        selected_values: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.default_selected_values = selected_values.into_iter().map(Into::into).collect();
        self
    }

    /// Seeds the adapter-owned focused value.
    pub fn default_focused(mut self, value: impl Into<String>) -> Self {
        self.focused_value = Some(value.into());
        self
    }

    /// Requires at least one selected item when possible.
    pub fn selection_required(mut self, selection_required: bool) -> Self {
        self.selection_required = selection_required;
        self
    }

    /// Marks the whole group as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Adds one toggle group item.
    pub fn item(mut self, item: ToggleGroupItem) -> Self {
        self.items.push(item);
        self
    }

    /// Adds many toggle group items.
    pub fn items(mut self, items: impl IntoIterator<Item = ToggleGroupItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// Registers a selection change handler.
    pub fn on_change(
        mut self,
        handler: impl Fn(ToggleGroupSelectionChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Returns the resolved toggle group state.
    pub fn state(&self) -> ToggleGroupState {
        ToggleGroupState::resolve(
            self.orientation,
            self.mode,
            self.selection_required,
            self.disabled,
            self.label.to_string(),
            self.selected_values
                .clone()
                .unwrap_or_else(|| self.default_selected_values.clone()),
            self.focused_value.as_deref(),
            self.items.iter().map(ToggleGroupItem::descriptor),
            self.size,
            self.tokens,
        )
    }
}

impl Sizable for ToggleGroup {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for ToggleGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ToggleGroup {
            id,
            label,
            orientation,
            mode,
            selected_values,
            default_selected_values,
            focused_value,
            selection_required,
            disabled,
            size,
            tokens,
            items,
            on_change,
        } = self;

        window.with_id(id.clone(), |window| {
            let label_text = label.to_string();
            let descriptors: Vec<ToggleGroupItemDescriptor> =
                items.iter().map(ToggleGroupItem::descriptor).collect();
            let selected_seed = selected_values
                .clone()
                .unwrap_or_else(|| default_selected_values.clone());
            let focused_seed = focused_value.clone();
            let runtime = window.use_keyed_state("runtime", cx, |_, _| ToggleGroupRuntime {
                selected_values: selected_seed,
                focused_value: focused_seed,
                focus_handles: BTreeMap::new(),
            });
            let (runtime_selected, runtime_focused) = {
                let runtime = runtime.read(cx);
                (
                    runtime.selected_values.clone(),
                    runtime.focused_value.clone(),
                )
            };
            let state = ToggleGroupState::resolve(
                orientation,
                mode,
                selection_required,
                disabled,
                label_text.clone(),
                selected_values.clone().unwrap_or(runtime_selected),
                runtime_focused.as_deref(),
                descriptors.clone(),
                size,
                tokens,
            );
            runtime.update(cx, |runtime, cx| runtime.sync(&state, &descriptors, cx));

            let metrics = state.metrics();
            let colors = state.colors();
            let selected_colors = state.selected_colors();
            let focus_ring = state.focus_ring();
            let is_vertical = matches!(orientation, Orientation::Vertical);
            let disabled_items = Rc::new(
                state
                    .items()
                    .iter()
                    .map(|item| !item.focusable())
                    .collect::<Vec<_>>(),
            );
            let focus_handles = {
                let runtime = runtime.read(cx);
                state
                    .items()
                    .iter()
                    .map(|item| runtime.focus_handles.get(item.value()).cloned())
                    .collect::<Vec<_>>()
            };
            let focusable_set_size = state.items().iter().filter(|item| item.focusable()).count();
            let tab_stop_index = state.tab_stop_index();
            let item_descriptors = Rc::new(descriptors);
            let mut focusable_position = 0usize;

            div()
                .id(id.clone())
                .debug_selector({
                    let debug_id = id.to_string();
                    move || format!("toggle-group:{debug_id}")
                })
                .ui_role(state.role())
                .aria_label(label.clone())
                .ui_aria_orientation(orientation)
                .aria_disabled(state.disabled())
                .flex()
                .gap(gpui_px_from_ui(metrics.gap()))
                .p(gpui_px_from_ui(metrics.padding()))
                .rounded(gpui_px_from_ui(metrics.radius()))
                .border_1()
                .border_color(ThemeResolver::resolve(colors.border()))
                .bg(ThemeResolver::resolve(colors.background()))
                .when(is_vertical, |this| this.flex_col().items_stretch())
                .when(!is_vertical, |this| this.flex_row().items_center())
                .children(state.items().iter().enumerate().map(|(index, item)| {
                    let descriptor = item_descriptors[index].clone();
                    let click_runtime = runtime.clone();
                    let key_runtime = runtime.clone();
                    let click_descriptors = item_descriptors.clone();
                    let key_descriptors = item_descriptors.clone();
                    let disabled_items = disabled_items.clone();
                    let on_click_change = on_change.clone();
                    let on_key_change = on_change.clone();
                    let focus_handle = focus_handles[index].clone();
                    let item_tab_stop = Some(index) == tab_stop_index;
                    let item_disabled = item.disabled();
                    let item_selected = item.selected();
                    let item_label = item.label().to_owned();
                    let item_value = item.value().to_owned();
                    let click_label = label_text.clone();
                    let key_label = label_text.clone();
                    let item_position = if item.focusable() {
                        focusable_position += 1;
                        Some(focusable_position)
                    } else {
                        None
                    };

                    div()
                        .id(format!("toggle-group-item:{item_value}"))
                        .debug_selector({
                            let group_id = id.to_string();
                            let item_value = item_value.clone();
                            move || format!("toggle-group:{group_id}:item:{item_value}")
                        })
                        .focusable()
                        .tab_stop(item_tab_stop)
                        .ui_role(item.role())
                        .ui_aria_toggled(item.toggled())
                        .aria_label(item_label.clone())
                        .aria_disabled(item_disabled)
                        .when_some(item_position, |this, position| {
                            this.aria_position_in_set(position)
                                .aria_size_of_set(focusable_set_size)
                        })
                        .when_some(focus_handle, |this, focus_handle| {
                            this.track_focus(&focus_handle)
                        })
                        .min_h(gpui_px_from_ui(metrics.item().height()))
                        .px(gpui_px_from_ui(metrics.item().padding_x()))
                        .py(gpui_px_from_ui(metrics.item().padding_y()))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(gpui_px_from_ui(metrics.item().radius()))
                        .border_1()
                        .border_color(ThemeResolver::resolve(if item_selected {
                            selected_colors.border()
                        } else {
                            colors.border()
                        }))
                        .bg(ThemeResolver::resolve(if item_selected {
                            selected_colors.background()
                        } else {
                            colors.background()
                        }))
                        .text_color(ThemeResolver::resolve(if item_selected {
                            selected_colors.foreground()
                        } else {
                            colors.foreground()
                        }))
                        .text_size(gpui_px_from_ui(metrics.item().text_size()))
                        .line_height(gpui_px_from_ui(metrics.item().text_size()))
                        .focus_visible(move |style| style.shadow(focus_ring_shadow(focus_ring)))
                        .when(!item_disabled, |this| {
                            this.cursor_pointer().hover(move |style| {
                                style.bg(ThemeResolver::resolve(colors.hover_background()))
                            })
                        })
                        .when(item_disabled, |this| {
                            this.opacity(0.56).cursor_not_allowed()
                        })
                        .on_click({
                            let descriptor = descriptor.clone();
                            move |_event: &ClickEvent, window, cx| {
                                if disabled || descriptor.disabled_state() {
                                    return;
                                }

                                cx.stop_propagation();
                                let change = click_runtime.update(cx, |runtime, cx| {
                                    runtime.activate(
                                        orientation,
                                        mode,
                                        selection_required,
                                        disabled,
                                        size,
                                        tokens,
                                        click_label.clone(),
                                        &descriptor,
                                        &click_descriptors,
                                        cx,
                                    )
                                });
                                if let Some((change, focus_handle)) = change {
                                    if let Some(handler) = on_click_change.clone() {
                                        handler(change, window, cx);
                                    }
                                    if let Some(focus_handle) = focus_handle {
                                        focus_handle.focus(window, cx);
                                    }
                                }
                            }
                        })
                        .on_key_down({
                            let descriptor = descriptor.clone();
                            move |event: &KeyDownEvent, window, cx| {
                                if disabled || descriptor.disabled_state() {
                                    return;
                                }
                                if event.keystroke.modifiers.modified() {
                                    return;
                                }

                                let key = event.keystroke.key.as_str();
                                let Some(target_index) = toggle_group_navigation_target(
                                    orientation,
                                    key,
                                    index,
                                    &disabled_items,
                                ) else {
                                    if !matches!(key, "space" | "enter") {
                                        return;
                                    }

                                    let change = key_runtime.update(cx, |runtime, cx| {
                                        runtime.activate(
                                            orientation,
                                            mode,
                                            selection_required,
                                            disabled,
                                            size,
                                            tokens,
                                            key_label.clone(),
                                            &descriptor,
                                            &key_descriptors,
                                            cx,
                                        )
                                    });
                                    if let Some((change, _)) = change {
                                        if let Some(handler) = on_key_change.clone() {
                                            handler(change, window, cx);
                                        }
                                    }
                                    cx.stop_propagation();
                                    return;
                                };

                                let target = &key_descriptors[target_index];
                                let target_value = target.value().to_owned();
                                let focus_handle = key_runtime.update(cx, |runtime, cx| {
                                    runtime.set_focused(&target_value, cx)
                                });

                                if let Some(focus_handle) = focus_handle {
                                    focus_handle.focus(window, cx);
                                }

                                cx.stop_propagation();
                            }
                        })
                        .child(item_label)
                }))
        })
    }
}

#[derive(Debug, Default)]
struct ToggleGroupRuntime {
    selected_values: Vec<String>,
    focused_value: Option<String>,
    focus_handles: BTreeMap<String, FocusHandle>,
}

impl ToggleGroupRuntime {
    fn sync(
        &mut self,
        state: &ToggleGroupState,
        items: &[ToggleGroupItemDescriptor],
        cx: &mut Context<Self>,
    ) {
        self.focus_handles.retain(|value, _| {
            items
                .iter()
                .any(|item| item.value() == value && !item.disabled_state())
        });

        for item in items.iter().filter(|item| !item.disabled_state()) {
            self.focus_handles
                .entry(item.value().to_owned())
                .or_insert_with(|| cx.focus_handle());
        }

        self.selected_values = state.selected_values().to_vec();
        self.focused_value = state.focused_value().map(str::to_owned);
    }

    fn set_focused(&mut self, value: &str, cx: &mut Context<Self>) -> Option<FocusHandle> {
        self.focused_value = Some(value.to_owned());
        self.focus_handles
            .entry(value.to_owned())
            .or_insert_with(|| cx.focus_handle())
            .clone()
            .into()
    }

    #[allow(clippy::too_many_arguments)]
    fn activate(
        &mut self,
        orientation: Orientation,
        mode: ToggleGroupSelectionMode,
        selection_required: bool,
        disabled: bool,
        size: Size,
        tokens: ThemeTokens,
        label: String,
        descriptor: &ToggleGroupItemDescriptor,
        items: &[ToggleGroupItemDescriptor],
        cx: &mut Context<Self>,
    ) -> Option<(ToggleGroupSelectionChange, Option<FocusHandle>)> {
        if disabled || descriptor.disabled_state() {
            return None;
        }

        let state = ToggleGroupState::resolve(
            orientation,
            mode,
            selection_required,
            disabled,
            label,
            self.selected_values.clone(),
            Some(descriptor.value()),
            items.iter().cloned(),
            size,
            tokens,
        );
        let change = state.selection_change_for_item(descriptor.value())?;
        self.selected_values = change.selected_values().to_vec();
        let focus_handle = self.set_focused(descriptor.value(), cx);
        Some((change, focus_handle))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui_ui_core::semantic;

    fn sample_items() -> [ToggleGroupItem; 3] {
        [
            ToggleGroupItem::new("left", "Left"),
            ToggleGroupItem::new("center", "Center").disabled(true),
            ToggleGroupItem::new("right", "Right"),
        ]
    }

    #[test]
    fn toggle_group_preserves_stable_values_and_skips_disabled_items() {
        let state = ToggleGroup::new("align", "Alignment")
            .items(sample_items())
            .selected_values(["center", "right"])
            .default_focused("center")
            .state();

        assert_eq!(state.role(), Role::Group);
        assert_eq!(state.selected_values(), &["right".to_owned()]);
        assert_eq!(state.focused_value(), Some("left"));
        assert!(state.items()[1].disabled());
        assert_eq!(state.items()[2].toggled(), Toggled::True);
        assert_eq!(
            state
                .navigation_target("right")
                .map(ToggleGroupItemState::value),
            Some("right")
        );
        assert_eq!(state.colors().border().token(), semantic::BORDER);
    }

    #[test]
    fn controlled_selected_values_take_precedence_over_default_seed() {
        let state = ToggleGroup::new("align", "Alignment")
            .items(sample_items())
            .selected_values(std::iter::empty::<&str>())
            .default_selected_values(["right"])
            .state();

        assert!(
            state.selected_values().is_empty(),
            "controlled empty selection should not be replaced by default_selected_values"
        );
    }

    #[test]
    fn single_mode_deselects_or_replaces_value() {
        let state = ToggleGroup::new("align", "Alignment")
            .items(sample_items())
            .selected_values(["right"])
            .state();

        let deselect = state.selection_change_for_item("right").unwrap();
        assert!(deselect.selected_values().is_empty());

        let select = state.selection_change_for_item("left").unwrap();
        assert_eq!(select.selected_values(), &["left".to_owned()]);
    }

    #[test]
    fn multiple_mode_toggles_values_without_duplicates() {
        let state = ToggleGroup::new("format", "Format")
            .mode(ToggleGroupSelectionMode::Multiple)
            .items(sample_items())
            .selected_values(["left", "right"])
            .state();

        let remove = state.selection_change_for_item("left").unwrap();
        assert_eq!(remove.selected_values(), &["right".to_owned()]);

        let add_state = ToggleGroup::new("format", "Format")
            .mode(ToggleGroupSelectionMode::Multiple)
            .items(sample_items())
            .selected_values(["left"])
            .state();
        let add = add_state.selection_change_for_item("right").unwrap();
        assert_eq!(
            add.selected_values(),
            &["left".to_owned(), "right".to_owned()]
        );
    }

    #[test]
    fn selection_required_keeps_last_value() {
        let state = ToggleGroup::new("align", "Alignment")
            .items(sample_items())
            .selected_values(["right"])
            .selection_required(true)
            .state();

        let change = state.selection_change_for_item("right").unwrap();
        assert_eq!(change.selected_values(), &["right".to_owned()]);
    }
}
