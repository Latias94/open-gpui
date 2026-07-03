//! GPUI action and keymap adapters for command registry projections.

use std::collections::BTreeMap;

use open_gpui::{Action, App, KeyBinding, Keymap, Window};

use crate::{
    CommandAvailability, CommandAvailabilityResolver, CommandRegistrySnapshot, CommandUsageHistory,
    command_effective_availability,
};

/// Result of attempting to dispatch a command id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandDispatchOutcome {
    /// The command id exists in the registry and a matching GPUI action was dispatched.
    Dispatched,
    /// No descriptor exists for the command id in the checked registry snapshot.
    MissingCommand,
    /// No GPUI action binding exists for the command id.
    MissingAction,
    /// The command is visible but disabled.
    Disabled {
        /// Optional disabled reason.
        reason: Option<String>,
    },
    /// The command is hidden in the current availability projection.
    Hidden,
}

impl CommandDispatchOutcome {
    /// Returns whether dispatch succeeded.
    pub const fn dispatched(&self) -> bool {
        matches!(self, Self::Dispatched)
    }
}

/// One command id mapped to a concrete GPUI action.
pub struct GpuiCommandAction {
    command_id: String,
    action: Box<dyn Action>,
}

impl Clone for GpuiCommandAction {
    fn clone(&self) -> Self {
        Self {
            command_id: self.command_id.clone(),
            action: self.action.boxed_clone(),
        }
    }
}

impl std::fmt::Debug for GpuiCommandAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuiCommandAction")
            .field("command_id", &self.command_id)
            .field("action", &self.action.name())
            .finish()
    }
}

impl GpuiCommandAction {
    /// Creates a command action binding from a stable command id and GPUI action value.
    pub fn new(command_id: impl Into<String>, action: impl Action + 'static) -> Self {
        Self {
            command_id: command_id.into(),
            action: Box::new(action),
        }
    }

    /// Creates a command action binding from a boxed GPUI action.
    pub fn boxed(command_id: impl Into<String>, action: Box<dyn Action>) -> Self {
        Self {
            command_id: command_id.into(),
            action,
        }
    }

    /// Returns the stable command id.
    pub fn command_id(&self) -> &str {
        &self.command_id
    }

    /// Returns the GPUI action prototype.
    pub fn action(&self) -> &dyn Action {
        self.action.as_ref()
    }

    /// Clones the GPUI action for window dispatch.
    pub fn boxed_action(&self) -> Box<dyn Action> {
        self.action.boxed_clone()
    }
}

/// Deterministic GPUI action map for command registry ids.
#[derive(Debug, Clone, Default)]
pub struct GpuiCommandActionMap {
    actions: Vec<GpuiCommandAction>,
}

impl GpuiCommandActionMap {
    /// Creates an empty command action map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a command action map from action bindings.
    pub fn from_actions(actions: impl IntoIterator<Item = GpuiCommandAction>) -> Self {
        Self {
            actions: actions.into_iter().collect(),
        }
    }

    /// Adds one command id to GPUI action binding.
    pub fn action(mut self, command_id: impl Into<String>, action: impl Action + 'static) -> Self {
        self.actions
            .push(GpuiCommandAction::new(command_id, action));
        self
    }

    /// Adds one boxed command id to GPUI action binding.
    pub fn boxed_action(mut self, command_id: impl Into<String>, action: Box<dyn Action>) -> Self {
        self.actions
            .push(GpuiCommandAction::boxed(command_id, action));
        self
    }

    /// Returns all registered command action bindings.
    pub fn actions(&self) -> &[GpuiCommandAction] {
        &self.actions
    }

    /// Returns the GPUI action binding for a command id.
    pub fn action_for_command(&self, command_id: &str) -> Option<&GpuiCommandAction> {
        self.actions
            .iter()
            .rev()
            .find(|candidate| candidate.command_id() == command_id)
    }

    /// Projects app-level keymap shortcuts onto a command registry snapshot.
    ///
    /// This uses [`Keymap::bindings_for_action`], whose display contract is that the final matching
    /// binding has precedence.
    pub fn registry_snapshot_with_keymap_shortcuts(
        &self,
        registry: &CommandRegistrySnapshot,
        keymap: &Keymap,
    ) -> CommandRegistrySnapshot {
        let shortcuts = self
            .actions
            .iter()
            .filter_map(|command_action| {
                command_shortcut_label_from_keymap(keymap, command_action.action())
                    .map(|label| (command_action.command_id().to_owned(), label))
            })
            .collect::<BTreeMap<_, _>>();

        registry_snapshot_with_shortcuts(registry, &shortcuts)
    }

