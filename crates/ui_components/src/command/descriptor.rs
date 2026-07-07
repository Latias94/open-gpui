//! Command descriptor, index snapshot, and search-ranking contracts.

use crate::action::{ActionDescriptor, ResolvedActionIcon, ResolvedActionState};
use crate::choice::{self, ChoiceItemProjection, ChoiceSelectionMode};
use crate::listbox::ListboxOptionDescriptor;
use crate::overlay::OverlayDisclosureOpenMode;
use open_gpui::{InvalidKeystrokeError, Keymap, Keystroke, Window};
use open_gpui_command::{
    CommandCenter, CommandDescriptor, CommandKeyBindingConflict, CommandKeyBindingDiagnostic,
    CommandKeyBindingEditTarget, CommandKeyBindingPatch, CommandKeyBindingPatchOperation,
    CommandKeyBindingPatchOutcome, CommandKeyBindingPatchPreview, CommandKeyBindingProjectedEntry,
    CommandKeyBindingProjection, CommandKeymapCommandState, CommandKeymapResolution,
    CommandKeymapResolvedCommand, CommandProviderId, CommandProviderRefreshController,
    CommandProviderRefreshProjection, CommandProviderRequest, CommandProviderResponse,
    CommandProviderState, CommandProviderStatus, CommandRegistryError, CommandRegistrySnapshot,
    CommandShortcutDiagnostic, CommandShortcutDiagnosticKind, parse_command_key_sequence,
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
    intent: CommandStatusIntent,
    message: String,
}

impl CommandStatusItem {
    /// Creates a command status item.
    pub fn new(intent: CommandStatusIntent, message: impl Into<String>) -> Self {
        Self {
            intent,
            message: message.into(),
        }
    }

    /// Creates an informational status item.
    pub fn info(message: impl Into<String>) -> Self {
        Self::new(CommandStatusIntent::Info, message)
    }

    /// Creates a warning status item.
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(CommandStatusIntent::Warning, message)
    }

    /// Creates an error status item.
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(CommandStatusIntent::Error, message)
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
        Role::Label
    }
}

const DEFAULT_PROVIDER_LOADING_MESSAGE: &str = "Loading commands";

/// UI-ready projection for an app-owned [`CommandCenter`] command palette.
///
/// This is the copyable bridge most applications should feed into [`crate::Command`]. It keeps
/// `open_gpui_command` renderer-neutral while carrying the UI snapshot, current query, provider
/// statuses, and shortcut diagnostics together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPaletteProjection {
    query: String,
    provider_statuses: Vec<CommandProviderStatus>,
    shortcut_diagnostics: Vec<CommandShortcutDiagnostic>,
    status_items: Vec<CommandStatusItem>,
    index_snapshot: CommandIndexSnapshot,
}

impl CommandPaletteProjection {
    /// Projects a command center for an app-level keymap and query.
    pub fn from_center_for_keymap(
        center: &CommandCenter,
        query: impl Into<String>,
        keymap: &Keymap,
    ) -> Self {
        let query = query.into();
        Self::from_parts(
            query.clone(),
            center.search_snapshot_for_keymap(query.as_str(), keymap),
            center.provider_statuses().cloned(),
            center.shortcut_diagnostics_for_keymap(keymap),
        )
    }

    /// Projects a command center for focused-window shortcut precedence and query.
    pub fn from_center_for_window(
        center: &CommandCenter,
        query: impl Into<String>,
        window: &Window,
    ) -> Self {
        let query = query.into();
        Self::from_parts(
            query.clone(),
            center.search_snapshot_for_window(query.as_str(), window),
            center.provider_statuses().cloned(),
            center.shortcut_diagnostics_for_window(window),
        )
    }

    fn from_parts(
        query: String,
        snapshot: CommandRegistrySnapshot,
        provider_statuses: impl IntoIterator<Item = CommandProviderStatus>,
        shortcut_diagnostics: impl IntoIterator<Item = CommandShortcutDiagnostic>,
    ) -> Self {
        let provider_statuses = provider_statuses.into_iter().collect::<Vec<_>>();
        let shortcut_diagnostics = shortcut_diagnostics.into_iter().collect::<Vec<_>>();
        let status_items =
            command_status_items_from_projection(&provider_statuses, &shortcut_diagnostics);
        let mut index_snapshot = CommandIndexSnapshot::from_registry_snapshot(&snapshot)
            .mode(CommandIndexSnapshotMode::PreFiltered);

        if let Some(loading_state) = provider_statuses
            .iter()
            .find_map(provider_status_loading_state)
        {
            index_snapshot = index_snapshot.loading(loading_state);
        }

        Self {
            query,
            provider_statuses,
            shortcut_diagnostics,
            status_items,
            index_snapshot,
        }
    }

    /// Returns the query used to create this projection.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns provider statuses retained by the command center.
    pub fn provider_statuses(&self) -> &[CommandProviderStatus] {
        &self.provider_statuses
    }

    /// Returns the first provider status, useful for single-provider palettes.
    pub fn provider_status(&self) -> Option<&CommandProviderStatus> {
        self.provider_statuses.first()
    }

    /// Returns shortcut/action/keymap diagnostics for the projected command center.
    pub fn shortcut_diagnostics(&self) -> &[CommandShortcutDiagnostic] {
        &self.shortcut_diagnostics
    }

