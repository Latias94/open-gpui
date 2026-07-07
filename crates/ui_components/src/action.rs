//! Shared renderer-neutral action and icon projection types.

use open_gpui_command::{CommandDescriptor, CommandIconDescriptor};

/// Renderer-neutral icon intent carried by action descriptors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionIconDescriptor {
    name: String,
    fallback_label: Option<String>,
}

impl ActionIconDescriptor {
    /// Creates an icon descriptor with an app-owned icon name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            fallback_label: None,
        }
    }

    /// Applies fallback text that can be rendered when the app cannot resolve the icon asset.
    pub fn fallback_label(mut self, fallback_label: impl Into<String>) -> Self {
        let fallback_label = fallback_label.into();
        if !fallback_label.is_empty() {
            self.fallback_label = Some(fallback_label);
        }
        self
    }

    /// Returns the app-owned icon name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns fallback text for unresolved icon assets.
    pub fn fallback_label_ref(&self) -> Option<&str> {
        self.fallback_label.as_deref()
    }

    /// Returns whether the icon name is empty.
    pub fn is_empty(&self) -> bool {
        self.name.is_empty()
    }
}

impl From<&str> for ActionIconDescriptor {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ActionIconDescriptor {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<CommandIconDescriptor> for ActionIconDescriptor {
    fn from(value: CommandIconDescriptor) -> Self {
        let mut descriptor = Self::new(value.name());
        if let Some(fallback_label) = value.fallback_label_ref() {
            descriptor = descriptor.fallback_label(fallback_label);
        }
        descriptor
    }
}

impl From<&CommandIconDescriptor> for ActionIconDescriptor {
    fn from(value: &CommandIconDescriptor) -> Self {
        value.clone().into()
    }
}

/// Diagnostic emitted when an app icon resolver cannot produce a concrete icon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionIconDiagnostic {
    icon_name: String,
    message: String,
}

impl ActionIconDiagnostic {
    /// Creates an icon diagnostic.
    pub fn new(icon_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            icon_name: icon_name.into(),
            message: message.into(),
        }
    }

    /// Returns the unresolved icon name.
    pub fn icon_name(&self) -> &str {
        &self.icon_name
    }

    /// Returns the diagnostic message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// App-resolved icon facts consumed by concrete components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedActionIcon {
    descriptor: ActionIconDescriptor,
    label: Option<String>,
    diagnostic: Option<ActionIconDiagnostic>,
}

impl ResolvedActionIcon {
    /// Creates resolved icon facts.
    pub fn resolved(descriptor: ActionIconDescriptor, label: impl Into<String>) -> Self {
        let label = label.into();
        Self {
            descriptor,
            label: (!label.is_empty()).then_some(label),
            diagnostic: None,
        }
    }

    /// Creates unresolved icon facts while preserving action visibility.
    pub fn unresolved(descriptor: ActionIconDescriptor) -> Self {
        let label = descriptor
            .fallback_label_ref()
            .map(str::to_owned)
            .filter(|label| !label.is_empty());
        Self {
            descriptor,
            label,
            diagnostic: None,
        }
    }

    /// Creates missing-icon facts with diagnostic metadata and a fallback render label.
    pub fn missing(descriptor: ActionIconDescriptor, message: impl Into<String>) -> Self {
        let fallback = descriptor
            .fallback_label_ref()
            .map(str::to_owned)
            .unwrap_or_else(|| descriptor.name().to_owned());
        let diagnostic = ActionIconDiagnostic::new(descriptor.name(), message);
        Self {
            descriptor,
            label: Some(fallback),
            diagnostic: Some(diagnostic),
        }
    }

    /// Returns the original renderer-neutral icon descriptor.
    pub const fn descriptor(&self) -> &ActionIconDescriptor {
        &self.descriptor
    }

    /// Returns the concrete render label produced by the app resolver.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Returns missing-icon diagnostic metadata, if present.
    pub const fn diagnostic(&self) -> Option<&ActionIconDiagnostic> {
        self.diagnostic.as_ref()
    }

    /// Returns whether the icon resolver reported a missing asset.
    pub const fn missing_asset(&self) -> bool {
        self.diagnostic.is_some()
    }
}

/// App-owned icon resolver for action descriptors.
pub trait ActionIconResolver {
    /// Resolves a renderer-neutral icon descriptor into concrete render facts.
    fn resolve_icon(&self, descriptor: &ActionIconDescriptor) -> ResolvedActionIcon;
}

impl<F> ActionIconResolver for F
where
    F: Fn(&ActionIconDescriptor) -> ResolvedActionIcon,
{
    fn resolve_icon(&self, descriptor: &ActionIconDescriptor) -> ResolvedActionIcon {
        self(descriptor)
    }
}

