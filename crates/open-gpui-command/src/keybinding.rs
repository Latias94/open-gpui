//! Command-id keyed GPUI key binding registration.

use std::rc::Rc;

use open_gpui::{DummyKeyboardMapper, KeyBinding, KeyBindingContextPredicate, Keymap};

use crate::{CommandSourceId, GpuiCommandActionMap};

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
        let mut key_bindings = Vec::new();
        let mut diagnostics = Vec::new();

        for entry in &self.entries {
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

            match KeyBinding::load(
                binding.keystrokes(),
                action.boxed_action(),
                context_predicate,
                false,
                None,
                &DummyKeyboardMapper,
            ) {
                Ok(key_binding) => key_bindings.push(key_binding),
                Err(error) => diagnostics.push(CommandKeyBindingDiagnostic::invalid_keystrokes(
                    entry,
                    error.to_string(),
                )),
            }
        }

        CommandKeyBindingProjection::new(key_bindings, diagnostics)
    }

    /// Adds valid projected GPUI key bindings to an app keymap and returns the projection.
    pub fn add_to_keymap(
        &self,
        actions: &GpuiCommandActionMap,
        keymap: &mut Keymap,
    ) -> CommandKeyBindingProjection {
        let projection = self.project(actions);
        keymap.add_bindings(projection.key_bindings().iter().cloned());
        projection
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
}

impl CommandKeyBindingProjection {
    fn new(key_bindings: Vec<KeyBinding>, diagnostics: Vec<CommandKeyBindingDiagnostic>) -> Self {
        Self {
            key_bindings,
            diagnostics,
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

    /// Returns whether every registered binding projected successfully.
    pub fn is_clean(&self) -> bool {
        self.diagnostics.is_empty()
    }
}
