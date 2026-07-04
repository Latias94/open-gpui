//! Command-id keyed GPUI key binding registration.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use open_gpui::{
    Action, App, DummyKeyboardMapper, InvalidKeystrokeError, KeyBinding,
    KeyBindingContextPredicate, KeyContext, Keymap, Keystroke,
};

use crate::{
    CommandAvailability, CommandAvailabilityResolver, CommandRegistrySnapshot, CommandSourceId,
    GpuiCommandActionMap, command_effective_availability, gpui::command_shortcut_label,
};

/// One command-id keyed key binding contribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandKeyBinding {
    command_id: String,
    keystrokes: String,
    context: Option<String>,
}

impl CommandKeyBinding {
    /// Creates a key binding contribution for a command id.
    pub fn new(command_id: impl Into<String>, keystrokes: impl Into<String>) -> Self {
        Self {
            command_id: command_id.into(),
            keystrokes: keystrokes.into(),
            context: None,
        }
    }

    /// Sets the GPUI key context predicate for this key binding.
    pub fn context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Returns the command id.
    pub fn command_id(&self) -> &str {
        &self.command_id
    }

    /// Returns the raw keystroke sequence.
    pub fn keystrokes(&self) -> &str {
        &self.keystrokes
    }

    /// Returns the optional GPUI key context predicate.
    pub fn context_ref(&self) -> Option<&str> {
        self.context.as_deref()
    }
}

/// Parses a whitespace-separated GPUI key sequence such as `ctrl-k ctrl-o`.
pub fn parse_command_key_sequence(sequence: &str) -> Result<Vec<Keystroke>, InvalidKeystrokeError> {
    sequence.split_whitespace().map(Keystroke::parse).collect()
}

/// Availability-aware state for a keymap-resolved command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandKeymapCommandState {
    /// The command exists, is visible, and can be dispatched.
    Dispatchable,
    /// The key binding resolved to a command action that is not present in the active registry.
    MissingCommand,
    /// The command is visible but disabled.
    Disabled {
        /// Optional disabled reason.
        reason: Option<String>,
    },
    /// The command exists but is hidden in the current availability projection.
    Hidden,
}

impl CommandKeymapCommandState {
    /// Returns whether this state can be dispatched.
    pub const fn is_dispatchable(&self) -> bool {
        matches!(self, Self::Dispatchable)
    }

    /// Returns whether this state is disabled.
    pub const fn is_disabled(&self) -> bool {
        matches!(self, Self::Disabled { .. })
    }

    /// Returns whether this state is hidden.
    pub const fn is_hidden(&self) -> bool {
        matches!(self, Self::Hidden)
    }

    /// Returns whether this state is missing from the active registry.
    pub const fn is_missing_command(&self) -> bool {
        matches!(self, Self::MissingCommand)
    }

    /// Returns the disabled reason, when this is a disabled state.
    pub fn reason_ref(&self) -> Option<&str> {
        match self {
            Self::Disabled { reason } => reason.as_deref(),
            Self::Dispatchable | Self::MissingCommand | Self::Hidden => None,
        }
    }
}

/// One command action resolved from a GPUI keymap input sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandKeymapResolvedCommand {
    command_id: String,
    shortcut: String,
    state: CommandKeymapCommandState,
}

impl CommandKeymapResolvedCommand {
    fn from_binding(
        binding: &KeyBinding,
        actions: &GpuiCommandActionMap,
        registry: &CommandRegistrySnapshot,
        availability: &impl CommandAvailabilityResolver,
    ) -> Option<Self> {
        let command_id = actions.command_id_for_action(binding.action())?.to_owned();
        Some(Self {
            shortcut: command_shortcut_label(binding),
            state: command_keymap_command_state(command_id.as_str(), registry, availability),
            command_id,
        })
    }

    /// Returns the stable command id.
    pub fn command_id(&self) -> &str {
        &self.command_id
    }

