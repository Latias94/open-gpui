//! Command palette component built from search input, grouped command items, and listbox state.

mod render_plan;
mod runtime;

use crate::choice::{
    self, ChoiceCollection, ChoiceInteractionPolicy, ChoiceItemProjection, ChoiceSelectionMode,
};
use crate::geometry::{gpui_px_from_ui, ui_px_from_gpui};
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, ElementId, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, anchored, deferred, div, point, px,
};
use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayLayerKind,
    OverlayPresence, Role, Sizable, Size, ThemeTokens, UiPx, ui_px,
};

use crate::a11y::UiA11yElementExt;
use crate::color::{ColorIntent, ColorState};
use crate::focus::{FocusRing, focus_ring_shadow};
use crate::listbox::{ListboxGroupDescriptor, ListboxOptionDescriptor, ListboxState};
use crate::overlay::{
    GpuiOverlayAdapterConfig, OverlayResolvedState, emit_overlay_open_change, gpui_overlay_state,
    resolve_overlay_open_state, set_overlay_open,
};
use crate::scroll_area::{ScrollAreaAxis, ScrollAreaState};
use crate::text_input::adapter::TextInputController;
use crate::text_input::{TextInputDisplayMode, TextInputState};
use crate::theme::ThemeResolver;
pub use render_plan::{CommandRenderPlan, CommandRowRenderPlan};
use runtime::{
    CommandOpenChangeHandler, CommandQueryChangeHandler, CommandRuntime,
    CommandSelectedValuesChangeHandler, CommandSelectionHandler, command_content_element,
    command_dialog_layer_element, command_scroll_reset_key,
};

/// Command dialog open-state ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommandOpenMode {
    /// Open state is owned by the component adapter after initialization.
    #[default]
    Uncontrolled,
    /// Open state is provided by the caller.
    Controlled,
}

/// Command query ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommandQueryMode {
    /// Query is owned by the component adapter after initialization.
    #[default]
    Uncontrolled,
    /// Query is provided by the caller.
    Controlled,
}

/// Command selection behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommandSelectionMode {
    /// Activating a command emits an action selection and may close dialog content.
    #[default]
    Single,
    /// Activating a command toggles persistent selected values.
    Multiple,
}

impl CommandSelectionMode {
    const fn is_multiple(self) -> bool {
        matches!(self, Self::Multiple)
    }
}

/// How a caller-owned command index snapshot should be interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommandIndexSnapshotMode {
    /// Apply the local deterministic filter and ranking pipeline.
    #[default]
    LocalRanked,
    /// Apply local filtering, but preserve the caller's snapshot order.
    PreRankedFilter,
    /// Treat the snapshot as already filtered and ranked by the caller.
    PreFiltered,
}

impl CommandIndexSnapshotMode {
    const fn should_filter_locally(self, query_is_empty: bool) -> bool {
        !query_is_empty && !matches!(self, Self::PreFiltered)
    }

    const fn should_rank_locally(self, query_is_empty: bool) -> bool {
        !query_is_empty && matches!(self, Self::LocalRanked)
    }
}

/// Command loading state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandLoadingState {
    message: String,
    progress_percent: Option<u8>,
}

impl CommandLoadingState {
    /// Creates command loading metadata.
    pub fn new(message: impl Into<String>, progress_percent: Option<u8>) -> Self {
        Self {
            message: message.into(),
            progress_percent: progress_percent.map(|progress| progress.min(100)),
        }
    }

    /// Returns loading message text.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns optional progress percentage.
    pub const fn progress_percent(&self) -> Option<u8> {
        self.progress_percent
    }

    /// Returns loading accessibility role.
    pub const fn role(&self) -> Role {
        Role::ProgressIndicator
    }
}

/// Command descriptor field that produced the strongest search match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandMatchSource {
    /// The visible label matched the query.
    Label,
    /// The stable command value matched the query.
    Value,
    /// The displayed shortcut matched the query.
    Shortcut,
    /// One of the command keywords matched the query.
    Keyword,
}

impl CommandMatchSource {
    const fn base_score(self) -> u16 {
        match self {
            Self::Label => 3200,
            Self::Value => 3100,
            Self::Shortcut => 2000,
            Self::Keyword => 1000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CommandMatchRank {
    source: Option<CommandMatchSource>,
    score: u16,
}

impl CommandMatchRank {
    const fn unfiltered() -> Self {
        Self {
            source: None,
            score: 0,
        }
    }
}

/// Pure descriptor for one command item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandItemDescriptor {
    value: String,
    label: String,
    keywords: Vec<String>,
    shortcut: Option<String>,
    disabled: bool,
}

impl CommandItemDescriptor {
    /// Creates a selectable command item descriptor.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            keywords: Vec::new(),
            shortcut: None,
            disabled: false,
        }
    }

    /// Adds one filtering keyword.
    pub fn keyword(mut self, keyword: impl Into<String>) -> Self {
        self.keywords.push(keyword.into());
        self
    }

    /// Adds many filtering keywords.
    pub fn keywords(mut self, keywords: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.keywords.extend(keywords.into_iter().map(Into::into));
        self
    }

