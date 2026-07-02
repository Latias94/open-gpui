//! Renderer-neutral command descriptor contracts.

/// Pure app-command metadata shared by command palettes and menu projections.
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

    /// Returns caller-owned availability metadata.
    pub fn when_ref(&self) -> Option<&str> {
        self.when.as_deref()
    }

    /// Returns the menu path projection.
    pub fn menu_path_ref(&self) -> &[String] {
        &self.menu_path
    }
}

#[cfg(test)]
mod tests {
    use super::CommandDescriptor;

    #[test]
    fn command_descriptor_records_projection_metadata_without_runtime() {
        let descriptor = CommandDescriptor::new("workspace.open", "Open Workspace")
            .group("Workspace")
            .keywords(["project", "folder"])
            .shortcut("Ctrl+Shift+O")
            .disabled(true)
            .when("workspace")
            .menu_path(["File", "", "Open"]);

        assert_eq!(descriptor.id(), "workspace.open");
        assert_eq!(descriptor.label(), "Open Workspace");
        assert_eq!(descriptor.group_ref(), Some("Workspace"));
        assert_eq!(descriptor.keywords_ref(), ["project", "folder"]);
        assert_eq!(descriptor.shortcut_ref(), Some("Ctrl+Shift+O"));
        assert!(descriptor.disabled_state());
        assert_eq!(descriptor.when_ref(), Some("workspace"));
        assert_eq!(descriptor.menu_path_ref(), ["File", "Open"]);
    }
}
