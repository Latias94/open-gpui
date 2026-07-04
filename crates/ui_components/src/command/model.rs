//! Renderer-neutral command descriptors, state, and behavior inputs.

use super::descriptor::{
    CommandGroupDescriptor, CommandIndexSnapshot, CommandIndexSnapshotMode, CommandItemDescriptor,
    CommandLoadingState, CommandMatchRank, CommandMatchSource, CommandOpenMode, CommandQueryMode,
    CommandSelectionMode, CommandStatusIntent, CommandStatusItem, command_choice_items,
    command_choice_selection_mode, command_item_rank_for_source, command_open_mode_from_disclosure,
    count_command_status_items,
};
use super::style::{CommandColors, CommandMetrics};
use crate::choice::{self, ChoiceCollection, ChoiceInteractionPolicy};
use crate::focus::FocusRing;
use crate::listbox::{ListboxGroupDescriptor, ListboxState};
use crate::overlay::{OverlayDisclosureConfig, OverlayResolvedState};
use crate::scroll_area::{ScrollAreaAxis, ScrollAreaState};
use crate::text_editing::TextEditingPolicy;
use crate::text_input::TextInputState;
use crate::theme::ThemeResolver;
use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayLayerKind,
    Role, Size, ThemeTokens,
};

/// Keyboard navigation behavior for a command surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandNavigationBehavior {
    loop_navigation: bool,
    group_navigation: bool,
}

impl CommandNavigationBehavior {
    /// Creates default command navigation behavior.
    pub const fn new() -> Self {
        Self {
            loop_navigation: true,
            group_navigation: true,
        }
    }

    /// Returns the same behavior with loop navigation enabled or disabled.
    pub const fn with_loop_navigation(mut self, enabled: bool) -> Self {
        self.loop_navigation = enabled;
        self
    }

    /// Returns the same behavior with group navigation enabled or disabled.
    pub const fn with_group_navigation(mut self, enabled: bool) -> Self {
        self.group_navigation = enabled;
        self
    }

    /// Returns whether Up/Down navigation wraps across list boundaries.
    pub const fn loop_navigation(self) -> bool {
        self.loop_navigation
    }

    /// Returns whether group-jump key aliases are enabled.
    pub const fn group_navigation(self) -> bool {
        self.group_navigation
    }
}

impl Default for CommandNavigationBehavior {
    fn default() -> Self {
        Self::new()
    }
}

/// Selection payload emitted by a command surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSelection {
    index: usize,
    value: String,
    label: String,
    shortcut: Option<String>,
}

impl CommandSelection {
    /// Creates a command selection payload.
    pub fn new(
        index: usize,
        value: impl Into<String>,
        label: impl Into<String>,
        shortcut: Option<String>,
    ) -> Self {
        Self {
            index,
            value: value.into(),
            label: label.into(),
            shortcut,
        }
    }

    /// Creates a selection payload from an item state.
    pub fn from_item(item: &CommandItemState) -> Option<Self> {
        item.activation_enabled().then(|| {
            Self::new(
                item.index,
                item.value.clone(),
                item.label.clone(),
                item.shortcut.clone(),
            )
        })
    }

    /// Returns the flattened item index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns selected item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns selected item label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns optional shortcut label.
    pub fn shortcut(&self) -> Option<&str> {
        self.shortcut.as_deref()
    }
}

/// Persistent selected-values change emitted by multi-select command surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSelectionChange {
    values: Vec<String>,
    toggled: CommandSelection,
    selected: bool,
}

impl CommandSelectionChange {
    /// Creates a multi-selection change payload.
    pub fn new(values: Vec<String>, toggled: CommandSelection, selected: bool) -> Self {
        Self {
            values,
            toggled,
            selected,
        }
    }

    /// Returns the next selected values.
    pub fn values(&self) -> &[String] {
        &self.values
    }

    /// Returns the command selection that was toggled.
    pub const fn toggled(&self) -> &CommandSelection {
        &self.toggled
    }

    /// Returns whether the toggled value is now selected.
    pub const fn selected(&self) -> bool {
        self.selected
    }
}