    /// Returns UI-ready provider and shortcut diagnostic status items.
    pub fn status_items(&self) -> &[CommandStatusItem] {
        &self.status_items
    }

    /// Returns whether provider or shortcut diagnostics should be displayed.
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

    /// Returns the UI command index snapshot.
    pub const fn index_snapshot(&self) -> &CommandIndexSnapshot {
        &self.index_snapshot
    }

    /// Returns loading metadata projected from any loading provider status.
    pub const fn loading_state(&self) -> Option<&CommandLoadingState> {
        self.index_snapshot.loading_state()
    }

    /// Consumes the projection and returns the UI command index snapshot.
    pub fn into_index_snapshot(self) -> CommandIndexSnapshot {
        self.index_snapshot
    }

    /// Consumes the projection and returns all UI-facing parts.
    pub fn into_parts(
        self,
    ) -> (
        String,
        Vec<CommandProviderStatus>,
        Vec<CommandShortcutDiagnostic>,
        CommandIndexSnapshot,
    ) {
        (
            self.query,
            self.provider_statuses,
            self.shortcut_diagnostics,
            self.index_snapshot,
        )
    }
}

/// UI-side controller for a command-center-backed command palette.
///
/// The controller owns query and provider-refresh lifecycle state, but not the command center
/// itself. Applications keep owning `CommandCenter`, async tasks, and dispatch policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPaletteController {
    query: String,
    query_history_prefix: Option<String>,
    providers: Vec<CommandProviderRefreshController>,
}

impl Default for CommandPaletteController {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandPaletteController {
    /// Creates an empty command palette controller.
    pub fn new() -> Self {
        Self {
            query: String::new(),
            query_history_prefix: None,
            providers: Vec::new(),
        }
    }

    /// Seeds the controller query.
    pub fn with_query(mut self, query: impl Into<String>) -> Self {
        self.query = query.into();
        self
    }

    /// Registers a provider refresh controller without a loading message.
    pub fn provider(mut self, provider_id: impl Into<CommandProviderId>) -> Self {
        self.add_provider(provider_id);
        self
    }

    /// Registers a provider refresh controller with loading metadata.
    pub fn provider_with_loading(
        mut self,
        provider_id: impl Into<CommandProviderId>,
        loading_message: impl Into<String>,
    ) -> Self {
        self.add_provider_with_loading(provider_id, loading_message);
        self
    }

    /// Adds or replaces a provider refresh controller without a loading message.
    pub fn add_provider(&mut self, provider_id: impl Into<CommandProviderId>) -> &mut Self {
        self.replace_provider_controller(CommandProviderRefreshController::new(provider_id));
        self
    }

    /// Adds or replaces a provider refresh controller with loading metadata.
    pub fn add_provider_with_loading(
        &mut self,
        provider_id: impl Into<CommandProviderId>,
        loading_message: impl Into<String>,
    ) -> &mut Self {
        self.replace_provider_controller(
            CommandProviderRefreshController::new(provider_id)
                .with_loading_message(loading_message),
        );
        self
    }

    /// Returns the current controller query.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns provider refresh controllers owned by this palette controller.
    pub fn provider_controllers(&self) -> &[CommandProviderRefreshController] {
        &self.providers
    }

    /// Projects current state without changing query or refreshing providers.
    pub fn projection_for_keymap(
        &self,
        center: &CommandCenter,
        keymap: &Keymap,
    ) -> CommandPaletteProjection {
        CommandPaletteProjection::from_center_for_keymap(center, self.query.as_str(), keymap)
    }

    /// Projects current state for focused-window shortcut precedence without refreshing providers.
    pub fn projection_for_window(
        &self,
        center: &CommandCenter,
        window: &Window,
    ) -> CommandPaletteProjection {
        CommandPaletteProjection::from_center_for_window(center, self.query.as_str(), window)
    }

    /// Resolves one app-level key sequence for shortcut inspectors and dispatch preflight.
    ///
    /// This does not dispatch. It keeps GPUI and `CommandCenter` as the keymap authority, then
    /// carries the controller query alongside the typed keymap resolution so app shells can use the
    /// same query when they later call command dispatch.
    pub fn preflight_key_sequence_for_keymap(
        &self,
        center: &CommandCenter,
        sequence: &str,
        keymap: &Keymap,
    ) -> Result<CommandPaletteKeymapPreflight, InvalidKeystrokeError> {
        Ok(CommandPaletteKeymapPreflight::new(
            self.query.clone(),
            center.resolve_key_sequence_for_keymap(sequence, keymap)?,
        ))
    }

    /// Sets query, refreshes configured providers, and projects app-level keymap state.
    pub fn set_query_for_keymap(
        &mut self,
        center: &mut CommandCenter,
        query: impl Into<String>,
        keymap: &Keymap,
    ) -> Result<CommandPaletteControllerUpdate, CommandRegistryError> {
        let query_changed = self.set_query_from_input(center, query);
        let (provider_projections, pending_provider_requests) =
            self.refresh_provider_controllers(center)?;
        Ok(CommandPaletteControllerUpdate::new(
            self.query.clone(),
            query_changed,
            provider_projections,
            pending_provider_requests,
            self.projection_for_keymap(center, keymap),
        ))
    }

