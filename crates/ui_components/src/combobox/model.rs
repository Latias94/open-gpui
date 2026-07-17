use std::collections::BTreeSet;

use open_gpui_ui_core::{
    FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayLayerKind,
    OverlayPlacementAlignment, OverlayPlacementSide, Role, Size, ThemeTokens,
};

use crate::choice::{self, ChoiceCollection, ChoiceInteractionPolicy};
use crate::focus::FocusRing;
use crate::listbox::{ListboxGroupDescriptor, ListboxState};
use crate::overlay::{OverlayDisclosureConfig, OverlayDisclosureOpenMode, OverlayResolvedState};
use crate::scroll_area::{ScrollAreaAxis, ScrollAreaState, ScrollResetPolicy};
use crate::text_editing::TextEditingPolicy;
use crate::text_input::TextInputState;
use crate::theme::ThemeResolver;

use super::descriptor::{
    ComboboxGroupDescriptor, ComboboxOptionDescriptor, flatten_combobox_choice_options,
};
use super::style::{ComboboxColors, ComboboxMetrics};

/// Combobox open-state ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ComboboxOpenMode {
    /// Open state is owned by the component adapter after initialization.
    #[default]
    Uncontrolled,
    /// Open state is provided by the caller.
    Controlled,
}

const fn combobox_open_mode_from_disclosure(mode: OverlayDisclosureOpenMode) -> ComboboxOpenMode {
    match mode {
        OverlayDisclosureOpenMode::Uncontrolled => ComboboxOpenMode::Uncontrolled,
        OverlayDisclosureOpenMode::Controlled => ComboboxOpenMode::Controlled,
    }
}

/// Selection payload emitted by a combobox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComboboxSelection {
    value: String,
    label: String,
}

impl ComboboxSelection {
    /// Creates a selection payload.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }

    /// Returns selected value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns selected label.
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Resolved combobox state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct ComboboxState {
    size: Size,
    disabled: bool,
    required: bool,
    open: bool,
    default_open: bool,
    open_mode: ComboboxOpenMode,
    label: String,
    placeholder: String,
    query: String,
    selected_value: Option<String>,
    selected_label: Option<String>,
    total_option_count: usize,
    filtered_option_count: usize,
    empty_label: String,
    placement_side: OverlayPlacementSide,
    placement_alignment: OverlayPlacementAlignment,
    outside_press_policy: OutsidePressPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    input: TextInputState,
    listbox: ListboxState,
    scroll_area: ScrollAreaState,
    metrics: ComboboxMetrics,
    colors: ComboboxColors,
    focus_ring: FocusRing,
    overlay: OverlayResolvedState,
}

/// Inputs used to resolve public combobox state.
#[derive(Debug, Clone)]
pub struct ComboboxStateRequest {
    /// Control size.
    pub size: Size,
    /// Whether interaction is disabled.
    pub disabled: bool,
    /// Whether a value is required.
    pub required: bool,
    /// Controlled open value, when caller-owned.
    pub open: Option<bool>,
    /// Adapter-owned initial open value.
    pub default_open: bool,
    /// Accessible combobox label.
    pub label: String,
    /// Input placeholder text.
    pub placeholder: String,
    /// Current query text.
    pub query: String,
    /// Controlled selected option value.
    pub selected_value: Option<String>,
    /// Controlled active option value.
    pub active_value: Option<String>,
    /// Empty result label.
    pub empty_label: String,
    /// Grouped option descriptors.
    pub groups: Vec<ComboboxGroupDescriptor>,
    /// Standalone option descriptors.
    pub options: Vec<ComboboxOptionDescriptor>,
    /// Preferred overlay placement side.
    pub placement_side: OverlayPlacementSide,
    /// Preferred overlay placement alignment.
    pub placement_alignment: OverlayPlacementAlignment,
    /// Outside press dismissal policy.
    pub outside_press_policy: OutsidePressPolicy,
    /// Initial focus policy when opening.
    pub initial_focus_intent: InitialFocusIntent,
    /// Focus restore policy when closing.
    pub focus_restore_intent: FocusRestoreIntent,
    /// Theme token bundle.
    pub tokens: ThemeTokens,
}

impl ComboboxState {
    /// Resolves public state for a combobox.
    pub fn resolve(request: ComboboxStateRequest) -> Self {
        let ComboboxStateRequest {
            size,
            disabled,
            required,
            open,
            default_open,
            label,
            placeholder,
            query,
            selected_value,
            active_value,
            empty_label,
            groups,
            options,
            placement_side,
            placement_alignment,
            outside_press_policy,
            initial_focus_intent,
            focus_restore_intent,
            tokens,
        } = request;
        let query = TextEditingPolicy::single_line().normalize_text(query.as_str());
        let disclosure = OverlayDisclosureConfig::new(OverlayLayerKind::NonModalDismissible)
            .controlled_open(open)
            .default_open(default_open)
            .disabled(disabled)
            .outside_press_policy(outside_press_policy)
            .initial_focus_intent(initial_focus_intent.clone())
            .focus_restore_intent(focus_restore_intent.clone())
            .resolve();
        let open = disclosure.open();
        let open_mode = combobox_open_mode_from_disclosure(disclosure.open_mode());
        let normalized_query = choice::normalize_query(query.as_str());
        let raw_groups = groups;
        let raw_options = options;
        let total_option_count = raw_options.len()
            + raw_groups
                .iter()
                .map(|group| group.options_ref().len())
                .sum::<usize>();
        let raw_collection = ChoiceCollection::resolve_unique(
            false,
            flatten_combobox_choice_options(&raw_groups, &raw_options),
            selected_value.as_deref(),
            active_value.as_deref(),
            ChoiceInteractionPolicy::listbox(),
        );
        let ambiguous_values = raw_collection
            .items()
            .iter()
            .filter(|option| option.ambiguous_value())
            .map(|option| option.value().to_owned())
            .collect::<BTreeSet<_>>();
        let filtered_options = raw_options
            .iter()
            .filter(|option| option.matches_normalized_query(normalized_query.as_str()))
            .map(|option| {
                option
                    .to_listbox_descriptor()
                    .disabled(option.disabled_state() || ambiguous_values.contains(option.value()))
            })
            .collect::<Vec<_>>();
        let filtered_groups = raw_groups
            .iter()
            .filter_map(|group| {
                let options = group
                    .options_ref()
                    .iter()
                    .filter(|option| option.matches_normalized_query(normalized_query.as_str()))
                    .map(|option| {
                        option.to_listbox_descriptor().disabled(
                            option.disabled_state() || ambiguous_values.contains(option.value()),
                        )
                    })
                    .collect::<Vec<_>>();
                (!options.is_empty()).then(|| {
                    ListboxGroupDescriptor::new(group.value().to_owned(), group.label().to_owned())
                        .options(options)
                })
            })
            .collect::<Vec<_>>();
        let filtered_option_count = filtered_options.len()
            + filtered_groups
                .iter()
                .map(|group| group.options_ref().len())
                .sum::<usize>();
        let selected_value = raw_collection.selected_value().map(str::to_owned);
        let selected_label = raw_collection
            .selected_item()
            .map(|item| item.label().to_owned());
        let listbox = ListboxState::resolve(
            size,
            disabled,
            label.clone(),
            selected_value.as_deref(),
            active_value.as_deref(),
            (!normalized_query.is_empty()).then_some(normalized_query.as_str()),
            empty_label.clone(),
            filtered_groups,
            filtered_options,
            tokens,
        );
        let input = TextInputState::resolve(
            query.clone(),
            Some(placeholder.clone()),
            size,
            disabled,
            false,
            false,
            required,
            false,
            tokens,
        );
        let overlay = disclosure.overlay().clone();
        let scroll_area = ScrollAreaState::resolve(
            format!("{label}:combobox-content-scroll"),
            ScrollAreaAxis::Vertical,
            size,
            ScrollResetPolicy::Preserve,
            None,
        );
        let colors = ThemeResolver::combobox_colors(tokens);

        Self {
            size,
            disabled,
            required,
            open,
            default_open,
            open_mode,
            label,
            placeholder,
            query,
            selected_value,
            selected_label,
            total_option_count,
            filtered_option_count,
            empty_label,
            placement_side,
            placement_alignment,
            outside_press_policy,
            initial_focus_intent,
            focus_restore_intent,
            input,
            listbox,
            scroll_area,
            metrics: ComboboxMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
            overlay,
        }
    }

    /// Returns foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether the combobox is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether a value is required.
    pub const fn required(&self) -> bool {
        self.required
    }

    /// Returns whether the popup is open.
    pub const fn open(&self) -> bool {
        self.open
    }

    /// Returns uncontrolled initial open state.
    pub const fn default_open(&self) -> bool {
        self.default_open
    }

    /// Returns open-state ownership.
    pub const fn open_mode(&self) -> ComboboxOpenMode {
        self.open_mode
    }

    /// Returns accessible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns placeholder text.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Returns current query text.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns selected option value.
    pub fn selected_value(&self) -> Option<&str> {
        self.selected_value.as_deref()
    }

    /// Returns the selected option label.
    pub fn selected_label(&self) -> Option<&str> {
        self.selected_label.as_deref()
    }

    /// Returns active option value.
    pub fn active_value(&self) -> Option<&str> {
        self.listbox.active_value()
    }