    /// Returns the full shortcut label for the resolved binding.
    pub fn shortcut(&self) -> &str {
        &self.shortcut
    }

    /// Returns the availability-aware dispatch state.
    pub const fn state(&self) -> &CommandKeymapCommandState {
        &self.state
    }

    /// Returns whether this command can be dispatched.
    pub const fn is_dispatchable(&self) -> bool {
        self.state.is_dispatchable()
    }
}

/// Result of resolving one key input sequence against a command-aware GPUI keymap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandKeymapResolution {
    input: Vec<String>,
    matched_commands: Vec<CommandKeymapResolvedCommand>,
    pending: bool,
    pending_commands: Vec<CommandKeymapResolvedCommand>,
}

impl CommandKeymapResolution {
    /// Resolves typed keystrokes into command ids through a GPUI keymap and context stack.
    pub fn resolve(
        actions: &GpuiCommandActionMap,
        registry: &CommandRegistrySnapshot,
        availability: &impl CommandAvailabilityResolver,
        keymap: &Keymap,
        context_stack: &[KeyContext],
        input: &[Keystroke],
    ) -> Self {
        let (bindings, pending) = keymap.bindings_for_input(input, context_stack);
        let matched_commands = bindings
            .iter()
            .filter_map(|binding| {
                CommandKeymapResolvedCommand::from_binding(binding, actions, registry, availability)
            })
            .collect::<Vec<_>>();
        let pending_commands = command_keymap_pending_commands(
            actions,
            registry,
            availability,
            keymap,
            context_stack,
            input,
        );

        Self {
            input: input.iter().map(Keystroke::unparse).collect(),
            matched_commands,
            pending,
            pending_commands,
        }
    }

    /// Returns the normalized input keystrokes.
    pub fn input(&self) -> &[String] {
        &self.input
    }

    /// Returns the normalized input sequence as one whitespace-separated label.
    pub fn input_label(&self) -> String {
        self.input.join(" ")
    }

    /// Returns matched commands in GPUI dispatch precedence order.
    pub fn matched_commands(&self) -> &[CommandKeymapResolvedCommand] {
        &self.matched_commands
    }

    /// Returns whether the GPUI keymap has any pending continuation for this input.
    pub const fn is_pending(&self) -> bool {
        self.pending
    }

    /// Returns pending command continuations in GPUI precedence order.
    pub fn pending_commands(&self) -> &[CommandKeymapResolvedCommand] {
        &self.pending_commands
    }

    /// Returns whether there is at least one command-specific pending continuation.
    pub fn has_pending_commands(&self) -> bool {
        !self.pending_commands.is_empty()
    }

    /// Returns the first matched command in GPUI dispatch precedence order.
    pub fn primary_command(&self) -> Option<&CommandKeymapResolvedCommand> {
        self.matched_commands.first()
    }

    /// Returns the first matched command that is visible and dispatchable.
    pub fn primary_dispatchable_command(&self) -> Option<&CommandKeymapResolvedCommand> {
        self.matched_commands
            .iter()
            .find(|command| command.is_dispatchable())
    }
}

impl GpuiCommandActionMap {
    /// Returns the last registered command id whose action prototype equals this action.
    pub fn command_id_for_action(&self, action: &dyn Action) -> Option<&str> {
        self.actions()
            .iter()
            .rev()
            .find(|candidate| candidate.action().partial_eq(action))
            .map(|candidate| candidate.command_id())
    }

    /// Resolves typed keystrokes into command ids through a GPUI keymap and context stack.
    pub fn resolve_keymap_input(
        &self,
        registry: &CommandRegistrySnapshot,
        availability: &impl CommandAvailabilityResolver,
        keymap: &Keymap,
        context_stack: &[KeyContext],
        input: &[Keystroke],
    ) -> CommandKeymapResolution {
        CommandKeymapResolution::resolve(self, registry, availability, keymap, context_stack, input)
    }

