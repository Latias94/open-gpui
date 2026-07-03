//! Command descriptor, index snapshot, and search-ranking contracts.

use crate::choice::{self, ChoiceItemProjection, ChoiceSelectionMode};
use crate::listbox::ListboxOptionDescriptor;
use crate::overlay::OverlayDisclosureOpenMode;
use open_gpui_command::{
    CommandDescriptor, CommandProviderRefreshProjection, CommandProviderState,
    CommandProviderStatus, CommandRegistrySnapshot,
};
use open_gpui_ui_core::Role;

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

const DEFAULT_PROVIDER_LOADING_MESSAGE: &str = "Loading commands";

/// UI-ready projection for a provider-backed command palette refresh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandProviderPaletteProjection {
    query: String,
    provider_status: Option<CommandProviderStatus>,
    index_snapshot: CommandIndexSnapshot,
}

impl CommandProviderPaletteProjection {
    /// Creates a UI command palette projection from a runtime-neutral provider refresh projection.
    pub fn from_refresh_projection(projection: &CommandProviderRefreshProjection) -> Self {
        let provider_status = projection.provider_status().cloned();
        let mut index_snapshot =
            CommandIndexSnapshot::from_registry_snapshot(projection.snapshot())
                .mode(CommandIndexSnapshotMode::PreFiltered);

        if let Some(loading_state) = provider_status
            .as_ref()
            .and_then(provider_status_loading_state)
        {
            index_snapshot = index_snapshot.loading(loading_state);
        }

        Self {
            query: projection.query().to_owned(),
            provider_status,
            index_snapshot,
        }
    }

    /// Returns the command query used for the provider refresh.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns the latest provider status retained by the command center.
    pub const fn provider_status(&self) -> Option<&CommandProviderStatus> {
        self.provider_status.as_ref()
    }

    /// Returns the UI command index snapshot.
    pub const fn index_snapshot(&self) -> &CommandIndexSnapshot {
        &self.index_snapshot
    }

    /// Returns loading metadata projected from the provider status, when still loading.
    pub const fn loading_state(&self) -> Option<&CommandLoadingState> {
        self.index_snapshot.loading_state()
    }

    /// Consumes the projection and returns the UI command index snapshot.
    pub fn into_index_snapshot(self) -> CommandIndexSnapshot {
        self.index_snapshot
    }

    /// Consumes the projection and returns all UI-facing parts.
    pub fn into_parts(self) -> (String, Option<CommandProviderStatus>, CommandIndexSnapshot) {
        (self.query, self.provider_status, self.index_snapshot)
    }
}

impl From<&CommandProviderRefreshProjection> for CommandProviderPaletteProjection {
    fn from(projection: &CommandProviderRefreshProjection) -> Self {
        Self::from_refresh_projection(projection)
    }
}

fn provider_status_loading_state(status: &CommandProviderStatus) -> Option<CommandLoadingState> {
    (status.state() == CommandProviderState::Loading).then(|| {
        CommandLoadingState::new(
            status.message().unwrap_or(DEFAULT_PROVIDER_LOADING_MESSAGE),
            None,
        )
    })
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
    pub(super) keywords: Vec<String>,
    pub(super) shortcut: Option<String>,
    pub(super) disabled: bool,
    pub(super) disabled_reason: Option<String>,
    pub(super) when: Option<String>,
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
            disabled_reason: None,
            when: None,
        }
    }

    /// Creates a selectable command item descriptor from shared app-command metadata.
    pub fn from_command_descriptor(descriptor: &CommandDescriptor) -> Self {
        let mut item = Self::new(descriptor.id(), descriptor.label())
            .keywords(descriptor.keywords_ref().iter().cloned())
            .disabled(descriptor.disabled_state());
        if let Some(reason) = descriptor.disabled_reason_ref() {
            item = item.disabled_reason(reason);
        }
        if let Some(shortcut) = descriptor.shortcut_ref() {
            item = item.shortcut(shortcut);
        }
        if let Some(when) = descriptor.when_ref() {
            item = item.when(when);
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

    /// Creates a command index snapshot from a renderer-neutral command registry snapshot.
    pub fn from_registry_snapshot(registry: &CommandRegistrySnapshot) -> Self {
        Self::new(registry.revision()).command_descriptors(registry.descriptors())
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

    /// Adds one shared app-command descriptor, preserving its optional group metadata.
    pub fn command_descriptor(mut self, descriptor: &CommandDescriptor) -> Self {
        let item = CommandItemDescriptor::from_command_descriptor(descriptor);
        if let Some(group) = descriptor.group_ref() {
            if let Some(existing) = self
                .groups
                .iter_mut()
                .find(|candidate| candidate.value() == group)
            {
                existing.items.push(item);
            } else {
                self.groups
                    .push(CommandGroupDescriptor::new(group, group).item(item));
            }
        } else {
            self.items.push(item);
        }
        self
    }

    /// Adds many shared app-command descriptors.
    pub fn command_descriptors<'a>(
        mut self,
        descriptors: impl IntoIterator<Item = &'a CommandDescriptor>,
    ) -> Self {
        for descriptor in descriptors {
            self = self.command_descriptor(descriptor);
        }
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
