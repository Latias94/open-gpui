//! Stable command metadata and deterministic registry snapshots.

use std::collections::BTreeSet;
use std::fmt;

/// Stable source identifier for app, crate, extension, or plugin command contributions.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommandSourceId(String);

impl CommandSourceId {
    /// Creates a source id.
    pub fn new(source: impl Into<String>) -> Self {
        Self(source.into())
    }

    /// Returns the source id as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns whether the source id is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for CommandSourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<String> for CommandSourceId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for CommandSourceId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

/// Pure app-command metadata shared by command palettes, menu projections, and dispatch adapters.
///
/// This type intentionally does not own callbacks, command execution, keybinding resolution, or a
/// global registry. Applications may use it as a stable fact record and project it into concrete UI
/// components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandDescriptor {
    id: String,
    label: String,
    group: Option<String>,
    keywords: Vec<String>,
    shortcut: Option<String>,
    disabled: bool,
    disabled_reason: Option<String>,
    when: Option<String>,
    menu_path: Vec<String>,
}

impl CommandDescriptor {
    /// Creates a command descriptor with stable id and visible label.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            group: None,
            keywords: Vec::new(),
            shortcut: None,
            disabled: false,
            disabled_reason: None,
            when: None,
            menu_path: Vec::new(),
        }
    }

    /// Applies an optional grouping label used by command palettes.
    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// Adds one search keyword.
    pub fn keyword(mut self, keyword: impl Into<String>) -> Self {
        self.keywords.push(keyword.into());
        self
    }

    /// Adds many search keywords.
    pub fn keywords(mut self, keywords: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.keywords.extend(keywords.into_iter().map(Into::into));
        self
    }

    /// Applies the display shortcut label.
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Applies caller-owned disabled metadata.
    pub const fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies caller-owned disabled metadata with a user-displayable reason.
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

    /// Applies a menu path projection such as `["File", "Open Recent"]`.
    pub fn menu_path(mut self, segments: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.menu_path = segments
            .into_iter()
            .map(Into::into)
            .filter(|segment: &String| !segment.is_empty())
            .collect();
        self
    }

    /// Returns the stable command id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the visible command label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the optional grouping label.
    pub fn group_ref(&self) -> Option<&str> {
        self.group.as_deref()
    }

    /// Returns search keywords.
    pub fn keywords_ref(&self) -> &[String] {
        &self.keywords
    }

    /// Returns the display shortcut label.
    pub fn shortcut_ref(&self) -> Option<&str> {
        self.shortcut.as_deref()
    }

    /// Returns caller-owned disabled metadata.
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

    /// Returns the menu path projection.
    pub fn menu_path_ref(&self) -> &[String] {
        &self.menu_path
    }

    pub(crate) fn with_projected_disabled(mut self, reason: Option<String>) -> Self {
        self.disabled = true;
        self.disabled_reason = reason.filter(|reason| !reason.is_empty());
        self
    }
}

/// One command contribution registered by an app or plugin-like module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandContribution {
    descriptor: CommandDescriptor,
    source: Option<CommandSourceId>,
}

impl CommandContribution {
    /// Creates a command contribution from command metadata.
    pub fn new(descriptor: CommandDescriptor) -> Self {
        Self {
            descriptor,
            source: None,
        }
    }

    /// Applies optional source metadata such as a crate, plugin, or module id.
    pub fn source(mut self, source: impl Into<CommandSourceId>) -> Self {
        let source = source.into();
        if !source.is_empty() {
            self.source = Some(source);
        }
        self
    }

    /// Returns the command descriptor.
    pub const fn descriptor(&self) -> &CommandDescriptor {
        &self.descriptor
    }

    /// Returns optional source metadata.
    pub fn source_ref(&self) -> Option<&str> {
        self.source.as_ref().map(CommandSourceId::as_str)
    }

    pub(crate) fn with_descriptor(&self, descriptor: CommandDescriptor) -> Self {
        Self {
            descriptor,
            source: self.source.clone(),
        }
    }
}

/// Immutable command registry projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRegistrySnapshot {
    revision: String,
    contributions: Vec<CommandContribution>,
}