    /// Returns unfiltered option count.
    pub const fn total_option_count(&self) -> usize {
        self.total_option_count
    }

    /// Returns filtered option count.
    pub const fn filtered_option_count(&self) -> usize {
        self.filtered_option_count
    }

    /// Returns empty-state label.
    pub fn empty_label(&self) -> &str {
        &self.empty_label
    }

    /// Returns preferred placement side.
    pub const fn placement_side(&self) -> OverlayPlacementSide {
        self.placement_side
    }

    /// Returns preferred placement alignment.
    pub const fn placement_alignment(&self) -> OverlayPlacementAlignment {
        self.placement_alignment
    }

    /// Returns outside-press policy.
    pub const fn outside_press_policy(&self) -> OutsidePressPolicy {
        self.outside_press_policy
    }

    /// Returns initial focus intent.
    pub fn initial_focus_intent(&self) -> &InitialFocusIntent {
        &self.initial_focus_intent
    }

    /// Returns focus restore intent.
    pub fn focus_restore_intent(&self) -> &FocusRestoreIntent {
        &self.focus_restore_intent
    }

    /// Returns input role.
    pub const fn input_role(&self) -> Role {
        Role::EditableComboBox
    }

    /// Returns popup content role.
    pub const fn content_role(&self) -> Role {
        Role::ListBox
    }

    /// Returns whether query filtering removed options.
    pub const fn filtered(&self) -> bool {
        self.filtered_option_count != self.total_option_count
    }

    /// Returns whether the visible option list is empty.
    pub const fn empty(&self) -> bool {
        self.filtered_option_count == 0
    }

    /// Returns whether popup content should use a scroll viewport.
    pub const fn scrollable_content(&self) -> bool {
        self.listbox.scrollable_content()
    }

    /// Returns resolved input state.
    pub const fn input(&self) -> &TextInputState {
        &self.input
    }

    /// Returns nested listbox state.
    pub const fn listbox(&self) -> &ListboxState {
        &self.listbox
    }

    /// Returns nested scroll area state.
    pub const fn scroll_area(&self) -> &ScrollAreaState {
        &self.scroll_area
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> ComboboxMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> ComboboxColors {
        self.colors
    }

    /// Returns focus-ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }

    /// Returns renderer-neutral overlay state.
    pub const fn overlay(&self) -> &OverlayResolvedState {
        &self.overlay
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ComboboxKeyboardAction {
    Navigate(String),
    Select(ComboboxSelection),
    Open,
    Ignore,
}

pub(super) fn combobox_keyboard_action(state: &ComboboxState, key: &str) -> ComboboxKeyboardAction {
    if state.disabled() {
        return ComboboxKeyboardAction::Ignore;
    }

    if let Some(target) = state.listbox().navigation_target(key) {
        return ComboboxKeyboardAction::Navigate(target.value().to_owned());
    }

    if matches!(key, "down" | "up" | "home" | "end") {
        return ComboboxKeyboardAction::Open;
    }

    if let Some(selection) = state.listbox().activation_for_key(key) {
        return ComboboxKeyboardAction::Select(ComboboxSelection::new(
            selection.value().to_owned(),
            selection.label().to_owned(),
        ));
    }

    ComboboxKeyboardAction::Ignore
}

#[cfg(test)]
mod tests {
    use crate::combobox::{Combobox, ComboboxOption};

    use super::*;

    fn keyboard_state(disabled: bool) -> ComboboxState {
        Combobox::new("frameworks", "Frameworks")
            .open(true)
            .disabled(disabled)
            .default_query("re")
            .selected(Some("solid".to_owned()))
            .option(ComboboxOption::new("react", "React"))
            .option(ComboboxOption::new("solid", "Solid"))
            .option(ComboboxOption::new("relay", "Relay"))
            .state()
    }

    #[test]
    fn keyboard_action_moves_and_selects_active_option() {
        let state = keyboard_state(false);

        assert_eq!(
            combobox_keyboard_action(&state, "down"),
            ComboboxKeyboardAction::Navigate("relay".to_string())
        );
        assert_eq!(
            combobox_keyboard_action(&state, "enter"),
            ComboboxKeyboardAction::Select(ComboboxSelection::new(
                "react".to_string(),
                "React".to_string(),
            ))
        );
    }

    #[test]
    fn keyboard_action_ignores_disabled_combobox() {
        let state = keyboard_state(true);

        assert_eq!(
            combobox_keyboard_action(&state, "down"),
            ComboboxKeyboardAction::Ignore
        );
        assert_eq!(
            combobox_keyboard_action(&state, "enter"),
            ComboboxKeyboardAction::Ignore
        );
    }
}