    /// Parses and resolves a whitespace-separated GPUI key sequence.
    pub fn resolve_keymap_sequence(
        &self,
        sequence: &str,
        registry: &CommandRegistrySnapshot,
        availability: &impl CommandAvailabilityResolver,
        keymap: &Keymap,
        context_stack: &[KeyContext],
    ) -> Result<CommandKeymapResolution, InvalidKeystrokeError> {
        let input = parse_command_key_sequence(sequence)?;
        Ok(self.resolve_keymap_input(registry, availability, keymap, context_stack, &input))
    }
}

fn command_keymap_pending_commands(
    actions: &GpuiCommandActionMap,
    registry: &CommandRegistrySnapshot,
    availability: &impl CommandAvailabilityResolver,
    keymap: &Keymap,
    context_stack: &[KeyContext],
    input: &[Keystroke],
) -> Vec<CommandKeymapResolvedCommand> {
    let mut seen = BTreeSet::<(String, String)>::new();
    keymap
        .possible_next_bindings_for_input(input, context_stack)
        .iter()
        .filter_map(|binding| {
            let command = CommandKeymapResolvedCommand::from_binding(
                binding,
                actions,
                registry,
                availability,
            )?;
            seen.insert((command.command_id.clone(), command.shortcut.clone()))
                .then_some(command)
        })
        .collect()
}

fn command_keymap_command_state(
    command_id: &str,
    registry: &CommandRegistrySnapshot,
    availability: &impl CommandAvailabilityResolver,
) -> CommandKeymapCommandState {
    let Some(descriptor) = registry.descriptor(command_id) else {
        return CommandKeymapCommandState::MissingCommand;
    };
    match command_effective_availability(descriptor, availability) {
        CommandAvailability::Available => CommandKeymapCommandState::Dispatchable,
        CommandAvailability::Disabled { reason } => CommandKeymapCommandState::Disabled { reason },
        CommandAvailability::Hidden => CommandKeymapCommandState::Hidden,
    }
}

/// A key binding contribution tied to its lifecycle source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandKeyBindingEntry {
    source_id: CommandSourceId,
    binding: CommandKeyBinding,
}

impl CommandKeyBindingEntry {
    fn new(source_id: CommandSourceId, binding: CommandKeyBinding) -> Self {
        Self { source_id, binding }
    }

    /// Returns the source id that registered this binding.
    pub const fn source_id(&self) -> &CommandSourceId {
        &self.source_id
    }

    /// Returns the command key binding contribution.
    pub const fn binding(&self) -> &CommandKeyBinding {
        &self.binding
    }
}

/// Explicit lifecycle handle for command key binding sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandKeyBindingHandle {
    source_id: CommandSourceId,
}

impl CommandKeyBindingHandle {
    /// Creates a key binding lifecycle handle.
    pub fn new(source_id: impl Into<CommandSourceId>) -> Self {
        Self {
            source_id: source_id.into(),
        }
    }

    /// Returns the source id.
    pub const fn source_id(&self) -> &CommandSourceId {
        &self.source_id
    }

    /// Unregisters this key binding source from a command center.
    pub fn unregister(self, center: &mut crate::CommandCenter) -> usize {
        center.unregister_key_binding_source(self.source_id)
    }

    /// Unregisters this key binding source without consuming the handle.
    pub fn unregister_from(&self, center: &mut crate::CommandCenter) -> usize {
        center.unregister_key_binding_handle(self)
    }
}

/// In-memory registry of command-id keyed key binding sources.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandKeyBindingRegistry {
    entries: Vec<CommandKeyBindingEntry>,
}

struct ProjectedCommandKeyBinding {
    key_binding: KeyBinding,
    source_id: CommandSourceId,
    command_id: String,
    keystrokes: String,
    context: Option<String>,
    index: usize,
}

/// One valid command key binding after GPUI parsing and context normalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandKeyBindingProjectedEntry {
    source_id: CommandSourceId,
    command_id: String,
    keystrokes: String,
    context: Option<String>,
}