    /// Adds a display shortcut label.
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Marks the item as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Returns stable item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns visible item label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns filtering keywords.
    pub fn keywords_ref(&self) -> &[String] {
        &self.keywords
    }

    /// Returns the display shortcut label.
    pub fn shortcut_ref(&self) -> Option<&str> {
        self.shortcut.as_deref()
    }

    /// Returns whether the item is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }

    fn match_rank(&self, normalized_query: &str) -> Option<CommandMatchRank> {
        if normalized_query.is_empty() {
            return Some(CommandMatchRank::unfiltered());
        }

        let best = command_text_match_rank(
            self.label.as_str(),
            normalized_query,
            CommandMatchSource::Label,
        )
        .into_iter()
        .chain(command_text_match_rank(
            self.value.as_str(),
            normalized_query,
            CommandMatchSource::Value,
        ))
        .chain(self.shortcut.as_ref().and_then(|shortcut| {
            command_text_match_rank(
                shortcut.as_str(),
                normalized_query,
                CommandMatchSource::Shortcut,
            )
        }));

        let keyword_best = self
            .keywords
            .iter()
            .filter_map(|keyword| {
                command_text_match_rank(
                    keyword.as_str(),
                    normalized_query,
                    CommandMatchSource::Keyword,
                )
            })
            .max_by_key(|rank| rank.score);

        best.chain(keyword_best).max_by_key(|rank| rank.score)
    }

    fn to_listbox_descriptor(&self) -> ListboxOptionDescriptor {
        ListboxOptionDescriptor::option(self.value.clone(), self.label.clone())
            .disabled(self.disabled)
    }
}

/// Pure descriptor for one command group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandGroupDescriptor {
    value: String,
    label: String,
    items: Vec<CommandItemDescriptor>,
}

impl CommandGroupDescriptor {
    /// Creates an empty command group descriptor.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            items: Vec::new(),
        }
    }

    /// Adds one command item.
    pub fn item(mut self, item: CommandItemDescriptor) -> Self {
        self.items.push(item);
        self
    }

    /// Adds many command items.
    pub fn items(mut self, items: impl IntoIterator<Item = CommandItemDescriptor>) -> Self {
        self.items.extend(items);
        self
    }

    /// Returns stable group value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns visible group label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns group items.
    pub fn items_ref(&self) -> &[CommandItemDescriptor] {
        &self.items
    }
}

fn command_choice_selection_mode(mode: CommandSelectionMode) -> ChoiceSelectionMode {
    match mode {
        CommandSelectionMode::Single => ChoiceSelectionMode::Single,
        CommandSelectionMode::Multiple => ChoiceSelectionMode::Multiple,
    }
}

fn command_choice_items(
    groups: &[CommandGroupDescriptor],
    standalone_items: &[CommandItemDescriptor],
) -> Vec<ChoiceItemProjection<()>> {
    let mut items = standalone_items
        .iter()
        .enumerate()
        .map(|(source_index, item)| {
            let label = item.label().to_owned();
            ChoiceItemProjection::new(
                source_index,
                None,
                item.value(),
                label.clone(),
                item.disabled_state(),
                (),
            )
            .text_value(label)
        })
        .collect::<Vec<_>>();

    for (group_index, group) in groups.iter().enumerate() {
        items.extend(
            group
                .items_ref()
                .iter()
                .enumerate()
                .map(|(source_index, item)| {
                    let label = item.label().to_owned();
                    ChoiceItemProjection::new(
                        source_index,
                        Some(group_index),
                        item.value(),
                        label.clone(),
                        item.disabled_state(),
                        (),
                    )
                    .text_value(label)
                }),
        );
    }

    items
}

/// Caller-owned indexed command snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandIndexSnapshot {
    revision: String,
    mode: CommandIndexSnapshotMode,
    loading_state: Option<CommandLoadingState>,
    groups: Vec<CommandGroupDescriptor>,
    items: Vec<CommandItemDescriptor>,
}

impl CommandIndexSnapshot {
    /// Creates an empty command index snapshot for the given revision.
    pub fn new(revision: impl Into<String>) -> Self {
        Self {
            revision: revision.into(),
            mode: CommandIndexSnapshotMode::LocalRanked,
            loading_state: None,
            groups: Vec::new(),
            items: Vec::new(),
        }
    }

    /// Applies snapshot ordering/filtering semantics.
    pub fn mode(mut self, mode: CommandIndexSnapshotMode) -> Self {
        self.mode = mode;
        self
    }

    /// Applies loading metadata that belongs to this snapshot.
    pub fn loading(mut self, loading_state: CommandLoadingState) -> Self {
        self.loading_state = Some(loading_state);
        self
    }

    /// Clears snapshot loading metadata.
    pub fn idle(mut self) -> Self {
        self.loading_state = None;
        self
    }

    /// Adds one standalone command item descriptor.
    pub fn item(mut self, item: CommandItemDescriptor) -> Self {
        self.items.push(item);
        self
    }

    /// Adds many standalone command item descriptors.
    pub fn items(mut self, items: impl IntoIterator<Item = CommandItemDescriptor>) -> Self {
        self.items.extend(items);
        self
    }