    /// Sets query, refreshes configured providers, and projects focused-window shortcut state.
    pub fn set_query_for_window(
        &mut self,
        center: &mut CommandCenter,
        query: impl Into<String>,
        window: &Window,
    ) -> Result<CommandPaletteControllerUpdate, CommandRegistryError> {
        let query_changed = self.set_query_from_input(center, query);
        let (provider_projections, pending_provider_requests) =
            self.refresh_provider_controllers(center)?;
        Ok(CommandPaletteControllerUpdate::new(
            self.query.clone(),
            query_changed,
            provider_projections,
            pending_provider_requests,
            self.projection_for_window(center, window),
        ))
    }

    /// Moves to the previous matching query in command-center history and projects keymap state.
    pub fn previous_query_for_keymap(
        &mut self,
        center: &mut CommandCenter,
        keymap: &Keymap,
    ) -> Option<Result<CommandPaletteControllerUpdate, CommandRegistryError>> {
        let query =
            self.query_history_query(center, CommandPaletteQueryHistoryDirection::Previous)?;
        Some(self.set_query_from_history_for_keymap(center, query, keymap))
    }

    /// Moves to the next matching query in command-center history and projects keymap state.
    pub fn next_query_for_keymap(
        &mut self,
        center: &mut CommandCenter,
        keymap: &Keymap,
    ) -> Option<Result<CommandPaletteControllerUpdate, CommandRegistryError>> {
        let query = self.query_history_query(center, CommandPaletteQueryHistoryDirection::Next)?;
        Some(self.set_query_from_history_for_keymap(center, query, keymap))
    }

    /// Moves to the previous matching query in command-center history and projects window state.
    pub fn previous_query_for_window(
        &mut self,
        center: &mut CommandCenter,
        window: &Window,
    ) -> Option<Result<CommandPaletteControllerUpdate, CommandRegistryError>> {
        let query =
            self.query_history_query(center, CommandPaletteQueryHistoryDirection::Previous)?;
        Some(self.set_query_from_history_for_window(center, query, window))
    }

    /// Moves to the next matching query in command-center history and projects window state.
    pub fn next_query_for_window(
        &mut self,
        center: &mut CommandCenter,
        window: &Window,
    ) -> Option<Result<CommandPaletteControllerUpdate, CommandRegistryError>> {
        let query = self.query_history_query(center, CommandPaletteQueryHistoryDirection::Next)?;
        Some(self.set_query_from_history_for_window(center, query, window))
    }

    /// Applies an async provider response and projects app-level keymap state.
    pub fn apply_provider_response_for_keymap(
        &mut self,
        center: &mut CommandCenter,
        provider_id: impl Into<CommandProviderId>,
        request: &CommandProviderRequest,
        response: CommandProviderResponse,
        keymap: &Keymap,
    ) -> Option<Result<CommandPaletteControllerUpdate, CommandRegistryError>> {
        let provider_projection =
            self.apply_provider_response(center, provider_id, request, response)?;
        Some(provider_projection.map(|provider_projection| {
            CommandPaletteControllerUpdate::new(
                self.query.clone(),
                false,
                [provider_projection],
                Vec::<CommandPalettePendingProviderRequest>::new(),
                self.projection_for_keymap(center, keymap),
            )
        }))
    }

    /// Applies an async provider response and projects focused-window shortcut state.
    pub fn apply_provider_response_for_window(
        &mut self,
        center: &mut CommandCenter,
        provider_id: impl Into<CommandProviderId>,
        request: &CommandProviderRequest,
        response: CommandProviderResponse,
        window: &Window,
    ) -> Option<Result<CommandPaletteControllerUpdate, CommandRegistryError>> {
        let provider_projection =
            self.apply_provider_response(center, provider_id, request, response)?;
        Some(provider_projection.map(|provider_projection| {
            CommandPaletteControllerUpdate::new(
                self.query.clone(),
                false,
                [provider_projection],
                Vec::<CommandPalettePendingProviderRequest>::new(),
                self.projection_for_window(center, window),
            )
        }))
    }

    fn set_query_from_input(
        &mut self,
        center: &mut CommandCenter,
        query: impl Into<String>,
    ) -> bool {
        let query = query.into();
        let query_changed = self.query != query;
        if query_changed {
            self.query_history_prefix = None;
            center.reset_query_navigation();
            self.query = query;
        }
        query_changed
    }

    fn query_history_query(
        &mut self,
        center: &mut CommandCenter,
        direction: CommandPaletteQueryHistoryDirection,
    ) -> Option<String> {
        match direction {
            CommandPaletteQueryHistoryDirection::Previous => {
                let prefix = self
                    .query_history_prefix
                    .get_or_insert_with(|| self.query.clone())
                    .clone();
                let query = center.previous_query(prefix.as_str());
                if query.is_none() {
                    self.query_history_prefix = None;
                    center.reset_query_navigation();
                }
                query
            }
            CommandPaletteQueryHistoryDirection::Next => {
                let prefix = self.query_history_prefix.clone()?;
                match center.next_query(prefix.as_str()) {
                    Some(query) => Some(query),
                    None => {
                        self.query_history_prefix = None;
                        center.reset_query_navigation();
                        (self.query != prefix).then_some(prefix)
                    }
                }
            }
        }
    }