impl CommandKeyBindingProjectedEntry {
    fn new(
        source_id: CommandSourceId,
        command_id: String,
        keystrokes: String,
        context: Option<String>,
    ) -> Self {
        Self {
            source_id,
            command_id,
            keystrokes,
            context,
        }
    }

    /// Returns the lifecycle source that contributed this valid binding.
    pub const fn source_id(&self) -> &CommandSourceId {
        &self.source_id
    }

    /// Returns the command id referenced by this binding.
    pub fn command_id(&self) -> &str {
        &self.command_id
    }

    /// Returns the canonical GPUI display keystroke sequence.
    pub fn keystrokes(&self) -> &str {
        &self.keystrokes
    }

    /// Returns the normalized GPUI context predicate, when present.
    pub fn context_ref(&self) -> Option<&str> {
        self.context.as_deref()
    }
}

impl CommandKeyBindingRegistry {
    /// Creates an empty command key binding registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers or replaces all bindings from one source id.
    pub fn register(
        &mut self,
        source_id: impl Into<CommandSourceId>,
        bindings: impl IntoIterator<Item = CommandKeyBinding>,
    ) -> CommandKeyBindingHandle {
        let source_id = source_id.into();
        self.unregister_source(source_id.clone());
        self.entries.extend(
            bindings
                .into_iter()
                .map(|binding| CommandKeyBindingEntry::new(source_id.clone(), binding)),
        );
        CommandKeyBindingHandle::new(source_id)
    }

    /// Unregisters every binding from one source id.
    pub fn unregister_source(&mut self, source_id: impl Into<CommandSourceId>) -> usize {
        let source_id = source_id.into();
        let before = self.entries.len();
        self.entries.retain(|entry| entry.source_id() != &source_id);
        before - self.entries.len()
    }

    /// Returns key binding entries in registration order.
    pub fn entries(&self) -> &[CommandKeyBindingEntry] {
        &self.entries
    }

    /// Projects registered command-id bindings into concrete GPUI key bindings.
    pub fn project(&self, actions: &GpuiCommandActionMap) -> CommandKeyBindingProjection {
        let mut projected = Vec::new();
        let mut diagnostics = Vec::new();

        for (index, entry) in self.entries.iter().enumerate() {
            let binding = entry.binding();
            let Some(action) = actions.action_for_command(binding.command_id()) else {
                diagnostics.push(CommandKeyBindingDiagnostic::missing_action(entry));
                continue;
            };

            let context_predicate = match binding.context_ref() {
                Some(context) => match KeyBindingContextPredicate::parse(context) {
                    Ok(predicate) => Some(Rc::new(predicate)),
                    Err(error) => {
                        diagnostics.push(CommandKeyBindingDiagnostic::invalid_context(
                            entry,
                            error.to_string(),
                        ));
                        continue;
                    }
                },
                None => None,
            };
            let normalized_context = context_predicate.as_ref().map(ToString::to_string);

            match KeyBinding::load(
                binding.keystrokes(),
                action.boxed_action(),
                context_predicate,
                false,
                None,
                &DummyKeyboardMapper,
            ) {
                Ok(key_binding) => projected.push(ProjectedCommandKeyBinding {
                    keystrokes: command_shortcut_label(&key_binding),
                    key_binding,
                    source_id: entry.source_id().clone(),
                    command_id: binding.command_id().to_owned(),
                    context: normalized_context,
                    index,
                }),
                Err(error) => diagnostics.push(CommandKeyBindingDiagnostic::invalid_keystrokes(
                    entry,
                    error.to_string(),
                )),
            }
        }

        let conflicts = key_binding_conflicts(&projected);
        let projected_entries = projected
            .iter()
            .map(|binding| {
                CommandKeyBindingProjectedEntry::new(
                    binding.source_id.clone(),
                    binding.command_id.clone(),
                    binding.keystrokes.clone(),
                    binding.context.clone(),
                )
            })
            .collect();
        let key_bindings = projected
            .into_iter()
            .map(|projected| projected.key_binding)
            .collect();

        CommandKeyBindingProjection::new(key_bindings, projected_entries, diagnostics, conflicts)
    }

