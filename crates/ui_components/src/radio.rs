//! Radio group component.

use crate::geometry::gpui_px_from_ui;
use std::collections::BTreeMap;
use std::rc::Rc;

use open_gpui::prelude::FluentBuilder;
use open_gpui::{
    App, ClickEvent, Context, ElementId, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled,
    Window, div, px,
};
use open_gpui_ui_core::{Orientation, Role, Sizable, Size, ThemeTokens, UiPx, ui_px};

use crate::a11y::UiA11yElementExt;
use crate::choice::{ChoiceCollection, ChoiceInteractionPolicy, ChoiceItemProjection};
use crate::color::ColorIntent;
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::theme::ThemeResolver;

/// Pure descriptor for one radio item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadioItemDescriptor {
    value: String,
    label: String,
    disabled: bool,
}

impl RadioItemDescriptor {
    /// Creates a new radio item descriptor.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
        }
    }

    /// Marks the radio item as disabled.
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

    /// Returns whether the item is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }
}

/// Resolved radio group metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadioGroupMetrics {
    item_gap: UiPx,
    control_size: UiPx,
    indicator_size: UiPx,
    label_text_size: UiPx,
    item_padding_x: UiPx,
    item_padding_y: UiPx,
    radius: UiPx,
}

impl RadioGroupMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        let control_size = match size {
            Size::XSmall => ui_px(16.0),
            Size::Small => ui_px(18.0),
            Size::Medium => ui_px(20.0),
            Size::Large => ui_px(22.0),
        };
        let indicator_size = match size {
            Size::XSmall => ui_px(6.0),
            Size::Small => ui_px(7.0),
            Size::Medium => ui_px(8.0),
            Size::Large => ui_px(9.0),
        };

        Self {
            item_gap: ui_px(8.0),
            control_size,
            indicator_size,
            label_text_size: size.control_text_px(),
            item_padding_x: ui_px(2.0),
            item_padding_y: ui_px(2.0),
            radius: size.control_radius(),
        }
    }

    /// Returns the gap between radio items.
    pub const fn item_gap(self) -> UiPx {
        self.item_gap
    }

    /// Returns the outer radio control size.
    pub const fn control_size(self) -> UiPx {
        self.control_size
    }

    /// Returns the selected indicator size.
    pub const fn indicator_size(self) -> UiPx {
        self.indicator_size
    }

    /// Returns the label text size.
    pub const fn label_text_size(self) -> UiPx {
        self.label_text_size
    }

    /// Returns item horizontal padding.
    pub const fn item_padding_x(self) -> UiPx {
        self.item_padding_x
    }

    /// Returns item vertical padding.
    pub const fn item_padding_y(self) -> UiPx {
        self.item_padding_y
    }

    /// Returns the item focus radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }
}

/// Resolved radio group color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadioGroupColors {
    pub(crate) control_background: ColorIntent,
    pub(crate) control_background_selected: ColorIntent,
    pub(crate) control_border: ColorIntent,
    pub(crate) control_border_selected: ColorIntent,
    pub(crate) indicator: ColorIntent,
    pub(crate) label: ColorIntent,
    pub(crate) label_muted: ColorIntent,
    pub(crate) hover_background: ColorIntent,
    pub(crate) focus_ring: ColorIntent,
}

impl RadioGroupColors {
    /// Returns the unselected control background.
    pub const fn control_background(self) -> ColorIntent {
        self.control_background
    }

    /// Returns the selected control background.
    pub const fn control_background_selected(self) -> ColorIntent {
        self.control_background_selected
    }

    /// Returns the unselected control border.
    pub const fn control_border(self) -> ColorIntent {
        self.control_border
    }

    /// Returns the selected control border.
    pub const fn control_border_selected(self) -> ColorIntent {
        self.control_border_selected
    }

    /// Returns the selected indicator color.
    pub const fn indicator(self) -> ColorIntent {
        self.indicator
    }

    /// Returns the enabled label color.
    pub const fn label(self) -> ColorIntent {
        self.label
    }

    /// Returns the muted label color.
    pub const fn label_muted(self) -> ColorIntent {
        self.label_muted
    }

    /// Returns the hover background color.
    pub const fn hover_background(self) -> ColorIntent {
        self.hover_background
    }

