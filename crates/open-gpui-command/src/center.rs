//! App-owned command runtime facade.

use open_gpui::{Action, App, Keymap, Window};

use crate::{
    CommandAvailabilityMap, CommandContribution, CommandDispatchOutcome, CommandMenuTree,
    CommandProjectionDiagnostic, CommandRegistryError, CommandRegistrySnapshot, CommandScopeId,
    CommandScopeProjection, CommandSourceId, CommandUsageHistory, GpuiCommandActionMap,
    MemoryCommandHistory, ScopedCommandRegistry,
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

    /// Registers contributions from one source in one scope.
    pub fn register_source(
        &mut self,
        scope_id: impl Into<CommandScopeId>,
        source_id: impl Into<CommandSourceId>,
        contributions: impl IntoIterator<Item = CommandContribution>,
    ) -> Result<CommandSourceRegistration, CommandRegistryError> {
        let scope_id = scope_id.into();
        let source_id = source_id.into();
        let sourced_contributions = contributions
            .into_iter()
            .map(|contribution| {
                CommandContribution::new(contribution.descriptor().clone())
                    .source(source_id.clone())
            })
            .collect::<Vec<_>>();

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
        CommandUsageHistory,
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