/// Renderer-neutral action facts before app-owned icon resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionDescriptor {
    value: String,
    label: String,
    icon: Option<ActionIconDescriptor>,
    shortcut: Option<String>,
    disabled: bool,
    disabled_reason: Option<String>,
    tooltip: Option<String>,
    accessibility_description: Option<String>,
}

impl ActionDescriptor {
    /// Creates an action descriptor with a stable value and visible label.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            icon: None,
            shortcut: None,
            disabled: false,
            disabled_reason: None,
            tooltip: None,
            accessibility_description: None,
        }
    }

    /// Creates an action descriptor from shared command metadata.
    pub fn from_command_descriptor(descriptor: &CommandDescriptor) -> Self {
        let mut action =
            Self::new(descriptor.id(), descriptor.label()).disabled(descriptor.disabled_state());
        if let Some(icon) = descriptor.icon_ref() {
            action = action.icon(ActionIconDescriptor::from(icon));
        }
        if let Some(shortcut) = descriptor.shortcut_ref() {
            action = action.shortcut(shortcut);
        }
        if let Some(reason) = descriptor.disabled_reason_ref() {
            action = action.disabled_reason(reason);
        }
        if let Some(tooltip) = descriptor.tooltip_ref() {
            action = action.tooltip(tooltip);
        }
        if let Some(description) = descriptor.accessibility_description_ref() {
            action = action.accessibility_description(description);
        }
        action
    }

    /// Applies renderer-neutral icon metadata.
    pub fn icon(mut self, icon: impl Into<ActionIconDescriptor>) -> Self {
        let icon = icon.into();
        if !icon.is_empty() {
            self.icon = Some(icon);
        }
        self
    }

    /// Applies a display shortcut label.
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

    /// Applies user-displayable tooltip metadata.
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        let tooltip = tooltip.into();
        if !tooltip.is_empty() {
            self.tooltip = Some(tooltip);
        }
        self
    }

    /// Applies an accessibility description in addition to the visible label.
    pub fn accessibility_description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        if !description.is_empty() {
            self.accessibility_description = Some(description);
        }
        self
    }

    /// Resolves this action with an app-owned icon resolver.
    pub fn resolve_with(self, resolver: &impl ActionIconResolver) -> ResolvedActionState {
        ResolvedActionState::from_descriptor_with_resolver(self, resolver)
    }

    /// Resolves this action using fallback icon labels without emitting diagnostics.
    pub fn resolve_without_icon_diagnostics(self) -> ResolvedActionState {
        ResolvedActionState::from_descriptor_without_icon_diagnostics(self)
    }

    /// Returns the stable action value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible action label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns renderer-neutral icon metadata.
    pub const fn icon_ref(&self) -> Option<&ActionIconDescriptor> {
        self.icon.as_ref()
    }

    /// Returns the display shortcut label.
    pub fn shortcut_ref(&self) -> Option<&str> {
        self.shortcut.as_deref()
    }

    /// Returns whether the action is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }

    /// Returns the optional disabled reason.
    pub fn disabled_reason_ref(&self) -> Option<&str> {
        self.disabled_reason.as_deref()
    }

    /// Returns user-displayable tooltip metadata.
    pub fn tooltip_ref(&self) -> Option<&str> {
        self.tooltip.as_deref()
    }

    /// Returns the optional accessibility description.
    pub fn accessibility_description_ref(&self) -> Option<&str> {
        self.accessibility_description.as_deref()
    }
}

/// UI-ready action facts after app-owned icon resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedActionState {
    value: String,
    label: String,
    icon: Option<ResolvedActionIcon>,
    shortcut: Option<String>,
    disabled: bool,
    disabled_reason: Option<String>,
    tooltip: Option<String>,
    accessibility_description: Option<String>,
    diagnostics: Vec<ActionIconDiagnostic>,
}

impl ResolvedActionState {
    /// Resolves renderer-neutral action metadata with an app-owned icon resolver.
    pub fn from_descriptor_with_resolver(
        descriptor: ActionDescriptor,
        resolver: &impl ActionIconResolver,
    ) -> Self {
        let icon = descriptor
            .icon
            .clone()
            .map(|icon| resolver.resolve_icon(&icon));
        Self::from_parts(descriptor, icon)
    }

    /// Resolves renderer-neutral action metadata using fallback icon labels only.
    pub fn from_descriptor_without_icon_diagnostics(descriptor: ActionDescriptor) -> Self {
        let icon = descriptor.icon.clone().map(ResolvedActionIcon::unresolved);
        Self::from_parts(descriptor, icon)
    }

