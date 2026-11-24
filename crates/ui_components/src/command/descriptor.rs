//! Command descriptor, index snapshot, and search-ranking contracts.

use crate::action::{ResolvedActionIcon, ResolvedActionState};
use crate::choice::{self, ChoiceItemProjection, ChoiceSelectionMode};
use crate::listbox::ListboxOptionDescriptor;
use crate::overlay::OverlayDisclosureOpenMode;
use open_gpui_ui_core::{LivePoliteness, Role};

/// Command dialog open-state ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommandOpenMode {
    /// Open state is owned by the component adapter after initialization.
    #[default]
    Uncontrolled,
    /// Open state is provided by the caller.
    Controlled,
}

pub(super) const fn command_open_mode_from_disclosure(
    mode: OverlayDisclosureOpenMode,
) -> CommandOpenMode {
    match mode {
        OverlayDisclosureOpenMode::Uncontrolled => CommandOpenMode::Uncontrolled,
        OverlayDisclosureOpenMode::Controlled => CommandOpenMode::Controlled,
    }
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
    pub(super) const fn is_multiple(self) -> bool {
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

    pub(super) const fn should_rank_locally(self, query_is_empty: bool) -> bool {
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

/// Semantic intent for a command palette status item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandStatusIntent {
    /// Informational command-palette metadata.
    Info,
    /// Recoverable warning or diagnostic metadata.
    Warning,
    /// Provider or projection error metadata.
    Error,
}

/// UI-ready command palette status item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandStatusItem {
    id: String,
    intent: CommandStatusIntent,
    message: String,
}

/// Why one or more command status identities were omitted from resolved state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandStatusIdentityDiagnostic {
    /// Status items with an empty identity were omitted.
    MissingId {
        /// Number of omitted items with no usable identity.
        occurrences: usize,
    },
    /// Every status item sharing this identity was omitted.
    DuplicateId {
        /// Stable, caller-owned, non-sensitive identity that collided.
        id: String,
        /// Number of candidate items that carried the identity.
        occurrences: usize,
    },
}

impl CommandStatusItem {
    /// Creates a command status item with a stable, non-sensitive identity.
    pub fn new(
        id: impl Into<String>,
        intent: CommandStatusIntent,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            intent,
            message: message.into(),
        }
    }

    /// Creates an informational status item.
    pub fn info(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(id, CommandStatusIntent::Info, message)
    }

    /// Creates a warning status item.
    pub fn warning(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(id, CommandStatusIntent::Warning, message)
    }

    /// Creates an error status item.
    pub fn error(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(id, CommandStatusIntent::Error, message)
    }

    /// Returns the stable, non-sensitive status identity.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the item intent.
    pub const fn intent(&self) -> CommandStatusIntent {
        self.intent
    }

    /// Returns display message text.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns whether this item carries an empty message.
    pub fn is_empty(&self) -> bool {
        self.message.trim().is_empty()
    }

    /// Returns status item accessibility role.
    pub const fn role(&self) -> Role {
        match self.intent {
            CommandStatusIntent::Info | CommandStatusIntent::Warning => Role::Status,
            CommandStatusIntent::Error => Role::Alert,
        }
    }

    /// Returns the live-region priority for this status item.
    pub const fn live(&self) -> LivePoliteness {
        match self.intent {
            CommandStatusIntent::Info | CommandStatusIntent::Warning => LivePoliteness::Polite,
            CommandStatusIntent::Error => LivePoliteness::Assertive,
        }
    }

    /// Returns whether the complete status text is announced atomically.
    pub const fn live_atomic(&self) -> bool {
        true
    }

    /// Returns whether this status item represents pending work.
    pub const fn busy(&self) -> bool {
        false
    }
}

pub(super) fn count_command_status_items(
    items: &[CommandStatusItem],
    intent: CommandStatusIntent,
) -> usize {
    items.iter().filter(|item| item.intent() == intent).count()
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
pub(super) struct CommandMatchRank {
    pub(super) source: Option<CommandMatchSource>,
    pub(super) score: u16,
}

impl CommandMatchRank {
    pub(super) const fn unfiltered() -> Self {
        Self {
            source: None,
            score: 0,
        }
    }
}

/// Pure descriptor for one command item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandItemDescriptor {
    pub(super) value: String,
    pub(super) label: String,
    pub(super) icon: Option<ResolvedActionIcon>,
    pub(super) keywords: Vec<String>,
    pub(super) shortcut: Option<String>,
    pub(super) disabled: bool,
    pub(super) disabled_reason: Option<String>,
    pub(super) tooltip: Option<String>,
    pub(super) accessibility_description: Option<String>,
    pub(super) when: Option<String>,
}