    fn set_query_from_history_for_keymap(
        &mut self,
        center: &mut CommandCenter,
        query: String,
        keymap: &Keymap,
    ) -> Result<CommandPaletteControllerUpdate, CommandRegistryError> {
        let (query, query_changed, provider_projections, pending_provider_requests) =
            self.set_query_from_history(center, query)?;
        Ok(CommandPaletteControllerUpdate::new(
            query,
            query_changed,
            provider_projections,
            pending_provider_requests,
            self.projection_for_keymap(center, keymap),
        ))
    }

    fn set_query_from_history_for_window(
        &mut self,
        center: &mut CommandCenter,
        query: String,
        window: &Window,
    ) -> Result<CommandPaletteControllerUpdate, CommandRegistryError> {
        let (query, query_changed, provider_projections, pending_provider_requests) =
            self.set_query_from_history(center, query)?;
        Ok(CommandPaletteControllerUpdate::new(
            query,
            query_changed,
            provider_projections,
            pending_provider_requests,
            self.projection_for_window(center, window),
        ))
    }

    fn set_query_from_history(
        &mut self,
        center: &mut CommandCenter,
        query: String,
    ) -> Result<
        (
            String,
            bool,
            Vec<CommandProviderRefreshProjection>,
            Vec<CommandPalettePendingProviderRequest>,
        ),
        CommandRegistryError,
    > {
        let query_changed = self.query != query;
        self.query = query;
        let (provider_projections, pending_provider_requests) =
            self.refresh_provider_controllers(center)?;
        Ok((
            self.query.clone(),
            query_changed,
            provider_projections,
            pending_provider_requests,
        ))
    }

    fn replace_provider_controller(&mut self, controller: CommandProviderRefreshController) {
        let provider_id = controller.provider_id().clone();
        self.providers
            .retain(|candidate| candidate.provider_id() != &provider_id);
        if !provider_id.is_empty() {
            self.providers.push(controller);
        }
    }

    fn refresh_provider_controllers(
        &mut self,
        center: &mut CommandCenter,
    ) -> Result<
        (
            Vec<CommandProviderRefreshProjection>,
            Vec<CommandPalettePendingProviderRequest>,
        ),
        CommandRegistryError,
    > {
        let mut provider_projections = Vec::with_capacity(self.providers.len());
        let mut pending_provider_requests = Vec::new();

        for controller in &mut self.providers {
            let projection = controller.set_query(center, self.query.clone())?;
            if !projection.query_changed() {
                provider_projections.push(projection);
                continue;
            }

            let Some(request) = projection.request().cloned() else {
                provider_projections.push(projection);
                continue;
            };
            let Some(response) =
                center.provider_response_for_request(controller.provider_id().clone(), &request)
            else {
                pending_provider_requests.push(CommandPalettePendingProviderRequest::new(
                    controller.provider_id().clone(),
                    request,
                ));
                provider_projections.push(projection);
                continue;
            };

            provider_projections.push(controller.apply_response(center, &request, response)?);
        }

        Ok((provider_projections, pending_provider_requests))
    }

    fn apply_provider_response(
        &mut self,
        center: &mut CommandCenter,
        provider_id: impl Into<CommandProviderId>,
        request: &CommandProviderRequest,
        response: CommandProviderResponse,
    ) -> Option<Result<CommandProviderRefreshProjection, CommandRegistryError>> {
        let provider_id = provider_id.into();
        let controller = self
            .providers
            .iter_mut()
            .find(|candidate| candidate.provider_id() == &provider_id)?;
        Some(controller.apply_response(center, request, response))
    }
}

/// A command palette keymap lookup enriched with the query that would accompany dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPaletteKeymapPreflight {
    query: String,
    resolution: CommandKeymapResolution,
}

impl CommandPaletteKeymapPreflight {
    /// Creates a preflight value from a controller query and command-aware keymap resolution.
    pub fn new(query: impl Into<String>, resolution: CommandKeymapResolution) -> Self {
        Self {
            query: query.into(),
            resolution,
        }
    }

    /// Returns the controller query captured when this preflight was produced.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns the underlying command-aware keymap resolution.
    pub const fn resolution(&self) -> &CommandKeymapResolution {
        &self.resolution
    }

    /// Consumes the preflight and returns the underlying keymap resolution.
    pub fn into_resolution(self) -> CommandKeymapResolution {
        self.resolution
    }

    /// Returns the normalized input sequence as one whitespace-separated label.
    pub fn input_label(&self) -> String {
        self.resolution.input_label()
    }

    /// Returns matched commands in GPUI dispatch precedence order.
    pub fn matched_commands(&self) -> &[CommandKeymapResolvedCommand] {
        self.resolution.matched_commands()
    }

    /// Returns whether the GPUI keymap has any pending continuation for this input.
    pub const fn is_pending(&self) -> bool {
        self.resolution.is_pending()
    }

    /// Returns pending command continuations in GPUI precedence order.
    pub fn pending_commands(&self) -> &[CommandKeymapResolvedCommand] {
        self.resolution.pending_commands()
    }