    /// Adds valid projected GPUI key bindings to an app keymap and returns the projection.
    pub fn add_to_keymap(
        &self,
        actions: &GpuiCommandActionMap,
        keymap: &mut Keymap,
    ) -> CommandKeyBindingProjection {
        self.install_into_keymap(actions, keymap).into_projection()
    }

    /// Installs valid projected GPUI key bindings into an app keymap.
    pub fn install_into_keymap(
        &self,
        actions: &GpuiCommandActionMap,
        keymap: &mut Keymap,
    ) -> CommandKeyBindingInstallReport {
        let projection = self.project(actions);
        keymap.add_bindings(projection.key_bindings().iter().cloned());
        CommandKeyBindingInstallReport::new(projection)
    }

    /// Installs valid projected GPUI key bindings into the app-level keymap.
    pub fn install_in_app(
        &self,
        actions: &GpuiCommandActionMap,
        cx: &mut App,
    ) -> CommandKeyBindingInstallReport {
        let projection = self.project(actions);
        cx.bind_keys(projection.key_bindings().iter().cloned());
        CommandKeyBindingInstallReport::new(projection)
    }
}

fn key_binding_conflicts(
    projected: &[ProjectedCommandKeyBinding],
) -> Vec<CommandKeyBindingConflict> {
    let mut groups =
        BTreeMap::<String, BTreeMap<Option<String>, Vec<&ProjectedCommandKeyBinding>>>::new();

    for binding in projected {
        groups
            .entry(binding.keystrokes.clone())
            .or_default()
            .entry(binding.context.clone())
            .or_default()
            .push(binding);
    }

    let mut conflicts = Vec::new();

    for (keystrokes, context_groups) in groups {
        let globals = context_groups.get(&None).cloned().unwrap_or_default();
        push_conflict_if_needed(&mut conflicts, keystrokes.clone(), None, globals.clone());

        for (context, contextual_bindings) in context_groups {
            let Some(context) = context else {
                continue;
            };
            let mut candidates = globals.clone();
            candidates.extend(contextual_bindings);
            push_conflict_if_needed(
                &mut conflicts,
                keystrokes.clone(),
                Some(context),
                candidates,
            );
        }
    }

    conflicts
}

fn push_conflict_if_needed(
    conflicts: &mut Vec<CommandKeyBindingConflict>,
    keystrokes: String,
    context: Option<String>,
    mut candidates: Vec<&ProjectedCommandKeyBinding>,
) {
    if !has_multiple_command_ids(&candidates) {
        return;
    }

    candidates.sort_by_key(|binding| binding.index);
    let entries = candidates
        .into_iter()
        .map(|binding| {
            CommandKeyBindingConflictEntry::new(
                binding.source_id.clone(),
                binding.command_id.clone(),
            )
        })
        .collect();
    conflicts.push(CommandKeyBindingConflict::new(keystrokes, context, entries));
}

fn has_multiple_command_ids(candidates: &[&ProjectedCommandKeyBinding]) -> bool {
    let Some(first) = candidates.first() else {
        return false;
    };
    candidates
        .iter()
        .any(|candidate| candidate.command_id != first.command_id)
}

/// One source/command participant in a command key binding conflict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandKeyBindingConflictEntry {
    source_id: CommandSourceId,
    command_id: String,
}

impl CommandKeyBindingConflictEntry {
    fn new(source_id: CommandSourceId, command_id: String) -> Self {
        Self {
            source_id,
            command_id,
        }
    }

    /// Returns the lifecycle source that contributed the conflicting binding.
    pub const fn source_id(&self) -> &CommandSourceId {
        &self.source_id
    }

    /// Returns the command id that owns the conflicting binding.
    pub fn command_id(&self) -> &str {
        &self.command_id
    }
}

