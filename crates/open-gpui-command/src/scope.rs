//! Scoped command registration and active-scope projection.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::{CommandContribution, CommandRegistryError, CommandRegistrySnapshot, CommandSourceId};

/// Stable command scope id, such as `global`, `workspace`, or a focused editor id.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommandScopeId(String);

impl CommandScopeId {
    /// Creates a command scope id.
    pub fn new(scope: impl Into<String>) -> Self {
        Self(scope.into())
    }

    /// Returns the scope id as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for CommandScopeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<String> for CommandScopeId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for CommandScopeId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct CommandScope {
    contributions: Vec<CommandContribution>,
    ids: BTreeSet<String>,
}

/// Diagnostic kind emitted while projecting active command scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandProjectionDiagnosticKind {
    /// A later active scope replaced an earlier contribution with the same command id.
    DuplicateCommandId,
}

/// One scoped command projection diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandProjectionDiagnostic {
    kind: CommandProjectionDiagnosticKind,
    command_id: String,
    scope_id: CommandScopeId,
    replaced_source: Option<CommandSourceId>,
    active_source: Option<CommandSourceId>,
}

impl CommandProjectionDiagnostic {
    fn duplicate(
        command_id: impl Into<String>,
        scope_id: CommandScopeId,
        replaced_source: Option<CommandSourceId>,
        active_source: Option<CommandSourceId>,
    ) -> Self {
        Self {
            kind: CommandProjectionDiagnosticKind::DuplicateCommandId,
            command_id: command_id.into(),
            scope_id,
            replaced_source,
            active_source,
        }
    }

    /// Returns the diagnostic kind.
    pub const fn kind(&self) -> CommandProjectionDiagnosticKind {
        self.kind
    }

    /// Returns the duplicate command id.
    pub fn command_id(&self) -> &str {
        &self.command_id
    }

    /// Returns the active scope that introduced this diagnostic.
    pub const fn scope_id(&self) -> &CommandScopeId {
        &self.scope_id
    }

    /// Returns the replaced source, when known.
    pub fn replaced_source_ref(&self) -> Option<&str> {
        self.replaced_source.as_ref().map(CommandSourceId::as_str)
    }

    /// Returns the source that remains active, when known.
    pub fn active_source_ref(&self) -> Option<&str> {
        self.active_source.as_ref().map(CommandSourceId::as_str)
    }
}

/// Result of projecting active command scopes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandScopeProjection {
    snapshot: CommandRegistrySnapshot,
    diagnostics: Vec<CommandProjectionDiagnostic>,
}

impl CommandScopeProjection {
    /// Creates a scoped projection.
    pub fn new(
        snapshot: CommandRegistrySnapshot,
        diagnostics: Vec<CommandProjectionDiagnostic>,
    ) -> Self {
        Self {
            snapshot,
            diagnostics,
        }
    }

    /// Returns the projected registry snapshot.
    pub const fn snapshot(&self) -> &CommandRegistrySnapshot {
        &self.snapshot
    }

    /// Consumes the projection and returns its snapshot.
    pub fn into_snapshot(self) -> CommandRegistrySnapshot {
        self.snapshot
    }

    /// Returns projection diagnostics.
    pub fn diagnostics(&self) -> &[CommandProjectionDiagnostic] {
        &self.diagnostics
    }
}

/// Deterministic scoped command registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedCommandRegistry {
    revision: String,
    scope_order: Vec<CommandScopeId>,
    scopes: BTreeMap<CommandScopeId, CommandScope>,
}

impl ScopedCommandRegistry {
    /// Creates an empty scoped command registry.
    pub fn new(revision: impl Into<String>) -> Self {
        Self {
            revision: revision.into(),
            scope_order: Vec::new(),
            scopes: BTreeMap::new(),
        }
    }

    /// Returns the caller-owned revision label.
    pub fn revision(&self) -> &str {
        &self.revision
    }

    /// Registers one command contribution in a scope.
    pub fn register_in_scope(
        &mut self,
        scope_id: impl Into<CommandScopeId>,
        contribution: CommandContribution,
    ) -> Result<(), CommandRegistryError> {
        let scope_id = scope_id.into();
        if scope_id.is_empty() {
            return Ok(());
        }
        if !self.scopes.contains_key(&scope_id) {
            self.scope_order.push(scope_id.clone());
        }
        let scope = self.scopes.entry(scope_id).or_default();
        let command_id = contribution.descriptor().id().to_owned();
        if !scope.ids.insert(command_id.clone()) {
            return Err(CommandRegistryError::duplicate(command_id));
        }
        scope.contributions.push(contribution);
        Ok(())
    }

    /// Registers many command contributions in a scope atomically.
    pub fn register_all_in_scope(
        &mut self,
        scope_id: impl Into<CommandScopeId>,
        contributions: impl IntoIterator<Item = CommandContribution>,
    ) -> Result<(), CommandRegistryError> {
        let scope_id = scope_id.into();
        if scope_id.is_empty() {
            return Ok(());
        }
        let contributions = contributions.into_iter().collect::<Vec<_>>();
        let mut next_ids = self
            .scopes
            .get(&scope_id)
            .map(|scope| scope.ids.clone())
            .unwrap_or_default();
        for contribution in &contributions {
            let command_id = contribution.descriptor().id().to_owned();
            if !next_ids.insert(command_id.clone()) {
                return Err(CommandRegistryError::duplicate(command_id));
            }
        }
        if !self.scopes.contains_key(&scope_id) {
            self.scope_order.push(scope_id.clone());
        }
        let scope = self.scopes.entry(scope_id).or_default();
        scope.ids = next_ids;
        scope.contributions.extend(contributions);
        Ok(())
    }