    /// Returns the first matched command in GPUI dispatch precedence order.
    pub fn primary_command(&self) -> Option<&CommandKeymapResolvedCommand> {
        self.resolution.primary_command()
    }

    /// Returns the first matched command that is visible and dispatchable.
    pub fn primary_dispatchable_command(&self) -> Option<&CommandKeymapResolvedCommand> {
        self.resolution.primary_dispatchable_command()
    }

    /// Returns the first dispatchable command id, if this key sequence can dispatch now.
    pub fn primary_dispatchable_command_id(&self) -> Option<&str> {
        self.primary_dispatchable_command()
            .map(CommandKeymapResolvedCommand::command_id)
    }
}

/// One command row shown by a shortcut inspector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandShortcutInspectorCommand {
    command_id: String,
    shortcut: String,
    state: CommandKeymapCommandState,
}

impl CommandShortcutInspectorCommand {
    fn from_resolved(command: &CommandKeymapResolvedCommand) -> Self {
        Self {
            command_id: command.command_id().to_owned(),
            shortcut: command.shortcut().to_owned(),
            state: command.state().clone(),
        }
    }

    /// Returns the stable command id.
    pub fn command_id(&self) -> &str {
        &self.command_id
    }

    /// Returns the full shortcut label that matched or can continue from the inspected input.
    pub fn shortcut(&self) -> &str {
        &self.shortcut
    }

    /// Returns the command state after scope and availability checks.
    pub const fn state(&self) -> &CommandKeymapCommandState {
        &self.state
    }

    /// Returns whether this command can be dispatched.
    pub const fn is_dispatchable(&self) -> bool {
        self.state.is_dispatchable()
    }
}

/// UI-ready shortcut inspector state for command palettes and app shells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandShortcutInspectorState {
    query: String,
    input_label: String,
    pending: bool,
    primary_dispatchable_command_id: Option<String>,
    matched_commands: Vec<CommandShortcutInspectorCommand>,
    pending_commands: Vec<CommandShortcutInspectorCommand>,
}

impl CommandShortcutInspectorState {
    /// Creates shortcut inspector state from palette controller preflight output.
    pub fn from_preflight(preflight: &CommandPaletteKeymapPreflight) -> Self {
        Self {
            query: preflight.query().to_owned(),
            input_label: preflight.input_label(),
            pending: preflight.is_pending(),
            primary_dispatchable_command_id: preflight
                .primary_dispatchable_command_id()
                .map(str::to_owned),
            matched_commands: preflight
                .matched_commands()
                .iter()
                .map(CommandShortcutInspectorCommand::from_resolved)
                .collect(),
            pending_commands: preflight
                .pending_commands()
                .iter()
                .map(CommandShortcutInspectorCommand::from_resolved)
                .collect(),
        }
    }

    /// Returns the palette query captured during keymap preflight.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns the normalized inspected input sequence.
    pub fn input_label(&self) -> &str {
        &self.input_label
    }

    /// Returns whether the inspected sequence is waiting for more chord input.
    pub const fn is_pending(&self) -> bool {
        self.pending
    }

    /// Returns the first dispatchable command id, if the inspected sequence can dispatch now.
    pub fn primary_dispatchable_command_id(&self) -> Option<&str> {
        self.primary_dispatchable_command_id.as_deref()
    }

    /// Returns commands that match the inspected input now.
    pub fn matched_commands(&self) -> &[CommandShortcutInspectorCommand] {
        &self.matched_commands
    }

    /// Returns command continuations if the inspected input is a pending chord.
    pub fn pending_commands(&self) -> &[CommandShortcutInspectorCommand] {
        &self.pending_commands
    }
}

/// Filter mode for command keybinding editor projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommandKeyBindingEditorFilterMode {
    /// Show every projected valid binding that matches the query.
    #[default]
    All,
    /// Show only bindings that participate in a same-context command conflict.
    ConflictsOnly,
}

/// Filtering applied to a keybinding editor state projection.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommandKeyBindingEditorFilter {
    query: String,
    mode: CommandKeyBindingEditorFilterMode,
}

impl CommandKeyBindingEditorFilter {
    /// Creates an empty all-bindings filter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the editor query.
    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = query.into();
        self
    }

    /// Shows only conflicting key bindings.
    pub fn conflicts_only(mut self) -> Self {
        self.mode = CommandKeyBindingEditorFilterMode::ConflictsOnly;
        self
    }

    /// Returns the query.
    pub fn query_ref(&self) -> &str {
        &self.query
    }

    /// Returns the filter mode.
    pub const fn mode(&self) -> CommandKeyBindingEditorFilterMode {
        self.mode
    }
}

/// Captured keybinding input state before it becomes a patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandKeyBindingCaptureState {
    raw_sequence: String,
    input_label: Option<String>,
    error: Option<String>,
}

impl CommandKeyBindingCaptureState {
    /// Parses a raw key sequence captured by a keybinding input surface.
    pub fn from_sequence(sequence: impl Into<String>) -> Self {
        let raw_sequence = sequence.into();
        if raw_sequence.trim().is_empty() {
            return Self {
                raw_sequence,
                input_label: None,
                error: None,
            };
        }

        match parse_command_key_sequence(&raw_sequence) {
            Ok(input) => Self {
                raw_sequence,
                input_label: Some(
                    input
                        .iter()
                        .map(Keystroke::unparse)
                        .collect::<Vec<_>>()
                        .join(" "),
                ),
                error: None,
            },
            Err(error) => Self {
                raw_sequence,
                input_label: None,
                error: Some(error.to_string()),
            },
        }
    }