    /// Adds one command group descriptor.
    pub fn group(mut self, group: CommandGroupDescriptor) -> Self {
        self.groups.push(group);
        self
    }

    /// Adds many command group descriptors.
    pub fn groups(mut self, groups: impl IntoIterator<Item = CommandGroupDescriptor>) -> Self {
        self.groups.extend(groups);
        self
    }

    /// Returns snapshot revision metadata.
    pub fn revision(&self) -> &str {
        &self.revision
    }

    /// Returns snapshot ordering/filtering semantics.
    pub const fn snapshot_mode(&self) -> CommandIndexSnapshotMode {
        self.mode
    }

    /// Returns snapshot loading metadata.
    pub const fn loading_state(&self) -> Option<&CommandLoadingState> {
        self.loading_state.as_ref()
    }

    /// Returns standalone command item descriptors.
    pub fn items_ref(&self) -> &[CommandItemDescriptor] {
        &self.items
    }

    /// Returns command group descriptors.
    pub fn groups_ref(&self) -> &[CommandGroupDescriptor] {
        &self.groups
    }
}

/// Resolved command color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandColors {
    surface: ColorIntent,
    foreground: ColorIntent,
    muted_foreground: ColorIntent,
    border: ColorIntent,
    shortcut_foreground: ColorIntent,
    focus_ring: ColorIntent,
}

impl CommandColors {
    /// Returns surface color intent.
    pub const fn surface(self) -> ColorIntent {
        self.surface
    }

    /// Returns foreground color intent.
    pub const fn foreground(self) -> ColorIntent {
        self.foreground
    }

    /// Returns muted foreground color intent.
    pub const fn muted_foreground(self) -> ColorIntent {
        self.muted_foreground
    }

    /// Returns border color intent.
    pub const fn border(self) -> ColorIntent {
        self.border
    }

    /// Returns shortcut label color intent.
    pub const fn shortcut_foreground(self) -> ColorIntent {
        self.shortcut_foreground
    }

    /// Returns focus-ring color intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved command metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommandMetrics {
    padding: UiPx,
    radius: UiPx,
    min_width: UiPx,
    max_width: UiPx,
    max_height: UiPx,
    row_height: UiPx,
    overscan_count: usize,
    shortcut_min_width: UiPx,
}