impl CommandRegistrySnapshot {
    /// Creates an immutable registry snapshot.
    pub fn new(
        revision: impl Into<String>,
        contributions: impl IntoIterator<Item = CommandContribution>,
    ) -> Self {
        Self {
            revision: revision.into(),
            contributions: contributions.into_iter().collect(),
        }
    }

    /// Returns the caller-owned revision label.
    pub fn revision(&self) -> &str {
        &self.revision
    }

    /// Returns registered command contributions in deterministic order.
    pub fn contributions(&self) -> &[CommandContribution] {
        &self.contributions
    }

    /// Iterates over command descriptors in deterministic order.
    pub fn descriptors(&self) -> impl Iterator<Item = &CommandDescriptor> {
        self.contributions
            .iter()
            .map(CommandContribution::descriptor)
    }

    /// Returns a contribution by command id.
    pub fn contribution(&self, command_id: &str) -> Option<&CommandContribution> {
        self.contributions
            .iter()
            .find(|contribution| contribution.descriptor().id() == command_id)
    }

    /// Returns a command descriptor by command id.
    pub fn descriptor(&self, command_id: &str) -> Option<&CommandDescriptor> {
        self.contribution(command_id)
            .map(CommandContribution::descriptor)
    }

    /// Returns whether the snapshot contains no commands.
    pub const fn is_empty(&self) -> bool {
        self.contributions.is_empty()
    }

    /// Returns the number of commands in the snapshot.
    pub const fn len(&self) -> usize {
        self.contributions.len()
    }
}

/// Duplicate command id registration error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRegistryError {
    id: String,
}

impl CommandRegistryError {
    /// Creates a duplicate-id registry error.
    pub(crate) fn duplicate(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    /// Returns the duplicated command id.
    pub fn id(&self) -> &str {
        &self.id
    }
}

impl fmt::Display for CommandRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "duplicate command id `{}`", self.id)
    }
}

impl std::error::Error for CommandRegistryError {}

/// Deterministic command registry for app and plugin-like command contributions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRegistry {
    revision: String,
    contributions: Vec<CommandContribution>,
    ids: BTreeSet<String>,
}

impl CommandRegistry {
    /// Creates an empty command registry for the given revision label.
    pub fn new(revision: impl Into<String>) -> Self {
        Self {
            revision: revision.into(),
            contributions: Vec::new(),
            ids: BTreeSet::new(),
        }
    }

    /// Returns the caller-owned revision label.
    pub fn revision(&self) -> &str {
        &self.revision
    }

    /// Registers one command descriptor.
    pub fn register(&mut self, descriptor: CommandDescriptor) -> Result<(), CommandRegistryError> {
        self.register_contribution(CommandContribution::new(descriptor))
    }

    /// Registers one command contribution.
    pub fn register_contribution(
        &mut self,
        contribution: CommandContribution,
    ) -> Result<(), CommandRegistryError> {
        let id = contribution.descriptor().id().to_owned();
        if !self.ids.insert(id.clone()) {
            return Err(CommandRegistryError::duplicate(id));
        }
        self.contributions.push(contribution);
        Ok(())
    }

    /// Registers many command contributions atomically.
    pub fn register_all(
        &mut self,
        contributions: impl IntoIterator<Item = CommandContribution>,
    ) -> Result<(), CommandRegistryError> {
        let contributions = contributions.into_iter().collect::<Vec<_>>();
        let mut next_ids = self.ids.clone();
        for contribution in &contributions {
            let id = contribution.descriptor().id().to_owned();
            if !next_ids.insert(id.clone()) {
                return Err(CommandRegistryError::duplicate(id));
            }
        }

        self.ids = next_ids;
        self.contributions.extend(contributions);
        Ok(())
    }

    /// Returns registered command contributions in insertion order.
    pub fn contributions(&self) -> &[CommandContribution] {
        &self.contributions
    }

    /// Captures an immutable snapshot of the current registry.
    pub fn snapshot(&self) -> CommandRegistrySnapshot {
        CommandRegistrySnapshot::new(self.revision.clone(), self.contributions.clone())
    }

    /// Returns whether the registry contains no commands.
    pub const fn is_empty(&self) -> bool {
        self.contributions.is_empty()
    }

    /// Returns the number of registered commands.
    pub const fn len(&self) -> usize {
        self.contributions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CommandContribution, CommandDescriptor, CommandRegistry, CommandRegistryError,
        CommandRegistrySnapshot,
    };