    /// Projects focused-window keymap shortcuts onto a command registry snapshot.
    pub fn registry_snapshot_with_window_shortcuts(
        &self,
        registry: &CommandRegistrySnapshot,
        window: &Window,
    ) -> CommandRegistrySnapshot {
        let shortcuts = self
            .actions
            .iter()
            .filter_map(|command_action| {
                window
                    .highest_precedence_binding_for_action(command_action.action())
                    .map(|binding| {
                        (
                            command_action.command_id().to_owned(),
                            command_shortcut_label(&binding),
                        )
                    })
            })
            .collect::<BTreeMap<_, _>>();

        registry_snapshot_with_shortcuts(registry, &shortcuts)
    }

    /// Dispatches a command id through the current GPUI window.
    pub fn dispatch_command_in_window(
        &self,
        command_id: &str,
        window: &mut Window,
        cx: &mut App,
    ) -> CommandDispatchOutcome {
        let Some(command_action) = self.action_for_command(command_id) else {
            return CommandDispatchOutcome::MissingAction;
        };

        window.dispatch_action(command_action.boxed_action(), cx);
        CommandDispatchOutcome::Dispatched
    }

    /// Dispatches a command id through the app's focused action window or global handlers.
    pub fn dispatch_command_in_app(
        &self,
        command_id: &str,
        cx: &mut App,
    ) -> CommandDispatchOutcome {
        let Some(command_action) = self.action_for_command(command_id) else {
            return CommandDispatchOutcome::MissingAction;
        };

        cx.dispatch_action(command_action.action());
        CommandDispatchOutcome::Dispatched
    }

    /// Dispatches a command id after checking a registry snapshot and availability resolver.
    pub fn dispatch_available_command_in_app(
        &self,
        command_id: &str,
        registry: &CommandRegistrySnapshot,
        resolver: &impl CommandAvailabilityResolver,
        cx: &mut App,
    ) -> CommandDispatchOutcome {
        let Some(descriptor) = registry.descriptor(command_id) else {
            return CommandDispatchOutcome::MissingCommand;
        };
        match command_effective_availability(descriptor, resolver) {
            CommandAvailability::Available => self.dispatch_command_in_app(command_id, cx),
            CommandAvailability::Disabled { reason } => CommandDispatchOutcome::Disabled { reason },
            CommandAvailability::Hidden => CommandDispatchOutcome::Hidden,
        }
    }

    /// Dispatches a command id in a window after checking registry availability.
    pub fn dispatch_available_command_in_window(
        &self,
        command_id: &str,
        registry: &CommandRegistrySnapshot,
        resolver: &impl CommandAvailabilityResolver,
        window: &mut Window,
        cx: &mut App,
    ) -> CommandDispatchOutcome {
        let Some(descriptor) = registry.descriptor(command_id) else {
            return CommandDispatchOutcome::MissingCommand;
        };
        match command_effective_availability(descriptor, resolver) {
            CommandAvailability::Available => {
                self.dispatch_command_in_window(command_id, window, cx)
            }
            CommandAvailability::Disabled { reason } => CommandDispatchOutcome::Disabled { reason },
            CommandAvailability::Hidden => CommandDispatchOutcome::Hidden,
        }
    }

    /// Dispatches an available app command and records successful usage in memory or custom history.
    pub fn dispatch_available_command_in_app_with_history(
        &self,
        command_id: &str,
        query: &str,
        registry: &CommandRegistrySnapshot,
        resolver: &impl CommandAvailabilityResolver,
        history: &mut impl CommandUsageHistory,
        cx: &mut App,
    ) -> CommandDispatchOutcome {
        let outcome = self.dispatch_available_command_in_app(command_id, registry, resolver, cx);
        if outcome.dispatched() {
            history.record_usage(command_id, query);
        }
        outcome
    }
}