/// A command key binding conflict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandKeyBindingConflict {
    keystrokes: String,
    context: Option<String>,
    entries: Vec<CommandKeyBindingConflictEntry>,
}

impl CommandKeyBindingConflict {
    fn new(
        keystrokes: String,
        context: Option<String>,
        entries: Vec<CommandKeyBindingConflictEntry>,
    ) -> Self {
        Self {
            keystrokes,
            context,
            entries,
        }
    }

    /// Returns the canonical GPUI display string for the conflicting keystrokes.
    pub fn keystrokes(&self) -> &str {
        &self.keystrokes
    }

    /// Returns the normalized GPUI context predicate where the conflict occurs.
    pub fn context_ref(&self) -> Option<&str> {
        self.context.as_deref()
    }

    /// Returns conflicting entries in registration order.
    pub fn entries(&self) -> &[CommandKeyBindingConflictEntry] {
        &self.entries
    }
}

/// Category for command key binding projection diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKeyBindingDiagnosticKind {
    /// The command id has no registered GPUI action prototype.
    MissingAction,
    /// The keystroke sequence could not be parsed by GPUI.
    InvalidKeystrokes,
    /// The key context predicate could not be parsed by GPUI.
    InvalidContext,
}

/// One command key binding projection diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandKeyBindingDiagnostic {
    kind: CommandKeyBindingDiagnosticKind,
    source_id: CommandSourceId,
    command_id: String,
    keystrokes: String,
    context: Option<String>,
    message: Option<String>,
}

impl CommandKeyBindingDiagnostic {
    fn missing_action(entry: &CommandKeyBindingEntry) -> Self {
        Self::new(entry, CommandKeyBindingDiagnosticKind::MissingAction, None)
    }

    fn invalid_keystrokes(entry: &CommandKeyBindingEntry, message: String) -> Self {
        Self::new(
            entry,
            CommandKeyBindingDiagnosticKind::InvalidKeystrokes,
            Some(message),
        )
    }

    fn invalid_context(entry: &CommandKeyBindingEntry, message: String) -> Self {
        Self::new(
            entry,
            CommandKeyBindingDiagnosticKind::InvalidContext,
            Some(message),
        )
    }

    fn new(
        entry: &CommandKeyBindingEntry,
        kind: CommandKeyBindingDiagnosticKind,
        message: Option<String>,
    ) -> Self {
        let binding = entry.binding();
        Self {
            kind,
            source_id: entry.source_id().clone(),
            command_id: binding.command_id().to_owned(),
            keystrokes: binding.keystrokes().to_owned(),
            context: binding.context_ref().map(str::to_owned),
            message,
        }
    }

    /// Returns the diagnostic kind.
    pub const fn kind(&self) -> CommandKeyBindingDiagnosticKind {
        self.kind
    }

    /// Returns the lifecycle source that contributed the bad binding.
    pub const fn source_id(&self) -> &CommandSourceId {
        &self.source_id
    }

    /// Returns the command id referenced by the binding.
    pub fn command_id(&self) -> &str {
        &self.command_id
    }

    /// Returns the raw keystroke sequence.
    pub fn keystrokes(&self) -> &str {
        &self.keystrokes
    }

    /// Returns the optional key context predicate.
    pub fn context_ref(&self) -> Option<&str> {
        self.context.as_deref()
    }

    /// Returns a parser or lookup message for invalid bindings.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }
}

/// Result of projecting command-id key bindings into concrete GPUI key bindings.
#[derive(Debug, Clone)]
pub struct CommandKeyBindingProjection {
    key_bindings: Vec<KeyBinding>,
    projected_entries: Vec<CommandKeyBindingProjectedEntry>,
    diagnostics: Vec<CommandKeyBindingDiagnostic>,
    conflicts: Vec<CommandKeyBindingConflict>,
}