    /// Returns the focus ring color.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved radio item state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadioItemState {
    index: usize,
    value: String,
    label: String,
    disabled: bool,
    selected: bool,
    focused: bool,
}

impl RadioItemState {
    /// Returns the zero-based index.
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

    /// Returns whether the item is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the item is selected.
    pub const fn selected(&self) -> bool {
        self.selected
    }

    /// Returns whether the item has roving focus.
    pub const fn focused(&self) -> bool {
        self.focused
    }

    /// Returns whether activation handlers should run for this item.
    pub const fn activation_enabled(&self) -> bool {
        !self.disabled
    }

    /// Returns the accessibility role.
    pub const fn role(&self) -> Role {
        Role::RadioButton
    }
}

/// Resolved radio selection change payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadioSelection {
    index: usize,
    value: String,
    label: String,
}

impl RadioSelection {
    /// Creates a selection payload from a descriptor.
    fn from_descriptor(index: usize, descriptor: &RadioItemDescriptor) -> Self {
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

    /// Returns the selected value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the selected label.
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Resolved radio group state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct RadioGroupState {
    orientation: Orientation,
    size: Size,
    disabled: bool,
    required: bool,
    metrics: RadioGroupMetrics,
    colors: RadioGroupColors,
    focus_ring: FocusRing,
    items: Vec<RadioItemState>,
    selected_index: Option<usize>,
    focused_index: Option<usize>,
}

impl RadioGroupState {
    /// Resolves the public state for a radio group.
    pub fn resolve(
        orientation: Orientation,
        size: Size,
        disabled: bool,
        required: bool,
        selected_value: Option<&str>,
        focused_value: Option<&str>,
        items: impl IntoIterator<Item = RadioItemDescriptor>,
        tokens: ThemeTokens,
    ) -> Self {
        let descriptors: Vec<RadioItemDescriptor> = items.into_iter().collect();
        let collection = ChoiceCollection::resolve(
            disabled,
            radio_choice_items(disabled, &descriptors),
            selected_value,
            focused_value,
            ChoiceInteractionPolicy::single_required(orientation),
        );
        let selected_index = collection.selected_index();
        let focused_index = collection.active_index();
        let metrics = RadioGroupMetrics::from_size(size);
        let colors = ThemeResolver::radio_group_colors(tokens);
        let focus_ring = FocusRing::from_color(colors.focus_ring());

        let items = descriptors
            .into_iter()
            .enumerate()
            .map(|(index, descriptor)| {
                let selected = Some(index) == selected_index;
                let focused = Some(index) == focused_index;

                RadioItemState {
                    index,
                    value: descriptor.value,
                    label: descriptor.label,
                    disabled: disabled || descriptor.disabled,
                    selected,
                    focused,
                }
            })
            .collect();

        Self {
            orientation,
            size,
            disabled,
            required,
            metrics,
            colors,
            focus_ring,
            items,
            selected_index,
            focused_index,
        }
    }

    /// Returns the group orientation.
    pub const fn orientation(&self) -> Orientation {
        self.orientation
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether the whole group is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the group is required.
    pub const fn required(&self) -> bool {
        self.required
    }

    /// Returns whether any activation handlers should run.
    pub const fn activation_enabled(&self) -> bool {
        !self.disabled
    }

    /// Returns the accessibility role.
    pub const fn role(&self) -> Role {
        Role::RadioGroup
    }

    /// Returns the resolved metrics.
    pub const fn metrics(&self) -> RadioGroupMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> RadioGroupColors {
        self.colors
    }

    /// Returns resolved focus ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }

    /// Returns all resolved radio items.
    pub fn items(&self) -> &[RadioItemState] {
        &self.items
    }

    /// Returns the selected item index.
    pub const fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Returns the selected value.
    pub fn selected_value(&self) -> Option<&str> {
        self.selected_index
            .and_then(|index| self.items.get(index))
            .map(RadioItemState::value)
    }

    /// Returns the selected item.
    pub fn selected_item(&self) -> Option<&RadioItemState> {
        self.selected_index.and_then(|index| self.items.get(index))
    }

    /// Returns the focused item index.
    pub const fn focused_index(&self) -> Option<usize> {
        self.focused_index
    }

    /// Returns the focused value.
    pub fn focused_value(&self) -> Option<&str> {
        self.focused_index
            .and_then(|index| self.items.get(index))
            .map(RadioItemState::value)
    }

    /// Returns the current tab-stop index.
    pub const fn tab_stop_index(&self) -> Option<usize> {
        if self.disabled {
            None
        } else if self.focused_index.is_some() {
            self.focused_index
        } else {
            self.selected_index
        }
    }

    /// Returns whether there are no radio items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// A concrete GPUI radio item.
pub struct RadioItem {
    value: String,
    label: SharedString,
    disabled: bool,
}

impl RadioItem {
    /// Creates a new radio item.
    pub fn new(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
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

    fn descriptor(&self) -> RadioItemDescriptor {
        RadioItemDescriptor {
            value: self.value.clone(),
            label: self.label.to_string(),
            disabled: self.disabled,
        }
    }
}

/// A concrete GPUI radio group component.
#[derive(IntoElement)]
pub struct RadioGroup {
    id: ElementId,
    label: Option<SharedString>,
    orientation: Orientation,
    selected_value: Option<String>,
    disabled: bool,
    required: bool,
    size: Size,
    tokens: ThemeTokens,
    items: Vec<RadioItem>,
    on_selection_change: Option<Rc<dyn Fn(RadioSelection, &mut Window, &mut App)>>,
}

impl RadioGroup {
    /// Creates a new radio group component.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: None,
            orientation: Orientation::Vertical,
            selected_value: None,
            disabled: false,
            required: false,
            size: Size::Medium,
            tokens: ThemeTokens::default(),
            items: Vec::new(),
            on_selection_change: None,
        }
    }

    /// Sets the accessible group label.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets the group orientation.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Applies the default selected radio value for adapter-owned runtime state.
    pub fn default_selected(mut self, value: impl Into<String>) -> Self {
        self.selected_value = Some(value.into());
        self
    }

    /// Marks the whole group as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks the group as required.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Adds a radio item.
    pub fn item(mut self, item: RadioItem) -> Self {
        self.items.push(item);
        self
    }

    /// Registers a selection change handler.
    pub fn on_selection_change(
        mut self,
        handler: impl Fn(RadioSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(Rc::new(handler));
        self
    }

    /// Returns the resolved state.
    pub fn state(&self) -> RadioGroupState {
        RadioGroupState::resolve(
            self.orientation,
            self.size,
            self.disabled,
            self.required,
            self.selected_value.as_deref(),
            None,
            self.items.iter().map(RadioItem::descriptor),
            self.tokens,
        )
    }
}

impl Sizable for RadioGroup {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for RadioGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(cx);
        let RadioGroup {
            id,
            label,
            orientation,
            selected_value,
            disabled,
            required,
            size,
            tokens,
            items,
            on_selection_change,
        } = self;

        window.with_id(id.clone(), |window| {
            let debug_id = id.to_string();
            let descriptors: Vec<RadioItemDescriptor> =
                items.iter().map(RadioItem::descriptor).collect();
            let selected_seed = selected_value.clone();
            let runtime = window.use_keyed_state("runtime", cx, |_, _| RadioRuntime {
                selected_value: selected_seed.clone(),
                focused_value: selected_seed.clone(),
                focus_handles: BTreeMap::new(),
            });
            let runtime_snapshot = {
                let runtime = runtime.read(cx);
                (
                    runtime.selected_value.clone(),
                    runtime.focused_value.clone(),
                )
            };
            let state = RadioGroupState::resolve(
                orientation,
                size,
                disabled,
                required,
                runtime_snapshot.0.as_deref(),
                runtime_snapshot.1.as_deref(),
                descriptors.clone(),
                tokens,
            );
            runtime.update(cx, |runtime, cx| runtime.sync(&state, &descriptors, cx));

            let item_descriptors = Rc::new(descriptors);
            let disabled_items = Rc::new(
                state
                    .items()
                    .iter()
                    .map(RadioItemState::disabled)
                    .collect::<Vec<_>>(),
            );
            let selected_value = state.selected_value().map(str::to_owned);
            let metrics = state.metrics();
            let colors = state.colors();
            let focus_ring = state.focus_ring();
            let is_vertical = matches!(orientation, Orientation::Vertical);
            let tab_stop_index = state.tab_stop_index();
            let focus_handles = {
                let runtime = runtime.read(cx);
                state
                    .items()
                    .iter()
                    .map(|item| runtime.focus_handles.get(item.value()).cloned())
                    .collect::<Vec<_>>()
            };

            div()
                .id(id.clone())
                .debug_selector({
                    let debug_id = debug_id.clone();
                    move || format!("radio-group:{debug_id}")
                })
                .ui_role(state.role())
                .ui_aria_orientation(orientation)
                .aria_label(label.unwrap_or_else(|| SharedString::from("Radio group")))
                .aria_required(state.required())
                .aria_disabled(state.disabled())
                .flex()
                .gap(gpui_px_from_ui(metrics.item_gap()))
                .when(is_vertical, |this| this.flex_col())
                .when(!is_vertical, |this| this.flex_row().flex_wrap())
                .children(state.items().iter().enumerate().map(|(index, item)| {
                    let descriptor = item_descriptors[index].clone();
                    let disabled_items = disabled_items.clone();
                    let focus_handle = focus_handles[index].clone();
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
                    let item_value = item.value().to_owned();
                    let label_color = theme.resolve(if item.disabled() {
                        colors.label_muted()
                    } else {
                        colors.label()
                    });
                    let hover_background = theme.resolve(colors.hover_background());
                    let control_border = theme.resolve(if is_selected {
                        colors.control_border_selected()
                    } else {
                        colors.control_border()
                    });
                    let control_background = theme.resolve(if is_selected {
                        colors.control_background_selected()
                    } else {
                        colors.control_background()
                    });
                    let indicator_color = theme.resolve(colors.indicator());
                    let item_focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);

                    div()
                        .id(radio_item_id(item.value()))
                        .debug_selector({
                            let debug_id = debug_id.clone();
                            let item_value = item_value.clone();
                            move || format!("radio-group:{debug_id}:item:{item_value}")
                        })
                        .focusable()
                        .tab_stop(is_tab_stop)
                        .ui_role(item.role())
                        .aria_label(descriptor.label())
                        .aria_selected(is_selected)
                        .aria_disabled(item.disabled())
                        .aria_position_in_set(item_index + 1)
                        .aria_size_of_set(state.items().len())
                        .when_some(focus_handle, |this, focus_handle| {
                            this.track_focus(&focus_handle)
                        })
                        .flex()
                        .items_center()
                        .gap_2()
                        .px(gpui_px_from_ui(metrics.item_padding_x()))
                        .py(gpui_px_from_ui(metrics.item_padding_y()))
                        .rounded(gpui_px_from_ui(metrics.radius()))
                        .text_size(gpui_px_from_ui(metrics.label_text_size()))
                        .line_height(gpui_px_from_ui(metrics.label_text_size()))
                        .text_color(label_color)
                        .focus_visible(move |style| style.shadow(item_focus_shadow.clone()))
                        .when(!item.disabled(), |this| {
                            this.cursor_pointer()
                                .hover(move |style| style.bg(hover_background))
                        })
                        .when(item.disabled(), |this| {
                            this.opacity(0.56).cursor_not_allowed()
                        })
                        .on_click({
                            let descriptor = descriptor.clone();
                            move |_event: &ClickEvent, window, cx| {
                                if descriptor.disabled_state() || disabled {
                                    return;
                                }

                                cx.stop_propagation();
                                let changed =
                                    click_selected_value.as_deref() != Some(descriptor.value());
                                let focus_handle = click_runtime.update(cx, |runtime, cx| {
                                    runtime.set_active(descriptor.value(), cx)
                                });

                                if changed && let Some(handler) = click_on_selection_change.clone()
                                {
                                    handler(
                                        RadioSelection::from_descriptor(item_index, &descriptor),
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
                            let disabled_items = disabled_items.clone();
                            move |event: &KeyDownEvent, window, cx| {
                                if descriptor.disabled_state() || disabled {
                                    return;
                                }
                                if event.keystroke.modifiers.modified() {
                                    return;
                                }

                                let key = event.keystroke.key.as_str();
                                let Some(target_index) =
                                    ChoiceInteractionPolicy::single_required(orientation)
                                        .navigation_target_index(key, item_index, &disabled_items)
                                else {
                                    if !matches!(key, "space" | "enter") {
                                        return;
                                    }

                                    let changed =
                                        key_selected_value.as_deref() != Some(descriptor.value());
                                    let focus_handle = key_runtime.update(cx, |runtime, cx| {
                                        runtime.set_active(descriptor.value(), cx)
                                    });

                                    if changed
                                        && let Some(handler) = key_on_selection_change.clone()
                                    {
                                        handler(
                                            RadioSelection::from_descriptor(
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
                                    RadioSelection::from_descriptor(target_index, target);
                                let changed = key_selected_value.as_deref() != Some(target.value());
                                let focus_handle = key_runtime.update(cx, |runtime, cx| {
                                    runtime.set_active(&target_value, cx)
                                });

                                if changed && let Some(handler) = key_on_selection_change.clone() {
                                    handler(target_selection, window, cx);
                                }

                                if let Some(focus_handle) = focus_handle {
                                    focus_handle.focus(window, cx);
                                }

                                cx.stop_propagation();
                            }
                        })
                        .child(
                            div()
                                .w(gpui_px_from_ui(metrics.control_size()))
                                .h(gpui_px_from_ui(metrics.control_size()))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(gpui_px_from_ui(metrics.control_size()))
                                .border_1()
                                .border_color(control_border)
                                .bg(control_background)
                                .child(if is_selected {
                                    div()
                                        .w(gpui_px_from_ui(metrics.indicator_size()))
                                        .h(gpui_px_from_ui(metrics.indicator_size()))
                                        .rounded(gpui_px_from_ui(metrics.indicator_size()))
                                        .bg(indicator_color)
                                } else {
                                    div().w(px(0.0)).h(px(0.0))
                                }),
                        )
                        .child(descriptor.label().to_string())
                }))
        })
    }
}

#[derive(Debug, Default)]
struct RadioRuntime {
    selected_value: Option<String>,
    focused_value: Option<String>,
    focus_handles: BTreeMap<String, FocusHandle>,
}

impl RadioRuntime {
    fn sync(
        &mut self,
        state: &RadioGroupState,
        items: &[RadioItemDescriptor],
        cx: &mut Context<Self>,
    ) {
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
        if self.selected_value.as_deref() == Some(value)
            && self.focused_value.as_deref() == Some(value)
        {
            return self.focus_handles.get(value).cloned();
        }

        let value = value.to_owned();
        self.selected_value = Some(value.clone());
        self.focused_value = Some(value.clone());
        cx.notify();
        self.focus_handles.get(&value).cloned()
    }
}

fn radio_choice_items(
    disabled: bool,
    items: &[RadioItemDescriptor],
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

fn radio_item_id(value: &str) -> ElementId {
    format!("radio-{value}").into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radio_group_state_skips_disabled_selection() {
        let state = RadioGroupState::resolve(
            Orientation::Vertical,
            Size::Medium,
            false,
            true,
            Some("team"),
            None,
            [
                RadioItemDescriptor::new("personal", "Personal"),
                RadioItemDescriptor::new("team", "Team").disabled(true),
                RadioItemDescriptor::new("enterprise", "Enterprise"),
            ],
            ThemeTokens::default(),
        );

        assert_eq!(state.role(), Role::RadioGroup);
        assert!(state.required());
        assert_eq!(state.selected_value(), Some("personal"));
        assert_eq!(state.focused_value(), Some("personal"));
        assert_eq!(state.tab_stop_index(), state.focused_index());
        assert!(state.items()[1].disabled());
        assert_eq!(state.items()[0].role(), Role::RadioButton);
    }

    #[test]
    fn radio_navigation_uses_choice_policy() {
        let disabled = [false, true, false];
        let horizontal = ChoiceInteractionPolicy::single_required(Orientation::Horizontal);
        let vertical = ChoiceInteractionPolicy::single_required(Orientation::Vertical);

        assert_eq!(
            horizontal.navigation_target_index("right", 0, &disabled),
            Some(2)
        );
        assert_eq!(
            horizontal.navigation_target_index("right", 2, &disabled),
            Some(0)
        );
        assert_eq!(
            vertical.navigation_target_index("up", 2, &disabled),
            Some(0)
        );
    }
}
