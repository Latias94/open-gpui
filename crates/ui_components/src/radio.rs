//! Radio group component.

mod render;

use std::collections::BTreeMap;
use std::rc::Rc;

use open_gpui::{App, ElementId, IntoElement, SharedString, Window};
use open_gpui_ui_core::{Orientation, Role, Sizable, Size, ThemeTokens, UiPx, ui_px};

use crate::activation::ActivationHandle;
use crate::choice::{ChoiceCollection, ChoiceInteractionPolicy, ChoiceItemProjection};
use crate::color::ColorIntent;
use crate::focus::FocusRing;
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
    read_only: bool,
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

    /// Returns whether the owning group is read-only.
    pub const fn read_only(&self) -> bool {
        self.read_only
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
        !self.disabled && !self.read_only
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
    read_only: bool,
    required: bool,
    metrics: RadioGroupMetrics,
    colors: RadioGroupColors,
    focus_ring: FocusRing,
    items: Vec<RadioItemState>,
    selected_index: Option<usize>,
    focused_index: Option<usize>,
}

/// Selection ownership passed to [`RadioGroupState::resolve`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadioSelectionAuthority<'a> {
    /// Preserve the caller-owned value exactly, including no or unavailable selection.
    Controlled(Option<&'a str>),
    /// Resolve an adapter-owned default, falling back according to the choice policy.
    Uncontrolled(Option<&'a str>),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum RadioSelectionControl {
    #[default]
    Uncontrolled,
    Controlled(Option<String>),
}

impl RadioSelectionControl {
    fn authority<'a>(
        &'a self,
        default_selected_value: Option<&'a str>,
    ) -> RadioSelectionAuthority<'a> {
        match self {
            Self::Uncontrolled => RadioSelectionAuthority::Uncontrolled(default_selected_value),
            Self::Controlled(value) => RadioSelectionAuthority::Controlled(value.as_deref()),
        }
    }

    const fn controlled(&self) -> bool {
        matches!(self, Self::Controlled(_))
    }

    fn initial_value(&self, default_selected_value: Option<&String>) -> Option<String> {
        match self {
            Self::Uncontrolled => default_selected_value.cloned(),
            Self::Controlled(value) => value.clone(),
        }
    }
}

impl RadioGroupState {
    /// Resolves the public state for a radio group.
    pub fn resolve(
        orientation: Orientation,
        size: Size,
        disabled: bool,
        required: bool,
        selection: RadioSelectionAuthority<'_>,
        focused_value: Option<&str>,
        items: impl IntoIterator<Item = RadioItemDescriptor>,
        tokens: ThemeTokens,
    ) -> Self {
        let descriptors: Vec<RadioItemDescriptor> = items.into_iter().collect();
        let selected_value = match selection {
            RadioSelectionAuthority::Controlled(value)
            | RadioSelectionAuthority::Uncontrolled(value) => value,
        };
        let collection = ChoiceCollection::resolve(
            disabled,
            radio_choice_items(disabled, &descriptors),
            selected_value,
            focused_value,
            ChoiceInteractionPolicy::single_required(orientation),
        );
        let selected_index = match selection {
            RadioSelectionAuthority::Controlled(Some(value)) => descriptors
                .iter()
                .position(|descriptor| descriptor.value() == value),
            RadioSelectionAuthority::Controlled(None) => None,
            RadioSelectionAuthority::Uncontrolled(_) => collection.selected_index(),
        };
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
                    read_only: false,
                    selected,
                    focused,
                }
            })
            .collect();

        Self {
            orientation,
            size,
            disabled,
            read_only: false,
            required,
            metrics,
            colors,
            focus_ring,
            items,
            selected_index,
            focused_index,
        }
    }

    /// Returns a copy with read-only state applied to the group and every item.
    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        for item in &mut self.items {
            item.read_only = read_only;
        }
        self
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

    /// Returns whether the group is read-only.
    pub const fn read_only(&self) -> bool {
        self.read_only
    }

    /// Returns whether the group is required.
    pub const fn required(&self) -> bool {
        self.required
    }

    /// Returns whether any activation handlers should run.
    pub const fn activation_enabled(&self) -> bool {
        !self.disabled && !self.read_only
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
    selection: RadioSelectionControl,
    default_selected_value: Option<String>,
    disabled: bool,
    read_only: bool,
    required: bool,
    size: Size,
    tokens: ThemeTokens,
    items: Vec<RadioItem>,
    on_selection_change: Option<Rc<dyn Fn(RadioSelection, &mut Window, &mut App)>>,
    activation_handles: BTreeMap<String, ActivationHandle>,
}

impl RadioGroup {
    /// Creates a new radio group component.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: None,
            orientation: Orientation::Vertical,
            selection: RadioSelectionControl::default(),
            default_selected_value: None,
            disabled: false,
            read_only: false,
            required: false,
            size: Size::Medium,
            tokens: ThemeTokens::default(),
            items: Vec::new(),
            on_selection_change: None,
            activation_handles: BTreeMap::new(),
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

    /// Applies the caller-owned selected radio value.
    pub fn selected(mut self, value: Option<String>) -> Self {
        self.selection = RadioSelectionControl::Controlled(value);
        self
    }

    /// Applies the default selected radio value for adapter-owned runtime state.
    pub fn default_selected(mut self, value: impl Into<String>) -> Self {
        self.default_selected_value = Some(value.into());
        self
    }

    /// Marks the whole group as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks the group as read-only while preserving focus and semantics.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
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

    /// Binds an application-owned activation handle to one stable item value.
    pub fn activation_handle(
        mut self,
        value: impl Into<String>,
        handle: &ActivationHandle,
    ) -> Self {
        self.activation_handles.insert(value.into(), handle.clone());
        self
    }

    /// Returns the resolved state.
    pub fn state(&self) -> RadioGroupState {
        RadioGroupState::resolve(
            self.orientation,
            self.size,
            self.disabled,
            self.required,
            self.selection
                .authority(self.default_selected_value.as_deref()),
            None,
            self.items.iter().map(RadioItem::descriptor),
            self.tokens,
        )
        .with_read_only(self.read_only)
    }
}

impl Sizable for RadioGroup {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
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
            RadioSelectionAuthority::Uncontrolled(Some("team")),
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