impl CommandMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            padding: ui_px(6.0),
            radius: size.control_radius(),
            min_width: ui_px(320.0),
            max_width: ui_px(560.0),
            max_height: match size {
                Size::XSmall => ui_px(220.0),
                Size::Small => ui_px(260.0),
                Size::Medium => ui_px(340.0),
                Size::Large => ui_px(420.0),
            },
            row_height: size.list_row_h(),
            overscan_count: match size {
                Size::XSmall | Size::Small => 4,
                Size::Medium => 6,
                Size::Large => 8,
            },
            shortcut_min_width: match size {
                Size::XSmall | Size::Small => ui_px(48.0),
                Size::Medium => ui_px(64.0),
                Size::Large => ui_px(76.0),
            },
        }
    }

    /// Returns panel padding.
    pub const fn padding(self) -> UiPx {
        self.padding
    }

    /// Returns panel radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns minimum panel width.
    pub const fn min_width(self) -> UiPx {
        self.min_width
    }

    /// Returns maximum panel width.
    pub const fn max_width(self) -> UiPx {
        self.max_width
    }

    /// Returns maximum command list height.
    pub const fn max_height(self) -> UiPx {
        self.max_height
    }

    /// Returns the fixed command result row height used by the virtualizer.
    pub const fn row_height(self) -> UiPx {
        self.row_height
    }

    /// Returns the number of rows kept beyond the visible command result viewport.
    pub const fn overscan_count(self) -> usize {
        self.overscan_count
    }

    /// Returns minimum shortcut label width.
    pub const fn shortcut_min_width(self) -> UiPx {
        self.shortcut_min_width
    }

    /// Returns the same metrics with a different fixed result row height.
    pub fn with_row_height(mut self, row_height: UiPx) -> Self {
        self.row_height = nonnegative_px(row_height);
        self
    }

    /// Returns the same metrics with a different overscan row budget.
    pub const fn with_overscan_count(mut self, overscan_count: usize) -> Self {
        self.overscan_count = overscan_count;
        self
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
    disabled: bool,
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

    /// Returns whether the item is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
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
    selection_mode: CommandSelectionMode,
    overlay: OverlayResolvedState,
    dialog: Option<CommandDialogState>,
    loading_state: Option<CommandLoadingState>,
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

impl CommandState {
    /// Resolves public state for a command surface.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
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
        groups: impl IntoIterator<Item = CommandGroupDescriptor>,
        items: impl IntoIterator<Item = CommandItemDescriptor>,
        outside_press_policy: OutsidePressPolicy,
        escape_key_policy: EscapeKeyPolicy,
        initial_focus_intent: InitialFocusIntent,
        focus_restore_intent: FocusRestoreIntent,
        tokens: ThemeTokens,
    ) -> Self {
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
            selected_value,
            selected_values,
            active_value,
            loading_state,
            empty_label,
            dialog_title,
            dialog_description,
            CommandDataSource::local(groups, items),
            outside_press_policy,
            escape_key_policy,
            initial_focus_intent,
            focus_restore_intent,
            tokens,
        )
    }

    /// Resolves public state from a caller-owned command index snapshot.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_from_index_snapshot(
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
        index_snapshot: CommandIndexSnapshot,
        outside_press_policy: OutsidePressPolicy,
        escape_key_policy: EscapeKeyPolicy,
        initial_focus_intent: InitialFocusIntent,
        focus_restore_intent: FocusRestoreIntent,
        tokens: ThemeTokens,
    ) -> Self {
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
            selected_value,
            selected_values,
            active_value,
            loading_state,
            empty_label,
            dialog_title,
            dialog_description,
            CommandDataSource::snapshot(index_snapshot),
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
        let empty_label = empty_label.into();
        let open_mode = if open.is_some() {
            CommandOpenMode::Controlled
        } else {
            CommandOpenMode::Uncontrolled
        };
        let open = open.unwrap_or(default_open) && !disabled;
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
                    disabled: item.descriptor.disabled,
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
        let presence = if dialog_enabled && open {
            OverlayPresence::open()
        } else {
            OverlayPresence::hidden()
        };
        let overlay =
            GpuiOverlayAdapterConfig::new(OverlayLayerKind::NonModalDismissible, presence)
                .outside_press_policy(outside_press_policy)
                .escape_key_policy(escape_key_policy)
                .initial_focus_intent(initial_focus_intent.clone())
                .focus_restore_intent(focus_restore_intent.clone())
                .resolved_state();
        let dialog_overlay = GpuiOverlayAdapterConfig::new(OverlayLayerKind::Modal, presence)
            .outside_press_policy(outside_press_policy)
            .escape_key_policy(escape_key_policy)
            .initial_focus_intent(initial_focus_intent)
            .focus_restore_intent(focus_restore_intent.clone())
            .resolved_state();
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
            selection_mode,
            overlay,
            dialog,
            loading_state,
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

/// A concrete GPUI command surface.
#[derive(IntoElement)]
pub struct Command {
    id: ElementId,
    label: SharedString,
    placeholder: SharedString,
    trigger_label: SharedString,
    items: Vec<CommandItem>,
    groups: Vec<CommandGroup>,
    index_snapshot: Option<CommandIndexSnapshot>,
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    dialog_enabled: bool,
    query: Option<String>,
    default_query: String,
    selection_mode: CommandSelectionMode,
    selected_value: Option<String>,
    selected_values: Option<Vec<String>>,
    active_value: Option<String>,
    viewport_item_count: usize,
    metrics: CommandMetrics,
    loading_state: Option<CommandLoadingState>,
    empty_label: SharedString,
    dialog_title: Option<String>,
    dialog_description: Option<String>,
    outside_press_policy: OutsidePressPolicy,
    escape_key_policy: EscapeKeyPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    tokens: ThemeTokens,
    on_open_change: Option<CommandOpenChangeHandler>,
    on_query_change: Option<CommandQueryChangeHandler>,
    on_select: Option<CommandSelectionHandler>,
    on_selected_values_change: Option<CommandSelectedValuesChangeHandler>,
}

impl Command {
    /// Creates an inline command surface.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        let size = Size::Medium;
        Self {
            id: id.into(),
            label: label.into(),
            placeholder: "Search commands".into(),
            trigger_label: "Open command menu".into(),
            items: Vec::new(),
            groups: Vec::new(),
            index_snapshot: None,
            size,
            disabled: false,
            open: None,
            default_open: false,
            dialog_enabled: false,
            query: None,
            default_query: String::new(),
            selection_mode: CommandSelectionMode::Single,
            selected_value: None,
            selected_values: None,
            active_value: None,
            viewport_item_count: DEFAULT_COMMAND_VIEWPORT_ITEM_COUNT,
            metrics: CommandMetrics::from_size(size),
            loading_state: None,
            empty_label: "No commands".into(),
            dialog_title: None,
            dialog_description: None,
            outside_press_policy: OutsidePressPolicy::DismissAndConsume,
            escape_key_policy: EscapeKeyPolicy::Dismiss,
            initial_focus_intent: InitialFocusIntent::FirstFocusable,
            focus_restore_intent: FocusRestoreIntent::Trigger,
            tokens: ThemeTokens::default(),
            on_open_change: None,
            on_query_change: None,
            on_select: None,
            on_selected_values_change: None,
        }
    }

    /// Applies placeholder text.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Applies dialog trigger label.
    pub fn trigger_label(mut self, label: impl Into<SharedString>) -> Self {
        self.trigger_label = label.into();
        self
    }

    /// Adds one standalone command item.
    pub fn item(mut self, item: CommandItem) -> Self {
        self.items.push(item);
        self
    }

    /// Adds many standalone command items.
    pub fn items(mut self, items: impl IntoIterator<Item = CommandItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// Adds one command group.
    pub fn group(mut self, group: CommandGroup) -> Self {
        self.groups.push(group);
        self
    }

    /// Adds many command groups.
    pub fn groups(mut self, groups: impl IntoIterator<Item = CommandGroup>) -> Self {
        self.groups.extend(groups);
        self
    }

    /// Applies a caller-owned command index snapshot.
    pub fn index_snapshot(mut self, snapshot: CommandIndexSnapshot) -> Self {
        self.index_snapshot = Some(snapshot);
        self
    }

    /// Marks the command surface as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies controlled dialog open state.
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Applies uncontrolled initial dialog open state.
    pub fn default_open(mut self, default_open: bool) -> Self {
        self.default_open = default_open;
        self
    }

    /// Enables dialog presentation with a title.
    pub fn dialog(mut self, title: impl Into<String>) -> Self {
        self.dialog_enabled = true;
        self.dialog_title = Some(title.into());
        self
    }

    /// Enables or disables dialog presentation.
    pub fn dialog_enabled(mut self, enabled: bool) -> Self {
        self.dialog_enabled = enabled;
        if !enabled {
            self.dialog_title = None;
            self.dialog_description = None;
        }
        self
    }

    /// Applies optional dialog description text.
    pub fn dialog_description(mut self, description: impl Into<String>) -> Self {
        self.dialog_description = Some(description.into());
        self
    }

    /// Applies controlled search query text.
    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = Some(query.into());
        self
    }

    /// Applies the default search query for adapter-owned input state.
    pub fn default_query(mut self, query: impl Into<String>) -> Self {
        self.default_query = query.into();
        self
    }

    /// Applies command selection behavior.
    pub fn selection_mode(mut self, mode: CommandSelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    /// Enables or disables persistent multi-selection behavior.
    pub fn multi_select(mut self, enabled: bool) -> Self {
        self.selection_mode = if enabled {
            CommandSelectionMode::Multiple
        } else {
            CommandSelectionMode::Single
        };
        self
    }

    /// Applies selected item value.
    pub fn selected(mut self, value: impl Into<String>) -> Self {
        self.selected_value = Some(value.into());
        self
    }

    /// Applies controlled selected values for multi-selection.
    pub fn selected_values(mut self, values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.selection_mode = CommandSelectionMode::Multiple;
        self.selected_values = Some(values.into_iter().map(Into::into).collect());
        self
    }

    /// Applies active item value.
    pub fn active(mut self, value: impl Into<String>) -> Self {
        self.active_value = Some(value.into());
        self
    }

    /// Applies the estimated number of command rows visible in the result viewport.
    pub fn viewport_item_count(mut self, count: usize) -> Self {
        self.viewport_item_count = count.max(1);
        self
    }

    /// Applies the fixed command result row height.
    pub fn row_height(mut self, row_height: UiPx) -> Self {
        self.metrics = self.metrics.with_row_height(row_height);
        self
    }

    /// Applies the command result overscan row budget.
    pub fn overscan(mut self, overscan: usize) -> Self {
        self.metrics = self.metrics.with_overscan_count(overscan);
        self
    }

    /// Applies loading metadata.
    pub fn loading(mut self, message: impl Into<String>, progress_percent: Option<u8>) -> Self {
        self.loading_state = Some(CommandLoadingState::new(message, progress_percent));
        self
    }

    /// Clears loading metadata.
    pub fn idle(mut self) -> Self {
        self.loading_state = None;
        self
    }

    /// Applies empty-state label.
    pub fn empty_label(mut self, label: impl Into<SharedString>) -> Self {
        self.empty_label = label.into();
        self
    }

    /// Applies outside-press policy.
    pub fn outside_press_policy(mut self, policy: OutsidePressPolicy) -> Self {
        self.outside_press_policy = policy;
        self
    }

    /// Applies Escape key policy.
    pub fn escape_key_policy(mut self, policy: EscapeKeyPolicy) -> Self {
        self.escape_key_policy = policy;
        self
    }

    /// Applies initial focus intent.
    pub fn initial_focus_intent(mut self, intent: InitialFocusIntent) -> Self {
        self.initial_focus_intent = intent;
        self
    }

    /// Applies focus restore intent.
    pub fn focus_restore_intent(mut self, intent: FocusRestoreIntent) -> Self {
        self.focus_restore_intent = intent;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers an open-change handler.
    pub fn on_open_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Rc::new(handler));
        self
    }

    /// Registers a query-change handler with the next sanitized query text.
    pub fn on_query_change(
        mut self,
        handler: impl Fn(String, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_query_change = Some(Rc::new(handler));
        self
    }

    /// Registers a command selection handler.
    pub fn on_select(
        mut self,
        handler: impl Fn(CommandSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Registers a selected-values change handler for multi-selection.
    pub fn on_selected_values_change(
        mut self,
        handler: impl Fn(CommandSelectionChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selected_values_change = Some(Rc::new(handler));
        self
    }

    /// Returns resolved command state.
    pub fn state(&self) -> CommandState {
        let query_mode = if self.query.is_some() {
            CommandQueryMode::Controlled
        } else {
            CommandQueryMode::Uncontrolled
        };
        let query = self.query.as_deref().unwrap_or(self.default_query.as_str());
        let selected_values = self.selected_values.clone().unwrap_or_default().into_iter();

        self.resolve_state_with_inputs(
            self.open,
            query,
            query_mode,
            self.selected_value.as_deref(),
            selected_values,
            self.active_value.as_deref(),
        )
    }

    /// Returns the default virtualized result render plan at the viewport origin.
    pub fn render_plan(&self) -> CommandRenderPlan {
        self.render_plan_with_viewport(
            UiPx::ZERO,
            self.metrics.row_height() * self.viewport_item_count as f32,
        )
    }

    /// Resolves the renderer-neutral command result render plan for a viewport snapshot.
    pub fn render_plan_with_viewport(
        &self,
        scroll_offset: UiPx,
        viewport_extent: UiPx,
    ) -> CommandRenderPlan {
        let state = self.state();
        CommandRenderPlan::resolve(
            self.id.to_string(),
            format!("{}-listbox", self.id),
            state,
            scroll_offset,
            viewport_extent,
        )
    }
}

impl Command {
    fn resolve_state_with_inputs(
        &self,
        open: Option<bool>,
        query: &str,
        query_mode: CommandQueryMode,
        selected_value: Option<&str>,
        selected_values: impl IntoIterator<Item = impl Into<String>>,
        active_value: Option<&str>,
    ) -> CommandState {
        let state = if let Some(index_snapshot) = self.index_snapshot.clone() {
            CommandState::resolve_from_index_snapshot(
                self.size,
                self.disabled,
                open,
                self.default_open,
                self.dialog_enabled,
                self.label.to_string(),
                self.placeholder.to_string(),
                query,
                query_mode,
                self.selection_mode,
                selected_value,
                selected_values,
                active_value,
                self.loading_state.clone(),
                self.empty_label.to_string(),
                self.dialog_title.clone(),
                self.dialog_description.clone(),
                index_snapshot,
                self.outside_press_policy,
                self.escape_key_policy,
                self.initial_focus_intent.clone(),
                self.focus_restore_intent.clone(),
                self.tokens,
            )
        } else {
            CommandState::resolve(
                self.size,
                self.disabled,
                open,
                self.default_open,
                self.dialog_enabled,
                self.label.to_string(),
                self.placeholder.to_string(),
                query,
                query_mode,
                self.selection_mode,
                selected_value,
                selected_values,
                active_value,
                self.loading_state.clone(),
                self.empty_label.to_string(),
                self.dialog_title.clone(),
                self.dialog_description.clone(),
                self.groups.iter().map(CommandGroup::descriptor),
                self.items.iter().map(CommandItem::descriptor),
                self.outside_press_policy,
                self.escape_key_policy,
                self.initial_focus_intent.clone(),
                self.focus_restore_intent.clone(),
                self.tokens,
            )
        };

        state.with_metrics(self.metrics)
    }
}

impl Sizable for Command {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self.metrics = CommandMetrics::from_size(size);
        self
    }
}

impl RenderOnce for Command {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let initial_query = self
            .query
            .clone()
            .unwrap_or_else(|| self.default_query.clone());
        let initial_selected_values = self
            .selected_values
            .clone()
            .unwrap_or_else(|| self.selected_value.iter().cloned().collect());
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, _| {
            CommandRuntime::new(
                self.default_open,
                self.active_value.clone(),
                self.selected_value.clone(),
                initial_selected_values.clone(),
                initial_query.clone(),
            )
        });
        let input_state_key: ElementId = (self.id.clone(), "input-state").into();
        let input_controller = window.use_keyed_state(input_state_key, cx, |_, cx| {
            let mut input = TextInputController::with_value(initial_query.clone(), cx);
            input.set_placeholder(self.placeholder.clone(), cx);
            input
        });
        let runtime_state = runtime.read(cx).clone();
        let scroll_handle = runtime_state.scroll_handle.clone();
        let open_state = resolve_overlay_open_state(self.open, runtime_state.open);
        let resolved_open = open_state.open();
        if open_state.runtime_changed() {
            runtime.update(cx, |runtime, _| {
                runtime.open = resolved_open;
            });
        }

        let query_mode = if self.query.is_some() {
            CommandQueryMode::Controlled
        } else {
            CommandQueryMode::Uncontrolled
        };
        let controller_query = input_controller.read(cx).value().to_owned();
        let query = self
            .query
            .as_deref()
            .unwrap_or(controller_query.as_str())
            .to_owned();
        let selected_value = self
            .selected_value
            .as_deref()
            .or(runtime_state.selected_value.as_deref());
        let selected_values = self
            .selected_values
            .clone()
            .unwrap_or_else(|| runtime_state.selected_values.clone());
        let active_value = self
            .active_value
            .as_deref()
            .or(runtime_state.active_value.as_deref())
            .or(selected_value);
        let state = self.resolve_state_with_inputs(
            Some(resolved_open),
            query.as_str(),
            query_mode,
            selected_value,
            selected_values.iter().cloned(),
            active_value,
        );
        let scroll_reset_key = command_scroll_reset_key(&state);
        if runtime_state.scroll_reset_key != scroll_reset_key {
            scroll_handle.set_offset(point(px(0.0), px(0.0)));
            runtime.update(cx, |runtime, _| {
                runtime.scroll_reset_key = scroll_reset_key.clone();
            });
        }
        let query_change_handler = self.on_query_change.clone();
        input_controller.update(cx, |controller, _cx| {
            let controlled_query =
                (query_mode == CommandQueryMode::Controlled).then(|| query.as_str());
            controller.sync_adapter_state(
                controlled_query,
                Some(self.placeholder.clone()),
                state.disabled(),
                false,
                TextInputDisplayMode::Plain,
                query_change_handler.clone(),
            );
        });
        let id = self.id;
        let debug_id = id.to_string();
        let trigger_id: ElementId = (id.clone(), "trigger").into();
        let input_id: ElementId = (id.clone(), "input").into();
        let content_id: ElementId = (id.clone(), "content").into();
        let listbox_id: ElementId = (id.clone(), "listbox").into();
        let viewport_extent = ui_px_from_gpui(scroll_handle.bounds().size.height);
        let scroll_offset =
            UiPx::new((-ui_px_from_gpui(scroll_handle.offset().y).as_f32()).max(0.0));
        let metrics = state.metrics();
        let colors = state.colors();
        let disabled = state.disabled();
        let focus_ring = state.focus_ring();
        let dialog_state = state.dialog().cloned();
        let dialog_open = dialog_state.clone().filter(|_| state.open());
        let dialog_priority = dialog_state
            .as_ref()
            .map(|dialog| gpui_overlay_state(dialog.overlay()).deferred_priority())
            .unwrap_or_else(|| gpui_overlay_state(state.overlay()).deferred_priority());
        let viewport = window.viewport_size();
        let dialog_enabled = self.dialog_enabled;
        let trigger_label = self.trigger_label;
        let on_open_change = self.on_open_change;
        let on_query_change = query_change_handler;
        let on_select = self.on_select;
        let on_selected_values_change = self.on_selected_values_change;
        let tokens = self.tokens;

        div()
            .id(id)
            .debug_selector({
                let debug_id = debug_id.clone();
                move || format!("command:{debug_id}:root")
            })
            .relative()
            .flex()
            .flex_col()
            .items_start()
            .gap_2()
            .when(dialog_state.is_some(), |this| {
                let runtime = runtime.clone();
                let on_open_change = on_open_change.clone();
                let trigger_label = trigger_label.clone();
                this.child(
                    div()
                        .id(trigger_id)
                        .debug_selector({
                            let debug_id = debug_id.clone();
                            move || format!("command:{debug_id}:trigger")
                        })
                        .min_h(gpui_px_from_ui(state.size().button_h()))
                        .px(gpui_px_from_ui(state.size().button_px()))
                        .py(gpui_px_from_ui(state.size().button_py()))
                        .rounded(gpui_px_from_ui(metrics.radius()))
                        .border_1()
                        .border_color(ThemeResolver::resolve(colors.border()))
                        .bg(ThemeResolver::resolve(colors.surface()))
                        .text_color(ThemeResolver::resolve(colors.foreground()))
                        .focusable()
                        .tab_stop(!disabled)
                        .ui_role(Role::Button)
                        .aria_label(trigger_label.clone())
                        .aria_expanded(state.open())
                        .aria_disabled(disabled)
                        .focus_visible(move |style| style.shadow(focus_ring_shadow(focus_ring)))
                        .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                        .when(!disabled, |this| {
                            this.cursor_pointer().on_click(
                                move |_event: &ClickEvent, window, cx| {
                                    cx.stop_propagation();
                                    runtime.update(cx, |runtime, _| {
                                        set_overlay_open(&mut runtime.open, true);
                                    });
                                    emit_overlay_open_change(
                                        true,
                                        on_open_change.as_deref(),
                                        window,
                                        cx,
                                    );
                                },
                            )
                        })
                        .child(trigger_label),
                )
            })
            .when(!dialog_enabled, |this| {
                this.child(command_content_element(
                    content_id.clone(),
                    input_id.clone(),
                    listbox_id.clone(),
                    debug_id.clone(),
                    state.clone(),
                    scroll_handle.clone(),
                    viewport_extent,
                    scroll_offset,
                    input_controller.clone(),
                    runtime.clone(),
                    on_open_change.clone(),
                    on_query_change.clone(),
                    on_select.clone(),
                    on_selected_values_change.clone(),
                    tokens,
                ))
            })
            .when_some(dialog_open, |this, dialog_state| {
                this.child(
                    deferred(
                        anchored()
                            .position(point(px(0.0), px(0.0)))
                            .snap_to_window()
                            .child(command_dialog_layer_element(
                                content_id,
                                input_id,
                                listbox_id,
                                debug_id,
                                state,
                                scroll_handle,
                                viewport_extent,
                                scroll_offset,
                                dialog_state,
                                viewport,
                                input_controller,
                                runtime,
                                on_open_change,
                                on_query_change,
                                on_select,
                                on_selected_values_change,
                                tokens,
                            )),
                    )
                    .priority(dialog_priority),
                )
            })
    }
}

const DEFAULT_COMMAND_VIEWPORT_ITEM_COUNT: usize = 8;

const fn nonnegative_px(value: UiPx) -> UiPx {
    if value.as_f32() < 0.0 {
        UiPx::ZERO
    } else {
        value
    }
}

/// A concrete GPUI command item.
#[derive(Clone)]
pub struct CommandItem {
    descriptor: CommandItemDescriptor,
}

impl CommandItem {
    /// Creates a selectable command item.
    pub fn new(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: CommandItemDescriptor::new(value, label.to_string()),
        }
    }

    /// Adds one filtering keyword.
    pub fn keyword(mut self, keyword: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.keyword(keyword);
        self
    }

    /// Adds a display shortcut label.
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.shortcut(shortcut);
        self
    }

    /// Marks the command as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.descriptor = self.descriptor.disabled(disabled);
        self
    }

    /// Returns the pure descriptor.
    pub fn descriptor(&self) -> CommandItemDescriptor {
        self.descriptor.clone()
    }
}

