//! App-owned command runtime facade.

use std::collections::BTreeMap;
use std::sync::Arc;

use open_gpui::{Action, App, Keymap, Window};

use crate::{
    CommandAvailabilityMap, CommandContribution, CommandDispatchOutcome, CommandMenuTree,
    CommandProjectionDiagnostic, CommandProvider, CommandProviderApplyOutcome, CommandProviderId,
    CommandProviderRequest, CommandProviderRequestId, CommandProviderResponse,
    CommandProviderSource, CommandProviderStaleResponse, CommandProviderStatus,
    CommandRegistryError, CommandRegistrySnapshot, CommandScopeId, CommandScopeProjection,
    CommandSourceId, CommandUsageHistory, GpuiCommandActionMap, MemoryCommandHistory,
    ScopedCommandRegistry,
};

/// A registered command source within one command scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSourceRegistration {
    scope_id: CommandScopeId,
    source_id: CommandSourceId,
}

impl CommandSourceRegistration {
    /// Creates a command source registration token.
    pub fn new(scope_id: impl Into<CommandScopeId>, source_id: impl Into<CommandSourceId>) -> Self {
        Self {
            scope_id: scope_id.into(),
            source_id: source_id.into(),
        }
    }

    /// Returns the registered scope id.
    pub const fn scope_id(&self) -> &CommandScopeId {
        &self.scope_id
    }

    /// Returns the registered source id.
    pub const fn source_id(&self) -> &CommandSourceId {
        &self.source_id
    }
}

/// A registered dynamic command provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandProviderRegistration {
    provider_id: CommandProviderId,
}

impl CommandProviderRegistration {
    /// Creates a command provider registration token.
    pub fn new(provider_id: impl Into<CommandProviderId>) -> Self {
        Self {
            provider_id: provider_id.into(),
        }
    }

    /// Returns the registered provider id.
    pub const fn provider_id(&self) -> &CommandProviderId {
        &self.provider_id
    }
}

#[derive(Clone)]
struct CommandProviderEntry {
    provider_id: CommandProviderId,
    provider: Arc<dyn CommandProvider>,
}

impl std::fmt::Debug for CommandProviderEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandProviderEntry")
            .field("provider_id", &self.provider_id)
            .finish_non_exhaustive()
    }
}

/// App-owned command runtime facade.
///
/// `CommandCenter` composes the lower-level command primitives into the default app/plugin
/// pipeline. It is intentionally not a singleton; applications can keep one center per app,
/// workspace, plugin host, window, or surface.
#[derive(Debug, Clone)]
pub struct CommandCenter {
    registry: ScopedCommandRegistry,
    actions: GpuiCommandActionMap,
    availability: CommandAvailabilityMap,
    active_scopes: Vec<CommandScopeId>,
    history: MemoryCommandHistory,
    providers: Vec<CommandProviderEntry>,
    provider_sources: BTreeMap<CommandProviderId, Vec<CommandSourceId>>,
    provider_statuses: BTreeMap<CommandProviderId, CommandProviderStatus>,
    provider_request_counters: BTreeMap<CommandProviderId, u64>,
    provider_latest_requests: BTreeMap<CommandProviderId, CommandProviderRequest>,
}

impl CommandCenter {
    /// Creates an empty command center.
    pub fn new(revision: impl Into<String>) -> Self {
        Self {
            registry: ScopedCommandRegistry::new(revision),
            actions: GpuiCommandActionMap::new(),
            availability: CommandAvailabilityMap::new(),
            active_scopes: Vec::new(),
            history: MemoryCommandHistory::default(),
            providers: Vec::new(),
            provider_sources: BTreeMap::new(),
            provider_statuses: BTreeMap::new(),
            provider_request_counters: BTreeMap::new(),
            provider_latest_requests: BTreeMap::new(),
        }
    }

    /// Returns the scoped registry.
    pub const fn registry(&self) -> &ScopedCommandRegistry {
        &self.registry
    }

    /// Returns mutable access to the scoped registry.
    pub fn registry_mut(&mut self) -> &mut ScopedCommandRegistry {
        &mut self.registry
    }

    /// Returns the command action map.
    pub const fn actions(&self) -> &GpuiCommandActionMap {
        &self.actions
    }

    /// Returns mutable access to the command action map.
    pub fn actions_mut(&mut self) -> &mut GpuiCommandActionMap {
        &mut self.actions
    }

    /// Returns the availability map.
    pub const fn availability(&self) -> &CommandAvailabilityMap {
        &self.availability
    }

    /// Returns mutable access to the availability map.
    pub fn availability_mut(&mut self) -> &mut CommandAvailabilityMap {
        &mut self.availability
    }

    /// Replaces the availability map.
    pub fn set_availability(&mut self, availability: CommandAvailabilityMap) -> &mut Self {
        self.availability = availability;
        self
    }

    /// Returns command usage and query history.
    pub const fn history(&self) -> &MemoryCommandHistory {
        &self.history
    }

    /// Returns mutable command usage and query history.
    pub fn history_mut(&mut self) -> &mut MemoryCommandHistory {
        &mut self.history
    }