    /// Returns the raw captured sequence.
    pub fn raw_sequence(&self) -> &str {
        &self.raw_sequence
    }

    /// Returns the normalized display label when parsing succeeded.
    pub fn input_label(&self) -> Option<&str> {
        self.input_label.as_deref()
    }

    /// Returns the parse error when the captured sequence is invalid.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Returns whether the capture has no keystrokes yet.
    pub fn is_empty(&self) -> bool {
        self.input_label.is_none() && self.error.is_none()
    }

    /// Returns whether the captured sequence can be used in a keybinding patch.
    pub fn is_valid(&self) -> bool {
        self.input_label.is_some() && self.error.is_none()
    }
}

/// One valid binding row in the command keybinding editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandKeyBindingEditorRow {
    edit_target: CommandKeyBindingEditTarget,
    keystrokes: String,
    context: Option<String>,
    conflict_count: usize,
}

impl CommandKeyBindingEditorRow {
    fn from_projected_entry(
        entry: &CommandKeyBindingProjectedEntry,
        conflicts: &[CommandKeyBindingConflict],
    ) -> Self {
        let conflict_count = conflicts
            .iter()
            .filter(|conflict| {
                conflict.keystrokes() == entry.keystrokes()
                    && conflict.context_ref() == entry.context_ref()
                    && conflict.entries().iter().any(|conflict_entry| {
                        conflict_entry.source_id() == entry.source_id()
                            && conflict_entry.command_id() == entry.command_id()
                    })
            })
            .count();

        Self {
            edit_target: entry.edit_target(),
            keystrokes: entry.keystrokes().to_owned(),
            context: entry.context_ref().map(str::to_owned),
            conflict_count,
        }
    }

    /// Returns the lifecycle source id for this binding.
    pub fn source_id(&self) -> &str {
        self.edit_target.source_id().as_str()
    }

    /// Returns the command id for this binding.
    pub fn command_id(&self) -> &str {
        self.edit_target.command_id()
    }

    /// Returns the canonical shortcut label.
    pub fn keystrokes(&self) -> &str {
        &self.keystrokes
    }

    /// Returns the raw source keystroke sequence for persistence patches.
    pub fn raw_keystrokes(&self) -> &str {
        self.edit_target.keystrokes()
    }

    /// Returns the normalized context predicate.
    pub fn context_ref(&self) -> Option<&str> {
        self.context.as_deref()
    }

    /// Returns the raw source context predicate for persistence patches.
    pub fn raw_context_ref(&self) -> Option<&str> {
        self.edit_target.context_ref()
    }

    /// Returns the patch target represented by this row.
    pub const fn edit_target(&self) -> &CommandKeyBindingEditTarget {
        &self.edit_target
    }

    /// Returns the number of conflicts involving this binding row.
    pub const fn conflict_count(&self) -> usize {
        self.conflict_count
    }

    /// Returns whether this row participates in at least one conflict.
    pub const fn has_conflict(&self) -> bool {
        self.conflict_count > 0
    }
}

/// UI-ready preview of a keybinding patch candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandKeyBindingEditorPreviewState {
    patch: CommandKeyBindingPatch,
    outcome: CommandKeyBindingPatchOutcome,
    editor: CommandKeyBindingEditorState,
}

impl CommandKeyBindingEditorPreviewState {
    /// Projects editor preview state from a command keybinding patch preview.
    pub fn from_patch_preview(
        preview: &CommandKeyBindingPatchPreview,
        filter: CommandKeyBindingEditorFilter,
    ) -> Self {
        Self {
            patch: preview.patch().clone(),
            outcome: preview.outcome(),
            editor: CommandKeyBindingEditorState::from_projection(preview.projection(), filter),
        }
    }

    /// Returns the previewed patch.
    pub const fn patch(&self) -> &CommandKeyBindingPatch {
        &self.patch
    }

    /// Returns the patch operation.
    pub const fn operation(&self) -> CommandKeyBindingPatchOperation {
        self.patch.operation()
    }

    /// Returns the patch application outcome.
    pub const fn outcome(&self) -> CommandKeyBindingPatchOutcome {
        self.outcome
    }

    /// Returns whether the candidate registry changed.
    pub const fn changed(&self) -> bool {
        self.outcome.changed()
    }

    /// Returns the editor state after applying the candidate patch.
    pub const fn editor(&self) -> &CommandKeyBindingEditorState {
        &self.editor
    }

    /// Returns whether the candidate edit changed the registry and has no diagnostics or conflicts.
    pub fn is_strictly_clean(&self) -> bool {
        self.changed() && !self.editor.has_diagnostics() && !self.editor.has_conflicts()
    }
}

/// UI-ready command keybinding editor state derived from a keybinding projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandKeyBindingEditorState {
    query: String,
    mode: CommandKeyBindingEditorFilterMode,
    total_binding_count: usize,
    rows: Vec<CommandKeyBindingEditorRow>,
    conflicts: Vec<CommandKeyBindingConflict>,
    diagnostics: Vec<CommandKeyBindingDiagnostic>,
}