/// Resolved command group state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandGroupState {
    index: usize,
    value: String,
    label: String,
    item_count: usize,
    standalone: bool,
    match_score: u16,
}

impl CommandGroupState {
    /// Returns group index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns stable group value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns visible group label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns visible item count.
    pub const fn item_count(&self) -> usize {
        self.item_count
    }

    /// Returns the deterministic search score for this group.
    pub const fn match_score(&self) -> u16 {
        self.match_score
    }

    /// Returns whether this is the synthetic standalone command group.
    pub const fn standalone(&self) -> bool {
        self.standalone
    }

    /// Returns group accessibility role.
    pub const fn role(&self) -> Role {
        Role::Group
    }
}

/// Resolved selected chip state for multi-select command surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSelectedChipState {
    index: usize,
    value: String,
    label: String,
}

impl CommandSelectedChipState {
    /// Returns chip index in the selected-values list.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns selected command value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns selected command label.
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Resolved command item state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandItemState {
    index: usize,
    group_index: Option<usize>,
    value: String,
    label: String,
    shortcut: Option<String>,
    when: Option<String>,
    disabled: bool,
    disabled_reason: Option<String>,
    selected: bool,
    active: bool,
    match_source: Option<CommandMatchSource>,
    match_score: u16,
    position_in_set: Option<usize>,
    size_of_set: usize,
}

impl CommandItemState {
    /// Returns flattened item index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns containing group index when grouped.
    pub const fn group_index(&self) -> Option<usize> {
        self.group_index
    }

    /// Returns stable item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns visible item label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns optional shortcut label.
    pub fn shortcut(&self) -> Option<&str> {
        self.shortcut.as_deref()
    }

    /// Returns caller-owned availability metadata.
    pub fn when_ref(&self) -> Option<&str> {
        self.when.as_deref()
    }

    /// Returns whether the item is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns the optional disabled reason.
    pub fn disabled_reason_ref(&self) -> Option<&str> {
        self.disabled_reason.as_deref()
    }

    /// Returns whether the item can be activated.
    pub const fn activation_enabled(&self) -> bool {
        !self.disabled
    }

    /// Returns whether the item is selected.
    pub const fn selected(&self) -> bool {
        self.selected
    }

    /// Returns whether the item is active.
    pub const fn active(&self) -> bool {
        self.active
    }

    /// Returns the descriptor field that produced the strongest query match.
    pub const fn match_source(&self) -> Option<CommandMatchSource> {
        self.match_source
    }

    /// Returns the deterministic search score for this item.
    pub const fn match_score(&self) -> u16 {
        self.match_score
    }

    /// Returns the item's accessibility role.
    pub const fn role(&self) -> Role {
        Role::ListBoxOption
    }

    /// Returns one-based position among command items.
    pub const fn position_in_set(&self) -> Option<usize> {
        self.position_in_set
    }

    /// Returns total command item count in the visible set.
    pub const fn size_of_set(&self) -> usize {
        self.size_of_set
    }
}

/// Dialog wrapper state for command surfaces.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandDialogState {
    enabled: bool,
    open: bool,
    title: String,
    description: Option<String>,
    overlay: OverlayResolvedState,
}

impl CommandDialogState {
    /// Returns whether this command is presented as a dialog.
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    /// Returns whether the dialog is open.
    pub const fn open(&self) -> bool {
        self.open
    }

    /// Returns dialog role.
    pub const fn role(&self) -> Role {
        Role::Window
    }

    /// Returns dialog content role.
    pub const fn content_role(&self) -> Role {
        Role::Window
    }

    /// Returns dialog title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns optional dialog description.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Returns renderer-neutral overlay state.
    pub const fn overlay(&self) -> &OverlayResolvedState {
        &self.overlay
    }
}

#[derive(Debug, Clone)]
struct FlattenedCommandItem {
    group_index: Option<usize>,
    descriptor: CommandItemDescriptor,
    rank: CommandMatchRank,
    source_index: usize,
}