    /// Replaces command usage and query history.
    pub fn set_history(&mut self, history: MemoryCommandHistory) -> &mut Self {
        self.history = history;
        self
    }

    /// Sets active command scopes.
    ///
    /// When no active scopes are set, snapshots project all registered scopes in registration order.
    pub fn set_active_scopes(
        &mut self,
        scopes: impl IntoIterator<Item = impl Into<CommandScopeId>>,
    ) -> &mut Self {
        self.active_scopes = scopes.into_iter().map(Into::into).collect();
        self
    }

    /// Clears explicit active scopes and returns to projecting all registered scopes.
    pub fn clear_active_scopes(&mut self) -> &mut Self {
        self.active_scopes.clear();
        self
    }

    /// Returns active command scopes.
    pub fn active_scopes(&self) -> &[CommandScopeId] {
        &self.active_scopes
    }

    /// Registers or replaces a dynamic command provider.
    ///
    /// Replacing an existing provider with the same id removes that provider's previously applied
    /// dynamic sources before installing the new provider callback.
    pub fn register_provider(
        &mut self,
        provider_id: impl Into<CommandProviderId>,
        provider: impl CommandProvider,
    ) -> CommandProviderRegistration {
        self.register_provider_arc(provider_id, Arc::new(provider))
    }

    /// Registers or replaces a dynamic command provider from a shared trait object.
    pub fn register_provider_arc(
        &mut self,
        provider_id: impl Into<CommandProviderId>,
        provider: Arc<dyn CommandProvider>,
    ) -> CommandProviderRegistration {
        let provider_id = provider_id.into();
        self.unregister_provider_id(provider_id.clone());
        if !provider_id.is_empty() {
            self.providers.push(CommandProviderEntry {
                provider_id: provider_id.clone(),
                provider,
            });
        }
        CommandProviderRegistration::new(provider_id)
    }

    /// Unregisters a dynamic command provider and removes its applied sources.
    pub fn unregister_provider(&mut self, registration: &CommandProviderRegistration) -> usize {
        self.unregister_provider_id(registration.provider_id().clone())
    }

    /// Unregisters a dynamic command provider id and removes its applied sources.
    pub fn unregister_provider_id(&mut self, provider_id: impl Into<CommandProviderId>) -> usize {
        let provider_id = provider_id.into();
        self.providers
            .retain(|entry| entry.provider_id != provider_id);
        self.provider_statuses.remove(&provider_id);
        self.provider_latest_requests.remove(&provider_id);
        self.unregister_provider_sources(&provider_id)
    }

    /// Returns the latest applied status for a provider.
    pub fn provider_status(
        &self,
        provider_id: impl Into<CommandProviderId>,
    ) -> Option<&CommandProviderStatus> {
        let provider_id = provider_id.into();
        self.provider_statuses.get(&provider_id)
    }