    /// Removes a complete scope.
    pub fn unregister_scope(&mut self, scope_id: impl Into<CommandScopeId>) -> bool {
        let scope_id = scope_id.into();
        let removed = self.scopes.remove(&scope_id).is_some();
        if removed {
            self.scope_order.retain(|candidate| candidate != &scope_id);
        }
        removed
    }

    /// Removes all contributions from a source across all scopes.
    pub fn unregister_source(&mut self, source_id: impl Into<CommandSourceId>) -> usize {
        let source_id = source_id.into();
        let mut removed = 0;
        for scope in self.scopes.values_mut() {
            let mut next_ids = BTreeSet::new();
            scope.contributions.retain(|contribution| {
                let should_keep = contribution.source_ref() != Some(source_id.as_str());
                if should_keep {
                    next_ids.insert(contribution.descriptor().id().to_owned());
                } else {
                    removed += 1;
                }
                should_keep
            });
            scope.ids = next_ids;
        }
        removed
    }

    /// Projects active scopes into one registry snapshot.
    ///
    /// Active scopes are applied in the caller-provided order. Later active scopes replace earlier
    /// contributions with the same command id and emit duplicate diagnostics.
    pub fn project_active_scopes(
        &self,
        active_scope_ids: impl IntoIterator<Item = impl Into<CommandScopeId>>,
    ) -> CommandScopeProjection {
        let mut contributions = Vec::<CommandContribution>::new();
        let mut positions = BTreeMap::<String, usize>::new();
        let mut diagnostics = Vec::new();

        for scope_id in active_scope_ids {
            let scope_id = scope_id.into();
            let Some(scope) = self.scopes.get(&scope_id) else {
                continue;
            };
            for contribution in &scope.contributions {
                let command_id = contribution.descriptor().id().to_owned();
                if let Some(position) = positions.get(command_id.as_str()).copied() {
                    let replaced = contributions[position]
                        .source_ref()
                        .map(CommandSourceId::new);
                    let active = contribution.source_ref().map(CommandSourceId::new);
                    diagnostics.push(CommandProjectionDiagnostic::duplicate(
                        command_id.clone(),
                        scope_id.clone(),
                        replaced,
                        active,
                    ));
                    contributions[position] = contribution.clone();
                } else {
                    positions.insert(command_id, contributions.len());
                    contributions.push(contribution.clone());
                }
            }
        }

        CommandScopeProjection::new(
            CommandRegistrySnapshot::new(self.revision.clone(), contributions),
            diagnostics,
        )
    }

    /// Projects all registered scopes in registration order.
    pub fn project_all_scopes(&self) -> CommandScopeProjection {
        self.project_active_scopes(self.scope_order.iter().cloned())
    }

    /// Returns registered scope ids in first-registration order.
    pub fn scope_ids(&self) -> &[CommandScopeId] {
        &self.scope_order
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        CommandContribution, CommandDescriptor, CommandProjectionDiagnosticKind,
        ScopedCommandRegistry,
    };

    #[test]
    fn scoped_projection_uses_active_scope_order_with_later_overrides() {
        let mut registry = ScopedCommandRegistry::new("commands:scoped");
        registry
            .register_in_scope(
                "global",
                CommandContribution::new(CommandDescriptor::new("file.save", "Save"))
                    .source("global"),
            )
            .unwrap();
        registry
            .register_in_scope(
                "editor",
                CommandContribution::new(CommandDescriptor::new("file.save", "Save Editor"))
                    .source("editor"),
            )
            .unwrap();
        registry
            .register_in_scope(
                "editor",
                CommandContribution::new(CommandDescriptor::new("editor.format", "Format"))
                    .source("editor"),
            )
            .unwrap();

        let projection = registry.project_active_scopes(["global", "editor"]);
        let labels = projection
            .snapshot()
            .descriptors()
            .map(|descriptor| descriptor.label().to_owned())
            .collect::<Vec<_>>();

        assert_eq!(labels, ["Save Editor", "Format"]);
        assert_eq!(projection.diagnostics().len(), 1);
        assert_eq!(
            projection.diagnostics()[0].kind(),
            CommandProjectionDiagnosticKind::DuplicateCommandId
        );
        assert_eq!(projection.diagnostics()[0].command_id(), "file.save");
        assert_eq!(
            projection.diagnostics()[0].replaced_source_ref(),
            Some("global")
        );
        assert_eq!(
            projection.diagnostics()[0].active_source_ref(),
            Some("editor")
        );
    }

    #[test]
    fn scoped_registry_unregisters_scope_and_source() {
        let mut registry = ScopedCommandRegistry::new("commands:scoped");
        registry
            .register_in_scope(
                "global",
                CommandContribution::new(CommandDescriptor::new("file.open", "Open"))
                    .source("files"),
            )
            .unwrap();
        registry
            .register_in_scope(
                "workspace",
                CommandContribution::new(CommandDescriptor::new("workspace.close", "Close"))
                    .source("workspace"),
            )
            .unwrap();

        assert_eq!(registry.unregister_source("files"), 1);
        assert_eq!(
            registry
                .project_all_scopes()
                .snapshot()
                .descriptors()
                .map(|descriptor| descriptor.id().to_owned())
                .collect::<Vec<_>>(),
            ["workspace.close"]
        );
        assert!(registry.unregister_scope("workspace"));
        assert!(registry.project_all_scopes().snapshot().is_empty());
    }
}