#[derive(Debug, Clone)]
struct RankedCommandGroup {
    source_index: usize,
    value: String,
    label: String,
    items: Vec<FlattenedCommandItem>,
    best_score: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommandDataSource {
    revision: Option<String>,
    mode: CommandIndexSnapshotMode,
    loading_state: Option<CommandLoadingState>,
    groups: Vec<CommandGroupDescriptor>,
    items: Vec<CommandItemDescriptor>,
}

impl CommandDataSource {
    fn local(
        groups: impl IntoIterator<Item = CommandGroupDescriptor>,
        items: impl IntoIterator<Item = CommandItemDescriptor>,
    ) -> Self {
        Self {
            revision: None,
            mode: CommandIndexSnapshotMode::LocalRanked,
            loading_state: None,
            groups: groups.into_iter().collect(),
            items: items.into_iter().collect(),
        }
    }

    fn snapshot(snapshot: CommandIndexSnapshot) -> Self {
        Self {
            revision: Some(snapshot.revision),
            mode: snapshot.mode,
            loading_state: snapshot.loading_state,
            groups: snapshot.groups,
            items: snapshot.items,
        }
    }
}

/// Command result descriptors supplied to [`CommandStateRequest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandStateDataSource {
    /// Locally supplied grouped and standalone command descriptors.
    Local {
        /// Local grouped command descriptors.
        groups: Vec<CommandGroupDescriptor>,
        /// Local standalone command descriptors.
        items: Vec<CommandItemDescriptor>,
    },
    /// Caller-owned indexed command snapshot.
    Snapshot(CommandIndexSnapshot),
}

impl CommandStateDataSource {
    /// Creates a local command data source.
    pub fn local(
        groups: impl IntoIterator<Item = CommandGroupDescriptor>,
        items: impl IntoIterator<Item = CommandItemDescriptor>,
    ) -> Self {
        Self::Local {
            groups: groups.into_iter().collect(),
            items: items.into_iter().collect(),
        }
    }

    /// Creates a snapshot-backed command data source.
    pub fn snapshot(snapshot: CommandIndexSnapshot) -> Self {
        Self::Snapshot(snapshot)
    }

    fn into_private(self) -> CommandDataSource {
        match self {
            Self::Local { groups, items } => CommandDataSource::local(groups, items),
            Self::Snapshot(snapshot) => CommandDataSource::snapshot(snapshot),
        }
    }
}

/// Resolved command state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandState {
    size: Size,
    disabled: bool,
    label: String,
    placeholder: String,
    query: String,
    query_mode: CommandQueryMode,
    open: bool,
    default_open: bool,
    open_mode: CommandOpenMode,
    navigation_behavior: CommandNavigationBehavior,
    selection_mode: CommandSelectionMode,
    overlay: OverlayResolvedState,
    dialog: Option<CommandDialogState>,
    loading_state: Option<CommandLoadingState>,
    status_items: Vec<CommandStatusItem>,
    index_revision: Option<String>,
    index_mode: CommandIndexSnapshotMode,
    empty_label: String,
    escape_key_policy: EscapeKeyPolicy,
    focus_restore_intent: FocusRestoreIntent,
    total_item_count: usize,
    filtered_item_count: usize,
    groups: Vec<CommandGroupState>,
    items: Vec<CommandItemState>,
    selected_values: Vec<String>,
    selected_chips: Vec<CommandSelectedChipState>,
    input: TextInputState,
    listbox: ListboxState,
    scroll_area: ScrollAreaState,
    metrics: CommandMetrics,
    colors: CommandColors,
    focus_ring: FocusRing,
}