    /// Iterates latest applied provider statuses in provider-id order.
    pub fn provider_statuses(&self) -> impl Iterator<Item = &CommandProviderStatus> + '_ {
        self.provider_statuses.values()
    }

    /// Returns the number of registered dynamic provider callbacks.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Starts a lifecycle-tracked provider request for app-owned async work.
    ///
    /// Bind the eventual response with [`CommandProviderResponse::for_request`] before applying it.
    /// If a newer request has started for the same provider, the old response is reported as stale
    /// and does not replace the current provider sources.
    pub fn begin_provider_request(
        &mut self,
        provider_id: impl Into<CommandProviderId>,
        query: &str,
    ) -> CommandProviderRequest {
        let provider_id = provider_id.into();
        let request_id = self.next_provider_request_id(provider_id.clone());
        let request = self.provider_request(query).request_id(request_id);
        self.provider_latest_requests
            .insert(provider_id, request.clone());
        request
    }

    /// Produces a response from one registered provider for an existing request.
    pub fn provider_response_for_request(
        &self,
        provider_id: impl Into<CommandProviderId>,
        request: &CommandProviderRequest,
    ) -> Option<CommandProviderResponse> {
        let provider_id = provider_id.into();
        self.providers
            .iter()
            .find(|entry| entry.provider_id == provider_id)
            .map(|entry| entry.provider.provide_commands(request))
    }

    /// Refreshes one registered provider for a query and applies its response.
    pub fn refresh_provider(
        &mut self,
        provider_id: impl Into<CommandProviderId>,
        query: &str,
    ) -> Option<Result<CommandProviderStatus, CommandRegistryError>> {
        let provider_id = provider_id.into();
        let provider = self
            .providers
            .iter()
            .find(|entry| entry.provider_id == provider_id)
            .map(|entry| Arc::clone(&entry.provider))?;
        let request = self.begin_provider_request(provider_id.clone(), query);
        let response = provider.provide_commands(&request).for_request(&request);
        Some(self.apply_current_provider_response(provider_id, response))
    }

    /// Refreshes all registered providers for a query and applies their responses in order.
    pub fn refresh_providers(
        &mut self,
        query: &str,
    ) -> Result<Vec<CommandProviderStatus>, CommandRegistryError> {
        let providers = self
            .providers
            .iter()
            .map(|entry| (entry.provider_id.clone(), Arc::clone(&entry.provider)))
            .collect::<Vec<_>>();
        let mut statuses = Vec::with_capacity(providers.len());
        for (provider_id, provider) in providers {
            let request = self.begin_provider_request(provider_id.clone(), query);
            let response = provider.provide_commands(&request);
            statuses.push(
                self.apply_current_provider_response(provider_id, response.for_request(&request))?,
            );
        }
        Ok(statuses)
    }

    /// Applies an externally produced provider response.
    ///
    /// This is the runtime-neutral async boundary: applications can compute provider results in
    /// their own task system and apply the latest response when it completes. Responses bound to a
    /// provider request id are ignored as stale when a newer request has started.
    pub fn apply_provider_response(
        &mut self,
        provider_id: impl Into<CommandProviderId>,
        response: CommandProviderResponse,
    ) -> Result<CommandProviderApplyOutcome, CommandRegistryError> {
        let provider_id = provider_id.into();
        if let Some(stale) = self.stale_provider_response(&provider_id, response.request_id_ref()) {
            return Ok(CommandProviderApplyOutcome::Stale(stale));
        }
        self.apply_provider_response_unchecked(provider_id, response)
            .map(CommandProviderApplyOutcome::Applied)
    }

    /// Applies an externally produced response for a specific provider request.
    pub fn apply_provider_response_for_request(
        &mut self,
        provider_id: impl Into<CommandProviderId>,
        request: &CommandProviderRequest,
        response: CommandProviderResponse,
    ) -> Result<CommandProviderApplyOutcome, CommandRegistryError> {
        self.apply_provider_response(provider_id, response.for_request(request))
    }

    fn apply_current_provider_response(
        &mut self,
        provider_id: CommandProviderId,
        response: CommandProviderResponse,
    ) -> Result<CommandProviderStatus, CommandRegistryError> {
        self.apply_provider_response_unchecked(provider_id, response)
    }

    fn apply_provider_response_unchecked(
        &mut self,
        provider_id: CommandProviderId,
        response: CommandProviderResponse,
    ) -> Result<CommandProviderStatus, CommandRegistryError> {
        let request_id = response.request_id_ref();
        let query = self.provider_response_query(&provider_id, request_id);
        let state = response.state();
        let message = response.message().map(str::to_owned);
        let sources = response.sources_ref().to_vec();
        let mut next_registry = self.registry.clone();
        if let Some(source_ids) = self.provider_sources.get(&provider_id) {
            for source_id in source_ids {
                next_registry.unregister_source(source_id.clone());
            }
        }
        for source in &sources {
            register_provider_source(&mut next_registry, source)?;
        }

        let source_ids = sources
            .iter()
            .map(|source| source.source_id().clone())
            .collect::<Vec<_>>();
        let command_count = sources
            .iter()
            .map(CommandProviderSource::len)
            .sum::<usize>();
        let status = CommandProviderStatus::new(
            provider_id.clone(),
            request_id,
            query,
            state,
            message,
            sources.len(),
            command_count,
        );

        self.registry = next_registry;
        self.provider_sources
            .insert(provider_id.clone(), source_ids);
        self.provider_statuses.insert(provider_id, status.clone());
        Ok(status)
    }

    /// Registers contributions from one source in one scope.
    pub fn register_source(
        &mut self,
        scope_id: impl Into<CommandScopeId>,
        source_id: impl Into<CommandSourceId>,
        contributions: impl IntoIterator<Item = CommandContribution>,
    ) -> Result<CommandSourceRegistration, CommandRegistryError> {
        let scope_id = scope_id.into();
        let source_id = source_id.into();
        let sourced_contributions = sourced_contributions(&source_id, contributions);

        self.registry
            .register_all_in_scope(scope_id.clone(), sourced_contributions)?;
        Ok(CommandSourceRegistration::new(scope_id, source_id))
    }

    /// Unregisters all commands from the given source registration.
    pub fn unregister(&mut self, registration: &CommandSourceRegistration) -> usize {
        self.unregister_source(registration.source_id().clone())
    }

    /// Unregisters all commands from a source id.
    pub fn unregister_source(&mut self, source_id: impl Into<CommandSourceId>) -> usize {
        self.registry.unregister_source(source_id)
    }

    /// Unregisters a complete scope.
    pub fn unregister_scope(&mut self, scope_id: impl Into<CommandScopeId>) -> bool {
        self.registry.unregister_scope(scope_id)
    }

    /// Registers a GPUI action for a command id.
    pub fn register_action(
        &mut self,
        command_id: impl Into<String>,
        action: impl Action + 'static,
    ) -> &mut Self {
        self.actions.register_action(command_id, action);
        self
    }

    /// Registers a boxed GPUI action for a command id.
    pub fn register_boxed_action(
        &mut self,
        command_id: impl Into<String>,
        action: Box<dyn Action>,
    ) -> &mut Self {
        self.actions.register_boxed_action(command_id, action);
        self
    }

    /// Projects active scopes before availability, shortcut, or history ranking.
    pub fn scope_projection(&self) -> CommandScopeProjection {
        if self.active_scopes.is_empty() {
            self.registry.project_all_scopes()
        } else {
            self.registry
                .project_active_scopes(self.active_scopes.iter().cloned())
        }
    }

    /// Returns diagnostics from the active scope projection.
    pub fn projection_diagnostics(&self) -> Vec<CommandProjectionDiagnostic> {
        self.scope_projection().diagnostics().to_vec()
    }

    /// Projects a UI-neutral command snapshot without shortcut projection.
    pub fn snapshot(&self) -> CommandRegistrySnapshot {
        let scoped = self.scope_projection().into_snapshot();
        self.rank_snapshot(scoped.with_availability(&self.availability))
    }

    /// Projects a UI-neutral command snapshot with app-level keymap shortcuts.
    pub fn snapshot_for_keymap(&self, keymap: &Keymap) -> CommandRegistrySnapshot {
        let scoped = self.scope_projection().into_snapshot();
        let available = scoped.with_availability(&self.availability);
        let with_shortcuts = self
            .actions
            .registry_snapshot_with_keymap_shortcuts(&available, keymap);
        self.rank_snapshot(with_shortcuts)
    }

    /// Projects a UI-neutral command snapshot with focused-window shortcut precedence.
    pub fn snapshot_for_window(&self, window: &Window) -> CommandRegistrySnapshot {
        let scoped = self.scope_projection().into_snapshot();
        let available = scoped.with_availability(&self.availability);
        let with_shortcuts = self
            .actions
            .registry_snapshot_with_window_shortcuts(&available, window);
        self.rank_snapshot(with_shortcuts)
    }

    /// Searches and ranks the center snapshot without shortcut projection.
    pub fn search_snapshot(&self, query: &str) -> CommandRegistrySnapshot {
        self.search_rank_snapshot(self.snapshot(), query)
    }

    /// Searches and ranks the center snapshot with app-level keymap shortcuts.
    pub fn search_snapshot_for_keymap(
        &self,
        query: &str,
        keymap: &Keymap,
    ) -> CommandRegistrySnapshot {
        self.search_rank_snapshot(self.snapshot_for_keymap(keymap), query)
    }

    /// Searches and ranks the center snapshot with focused-window shortcuts.
    pub fn search_snapshot_for_window(
        &self,
        query: &str,
        window: &Window,
    ) -> CommandRegistrySnapshot {
        self.search_rank_snapshot(self.snapshot_for_window(window), query)
    }

    /// Builds a command menu tree from the center snapshot.
    pub fn menu_tree(&self) -> CommandMenuTree {
        CommandMenuTree::from_registry_snapshot(&self.snapshot())
    }

    /// Builds a command menu tree from the center keymap snapshot.
    pub fn menu_tree_for_keymap(&self, keymap: &Keymap) -> CommandMenuTree {
        CommandMenuTree::from_registry_snapshot(&self.snapshot_for_keymap(keymap))
    }

    /// Builds a command menu tree from the center window snapshot.
    pub fn menu_tree_for_window(&self, window: &Window) -> CommandMenuTree {
        CommandMenuTree::from_registry_snapshot(&self.snapshot_for_window(window))
    }

    /// Dispatches a command through GPUI app routing and records usage on success.
    pub fn dispatch_in_app(
        &mut self,
        command_id: &str,
        query: &str,
        cx: &mut App,
    ) -> CommandDispatchOutcome {
        let registry = self.scope_projection().into_snapshot();
        let outcome = self.actions.dispatch_available_command_in_app(
            command_id,
            &registry,
            &self.availability,
            cx,
        );
        self.record_successful_dispatch(&outcome, command_id, query);
        outcome
    }

    /// Dispatches a command through one GPUI window and records usage on success.
    pub fn dispatch_in_window(
        &mut self,
        command_id: &str,
        query: &str,
        window: &mut Window,
        cx: &mut App,
    ) -> CommandDispatchOutcome {
        let registry = self.scope_projection().into_snapshot();
        let outcome = self.actions.dispatch_available_command_in_window(
            command_id,
            &registry,
            &self.availability,
            window,
            cx,
        );
        self.record_successful_dispatch(&outcome, command_id, query);
        outcome
    }

    fn rank_snapshot(&self, snapshot: CommandRegistrySnapshot) -> CommandRegistrySnapshot {
        self.history.rank_registry_snapshot(&snapshot)
    }

    fn search_rank_snapshot(
        &self,
        snapshot: CommandRegistrySnapshot,
        query: &str,
    ) -> CommandRegistrySnapshot {
        if query.is_empty() {
            return snapshot;
        }

        let mut scored = snapshot
            .contributions()
            .iter()
            .cloned()
            .enumerate()
            .filter_map(|(index, contribution)| {
                command_search_score(contribution.descriptor(), query).map(|search_score| {
                    let history_score = self.history.usage_count(contribution.descriptor().id());
                    (search_score, history_score, index, contribution)
                })
            })
            .collect::<Vec<_>>();
        scored.sort_by(
            |(left_search, left_history, left_index, _),
             (right_search, right_history, right_index, _)| {
                right_search
                    .cmp(left_search)
                    .then_with(|| right_history.cmp(left_history))
                    .then_with(|| left_index.cmp(right_index))
            },
        );

        CommandRegistrySnapshot::new(
            snapshot.revision(),
            scored
                .into_iter()
                .map(|(_, _, _, contribution)| contribution)
                .collect::<Vec<_>>(),
        )
    }

    fn record_successful_dispatch(
        &mut self,
        outcome: &CommandDispatchOutcome,
        command_id: &str,
        query: &str,
    ) {
        if outcome.dispatched() {
            self.history.record_usage(command_id, query);
        }
    }

    fn provider_request(&self, query: &str) -> CommandProviderRequest {
        CommandProviderRequest::new(query).active_scopes(self.active_scopes.iter().cloned())
    }

    fn next_provider_request_id(
        &mut self,
        provider_id: CommandProviderId,
    ) -> CommandProviderRequestId {
        let counter = self
            .provider_request_counters
            .entry(provider_id)
            .or_default();
        *counter = counter.saturating_add(1);
        CommandProviderRequestId::new(*counter)
    }

    fn stale_provider_response(
        &self,
        provider_id: &CommandProviderId,
        response_request_id: Option<CommandProviderRequestId>,
    ) -> Option<CommandProviderStaleResponse> {
        let response_request_id = response_request_id?;
        let current_request_id = self
            .provider_latest_requests
            .get(provider_id)
            .and_then(CommandProviderRequest::request_id_ref);
        (current_request_id != Some(response_request_id)).then(|| {
            CommandProviderStaleResponse::new(
                provider_id.clone(),
                response_request_id,
                current_request_id,
            )
        })
    }

    fn provider_response_query(
        &self,
        provider_id: &CommandProviderId,
        response_request_id: Option<CommandProviderRequestId>,
    ) -> Option<String> {
        let response_request_id = response_request_id?;
        self.provider_latest_requests
            .get(provider_id)
            .filter(|request| request.request_id_ref() == Some(response_request_id))
            .map(|request| request.query().to_owned())
    }

    fn unregister_provider_sources(&mut self, provider_id: &CommandProviderId) -> usize {
        let source_ids = self
            .provider_sources
            .remove(provider_id)
            .unwrap_or_default();
        source_ids
            .into_iter()
            .map(|source_id| self.registry.unregister_source(source_id))
            .sum()
    }
}