impl CommandKeyBindingProjection {
    fn new(
        key_bindings: Vec<KeyBinding>,
        projected_entries: Vec<CommandKeyBindingProjectedEntry>,
        diagnostics: Vec<CommandKeyBindingDiagnostic>,
        conflicts: Vec<CommandKeyBindingConflict>,
    ) -> Self {
        Self {
            key_bindings,
            projected_entries,
            diagnostics,
            conflicts,
        }
    }

    /// Returns valid GPUI key bindings in registry order.
    pub fn key_bindings(&self) -> &[KeyBinding] {
        &self.key_bindings
    }

    /// Returns valid projected command binding metadata in registry order.
    pub fn projected_entries(&self) -> &[CommandKeyBindingProjectedEntry] {
        &self.projected_entries
    }

    /// Returns diagnostics for skipped bindings.
    pub fn diagnostics(&self) -> &[CommandKeyBindingDiagnostic] {
        &self.diagnostics
    }

    /// Returns same-context command shortcut conflicts.
    pub fn conflicts(&self) -> &[CommandKeyBindingConflict] {
        &self.conflicts
    }

    /// Returns whether every registered binding projected without errors.
    pub fn is_clean(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Returns whether any projected binding conflicts with another command binding.
    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }

    /// Returns whether every registered binding projected without errors or conflicts.
    pub fn is_strictly_clean(&self) -> bool {
        self.diagnostics.is_empty() && self.conflicts.is_empty()
    }
}

/// Result of installing command key bindings into a GPUI keymap.
#[derive(Debug, Clone)]
pub struct CommandKeyBindingInstallReport {
    projection: CommandKeyBindingProjection,
}

impl CommandKeyBindingInstallReport {
    fn new(projection: CommandKeyBindingProjection) -> Self {
        Self { projection }
    }

    /// Returns the projection that was installed.
    pub const fn projection(&self) -> &CommandKeyBindingProjection {
        &self.projection
    }

    fn into_projection(self) -> CommandKeyBindingProjection {
        self.projection
    }

    /// Returns the number of concrete GPUI bindings appended to the target keymap.
    pub fn installed_count(&self) -> usize {
        self.projection.key_bindings().len()
    }

    /// Returns valid GPUI key bindings in registry order.
    pub fn key_bindings(&self) -> &[KeyBinding] {
        self.projection.key_bindings()
    }

    /// Returns diagnostics for skipped bindings.
    pub fn diagnostics(&self) -> &[CommandKeyBindingDiagnostic] {
        self.projection.diagnostics()
    }

    /// Returns same-context command shortcut conflicts.
    pub fn conflicts(&self) -> &[CommandKeyBindingConflict] {
        self.projection.conflicts()
    }

    /// Returns whether any installed binding conflicts with another command binding.
    pub fn has_conflicts(&self) -> bool {
        self.projection.has_conflicts()
    }

    /// Returns whether installation projected without errors or conflicts.
    pub fn is_clean(&self) -> bool {
        self.projection.is_strictly_clean()
    }
}

#[cfg(test)]
mod tests {
    use open_gpui::{KeyBinding, KeyContext, Keymap, actions};

    use crate::{
        CommandAvailabilityMap, CommandContribution, CommandDescriptor, CommandRegistrySnapshot,
        GpuiCommandActionMap,
    };

    use super::{CommandKeymapCommandState, CommandKeymapResolution, parse_command_key_sequence};

    actions!(
        keymap_resolution_tests,
        [
            OpenWorkspace,
            SaveWorkspace,
            HiddenWorkspace,
            MissingWorkspace
        ]
    );

    fn registry_snapshot() -> CommandRegistrySnapshot {
        CommandRegistrySnapshot::new(
            "keymap-resolution-v1",
            [
                CommandContribution::new(CommandDescriptor::new(
                    "workspace.open",
                    "Open Workspace",
                )),
                CommandContribution::new(CommandDescriptor::new(
                    "workspace.save",
                    "Save Workspace",
                )),
                CommandContribution::new(CommandDescriptor::new(
                    "workspace.hidden",
                    "Hidden Workspace",
                )),
            ],
        )
    }