/// Inputs used to resolve public command state.
#[derive(Debug, Clone)]
pub struct CommandStateRequest {
    /// Control size.
    pub size: Size,
    /// Whether interaction is disabled.
    pub disabled: bool,
    /// Controlled open value, when caller-owned.
    pub open: Option<bool>,
    /// Adapter-owned initial open value.
    pub default_open: bool,
    /// Whether the command surface resolves dialog overlay metadata.
    pub dialog_enabled: bool,
    /// Accessible command label.
    pub label: String,
    /// Search input placeholder.
    pub placeholder: String,
    /// Current query text.
    pub query: String,
    /// Query ownership mode.
    pub query_mode: CommandQueryMode,
    /// Selection ownership mode.
    pub selection_mode: CommandSelectionMode,
    /// Controlled single selected value.
    pub selected_value: Option<String>,
    /// Controlled selected values for multi-select mode.
    pub selected_values: Vec<String>,
    /// Controlled active value.
    pub active_value: Option<String>,
    /// Loading state supplied outside an index snapshot.
    pub loading_state: Option<CommandLoadingState>,
    /// Empty result label.
    pub empty_label: String,
    /// Dialog title when dialog mode is enabled.
    pub dialog_title: Option<String>,
    /// Dialog description when dialog mode is enabled.
    pub dialog_description: Option<String>,
    /// Command descriptors or indexed snapshot used to resolve results.
    pub data_source: CommandStateDataSource,
    /// Outside press dismissal policy.
    pub outside_press_policy: OutsidePressPolicy,
    /// Escape key dismissal policy.
    pub escape_key_policy: EscapeKeyPolicy,
    /// Initial focus policy when opening.
    pub initial_focus_intent: InitialFocusIntent,
    /// Focus restore policy when closing.
    pub focus_restore_intent: FocusRestoreIntent,
    /// Theme token bundle.
    pub tokens: ThemeTokens,
}