/// A concrete GPUI command group.
#[derive(Clone)]
pub struct CommandGroup {
    descriptor: CommandGroupDescriptor,
    items: Vec<CommandItem>,
}

impl CommandGroup {
    /// Creates an empty command group.
    pub fn new(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: CommandGroupDescriptor::new(value, label.to_string()),
            items: Vec::new(),
        }
    }

    /// Adds one command item.
    pub fn item(mut self, item: CommandItem) -> Self {
        self.items.push(item);
        self
    }

    /// Adds many command items.
    pub fn items(mut self, items: impl IntoIterator<Item = CommandItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// Returns the pure descriptor.
    pub fn descriptor(&self) -> CommandGroupDescriptor {
        self.items
            .iter()
            .fold(self.descriptor.clone(), |descriptor, item| {
                descriptor.item(item.descriptor())
            })
    }
}

fn command_text_match_rank(
    text: &str,
    normalized_query: &str,
    source: CommandMatchSource,
) -> Option<CommandMatchRank> {
    let text = choice::normalize_query(text);
    if text.is_empty() || normalized_query.is_empty() {
        return None;
    }

    let quality = if text == normalized_query {
        300
    } else if text.starts_with(normalized_query) {
        220
    } else if command_words_start_with(text.as_str(), normalized_query) {
        180
    } else if text.contains(normalized_query) {
        120
    } else {
        return None;
    };

    Some(CommandMatchRank {
        source: Some(source),
        score: source.base_score() + quality,
    })
}

