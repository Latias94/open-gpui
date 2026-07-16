//! Toggle group component.

mod render;

use crate::button::{ButtonColors, ButtonMetrics, ButtonVariant};
use crate::choice::{
    self, ChoiceCollection, ChoiceInteractionPolicy, ChoiceItemProjection, ChoiceSelectionMode,
};
use crate::focus::FocusRing;
use crate::theme::ThemeResolver;
use open_gpui::{App, ElementId, IntoElement, SharedString, Window};
use open_gpui_ui_core::{Orientation, Role, Sizable, Size, ThemeTokens, Toggled, UiPx, ui_px};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use crate::activation::ActivationHandle;

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

/// Toggle group selection transition.
///
/// The item snapshot describes the activated item before the transition, while
/// [`Self::selected_values`] contains the next selected stable values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToggleGroupSelectionChange {
    item: ToggleGroupItemState,
    selected_values: Vec<String>,
}

impl ToggleGroupSelectionChange {
    fn new(item: ToggleGroupItemState, selected_values: Vec<String>) -> Self {
        Self {
            item,
            selected_values,
        }
    }

    /// Returns the activated item's pre-transition state.
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
        resolve_toggle_group_selection_change(
            self.mode,
            self.selection_required,
            &self.selected_values,
            item,
        )
    }

    /// Resolves a selection change for the currently focused item.
    pub fn selection_change_for_key(&self, key: &str) -> Option<ToggleGroupSelectionChange> {
        if key != "space" {
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

fn resolve_toggle_group_selection_change(
    mode: ToggleGroupSelectionMode,
    selection_required: bool,
    selected_values: &[String],
    item: ToggleGroupItemState,
) -> Option<ToggleGroupSelectionChange> {
    if !item.focusable() {
        return None;
    }

    let next_selected = choice::next_selected_values(
        toggle_group_choice_selection_mode(mode),
        selection_required,
        selected_values,
        item.value(),
    );
    (next_selected != selected_values).then(|| ToggleGroupSelectionChange::new(item, next_selected))
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
    activation_handles: BTreeMap<String, ActivationHandle>,
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
            activation_handles: BTreeMap::new(),
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

    /// Binds an application-owned activation handle to one stable item value.
    pub fn activation_handle(
        mut self,
        value: impl Into<String>,
        handle: &ActivationHandle,
    ) -> Self {
        self.activation_handles.insert(value.into(), handle.clone());
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
        assert!(state.selection_change_for_key("enter").is_none());
        assert_eq!(
            state
                .selection_change_for_key("space")
                .expect("Space should activate the focused toggle")
                .selected_values(),
            deselect.selected_values()
        );

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
    fn selection_required_suppresses_noop_change() {
        let state = ToggleGroup::new("align", "Alignment")
            .items(sample_items())
            .selected_values(["right"])
            .selection_required(true)
            .state();

        assert!(state.selection_change_for_item("right").is_none());
        assert!(state.selection_change_for_key("space").is_none());
    }
}