impl CommandState {
    /// Resolves public state for a command surface.
    pub fn resolve(request: CommandStateRequest) -> Self {
        let CommandStateRequest {
            size,
            disabled,
            open,
            default_open,
            dialog_enabled,
            label,
            placeholder,
            query,
            query_mode,
            selection_mode,
            selected_value,
            selected_values,
            active_value,
            loading_state,
            empty_label,
            dialog_title,
            dialog_description,
            data_source,
            outside_press_policy,
            escape_key_policy,
            initial_focus_intent,
            focus_restore_intent,
            tokens,
        } = request;

        Self::resolve_from_data_source(
            size,
            disabled,
            open,
            default_open,
            dialog_enabled,
            label,
            placeholder,
            query,
            query_mode,
            selection_mode,
            selected_value.as_deref(),
            selected_values,
            active_value.as_deref(),
            loading_state,
            empty_label,
            dialog_title,
            dialog_description,
            data_source.into_private(),
            outside_press_policy,
            escape_key_policy,
            initial_focus_intent,
            focus_restore_intent,
            tokens,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_from_data_source(
        size: Size,
        disabled: bool,
        open: Option<bool>,
        default_open: bool,
        dialog_enabled: bool,
        label: impl Into<String>,
        placeholder: impl Into<String>,
        query: impl Into<String>,
        query_mode: CommandQueryMode,
        selection_mode: CommandSelectionMode,
        selected_value: Option<&str>,
        selected_values: impl IntoIterator<Item = impl Into<String>>,
        active_value: Option<&str>,
        loading_state: Option<CommandLoadingState>,
        empty_label: impl Into<String>,
        dialog_title: Option<String>,
        dialog_description: Option<String>,
        data_source: CommandDataSource,
        outside_press_policy: OutsidePressPolicy,
        escape_key_policy: EscapeKeyPolicy,
        initial_focus_intent: InitialFocusIntent,
        focus_restore_intent: FocusRestoreIntent,
        tokens: ThemeTokens,
    ) -> Self {
        let label = label.into();
        let placeholder = placeholder.into();
        let query = query.into();
        let query = TextEditingPolicy::single_line().normalize_text(query.as_str());
        let empty_label = empty_label.into();
        let disclosure = OverlayDisclosureConfig::new(OverlayLayerKind::NonModalDismissible)
            .controlled_open(open)
            .default_open(default_open)
            .disabled(disabled)
            .outside_press_policy(outside_press_policy)
            .escape_key_policy(escape_key_policy)
            .initial_focus_intent(initial_focus_intent.clone())
            .focus_restore_intent(focus_restore_intent.clone())
            .resolve();
        let open = disclosure.open();
        let open_mode = command_open_mode_from_disclosure(disclosure.open_mode());
        let normalized_query = choice::normalize_query(query.as_str());
        let query_is_empty = normalized_query.is_empty();
        let CommandDataSource {
            revision: index_revision,
            mode: index_mode,
            loading_state: index_loading_state,
            groups: raw_groups,
            items: raw_items,
        } = data_source;
        let loading_state = index_loading_state.or(loading_state);
        let total_item_count = raw_items.len()
            + raw_groups
                .iter()
                .map(|group| group.items_ref().len())
                .sum::<usize>();
        let raw_choice_items = command_choice_items(&raw_groups, &raw_items);
        let selection_mode_policy = command_choice_selection_mode(selection_mode);
        let raw_collection = ChoiceCollection::resolve(
            false,
            raw_choice_items.clone(),
            selected_value,
            active_value,
            ChoiceInteractionPolicy::listbox().with_selection_mode(selection_mode_policy),
        );
        let selected_value = raw_collection.selected_value().map(str::to_owned);
        let selected_values = choice::resolve_projected_selected_values(
            selection_mode_policy,
            &raw_choice_items,
            selected_value.as_deref(),
            selected_values,
        );
        let listbox_selected_value = (!selection_mode.is_multiple())
            .then_some(selected_value.as_deref())
            .flatten();

        let mut standalone_items = raw_items
            .iter()
            .enumerate()
            .filter_map(|(source_index, item)| {
                let rank =
                    command_item_rank_for_source(item, normalized_query.as_str(), index_mode)?;
                Some(FlattenedCommandItem {
                    group_index: None,
                    descriptor: item.clone(),
                    rank,
                    source_index,
                })
            })
            .collect::<Vec<_>>();
        if index_mode.should_rank_locally(query_is_empty) {
            sort_ranked_command_items(&mut standalone_items);
        }

        let mut ranked_groups = raw_groups
            .iter()
            .enumerate()
            .filter_map(|(group_source_index, group)| {
                let mut items = group
                    .items_ref()
                    .iter()
                    .enumerate()
                    .filter_map(|(source_index, item)| {
                        let rank = command_item_rank_for_source(
                            item,
                            normalized_query.as_str(),
                            index_mode,
                        )?;
                        Some(FlattenedCommandItem {
                            group_index: None,
                            descriptor: item.clone(),
                            rank,
                            source_index,
                        })
                    })
                    .collect::<Vec<_>>();
                if items.is_empty() {
                    return None;
                }

                if index_mode.should_rank_locally(query_is_empty) {
                    sort_ranked_command_items(&mut items);
                }

                let best_score = items.iter().map(|item| item.rank.score).max().unwrap_or(0);

                Some(RankedCommandGroup {
                    source_index: group_source_index,
                    value: group.value().to_owned(),
                    label: group.label().to_owned(),
                    items,
                    best_score,
                })
            })
            .collect::<Vec<_>>();
        if index_mode.should_rank_locally(query_is_empty) {
            ranked_groups.sort_by(|a, b| {
                b.best_score
                    .cmp(&a.best_score)
                    .then_with(|| a.source_index.cmp(&b.source_index))
            });
        }

        let mut filtered_group_descriptors = Vec::new();
        let mut filtered_item_descriptors = Vec::new();
        let mut command_groups = Vec::new();
        let mut flattened = Vec::new();
        let standalone_best_score = standalone_items
            .iter()
            .map(|item| item.rank.score)
            .max()
            .unwrap_or(0);

        if !standalone_items.is_empty() {
            let group_index = command_groups.len();
            command_groups.push(CommandGroupState {
                index: group_index,
                value: "commands".to_string(),
                label: "Commands".to_string(),
                item_count: standalone_items.len(),
                standalone: true,
                match_score: standalone_best_score,
            });
            filtered_item_descriptors = standalone_items
                .iter()
                .map(|item| item.descriptor.to_listbox_descriptor())
                .collect::<Vec<_>>();
            for item in &mut standalone_items {
                item.group_index = Some(group_index);
            }
            flattened.extend(standalone_items);
        }

        for group in ranked_groups {
            let group_index = command_groups.len();
            command_groups.push(CommandGroupState {
                index: group_index,
                value: group.value.clone(),
                label: group.label.clone(),
                item_count: group.items.len(),
                standalone: false,
                match_score: group.best_score,
            });
            filtered_group_descriptors.push(
                ListboxGroupDescriptor::new(group.value.clone(), group.label.clone()).options(
                    group
                        .items
                        .iter()
                        .map(|item| item.descriptor.to_listbox_descriptor()),
                ),
            );
            flattened.extend(group.items.into_iter().map(|mut item| {
                item.group_index = Some(group_index);
                item
            }));
        }

        let filtered_item_count = flattened.len();
        let listbox = ListboxState::resolve(
            size,
            disabled,
            label.clone(),
            listbox_selected_value,
            active_value,
            (!query_is_empty).then_some(normalized_query.as_str()),
            empty_label.clone(),
            filtered_group_descriptors,
            filtered_item_descriptors,
            tokens,
        );
        let items = flattened
            .into_iter()
            .enumerate()
            .filter_map(|(index, item)| {
                let option = listbox.options().get(index)?;
                let selected = if selection_mode.is_multiple() {
                    selected_values
                        .iter()
                        .any(|value| value == item.descriptor.value())
                } else {
                    option.selected()
                };
                Some(CommandItemState {
                    index,
                    group_index: item.group_index,
                    value: item.descriptor.value,
                    label: item.descriptor.label,
                    shortcut: item.descriptor.shortcut,
                    when: item.descriptor.when,
                    disabled: item.descriptor.disabled,
                    disabled_reason: item.descriptor.disabled_reason,
                    selected,
                    active: option.active(),
                    match_source: item.rank.source,
                    match_score: item.rank.score,
                    position_in_set: option.position_in_set(),
                    size_of_set: option.size_of_set(),
                })
            })
            .collect::<Vec<_>>();
        let selected_chips = selected_values
            .iter()
            .enumerate()
            .filter_map(|(index, value)| {
                choice::find_value_in_flat_groups(
                    &raw_items,
                    raw_groups.iter().map(|group| group.items_ref()),
                    value,
                    CommandItemDescriptor::value,
                )
                .map(|item| CommandSelectedChipState {
                    index,
                    value: item.value().to_owned(),
                    label: item.label().to_owned(),
                })
            })
            .collect::<Vec<_>>();
        let input = TextInputState::resolve(
            query.clone(),
            Some(placeholder.clone()),
            size,
            disabled,
            false,
            false,
            false,
            true,
            tokens,
        );
        let overlay = OverlayDisclosureConfig::new(OverlayLayerKind::NonModalDismissible)
            .controlled_open(Some(open))
            .openable(dialog_enabled)
            .outside_press_policy(outside_press_policy)
            .escape_key_policy(escape_key_policy)
            .initial_focus_intent(initial_focus_intent.clone())
            .focus_restore_intent(focus_restore_intent.clone())
            .resolve()
            .overlay()
            .clone();
        let dialog_overlay = OverlayDisclosureConfig::new(OverlayLayerKind::Modal)
            .controlled_open(Some(open))
            .openable(dialog_enabled)
            .outside_press_policy(outside_press_policy)
            .escape_key_policy(escape_key_policy)
            .initial_focus_intent(initial_focus_intent)
            .focus_restore_intent(focus_restore_intent.clone())
            .resolve()
            .overlay()
            .clone();
        let dialog = dialog_enabled.then(|| CommandDialogState {
            enabled: true,
            open,
            title: dialog_title.unwrap_or_else(|| label.clone()),
            description: dialog_description,
            overlay: dialog_overlay,
        });
        let scroll_area = ScrollAreaState::resolve(
            format!("{label}:command-list-scroll"),
            ScrollAreaAxis::Vertical,
            size,
            crate::scroll_area::ScrollResetPolicy::Preserve,
            None,
        );
        let colors = ThemeResolver::command_colors(tokens);

        Self {
            size,
            disabled,
            label,
            placeholder,
            query,
            query_mode,
            open,
            default_open,
            open_mode,
            navigation_behavior: CommandNavigationBehavior::default(),
            selection_mode,
            overlay,
            dialog,
            loading_state,
            status_items: Vec::new(),
            index_revision,
            index_mode,
            empty_label,
            escape_key_policy,
            focus_restore_intent,
            total_item_count,
            filtered_item_count,
            groups: command_groups,
            items,
            selected_values,
            selected_chips,
            input,
            listbox,
            scroll_area,
            metrics: CommandMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
        }
    }

    /// Returns foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether the command surface is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns accessible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns placeholder text.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Returns current search query.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns query ownership.
    pub const fn query_mode(&self) -> CommandQueryMode {
        self.query_mode
    }

    /// Returns selected command value.
    pub fn selected_value(&self) -> Option<&str> {
        self.listbox.selected_value()
    }

    /// Returns active command value.
    pub fn active_value(&self) -> Option<&str> {
        self.listbox.active_value()
    }

    /// Returns whether the dialog wrapper is open.
    pub const fn open(&self) -> bool {
        self.open
    }

    /// Returns uncontrolled initial open state.
    pub const fn default_open(&self) -> bool {
        self.default_open
    }

    /// Returns open-state ownership.
    pub const fn open_mode(&self) -> CommandOpenMode {
        self.open_mode
    }

    /// Returns keyboard navigation behavior.
    pub const fn navigation_behavior(&self) -> CommandNavigationBehavior {
        self.navigation_behavior
    }

    /// Returns whether Up/Down navigation wraps across list boundaries.
    pub const fn loop_navigation(&self) -> bool {
        self.navigation_behavior.loop_navigation()
    }

    /// Returns whether group-jump key aliases are enabled.
    pub const fn group_navigation(&self) -> bool {
        self.navigation_behavior.group_navigation()
    }

    /// Returns selection behavior.
    pub const fn selection_mode(&self) -> CommandSelectionMode {
        self.selection_mode
    }

    /// Returns dialog wrapper state.
    pub const fn overlay(&self) -> &OverlayResolvedState {
        &self.overlay
    }

    /// Returns optional dialog wrapper state.
    pub const fn dialog(&self) -> Option<&CommandDialogState> {
        self.dialog.as_ref()
    }

    /// Returns loading state.
    pub const fn loading_state(&self) -> Option<&CommandLoadingState> {
        self.loading_state.as_ref()
    }

    /// Returns optional loading metadata.
    pub const fn loading(&self) -> Option<&CommandLoadingState> {
        self.loading_state.as_ref()
    }

    /// Returns UI-ready command palette status items.
    pub fn status_items(&self) -> &[CommandStatusItem] {
        &self.status_items
    }

    /// Returns whether provider or shortcut status should be displayed.
    pub fn has_status_items(&self) -> bool {
        !self.status_items.is_empty()
    }

    /// Returns the number of warning status items.
    pub fn status_warning_count(&self) -> usize {
        count_command_status_items(&self.status_items, CommandStatusIntent::Warning)
    }

    /// Returns the number of error status items.
    pub fn status_error_count(&self) -> usize {
        count_command_status_items(&self.status_items, CommandStatusIntent::Error)
    }

    /// Returns caller-owned command index revision metadata.
    pub fn index_revision(&self) -> Option<&str> {
        self.index_revision.as_deref()
    }

    /// Returns command index ordering/filtering semantics.
    pub const fn index_mode(&self) -> CommandIndexSnapshotMode {
        self.index_mode
    }

    /// Returns empty-state label.
    pub fn empty_label(&self) -> &str {
        &self.empty_label
    }

    /// Returns Escape key policy.
    pub const fn escape_key_policy(&self) -> EscapeKeyPolicy {
        self.escape_key_policy
    }

    /// Returns focus restore intent.
    pub fn focus_restore_intent(&self) -> &FocusRestoreIntent {
        &self.focus_restore_intent
    }

    /// Returns unfiltered command count.
    pub const fn total_item_count(&self) -> usize {
        self.total_item_count
    }

    /// Returns filtered command count.
    pub const fn filtered_item_count(&self) -> usize {
        self.filtered_item_count
    }

    /// Returns whether the visible list is empty.
    pub const fn empty(&self) -> bool {
        self.filtered_item_count == 0
    }

    /// Returns whether query filtering removed commands.
    pub const fn filtered(&self) -> bool {
        self.filtered_item_count != self.total_item_count
    }

    /// Returns whether command content should be rendered.
    pub const fn content_visible(&self) -> bool {
        self.dialog.is_none() || self.open
    }

    /// Returns input role.
    pub const fn input_role(&self) -> Role {
        Role::TextInput
    }

    /// Returns list role.
    pub const fn list_role(&self) -> Role {
        Role::ListBox
    }

    /// Returns list role.
    pub const fn content_role(&self) -> Role {
        self.list_role()
    }

    /// Returns resolved group states.
    pub fn groups(&self) -> &[CommandGroupState] {
        &self.groups
    }

    /// Returns resolved standalone command items.
    pub fn standalone_items(&self) -> impl Iterator<Item = &CommandItemState> + '_ {
        let standalone_group_index = self.groups.iter().find(|group| group.standalone());
        self.items
            .iter()
            .filter(move |item| match standalone_group_index {
                Some(group) => item.group_index() == Some(group.index()),
                None => item.group_index().is_none(),
            })
    }

    /// Returns resolved non-synthetic command groups.
    pub fn grouped_groups(&self) -> impl Iterator<Item = &CommandGroupState> + '_ {
        self.groups.iter().filter(|group| !group.standalone())
    }

