//! Command descriptor, index snapshot, and search-ranking contracts.

use crate::choice::{self, ChoiceItemProjection, ChoiceSelectionMode};
use crate::listbox::ListboxOptionDescriptor;
use crate::overlay::OverlayDisclosureOpenMode;
use open_gpui::{Keymap, Window};
use open_gpui_command::{
    CommandCenter, CommandDescriptor, CommandProviderId, CommandProviderRefreshController,
    CommandProviderRefreshProjection, CommandProviderRequest, CommandProviderResponse,
    CommandProviderState, CommandProviderStatus, CommandRegistryError, CommandRegistrySnapshot,
    CommandShortcutDiagnostic, CommandShortcutDiagnosticKind,
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

    /// Sets query, refreshes configured providers, and projects app-level keymap state.
    pub fn set_query_for_keymap(
        &mut self,
        center: &mut CommandCenter,
        query: impl Into<String>,
        keymap: &Keymap,
    ) -> Result<CommandPaletteControllerUpdate, CommandRegistryError> {
        let query_changed = self.set_query_from_input(center, query);
        let (provider_projections, missing_provider_ids) =
            self.refresh_provider_controllers(center)?;
        Ok(CommandPaletteControllerUpdate::new(
            self.query.clone(),
            query_changed,
            provider_projections,
            missing_provider_ids,
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
        let (provider_projections, missing_provider_ids) =
            self.refresh_provider_controllers(center)?;
        Ok(CommandPaletteControllerUpdate::new(
            self.query.clone(),
            query_changed,
            provider_projections,
            missing_provider_ids,
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
                [],
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
                [],
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
        let (query, query_changed, provider_projections, missing_provider_ids) =
            self.set_query_from_history(center, query)?;
        Ok(CommandPaletteControllerUpdate::new(
            query,
            query_changed,
            provider_projections,
            missing_provider_ids,
            self.projection_for_keymap(center, keymap),
        ))
    }

    fn set_query_from_history_for_window(
        &mut self,
        center: &mut CommandCenter,
        query: String,
        window: &Window,
    ) -> Result<CommandPaletteControllerUpdate, CommandRegistryError> {
        let (query, query_changed, provider_projections, missing_provider_ids) =
            self.set_query_from_history(center, query)?;
        Ok(CommandPaletteControllerUpdate::new(
            query,
            query_changed,
            provider_projections,
            missing_provider_ids,
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
            Vec<CommandProviderId>,
        ),
        CommandRegistryError,
    > {
        let query_changed = self.query != query;
        self.query = query;
        let (provider_projections, missing_provider_ids) =
            self.refresh_provider_controllers(center)?;
        Ok((
            self.query.clone(),
            query_changed,
            provider_projections,
            missing_provider_ids,
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
            Vec<CommandProviderId>,
        ),
        CommandRegistryError,
    > {
        let mut provider_projections = Vec::with_capacity(self.providers.len());
        let mut missing_provider_ids = Vec::new();

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
                missing_provider_ids.push(controller.provider_id().clone());
                provider_projections.push(projection);
                continue;
            };

            provider_projections.push(controller.apply_response(center, &request, response)?);
        }

        Ok((provider_projections, missing_provider_ids))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandPaletteQueryHistoryDirection {
    Previous,
    Next,
}

/// Result of a command palette controller query or async response step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPaletteControllerUpdate {
    query: String,
    query_changed: bool,
    provider_projections: Vec<CommandProviderRefreshProjection>,
    missing_provider_ids: Vec<CommandProviderId>,
    palette_projection: CommandPaletteProjection,
}

impl CommandPaletteControllerUpdate {
    fn new(
        query: String,
        query_changed: bool,
        provider_projections: impl IntoIterator<Item = CommandProviderRefreshProjection>,
        missing_provider_ids: impl IntoIterator<Item = CommandProviderId>,
        palette_projection: CommandPaletteProjection,
    ) -> Self {
        Self {
            query,
            query_changed,
            provider_projections: provider_projections.into_iter().collect(),
            missing_provider_ids: missing_provider_ids.into_iter().collect(),
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