    #[test]
    fn command_descriptor_records_projection_metadata_without_runtime() {
        let descriptor = CommandDescriptor::new("workspace.open", "Open Workspace")
            .group("Workspace")
            .keywords(["project", "folder"])
            .shortcut("Ctrl+Shift+O")
            .disabled_reason("No workspace")
            .when("workspace")
            .menu_path(["File", "", "Open"]);

        assert_eq!(descriptor.id(), "workspace.open");
        assert_eq!(descriptor.label(), "Open Workspace");
        assert_eq!(descriptor.group_ref(), Some("Workspace"));
        assert_eq!(descriptor.keywords_ref(), ["project", "folder"]);
        assert_eq!(descriptor.shortcut_ref(), Some("Ctrl+Shift+O"));
        assert!(descriptor.disabled_state());
        assert_eq!(descriptor.disabled_reason_ref(), Some("No workspace"));
        assert_eq!(descriptor.when_ref(), Some("workspace"));
        assert_eq!(descriptor.menu_path_ref(), ["File", "Open"]);
    }

    #[test]
    fn command_registry_preserves_deterministic_contribution_order() {
        let mut registry = CommandRegistry::new("commands:1");
        registry
            .register_contribution(
                CommandContribution::new(
                    CommandDescriptor::new("workspace.open", "Open Workspace")
                        .group("Workspace")
                        .keyword("project")
                        .shortcut("Ctrl+O"),
                )
                .source("workspace"),
            )
            .unwrap();
        registry
            .register(CommandDescriptor::new("file.save", "Save File").group("File"))
            .unwrap();

        let snapshot = registry.snapshot();
        let ids = snapshot
            .descriptors()
            .map(CommandDescriptor::id)
            .collect::<Vec<_>>();

        assert_eq!(registry.revision(), "commands:1");
        assert_eq!(snapshot.revision(), "commands:1");
        assert_eq!(ids, ["workspace.open", "file.save"]);
        assert_eq!(snapshot.contributions()[0].source_ref(), Some("workspace"));
        assert_eq!(
            snapshot.contributions()[0].descriptor().keywords_ref(),
            ["project"]
        );
        assert_eq!(
            snapshot.contributions()[0].descriptor().shortcut_ref(),
            Some("Ctrl+O")
        );
    }

    #[test]
    fn command_registry_rejects_duplicate_ids() {
        let mut registry = CommandRegistry::new("commands:1");
        registry
            .register(CommandDescriptor::new("file.save", "Save File"))
            .unwrap();

        let error = registry
            .register(CommandDescriptor::new("file.save", "Save Again"))
            .unwrap_err();

        assert_eq!(
            error,
            CommandRegistryError {
                id: "file.save".into()
            }
        );
        assert_eq!(error.id(), "file.save");
        assert_eq!(error.to_string(), "duplicate command id `file.save`");
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn command_registry_register_all_stops_at_first_duplicate() {
        let mut registry = CommandRegistry::new("commands:1");
        let result = registry.register_all([
            CommandContribution::new(CommandDescriptor::new("file.open", "Open File")),
            CommandContribution::new(CommandDescriptor::new("file.open", "Open Again")),
            CommandContribution::new(CommandDescriptor::new("file.save", "Save File")),
        ]);

        assert_eq!(result.unwrap_err().id(), "file.open");
        assert!(registry.is_empty());
    }

    #[test]
    fn command_registry_snapshot_can_be_built_directly() {
        let snapshot = CommandRegistrySnapshot::new(
            "commands:manual",
            [CommandContribution::new(
                CommandDescriptor::new("workspace.close", "Close Workspace")
                    .disabled(true)
                    .when("workspace")
                    .menu_path(["File", "Close Workspace"]),
            )
            .source("workspace")],
        );

        assert!(!snapshot.is_empty());
        assert_eq!(snapshot.len(), 1);
        let descriptor = snapshot.contributions()[0].descriptor();
        assert_eq!(descriptor.id(), "workspace.close");
        assert!(descriptor.disabled_state());
        assert_eq!(descriptor.when_ref(), Some("workspace"));
        assert_eq!(descriptor.menu_path_ref(), ["File", "Close Workspace"]);
    }
}