    /// Returns resolved items for one command group.
    pub fn group_items(&self, group_index: usize) -> impl Iterator<Item = &CommandItemState> + '_ {
        self.items
            .iter()
            .filter(move |item| item.group_index() == Some(group_index))
    }

    /// Returns resolved item states.
    pub fn items(&self) -> &[CommandItemState] {
        &self.items
    }

    /// Returns persistent selected values.
    pub fn selected_values(&self) -> &[String] {
        &self.selected_values
    }

    /// Returns selected chip states.
    pub fn selected_chips(&self) -> &[CommandSelectedChipState] {
        &self.selected_chips
    }

    /// Returns resolved input state.
    pub const fn input(&self) -> &TextInputState {
        &self.input
    }

    /// Returns nested listbox state.
    pub const fn listbox(&self) -> &ListboxState {
        &self.listbox
    }

    /// Returns scroll area state.
    pub const fn scroll_area(&self) -> &ScrollAreaState {
        &self.scroll_area
    }

    /// Returns metrics.
    pub const fn metrics(&self) -> CommandMetrics {
        self.metrics
    }

    /// Returns color intents.
    pub const fn colors(&self) -> CommandColors {
        self.colors
    }

    /// Returns focus-ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }

    /// Returns the same state with an adjusted metric bundle.
    pub const fn with_metrics(mut self, metrics: CommandMetrics) -> Self {
        self.metrics = metrics;
        self
    }

    /// Returns the same state with adjusted keyboard navigation behavior.
    pub const fn with_navigation_behavior(mut self, behavior: CommandNavigationBehavior) -> Self {
        self.navigation_behavior = behavior;
        self
    }

    /// Returns the same state with UI-ready status items.
    pub fn with_status_items(mut self, items: impl IntoIterator<Item = CommandStatusItem>) -> Self {
        self.status_items = items.into_iter().filter(|item| !item.is_empty()).collect();
        self
    }

    /// Resolves an activation payload for an APG-style activation key.
    pub fn activation_for_key(&self, key: &str) -> Option<CommandSelection> {
        if !matches!(key, "enter" | "space") {
            return None;
        }
        self.items
            .iter()
            .find(|item| item.active())
            .and_then(CommandSelection::from_item)
    }
}

fn sort_ranked_command_items(items: &mut [FlattenedCommandItem]) {
    items.sort_by(|a, b| {
        b.rank
            .score
            .cmp(&a.rank.score)
            .then_with(|| a.source_index.cmp(&b.source_index))
    });
}