/// Returns the display shortcut label for a key binding.
pub fn command_shortcut_label(binding: &KeyBinding) -> String {
    binding
        .keystrokes()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Returns the display shortcut label for an action in an app-level keymap.
pub fn command_shortcut_label_from_keymap(keymap: &Keymap, action: &dyn Action) -> Option<String> {
    keymap
        .bindings_for_action(action)
        .next_back()
        .map(command_shortcut_label)
}

fn registry_snapshot_with_shortcuts(
    registry: &CommandRegistrySnapshot,
    shortcuts: &BTreeMap<String, String>,
) -> CommandRegistrySnapshot {
    let contributions = registry
        .contributions()
        .iter()
        .map(|contribution| {
            let descriptor = if let Some(shortcut) = shortcuts.get(contribution.descriptor().id()) {
                contribution.descriptor().clone().shortcut(shortcut.clone())
            } else {
                contribution.descriptor().clone()
            };
            contribution.with_descriptor(descriptor)
        })
        .collect::<Vec<_>>();

    CommandRegistrySnapshot::new(registry.revision(), contributions)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use open_gpui::{KeyBinding, Keymap, actions};

    use crate::{
        CommandAvailabilityMap, CommandDescriptor, CommandDispatchOutcome, CommandRegistry,
        CommandUsageHistory, MemoryCommandHistory,
    };

    use super::*;

    actions!(test_only, [OpenWorkspace, SaveWorkspace,]);

    #[derive(Clone, PartialEq, Default, Debug, Action)]
    #[action(no_json)]
    struct DispatchProbe;

    #[test]
    fn shortcut_label_uses_keybinding_display_text() {
        let binding = KeyBinding::new("ctrl-k ctrl-s", SaveWorkspace, None);

        assert_eq!(command_shortcut_label(&binding), "ctrl-K ctrl-S");
    }

    #[test]
    fn keymap_shortcut_projection_uses_last_binding_for_display() {
        let mut keymap = Keymap::default();
        keymap.add_bindings([
            KeyBinding::new("ctrl-o", OpenWorkspace, None),
            KeyBinding::new("ctrl-shift-o", OpenWorkspace, None),
        ]);
        let registry = command_registry_snapshot();
        let action_map = GpuiCommandActionMap::new()
            .action("workspace.open", OpenWorkspace)
            .action("workspace.save", SaveWorkspace);

        let projected = action_map.registry_snapshot_with_keymap_shortcuts(&registry, &keymap);

        let shortcuts = projected
            .descriptors()
            .map(|descriptor| {
                (
                    descriptor.id().to_owned(),
                    descriptor.shortcut_ref().map(str::to_owned),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            shortcuts,
            [
                ("workspace.open".to_owned(), Some("ctrl-shift-O".to_owned())),
                ("workspace.save".to_owned(), Some("Ctrl+S".to_owned())),
            ]
        );
    }

    #[test]
    fn dispatch_command_reports_missing_action_without_dispatching() {
        let action_map = GpuiCommandActionMap::new().action("workspace.open", OpenWorkspace);

        assert!(action_map.action_for_command("workspace.open").is_some());
        assert!(action_map.action_for_command("workspace.missing").is_none());
    }

    #[open_gpui::test]
    fn dispatch_command_in_app_routes_registered_gpui_action(cx: &mut open_gpui::TestAppContext) {
        let dispatched = Rc::new(RefCell::new(Vec::new()));
        cx.update(|cx| {
            let dispatched = dispatched.clone();
            cx.on_action(move |_: &DispatchProbe, _| {
                dispatched.borrow_mut().push("workspace.open".to_owned());
            });
        });

        let action_map = GpuiCommandActionMap::new().action("workspace.open", DispatchProbe);

        cx.update(|cx| {
            assert_eq!(
                action_map.dispatch_command_in_app("workspace.open", cx),
                CommandDispatchOutcome::Dispatched
            );
        });

        assert_eq!(dispatched.borrow().as_slice(), ["workspace.open"]);
    }

    #[open_gpui::test]
    fn dispatch_available_command_blocks_hidden_commands(cx: &mut open_gpui::TestAppContext) {
        let action_map = GpuiCommandActionMap::new().action("workspace.open", DispatchProbe);
        let registry = command_registry_snapshot();
        let availability = CommandAvailabilityMap::new().hidden("workspace.open");

        cx.update(|cx| {
            assert_eq!(
                action_map.dispatch_available_command_in_app(
                    "workspace.open",
                    &registry,
                    &availability,
                    cx,
                ),
                CommandDispatchOutcome::Hidden
            );
        });
    }

    #[open_gpui::test]
    fn dispatch_available_command_records_usage_on_success(cx: &mut open_gpui::TestAppContext) {
        cx.update(|cx| {
            cx.on_action(|_: &DispatchProbe, _| {});
        });
        let action_map = GpuiCommandActionMap::new().action("workspace.open", DispatchProbe);
        let registry = command_registry_snapshot();
        let availability = CommandAvailabilityMap::new();
        let mut history = MemoryCommandHistory::default();

        cx.update(|cx| {
            assert_eq!(
                action_map.dispatch_available_command_in_app_with_history(
                    "workspace.open",
                    "open",
                    &registry,
                    &availability,
                    &mut history,
                    cx,
                ),
                CommandDispatchOutcome::Dispatched
            );
        });

        assert_eq!(history.usage_count("workspace.open"), 1);
    }

    fn command_registry_snapshot() -> CommandRegistrySnapshot {
        let mut registry = CommandRegistry::new("workspace-v1");
        registry
            .register(
                CommandDescriptor::new("workspace.open", "Open Workspace")
                    .group("Workspace")
                    .shortcut("Ctrl+O"),
            )
            .unwrap();
        registry
            .register(
                CommandDescriptor::new("workspace.save", "Save Workspace")
                    .group("Workspace")
                    .shortcut("Ctrl+S"),
            )
            .unwrap();
        registry.snapshot()
    }
}