impl CommandKeyBindingEditorState {
    /// Projects valid binding rows, conflicts, and diagnostics for a keybinding editor UI.
    pub fn from_projection(
        projection: &CommandKeyBindingProjection,
        filter: CommandKeyBindingEditorFilter,
    ) -> Self {
        let query = filter.query.trim().to_lowercase();
        let rows = projection
            .projected_entries()
            .iter()
            .map(|entry| {
                CommandKeyBindingEditorRow::from_projected_entry(entry, projection.conflicts())
            })
            .filter(|row| row_matches_keybinding_editor_filter(row, &query, filter.mode))
            .collect();

        Self {
            query: filter.query,
            mode: filter.mode,
            total_binding_count: projection.projected_entries().len(),
            rows,
            conflicts: projection.conflicts().to_vec(),
            diagnostics: projection.diagnostics().to_vec(),
        }
    }

    /// Returns the query used to filter editor rows.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns the active filter mode.
    pub const fn mode(&self) -> CommandKeyBindingEditorFilterMode {
        self.mode
    }

    /// Returns all valid binding rows before filtering.
    pub const fn total_binding_count(&self) -> usize {
        self.total_binding_count
    }

    /// Returns valid binding rows after query and mode filtering.
    pub fn rows(&self) -> &[CommandKeyBindingEditorRow] {
        &self.rows
    }

    /// Returns the filtered valid binding count.
    pub fn filtered_binding_count(&self) -> usize {
        self.rows.len()
    }

    /// Returns projection conflicts in canonical command projection form.
    pub fn conflicts(&self) -> &[CommandKeyBindingConflict] {
        &self.conflicts
    }

    /// Returns projection diagnostics for invalid or unresolved bindings.
    pub fn diagnostics(&self) -> &[CommandKeyBindingDiagnostic] {
        &self.diagnostics
    }

    /// Returns whether the projected keymap has command conflicts.
    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }

    /// Returns whether the projected keymap has invalid binding diagnostics.
    pub fn has_diagnostics(&self) -> bool {
        !self.diagnostics.is_empty()
    }
}

fn row_matches_keybinding_editor_filter(
    row: &CommandKeyBindingEditorRow,
    normalized_query: &str,
    mode: CommandKeyBindingEditorFilterMode,
) -> bool {
    if matches!(mode, CommandKeyBindingEditorFilterMode::ConflictsOnly) && !row.has_conflict() {
        return false;
    }

    if normalized_query.is_empty() {
        return true;
    }

    row.command_id().to_lowercase().contains(normalized_query)
        || row.source_id().to_lowercase().contains(normalized_query)
        || row.keystrokes().to_lowercase().contains(normalized_query)
        || row
            .context_ref()
            .is_some_and(|context| context.to_lowercase().contains(normalized_query))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandPaletteQueryHistoryDirection {
    Previous,
    Next,
}

/// App-owned async provider request emitted by a command palette controller step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPalettePendingProviderRequest {
    provider_id: CommandProviderId,
    request: CommandProviderRequest,
}

impl CommandPalettePendingProviderRequest {
    /// Creates a pending provider request.
    pub fn new(provider_id: impl Into<CommandProviderId>, request: CommandProviderRequest) -> Self {
        Self {
            provider_id: provider_id.into(),
            request,
        }
    }

    /// Returns the provider id that should produce this request.
    pub const fn provider_id(&self) -> &CommandProviderId {
        &self.provider_id
    }

    /// Returns the provider request to pass back with the async response.
    pub const fn request(&self) -> &CommandProviderRequest {
        &self.request
    }

    /// Consumes the pending request into its provider id and request.
    pub fn into_parts(self) -> (CommandProviderId, CommandProviderRequest) {
        (self.provider_id, self.request)
    }
}

/// Result of a command palette controller query or async response step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPaletteControllerUpdate {
    query: String,
    query_changed: bool,
    provider_projections: Vec<CommandProviderRefreshProjection>,
    missing_provider_ids: Vec<CommandProviderId>,
    pending_provider_requests: Vec<CommandPalettePendingProviderRequest>,
    palette_projection: CommandPaletteProjection,
}

impl CommandPaletteControllerUpdate {
    fn new(
        query: String,
        query_changed: bool,
        provider_projections: impl IntoIterator<Item = CommandProviderRefreshProjection>,
        pending_provider_requests: impl IntoIterator<Item = CommandPalettePendingProviderRequest>,
        palette_projection: CommandPaletteProjection,
    ) -> Self {
        let pending_provider_requests: Vec<_> = pending_provider_requests.into_iter().collect();
        let missing_provider_ids = pending_provider_requests
            .iter()
            .map(|pending| pending.provider_id().clone())
            .collect();
        Self {
            query,
            query_changed,
            provider_projections: provider_projections.into_iter().collect(),
            missing_provider_ids,
            pending_provider_requests,
            palette_projection,
        }
    }

    /// Returns the query used for this controller step.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns whether the controller query changed in this step.
    pub const fn query_changed(&self) -> bool {
        self.query_changed
    }

    /// Returns provider refresh projections produced by this step.
    pub fn provider_projections(&self) -> &[CommandProviderRefreshProjection] {
        &self.provider_projections
    }

