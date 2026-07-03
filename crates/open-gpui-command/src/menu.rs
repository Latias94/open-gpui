//! Neutral command menu projection.

use crate::CommandRegistrySnapshot;

/// Neutral command menu tree built from command metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandMenuTree {
    revision: String,
    entries: Vec<CommandMenuEntry>,
}

impl CommandMenuTree {
    /// Creates a command menu tree.
    pub fn new(
        revision: impl Into<String>,
        entries: impl IntoIterator<Item = CommandMenuEntry>,
    ) -> Self {
        Self {
            revision: revision.into(),
            entries: entries.into_iter().collect(),
        }
    }

    /// Builds a menu tree from a registry snapshot.
    pub fn from_registry_snapshot(snapshot: &CommandRegistrySnapshot) -> Self {
        let mut entries = Vec::new();
        for descriptor in snapshot.descriptors() {
            insert_command_entry(
                &mut entries,
                descriptor.menu_path_ref(),
                CommandMenuCommand::from_descriptor(descriptor),
            );
        }
        Self::new(snapshot.revision(), entries)
    }

    /// Returns the tree revision.
    pub fn revision(&self) -> &str {
        &self.revision
    }

    /// Returns root menu entries.
    pub fn entries(&self) -> &[CommandMenuEntry] {
        &self.entries
    }
}

/// One neutral menu entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandMenuEntry {
    /// A concrete command leaf.
    Command(CommandMenuCommand),
    /// A submenu grouping node.
    Submenu(CommandMenuSubmenu),
}

impl CommandMenuEntry {
    /// Returns the entry label.
    pub fn label(&self) -> &str {
        match self {
            Self::Command(command) => command.label(),
            Self::Submenu(submenu) => submenu.label(),
        }
    }

    /// Returns this entry as a command, when it is a command leaf.
    pub const fn as_command(&self) -> Option<&CommandMenuCommand> {
        match self {
            Self::Command(command) => Some(command),
            Self::Submenu(_) => None,
        }
    }

    /// Returns this entry as a submenu, when it is a submenu node.
    pub const fn as_submenu(&self) -> Option<&CommandMenuSubmenu> {
        match self {
            Self::Command(_) => None,
            Self::Submenu(submenu) => Some(submenu),
        }
    }
}

/// Neutral command menu leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandMenuCommand {
    command_id: String,
    label: String,
    shortcut: Option<String>,
    disabled: bool,
    disabled_reason: Option<String>,
    when: Option<String>,
}

impl CommandMenuCommand {
    fn from_descriptor(descriptor: &crate::CommandDescriptor) -> Self {
        Self {
            command_id: descriptor.id().to_owned(),
            label: descriptor.label().to_owned(),
            shortcut: descriptor.shortcut_ref().map(str::to_owned),
            disabled: descriptor.disabled_state(),
            disabled_reason: descriptor.disabled_reason_ref().map(str::to_owned),
            when: descriptor.when_ref().map(str::to_owned),
        }
    }

    /// Returns the stable command id.
    pub fn command_id(&self) -> &str {
        &self.command_id
    }

    /// Returns the visible command label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the display shortcut label.
    pub fn shortcut_ref(&self) -> Option<&str> {
        self.shortcut.as_deref()
    }

    /// Returns whether the command is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }

    /// Returns the disabled reason, if present.
    pub fn disabled_reason_ref(&self) -> Option<&str> {
        self.disabled_reason.as_deref()
    }

    /// Returns caller-owned availability metadata.
    pub fn when_ref(&self) -> Option<&str> {
        self.when.as_deref()
    }
}

/// Neutral command menu submenu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandMenuSubmenu {
    label: String,
    entries: Vec<CommandMenuEntry>,
}

impl CommandMenuSubmenu {
    fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            entries: Vec::new(),
        }
    }

    /// Returns the submenu label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns submenu entries.
    pub fn entries(&self) -> &[CommandMenuEntry] {
        &self.entries
    }
}

fn insert_command_entry(
    entries: &mut Vec<CommandMenuEntry>,
    menu_path: &[String],
    command: CommandMenuCommand,
) {
    let Some((segment, rest)) = menu_path.split_first() else {
        entries.push(CommandMenuEntry::Command(command));
        return;
    };

    let submenu_index = entries.iter().position(|entry| {
        entry
            .as_submenu()
            .is_some_and(|submenu| submenu.label() == segment)
    });
    let submenu_index = if let Some(index) = submenu_index {
        index
    } else {
        entries.push(CommandMenuEntry::Submenu(CommandMenuSubmenu::new(segment)));
        entries.len() - 1
    };

    let Some(submenu) = entries[submenu_index].as_submenu_mut() else {
        return;
    };
    insert_command_entry(&mut submenu.entries, rest, command);
}

impl CommandMenuEntry {
    fn as_submenu_mut(&mut self) -> Option<&mut CommandMenuSubmenu> {
        match self {
            Self::Command(_) => None,
            Self::Submenu(submenu) => Some(submenu),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{CommandDescriptor, CommandMenuEntry, CommandMenuTree, CommandRegistry};

    #[test]
    fn command_menu_tree_projects_nested_menu_paths() {
        let mut registry = CommandRegistry::new("commands:menu");
        registry
            .register(
                CommandDescriptor::new("workspace.open", "Open Workspace")
                    .shortcut("Ctrl+O")
                    .menu_path(["File", "Open Recent"]),
            )
            .unwrap();
        registry
            .register(
                CommandDescriptor::new("workspace.close", "Close Workspace")
                    .disabled_reason("No workspace")
                    .menu_path(["File"]),
            )
            .unwrap();

        let tree = CommandMenuTree::from_registry_snapshot(&registry.snapshot());

        assert_eq!(tree.revision(), "commands:menu");
        let file = tree.entries()[0].as_submenu().unwrap();
        assert_eq!(file.label(), "File");
        let recent = file.entries()[0].as_submenu().unwrap();
        let open = recent.entries()[0].as_command().unwrap();
        assert_eq!(open.command_id(), "workspace.open");
        assert_eq!(open.shortcut_ref(), Some("Ctrl+O"));
        let close = file.entries()[1].as_command().unwrap();
        assert_eq!(close.disabled_reason_ref(), Some("No workspace"));
    }

    #[test]
    fn command_menu_tree_keeps_root_commands_without_menu_path() {
        let mut registry = CommandRegistry::new("commands:menu");
        registry
            .register(CommandDescriptor::new("help.search", "Search Help"))
            .unwrap();

        let tree = CommandMenuTree::from_registry_snapshot(&registry.snapshot());

        assert!(matches!(tree.entries()[0], CommandMenuEntry::Command(_)));
        assert_eq!(
            tree.entries()[0].as_command().unwrap().command_id(),
            "help.search"
        );
    }
}