fn command_words_start_with(text: &str, normalized_query: &str) -> bool {
    text.split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|word| word.starts_with(normalized_query))
}

fn command_item_rank_for_source(
    item: &CommandItemDescriptor,
    normalized_query: &str,
    mode: CommandIndexSnapshotMode,
) -> Option<CommandMatchRank> {
    let query_is_empty = normalized_query.is_empty();
    if !mode.should_filter_locally(query_is_empty) {
        return Some(CommandMatchRank::unfiltered());
    }

    item.match_rank(normalized_query)
}

fn sort_ranked_command_items(items: &mut [FlattenedCommandItem]) {
    items.sort_by(|a, b| {
        b.rank
            .score
            .cmp(&a.rank.score)
            .then_with(|| a.source_index.cmp(&b.source_index))
    });
}

impl ThemeResolver {
    pub(crate) const fn command_colors(tokens: ThemeTokens) -> CommandColors {
        CommandColors {
            surface: ColorIntent::new(tokens.surface, 0xffffff),
            foreground: ColorIntent::new(tokens.text, 0x18202a),
            muted_foreground: ColorIntent::new(tokens.text_muted, 0x5a6472),
            border: ColorIntent::new(tokens.border, 0xcfd5cc),
            shortcut_foreground: ColorIntent::with_state(
                tokens.text_muted,
                ColorState::Message,
                0x5a6472,
            ),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                0x2f80ed,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyboard_state() -> CommandState {
        Command::new("palette", "Command palette")
            .open(true)
            .default_query("file")
            .selected("new-file")
            .item(CommandItem::new("open-file", "Open File").shortcut("Ctrl+O"))
            .group(
                CommandGroup::new("file", "File")
                    .item(CommandItem::new("new-file", "New File").shortcut("Ctrl+N"))
                    .item(CommandItem::new("close-window", "Close Window").shortcut("Alt+F4")),
            )
            .state()
    }

    #[test]
    fn command_state_exposes_standalone_and_grouped_views() {
        let state = keyboard_state();

        let standalone_values = state
            .standalone_items()
            .map(|item| item.value().to_owned())
            .collect::<Vec<_>>();
        let grouped_values = state
            .grouped_groups()
            .map(|group| group.value().to_owned())
            .collect::<Vec<_>>();

        assert_eq!(standalone_values, vec!["open-file".to_string()]);
        assert_eq!(grouped_values, vec!["file".to_string()]);
        assert_eq!(state.standalone_items().count(), 1);
    }
}
