//! Command-id keyed GPUI key binding registration.

use std::collections::BTreeMap;
use std::rc::Rc;

use open_gpui::{App, DummyKeyboardMapper, KeyBinding, KeyBindingContextPredicate, Keymap};

use crate::{CommandSourceId, GpuiCommandActionMap, gpui::command_shortcut_label};

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
        let key_bindings = projected
            .into_iter()
            .map(|projected| projected.key_binding)
            .collect();

        CommandKeyBindingProjection::new(key_bindings, diagnostics, conflicts)
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
    diagnostics: Vec<CommandKeyBindingDiagnostic>,
    conflicts: Vec<CommandKeyBindingConflict>,
}

impl CommandKeyBindingProjection {
    fn new(
        key_bindings: Vec<KeyBinding>,
        diagnostics: Vec<CommandKeyBindingDiagnostic>,
        conflicts: Vec<CommandKeyBindingConflict>,
    ) -> Self {
        Self {
            key_bindings,
            diagnostics,
            conflicts,
        }
    }

    /// Returns valid GPUI key bindings in registry order.
    pub fn key_bindings(&self) -> &[KeyBinding] {
        &self.key_bindings
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