    fn action_map() -> GpuiCommandActionMap {
        GpuiCommandActionMap::new()
            .action("workspace.open", OpenWorkspace)
            .action("workspace.save", SaveWorkspace)
            .action("workspace.hidden", HiddenWorkspace)
            .action("workspace.missing", MissingWorkspace)
    }

    #[test]
    fn keymap_resolution_reports_chord_pending_and_command_match() {
        let mut keymap = Keymap::default();
        keymap.add_bindings([
            KeyBinding::new("ctrl-k ctrl-o", OpenWorkspace, Some("Workspace")),
            KeyBinding::new("ctrl-k ctrl-s", SaveWorkspace, Some("Workspace")),
        ]);
        let contexts = [KeyContext::parse("Workspace").unwrap()];

        let pending_input = parse_command_key_sequence("ctrl-k").unwrap();
        let pending = CommandKeymapResolution::resolve(
            &action_map(),
            &registry_snapshot(),
            &CommandAvailabilityMap::new(),
            &keymap,
            &contexts,
            &pending_input,
        );

        assert_eq!(pending.input_label(), "ctrl-k");
        assert!(pending.is_pending());
        assert!(pending.matched_commands().is_empty());
        assert!(pending.has_pending_commands());
        assert_eq!(
            pending
                .pending_commands()
                .iter()
                .map(|command| command.command_id())
                .collect::<Vec<_>>(),
            ["workspace.save", "workspace.open"]
        );

        let matched_input = parse_command_key_sequence("ctrl-k ctrl-o").unwrap();
        let matched = CommandKeymapResolution::resolve(
            &action_map(),
            &registry_snapshot(),
            &CommandAvailabilityMap::new(),
            &keymap,
            &contexts,
            &matched_input,
        );

        assert!(!matched.is_pending());
        assert!(matched.pending_commands().is_empty());
        let primary = matched.primary_command().unwrap();
        assert_eq!(primary.command_id(), "workspace.open");
        assert_eq!(primary.state(), &CommandKeymapCommandState::Dispatchable);
        assert_eq!(
            matched
                .primary_dispatchable_command()
                .map(|command| command.command_id()),
            Some("workspace.open")
        );
    }

    #[test]
    fn keymap_resolution_reports_availability_and_registry_state() {
        let mut keymap = Keymap::default();
        keymap.add_bindings([
            KeyBinding::new("ctrl-s", SaveWorkspace, Some("Workspace")),
            KeyBinding::new("ctrl-h", HiddenWorkspace, Some("Workspace")),
            KeyBinding::new("ctrl-m", MissingWorkspace, Some("Workspace")),
        ]);
        let contexts = [KeyContext::parse("Workspace").unwrap()];
        let availability = CommandAvailabilityMap::new()
            .disabled("workspace.save", "Read-only")
            .hidden("workspace.hidden");

        let disabled = action_map()
            .resolve_keymap_sequence(
                "ctrl-s",
                &registry_snapshot(),
                &availability,
                &keymap,
                &contexts,
            )
            .unwrap();
        assert_eq!(
            disabled.primary_command().map(|command| command.state()),
            Some(&CommandKeymapCommandState::Disabled {
                reason: Some("Read-only".to_string()),
            })
        );
        assert!(disabled.primary_dispatchable_command().is_none());

        let hidden = action_map()
            .resolve_keymap_sequence(
                "ctrl-h",
                &registry_snapshot(),
                &availability,
                &keymap,
                &contexts,
            )
            .unwrap();
        assert_eq!(
            hidden.primary_command().map(|command| command.state()),
            Some(&CommandKeymapCommandState::Hidden)
        );

        let missing = action_map()
            .resolve_keymap_sequence(
                "ctrl-m",
                &registry_snapshot(),
                &availability,
                &keymap,
                &contexts,
            )
            .unwrap();
        assert_eq!(
            missing.primary_command().map(|command| command.state()),
            Some(&CommandKeymapCommandState::MissingCommand)
        );
    }
}