fn register_provider_source(
    registry: &mut ScopedCommandRegistry,
    source: &CommandProviderSource,
) -> Result<(), CommandRegistryError> {
    registry.register_all_in_scope(
        source.scope_id().clone(),
        sourced_contributions(source.source_id(), source.contributions().iter().cloned()),
    )
}

fn sourced_contributions(
    source_id: &CommandSourceId,
    contributions: impl IntoIterator<Item = CommandContribution>,
) -> Vec<CommandContribution> {
    contributions
        .into_iter()
        .map(|contribution| {
            CommandContribution::new(contribution.descriptor().clone()).source(source_id.clone())
        })
        .collect()
}

fn command_search_score(descriptor: &crate::CommandDescriptor, query: &str) -> Option<u16> {
    let query = normalize_command_text(query);
    if query.is_empty() {
        return Some(0);
    }

    [
        (descriptor.label(), 4000),
        (descriptor.id(), 3600),
        (descriptor.shortcut_ref().unwrap_or_default(), 2400),
    ]
    .into_iter()
    .chain(
        descriptor
            .keywords_ref()
            .iter()
            .map(|keyword| (keyword.as_str(), 1800)),
    )
    .filter_map(|(text, base)| command_text_score(text, &query).map(|score| base + score))
    .max()
}