    /// Returns the projection for one provider id.
    pub fn provider_projection(
        &self,
        provider_id: &str,
    ) -> Option<&CommandProviderRefreshProjection> {
        self.provider_projections
            .iter()
            .find(|projection| projection.provider_id().as_str() == provider_id)
    }

    /// Returns configured provider ids that had no registered synchronous callback.
    pub fn missing_provider_ids(&self) -> &[CommandProviderId] {
        &self.missing_provider_ids
    }

    /// Returns app-owned async provider requests to execute after this step.
    pub fn pending_provider_requests(&self) -> &[CommandPalettePendingProviderRequest] {
        &self.pending_provider_requests
    }

    /// Returns a pending async provider request by provider id.
    pub fn pending_provider_request(
        &self,
        provider_id: &str,
    ) -> Option<&CommandPalettePendingProviderRequest> {
        self.pending_provider_requests
            .iter()
            .find(|pending| pending.provider_id().as_str() == provider_id)
    }

    /// Returns the UI-ready palette projection for this step.
    pub const fn palette_projection(&self) -> &CommandPaletteProjection {
        &self.palette_projection
    }

    /// Consumes the update and returns the UI-ready palette projection.
    pub fn into_palette_projection(self) -> CommandPaletteProjection {
        self.palette_projection
    }
}

/// UI-ready projection for a provider-backed command palette refresh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandProviderPaletteProjection {
    query: String,
    provider_status: Option<CommandProviderStatus>,
    status_items: Vec<CommandStatusItem>,
    index_snapshot: CommandIndexSnapshot,
}

impl CommandProviderPaletteProjection {
    /// Creates a UI command palette projection from a runtime-neutral provider refresh projection.
    pub fn from_refresh_projection(projection: &CommandProviderRefreshProjection) -> Self {
        let provider_status = projection.provider_status().cloned();
        let status_items = provider_status
            .as_ref()
            .into_iter()
            .filter_map(command_status_item_from_provider_status)
            .collect::<Vec<_>>();
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
            status_items,
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

    /// Returns UI-ready provider status items.
    pub fn status_items(&self) -> &[CommandStatusItem] {
        &self.status_items
    }

    /// Returns whether provider status should be displayed.
    pub fn has_status_items(&self) -> bool {
        !self.status_items.is_empty()
    }

    /// Returns the number of error status items.
    pub fn status_error_count(&self) -> usize {
        count_command_status_items(&self.status_items, CommandStatusIntent::Error)
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

fn command_status_items_from_projection(
    provider_statuses: &[CommandProviderStatus],
    shortcut_diagnostics: &[CommandShortcutDiagnostic],
) -> Vec<CommandStatusItem> {
    provider_statuses
        .iter()
        .filter_map(command_status_item_from_provider_status)
        .chain(
            shortcut_diagnostics
                .iter()
                .map(command_status_item_from_shortcut_diagnostic),
        )
        .filter(|item| !item.is_empty())
        .collect()
}

fn command_status_item_from_provider_status(
    status: &CommandProviderStatus,
) -> Option<CommandStatusItem> {
    (status.state() == CommandProviderState::Failed).then(|| {
        let message = status
            .message()
            .unwrap_or("Provider failed to load commands");
        CommandStatusItem::error(format!(
            "Provider {} failed: {}",
            status.provider_id().as_str(),
            message
        ))
    })
}

fn command_status_item_from_shortcut_diagnostic(
    diagnostic: &CommandShortcutDiagnostic,
) -> CommandStatusItem {
    let message = match diagnostic.kind() {
        CommandShortcutDiagnosticKind::MissingAction => {
            format!(
                "Command {} has no registered action",
                diagnostic.command_id().unwrap_or("unknown")
            )
        }
        CommandShortcutDiagnosticKind::OrphanAction => {
            format!(
                "Command {} is not present for registered action {}",
                diagnostic.command_id().unwrap_or("unknown"),
                diagnostic.action_name().unwrap_or("unknown")
            )
        }
        CommandShortcutDiagnosticKind::MissingShortcut => {
            format!(
                "Command {} has no projected shortcut",
                diagnostic.command_id().unwrap_or("unknown")
            )
        }
        CommandShortcutDiagnosticKind::DuplicateShortcut => {
            format!(
                "Shortcut {} is shared by {}",
                diagnostic.shortcut().unwrap_or("unknown"),
                diagnostic.command_ids().join(", ")
            )
        }
    };
    CommandStatusItem::warning(message)
}

pub(super) fn count_command_status_items(
    items: &[CommandStatusItem],
    intent: CommandStatusIntent,
) -> usize {
    items.iter().filter(|item| item.intent() == intent).count()
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

    /// Creates a selectable command item descriptor from shared app-command metadata.
    pub fn from_command_descriptor(descriptor: &CommandDescriptor) -> Self {
        let action = ActionDescriptor::from_command_descriptor(descriptor)
            .resolve_without_icon_diagnostics();
        let mut item = Self::from_resolved_action(&action)
            .keywords(descriptor.keywords_ref().iter().cloned())
            .disabled(descriptor.disabled_state());
        if let Some(when) = descriptor.when_ref() {
            item = item.when(when);
        }
        item
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