impl CommandItemDescriptor {
    /// Creates a selectable command item descriptor.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            icon: None,
            keywords: Vec::new(),
            shortcut: None,
            disabled: false,
            disabled_reason: None,
            tooltip: None,
            accessibility_description: None,
            when: None,
        }
    }

    /// Creates a selectable command item descriptor from resolved action metadata.
    pub fn from_resolved_action(action: &ResolvedActionState) -> Self {
        let mut item = Self::new(action.value(), action.label()).disabled(action.disabled());
        if let Some(icon) = action.icon() {
            item.icon = Some(icon.clone());
        }
        if let Some(shortcut) = action.shortcut() {
            item = item.shortcut(shortcut);
        }
        if let Some(reason) = action.disabled_reason() {
            item = item.disabled_reason(reason);
        }
        if let Some(tooltip) = action.tooltip() {
            item = item.tooltip(tooltip);
        }
        if let Some(description) = action.accessibility_description() {
            item = item.accessibility_description(description);
        }
        item
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

    /// Applies app-resolved icon metadata.
    pub fn icon(mut self, icon: ResolvedActionIcon) -> Self {
        self.icon = Some(icon);
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

    /// Marks the item as disabled with a user-displayable reason.
    pub fn disabled_reason(mut self, reason: impl Into<String>) -> Self {
        let reason = reason.into();
        if !reason.is_empty() {
            self.disabled = true;
            self.disabled_reason = Some(reason);
        }
        self
    }

    /// Applies user-displayable tooltip metadata.
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        let tooltip = tooltip.into();
        if !tooltip.is_empty() {
            self.tooltip = Some(tooltip);
        }
        self
    }

    /// Applies an accessibility description in addition to the visible label.
    pub fn accessibility_description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        if !description.is_empty() {
            self.accessibility_description = Some(description);
        }
        self
    }

    /// Applies caller-owned availability metadata without evaluating it.
    pub fn when(mut self, when: impl Into<String>) -> Self {
        self.when = Some(when.into());
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

    /// Returns app-resolved icon metadata.
    pub const fn icon_ref(&self) -> Option<&ResolvedActionIcon> {
        self.icon.as_ref()
    }

    /// Returns a concrete render label for the resolved icon.
    pub fn icon_label(&self) -> Option<&str> {
        self.icon.as_ref().and_then(ResolvedActionIcon::label)
    }

    /// Returns the display shortcut label.
    pub fn shortcut_ref(&self) -> Option<&str> {
        self.shortcut.as_deref()
    }

    /// Returns whether the item is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }

    /// Returns the optional disabled reason.
    pub fn disabled_reason_ref(&self) -> Option<&str> {
        self.disabled_reason.as_deref()
    }

    /// Returns user-displayable tooltip metadata.
    pub fn tooltip_ref(&self) -> Option<&str> {
        self.tooltip.as_deref()
    }

    /// Returns the optional accessibility description.
    pub fn accessibility_description_ref(&self) -> Option<&str> {
        self.accessibility_description.as_deref()
    }

    /// Returns caller-owned availability metadata.
    pub fn when_ref(&self) -> Option<&str> {
        self.when.as_deref()
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

    pub(super) fn to_listbox_descriptor(&self) -> ListboxOptionDescriptor {
        ListboxOptionDescriptor::option(self.value.clone(), self.label.clone())
            .disabled(self.disabled)
    }
}

/// Pure descriptor for one command group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandGroupDescriptor {
    pub(super) value: String,
    pub(super) label: String,
    pub(super) items: Vec<CommandItemDescriptor>,
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

pub(super) fn command_choice_selection_mode(mode: CommandSelectionMode) -> ChoiceSelectionMode {
    match mode {
        CommandSelectionMode::Single => ChoiceSelectionMode::Single,
        CommandSelectionMode::Multiple => ChoiceSelectionMode::Multiple,
    }
}

pub(super) fn command_choice_items(
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
    pub(super) revision: String,
    pub(super) mode: CommandIndexSnapshotMode,
    pub(super) loading_state: Option<CommandLoadingState>,
    pub(super) groups: Vec<CommandGroupDescriptor>,
    pub(super) items: Vec<CommandItemDescriptor>,
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

pub(super) fn command_item_rank_for_source(
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