fn command_text_score(text: &str, normalized_query: &str) -> Option<u16> {
    let text = normalize_command_text(text);
    if text.is_empty() {
        return None;
    }
    if text == normalized_query {
        return Some(1000);
    }
    if text.starts_with(normalized_query) {
        return Some(850);
    }
    if text
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|word| word.starts_with(normalized_query))
    {
        return Some(760);
    }
    if text.contains(normalized_query) {
        return Some(640);
    }
    fuzzy_subsequence_score(&text, normalized_query).map(|score| 420 + score)
}

fn fuzzy_subsequence_score(text: &str, normalized_query: &str) -> Option<u16> {
    let mut score = 0u16;
    let mut query = normalized_query.chars();
    let mut current = query.next()?;
    let mut previous_match = None;

    for (index, ch) in text.chars().enumerate() {
        if ch == current {
            score = score.saturating_add(if previous_match == Some(index.saturating_sub(1)) {
                24
            } else {
                8
            });
            previous_match = Some(index);
            if let Some(next) = query.next() {
                current = next;
            } else {
                return Some(score);
            }
        }
    }

    None
}

fn normalize_command_text(text: &str) -> String {
    text.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use open_gpui::{Action, KeyBinding, Keymap, actions};

    use crate::{
        CommandAvailabilityMap, CommandCenter, CommandContribution, CommandDescriptor,
        CommandDispatchOutcome, CommandMenuEntry, CommandProjectionDiagnosticKind,
        CommandProviderApplyOutcome, CommandProviderResponse, CommandProviderSource,
        CommandProviderState, CommandProviderStatus, CommandUsageHistory,
    };

    actions!(center_test_only, [OpenWorkspace, SaveWorkspace]);

    #[derive(Clone, PartialEq, Default, Debug, Action)]
    #[action(no_json)]
    struct CenterDispatchProbe;

    #[test]
    fn center_projects_scopes_availability_shortcuts_and_history() {
        let mut center = CommandCenter::new("center-v1");
        center
            .register_source(
                "global",
                "core",
                [
                    CommandContribution::new(
                        CommandDescriptor::new("workspace.open", "Open Workspace")
                            .group("Workspace")
                            .keyword("project")
                            .menu_path(["File", "Open"]),
                    ),
                    CommandContribution::new(
                        CommandDescriptor::new("workspace.save", "Save Workspace")
                            .group("Workspace")
                            .menu_path(["File", "Save"]),
                    ),
                ],
            )
            .unwrap();
        center
            .register_source(
                "editor",
                "editor",
                [CommandContribution::new(CommandDescriptor::new(
                    "workspace.open",
                    "Open Editor Workspace",
                ))],
            )
            .unwrap();
        center
            .set_active_scopes(["global", "editor"])
            .set_availability(
                CommandAvailabilityMap::new()
                    .disabled("workspace.open", "Editor is busy")
                    .hidden("workspace.save"),
            )
            .register_action("workspace.open", OpenWorkspace)
            .register_action("workspace.save", SaveWorkspace);
        center.history_mut().record_usage("workspace.open", "open");

        let mut keymap = Keymap::default();
        keymap.add_bindings([
            KeyBinding::new("ctrl-o", OpenWorkspace, None),
            KeyBinding::new("ctrl-shift-o", OpenWorkspace, None),
        ]);
        let snapshot = center.snapshot_for_keymap(&keymap);
        let descriptors = snapshot.descriptors().collect::<Vec<_>>();

        assert_eq!(descriptors.len(), 1);
        assert_eq!(descriptors[0].id(), "workspace.open");
        assert_eq!(descriptors[0].label(), "Open Editor Workspace");
        assert!(descriptors[0].disabled_state());
        assert_eq!(descriptors[0].disabled_reason_ref(), Some("Editor is busy"));
        assert_eq!(descriptors[0].shortcut_ref(), Some("ctrl-shift-O"));
        assert_eq!(center.projection_diagnostics().len(), 1);
        assert_eq!(
            center.projection_diagnostics()[0].kind(),
            CommandProjectionDiagnosticKind::DuplicateCommandId
        );

        let menu = center.menu_tree_for_keymap(&keymap);
        assert!(
            menu.entries().iter().all(|entry| {
                entry
                    .as_command()
                    .is_none_or(|command| command.command_id() != "workspace.save")
            }),
            "hidden command should not be present in the center menu"
        );
    }

    #[test]
    fn center_unregisters_sources_from_snapshots_and_menus() {
        let mut center = CommandCenter::new("center-v1");
        let registration = center
            .register_source(
                "global",
                "plugin",
                [CommandContribution::new(
                    CommandDescriptor::new("plugin.run", "Run Plugin").menu_path(["Tools"]),
                )],
            )
            .unwrap();

        assert!(center.snapshot().descriptor("plugin.run").is_some());
        assert_eq!(center.unregister(&registration), 1);
        assert!(center.snapshot().descriptor("plugin.run").is_none());
        assert!(center.menu_tree().entries().is_empty());
        assert_eq!(center.unregister(&registration), 0);
    }

    #[open_gpui::test]
    fn center_dispatch_checks_availability_and_records_history(cx: &mut open_gpui::TestAppContext) {
        let dispatched = Rc::new(RefCell::new(Vec::new()));
        cx.update(|cx| {
            let dispatched = dispatched.clone();
            cx.on_action(move |_: &CenterDispatchProbe, _| {
                dispatched.borrow_mut().push("workspace.open".to_owned());
            });
        });

        let mut center = CommandCenter::new("center-v1");
        center
            .register_source(
                "global",
                "core",
                [CommandContribution::new(CommandDescriptor::new(
                    "workspace.open",
                    "Open Workspace",
                ))],
            )
            .unwrap();
        center.register_action("workspace.open", CenterDispatchProbe);

        cx.update(|cx| {
            assert_eq!(
                center.dispatch_in_app("workspace.open", "open", cx),
                CommandDispatchOutcome::Dispatched
            );
        });

        assert_eq!(dispatched.borrow().as_slice(), ["workspace.open"]);
        assert_eq!(center.history().usage_count("workspace.open"), 1);
        assert_eq!(center.history().last_query(), Some("open"));

        center.set_availability(CommandAvailabilityMap::new().hidden("workspace.open"));
        cx.update(|cx| {
            assert_eq!(
                center.dispatch_in_app("workspace.open", "open", cx),
                CommandDispatchOutcome::Hidden
            );
        });
        assert_eq!(center.history().usage_count("workspace.open"), 1);
    }

    #[test]
    fn center_search_snapshot_uses_fuzzy_matching_and_history_tiebreaks() {
        let mut center = CommandCenter::new("center-v1");
        center
            .register_source(
                "global",
                "core",
                [
                    CommandContribution::new(CommandDescriptor::new(
                        "workspace.open",
                        "Open Workspace",
                    )),
                    CommandContribution::new(
                        CommandDescriptor::new("workspace.save", "Save Workspace")
                            .keyword("persist"),
                    ),
                    CommandContribution::new(CommandDescriptor::new(
                        "pane.toggle",
                        "Toggle Left Pane",
                    )),
                ],
            )
            .unwrap();
        center
            .history_mut()
            .record_usage("workspace.save", "workspace");

        let ids = center
            .search_snapshot("ws")
            .descriptors()
            .map(|descriptor| descriptor.id().to_owned())
            .collect::<Vec<_>>();

        assert_eq!(ids, ["workspace.save", "workspace.open"]);
    }

    #[test]
    fn center_refreshes_provider_sources_with_active_scope_request() {
        let mut center = CommandCenter::new("center-v1");
        center.set_active_scopes(["global"]);
        center.register_provider("recent-files", |request: &crate::CommandProviderRequest| {
            assert!(request.request_id_ref().is_some());
            assert_eq!(request.active_scopes_ref()[0].as_str(), "global");
            CommandProviderResponse::ready().source(CommandProviderSource::new(
                "global",
                "recent-files-source",
                [CommandContribution::new(
                    CommandDescriptor::new(
                        format!("recent.{}", request.query()),
                        format!("Recent {}", request.query()),
                    )
                    .keyword("dynamic"),
                )],
            ))
        });

        let status = center
            .refresh_provider("recent-files", "alpha")
            .expect("provider should be registered")
            .unwrap();
        let first_request_id = status.request_id().expect("refreshes are tracked");
        assert_eq!(first_request_id.get(), 1);
        assert_eq!(status.query(), Some("alpha"));
        assert_eq!(status.state(), CommandProviderState::Ready);
        assert_eq!(status.source_count(), 1);
        assert_eq!(status.command_count(), 1);
        assert_eq!(
            center
                .provider_status("recent-files")
                .map(CommandProviderStatus::command_count),
            Some(1)
        );
        assert!(center.snapshot().descriptor("recent.alpha").is_some());

        let status = center
            .refresh_provider("recent-files", "beta")
            .expect("provider should still be registered")
            .unwrap();
        assert_eq!(
            status.request_id().map(|request_id| request_id.get()),
            Some(2)
        );
        assert_eq!(status.query(), Some("beta"));
        let snapshot = center.snapshot();
        assert!(snapshot.descriptor("recent.alpha").is_none());
        assert_eq!(
            snapshot
                .contribution("recent.beta")
                .and_then(CommandContribution::source_ref),
            Some("recent-files-source")
        );
    }

    #[test]
    fn center_applies_external_provider_response_for_async_boundaries() {
        let mut center = CommandCenter::new("center-v1");

        let outcome = center
            .apply_provider_response(
                "async-search",
                CommandProviderResponse::loading("Searching").source(CommandProviderSource::new(
                    "global",
                    "async-search-source",
                    [CommandContribution::new(
                        CommandDescriptor::new("async.open", "Open Async Result").keyword("search"),
                    )],
                )),
            )
            .unwrap();
        let status = outcome.status().expect("unbound response should apply");

        assert!(outcome.applied());
        assert_eq!(status.request_id(), None);
        assert_eq!(status.query(), None);
        assert_eq!(status.state(), CommandProviderState::Loading);
        assert_eq!(status.message(), Some("Searching"));
        assert_eq!(
            center
                .search_snapshot("async")
                .descriptors()
                .map(CommandDescriptor::id)
                .collect::<Vec<_>>(),
            ["async.open"]
        );
    }

    #[test]
    fn center_ignores_stale_provider_response_for_old_request() {
        let mut center = CommandCenter::new("center-v1");
        let alpha_request = center.begin_provider_request("async-search", "alpha");
        let alpha_outcome = center
            .apply_provider_response_for_request(
                "async-search",
                &alpha_request,
                CommandProviderResponse::ready().source(CommandProviderSource::new(
                    "global",
                    "async-search-source",
                    [CommandContribution::new(CommandDescriptor::new(
                        "async.alpha",
                        "Async Alpha",
                    ))],
                )),
            )
            .unwrap();
        assert!(alpha_outcome.applied());
        assert!(center.snapshot().descriptor("async.alpha").is_some());

        let beta_request = center.begin_provider_request("async-search", "beta");
        let stale_outcome = center
            .apply_provider_response_for_request(
                "async-search",
                &alpha_request,
                CommandProviderResponse::ready().source(CommandProviderSource::new(
                    "global",
                    "async-search-source",
                    [CommandContribution::new(CommandDescriptor::new(
                        "async.alpha.late",
                        "Late Async Alpha",
                    ))],
                )),
            )
            .unwrap();

        let CommandProviderApplyOutcome::Stale(stale) = stale_outcome else {
            panic!("expected stale response");
        };
        assert_eq!(stale.provider_id().as_str(), "async-search");
        assert_eq!(
            stale.response_request_id(),
            alpha_request.request_id_ref().unwrap()
        );
        assert_eq!(stale.current_request_id(), beta_request.request_id_ref());
        assert!(center.snapshot().descriptor("async.alpha").is_some());
        assert!(center.snapshot().descriptor("async.alpha.late").is_none());

        let beta_outcome = center
            .apply_provider_response_for_request(
                "async-search",
                &beta_request,
                CommandProviderResponse::ready().source(CommandProviderSource::new(
                    "global",
                    "async-search-source",
                    [CommandContribution::new(CommandDescriptor::new(
                        "async.beta",
                        "Async Beta",
                    ))],
                )),
            )
            .unwrap();
        let beta_status = beta_outcome
            .status()
            .expect("current response should apply");
        assert_eq!(beta_status.request_id(), beta_request.request_id_ref());
        assert_eq!(beta_status.query(), Some("beta"));
        assert!(center.snapshot().descriptor("async.alpha").is_none());
        assert!(center.snapshot().descriptor("async.beta").is_some());
    }

    #[test]
    fn center_provider_request_ids_do_not_reuse_after_unregister() {
        let mut center = CommandCenter::new("center-v1");
        let first_request = center.begin_provider_request("dynamic", "alpha");

        center.unregister_provider_id("dynamic");
        let second_request = center.begin_provider_request("dynamic", "beta");

        assert_eq!(
            second_request
                .request_id_ref()
                .map(|request_id| request_id.get()),
            first_request
                .request_id_ref()
                .map(|request_id| request_id.get() + 1)
        );
        let stale_outcome = center
            .apply_provider_response_for_request(
                "dynamic",
                &first_request,
                CommandProviderResponse::ready().source(CommandProviderSource::new(
                    "global",
                    "dynamic-source",
                    [CommandContribution::new(CommandDescriptor::new(
                        "dynamic.old",
                        "Old Dynamic",
                    ))],
                )),
            )
            .unwrap();

        assert!(stale_outcome.stale());
        assert!(center.snapshot().descriptor("dynamic.old").is_none());
    }

    #[test]
    fn center_unregister_provider_removes_applied_sources() {
        let mut center = CommandCenter::new("center-v1");
        let registration =
            center.register_provider("dynamic", |_: &crate::CommandProviderRequest| {
                CommandProviderResponse::ready().source(CommandProviderSource::new(
                    "global",
                    "dynamic-source",
                    [CommandContribution::new(CommandDescriptor::new(
                        "dynamic.open",
                        "Open Dynamic",
                    ))],
                ))
            });

        center
            .refresh_provider("dynamic", "")
            .expect("provider should be registered")
            .unwrap();
        assert_eq!(center.provider_count(), 1);
        assert!(center.snapshot().descriptor("dynamic.open").is_some());

        assert_eq!(center.unregister_provider(&registration), 1);
        assert_eq!(center.provider_count(), 0);
        assert!(center.provider_status("dynamic").is_none());
        assert!(center.snapshot().descriptor("dynamic.open").is_none());
    }

    #[test]
    fn center_provider_response_is_atomic_on_registration_error() {
        let mut center = CommandCenter::new("center-v1");
        center
            .apply_provider_response(
                "dynamic",
                CommandProviderResponse::ready().source(CommandProviderSource::new(
                    "global",
                    "dynamic-source",
                    [CommandContribution::new(CommandDescriptor::new(
                        "dynamic.ok",
                        "Dynamic OK",
                    ))],
                )),
            )
            .unwrap()
            .into_status()
            .unwrap();

        let error = center
            .apply_provider_response(
                "dynamic",
                CommandProviderResponse::ready().source(CommandProviderSource::new(
                    "global",
                    "bad-source",
                    [
                        CommandContribution::new(CommandDescriptor::new("duplicate", "First")),
                        CommandContribution::new(CommandDescriptor::new("duplicate", "Second")),
                    ],
                )),
            )
            .unwrap_err();

        assert_eq!(error.id(), "duplicate");
        let snapshot = center.snapshot();
        assert!(snapshot.descriptor("dynamic.ok").is_some());
        assert!(snapshot.descriptor("duplicate").is_none());
        assert_eq!(
            center
                .provider_status("dynamic")
                .map(CommandProviderStatus::command_count),
            Some(1)
        );
    }

    #[test]
    fn center_menu_tree_uses_projected_snapshot() {
        let mut center = CommandCenter::new("center-v1");
        center
            .register_source(
                "global",
                "core",
                [CommandContribution::new(
                    CommandDescriptor::new("workspace.open", "Open Workspace")
                        .menu_path(["File", "Open"]),
                )],
            )
            .unwrap();

        let menu = center.menu_tree();
        let Some(CommandMenuEntry::Submenu(file)) = menu.entries().first() else {
            panic!("expected File submenu");
        };
        assert_eq!(file.label(), "File");
    }
}