    fn from_parts(descriptor: ActionDescriptor, icon: Option<ResolvedActionIcon>) -> Self {
        let diagnostics = icon
            .iter()
            .filter_map(|icon| icon.diagnostic().cloned())
            .collect();
        Self {
            value: descriptor.value,
            label: descriptor.label,
            icon,
            shortcut: descriptor.shortcut,
            disabled: descriptor.disabled,
            disabled_reason: descriptor.disabled_reason,
            tooltip: descriptor.tooltip,
            accessibility_description: descriptor.accessibility_description,
            diagnostics,
        }
    }

    /// Returns the stable action value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible action label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns app-resolved icon facts.
    pub const fn icon(&self) -> Option<&ResolvedActionIcon> {
        self.icon.as_ref()
    }

    /// Returns the concrete render label for the resolved icon.
    pub fn icon_label(&self) -> Option<&str> {
        self.icon.as_ref().and_then(ResolvedActionIcon::label)
    }

    /// Returns the display shortcut label.
    pub fn shortcut(&self) -> Option<&str> {
        self.shortcut.as_deref()
    }

    /// Returns whether the action is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns the optional disabled reason.
    pub fn disabled_reason(&self) -> Option<&str> {
        self.disabled_reason.as_deref()
    }

    /// Returns user-displayable tooltip metadata.
    pub fn tooltip(&self) -> Option<&str> {
        self.tooltip.as_deref()
    }

    /// Returns the optional accessibility description.
    pub fn accessibility_description(&self) -> Option<&str> {
        self.accessibility_description.as_deref()
    }

    /// Returns icon/action diagnostics emitted during resolution.
    pub fn diagnostics(&self) -> &[ActionIconDiagnostic] {
        &self.diagnostics
    }

    /// Returns whether this action has icon/action diagnostics.
    pub fn has_diagnostics(&self) -> bool {
        !self.diagnostics.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{ActionDescriptor, ActionIconDescriptor, ResolvedActionIcon, ResolvedActionState};
    use open_gpui_command::{CommandDescriptor, CommandIconDescriptor};

    #[test]
    fn action_descriptor_projects_command_metadata() {
        let command = CommandDescriptor::new("workspace.open", "Open Workspace")
            .icon(CommandIconDescriptor::new("folder-open").fallback_label("O"))
            .shortcut("Ctrl+O")
            .disabled_reason("No workspace")
            .tooltip("Open a workspace")
            .accessibility_description("Opens the workspace picker");

        let action = ActionDescriptor::from_command_descriptor(&command);

        assert_eq!(action.value(), "workspace.open");
        assert_eq!(action.label(), "Open Workspace");
        assert_eq!(action.icon_ref().unwrap().name(), "folder-open");
        assert_eq!(action.icon_ref().unwrap().fallback_label_ref(), Some("O"));
        assert_eq!(action.shortcut_ref(), Some("Ctrl+O"));
        assert!(action.disabled_state());
        assert_eq!(action.disabled_reason_ref(), Some("No workspace"));
        assert_eq!(action.tooltip_ref(), Some("Open a workspace"));
        assert_eq!(
            action.accessibility_description_ref(),
            Some("Opens the workspace picker")
        );
    }

    #[test]
    fn unknown_icon_resolution_reports_diagnostic_without_hiding_action() {
        let action = ActionDescriptor::new("workspace.open", "Open Workspace")
            .icon(ActionIconDescriptor::new("missing-folder").fallback_label("O"));

        let resolved = action.resolve_with(&|icon: &ActionIconDescriptor| {
            ResolvedActionIcon::missing(icon.clone(), "icon asset is not registered")
        });

        assert_eq!(resolved.value(), "workspace.open");
        assert_eq!(resolved.label(), "Open Workspace");
        assert_eq!(resolved.icon_label(), Some("O"));
        assert!(resolved.has_diagnostics());
        assert_eq!(resolved.diagnostics()[0].icon_name(), "missing-folder");
        assert_eq!(
            resolved.diagnostics()[0].message(),
            "icon asset is not registered"
        );
    }

    #[test]
    fn fallback_resolution_keeps_action_visible_without_diagnostics() {
        let resolved = ResolvedActionState::from_descriptor_without_icon_diagnostics(
            ActionDescriptor::new("file.save", "Save")
                .icon(ActionIconDescriptor::new("save").fallback_label("S")),
        );

        assert_eq!(resolved.icon_label(), Some("S"));
        assert!(!resolved.has_diagnostics());
    }
}
