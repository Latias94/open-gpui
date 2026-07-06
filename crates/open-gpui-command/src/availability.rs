//! Command availability projection.

use std::collections::BTreeMap;

use crate::{CommandDescriptor, CommandRegistrySnapshot};

/// Projected availability for one command in the current app context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandAvailability {
    /// The command may be shown and dispatched.
    Available,
    /// The command may be shown but must not be dispatched.
    Disabled {
        /// Optional reason suitable for tooltips or future detail UI.
        reason: Option<String>,
    },
    /// The command should be hidden and must not be dispatched.
    Hidden,
}

impl CommandAvailability {
    /// Creates a disabled availability state.
    pub fn disabled(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        Self::Disabled {
            reason: (!reason.is_empty()).then_some(reason),
        }
    }

    /// Returns whether the command is available.
    pub const fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }

    /// Returns whether the command is disabled.
    pub const fn is_disabled(&self) -> bool {
        matches!(self, Self::Disabled { .. })
    }

    /// Returns whether the command is hidden.
    pub const fn is_hidden(&self) -> bool {
        matches!(self, Self::Hidden)
    }

    /// Returns the disabled reason, when this is a disabled state.
    pub fn reason_ref(&self) -> Option<&str> {
        match self {
            Self::Disabled { reason } => reason.as_deref(),
            Self::Available | Self::Hidden => None,
        }
    }
}

/// App-owned availability resolver.
pub trait CommandAvailabilityResolver {
    /// Returns projected availability for a descriptor.
    fn availability_for(&self, descriptor: &CommandDescriptor) -> CommandAvailability;
}

/// Deterministic availability map keyed by stable command id.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommandAvailabilityMap {
    states: BTreeMap<String, CommandAvailability>,
}

impl CommandAvailabilityMap {
    /// Creates an empty availability map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets one command availability state.
    pub fn set(mut self, command_id: impl Into<String>, state: CommandAvailability) -> Self {
        let command_id = command_id.into();
        if !command_id.is_empty() {
            self.states.insert(command_id, state);
        }
        self
    }

    /// Marks a command as available.
    pub fn available(self, command_id: impl Into<String>) -> Self {
        self.set(command_id, CommandAvailability::Available)
    }

    /// Marks a command as disabled.
    pub fn disabled(self, command_id: impl Into<String>, reason: impl Into<String>) -> Self {
        self.set(command_id, CommandAvailability::disabled(reason))
    }

    /// Marks a command as hidden.
    pub fn hidden(self, command_id: impl Into<String>) -> Self {
        self.set(command_id, CommandAvailability::Hidden)
    }

    /// Returns the explicit availability state for a command id.
    pub fn get(&self, command_id: &str) -> Option<&CommandAvailability> {
        self.states.get(command_id)
    }

    /// Returns all explicit states.
    pub fn states(&self) -> &BTreeMap<String, CommandAvailability> {
        &self.states
    }
}

impl CommandAvailabilityResolver for CommandAvailabilityMap {
    fn availability_for(&self, descriptor: &CommandDescriptor) -> CommandAvailability {
        self.states
            .get(descriptor.id())
            .cloned()
            .unwrap_or(CommandAvailability::Available)
    }
}

impl<T> CommandAvailabilityResolver for &T
where
    T: CommandAvailabilityResolver + ?Sized,
{
    fn availability_for(&self, descriptor: &CommandDescriptor) -> CommandAvailability {
        (*self).availability_for(descriptor)
    }
}

/// Returns the effective availability after descriptor-level disabled metadata is applied.
pub fn command_effective_availability(
    descriptor: &CommandDescriptor,
    resolver: &impl CommandAvailabilityResolver,
) -> CommandAvailability {
    match resolver.availability_for(descriptor) {
        CommandAvailability::Available if descriptor.disabled_state() => {
            CommandAvailability::Disabled {
                reason: descriptor.disabled_reason_ref().map(str::to_owned),
            }
        }
        state => state,
    }
}

impl CommandRegistrySnapshot {
    /// Projects this snapshot through an availability resolver.
    ///
    /// Hidden commands are omitted. Disabled commands remain visible with disabled metadata copied
    /// into the projected descriptor.
    pub fn with_availability(&self, resolver: &impl CommandAvailabilityResolver) -> Self {
        let contributions = self
            .contributions()
            .iter()
            .filter_map(|contribution| {
                let descriptor = contribution.descriptor();
                match command_effective_availability(descriptor, resolver) {
                    CommandAvailability::Available => Some(contribution.clone()),
                    CommandAvailability::Disabled { reason } => Some(
                        contribution
                            .with_descriptor(descriptor.clone().with_projected_disabled(reason)),
                    ),
                    CommandAvailability::Hidden => None,
                }
            })
            .collect::<Vec<_>>();

        Self::new(self.revision(), contributions)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        CommandAvailabilityMap, CommandDescriptor, CommandRegistry,
        availability::CommandAvailability,
    };

    #[test]
    fn availability_projection_hides_and_disables_commands() {
        let mut registry = CommandRegistry::new("commands:1");
        registry
            .register(CommandDescriptor::new("file.open", "Open File"))
            .unwrap();
        registry
            .register(CommandDescriptor::new("file.save", "Save File"))
            .unwrap();
        registry
            .register(CommandDescriptor::new("workspace.close", "Close Workspace"))
            .unwrap();

        let availability = CommandAvailabilityMap::new()
            .disabled("file.save", "Read-only")
            .hidden("workspace.close");
        let projected = registry.snapshot().with_availability(&availability);
        let descriptors = projected.descriptors().collect::<Vec<_>>();

        assert_eq!(descriptors.len(), 2);
        assert_eq!(descriptors[0].id(), "file.open");
        assert!(!descriptors[0].disabled_state());
        assert_eq!(descriptors[1].id(), "file.save");
        assert!(descriptors[1].disabled_state());
        assert_eq!(descriptors[1].disabled_reason_ref(), Some("Read-only"));
        assert_eq!(
            availability.get("workspace.close"),
            Some(&CommandAvailability::Hidden)
        );
    }

    #[test]
    fn descriptor_disabled_state_participates_in_effective_availability() {
        let mut registry = CommandRegistry::new("commands:1");
        registry
            .register(CommandDescriptor::new("file.save", "Save File").disabled_reason("Busy"))
            .unwrap();

        let projected = registry
            .snapshot()
            .with_availability(&CommandAvailabilityMap::new());
        let descriptor = projected.descriptor("file.save").unwrap();

        assert!(descriptor.disabled_state());
        assert_eq!(descriptor.disabled_reason_ref(), Some("Busy"));
    }
}
