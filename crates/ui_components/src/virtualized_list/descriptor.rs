use open_gpui_ui_core::Role;

use super::model::VirtualizedListStateItem;

/// Anatomy of one virtualized-list row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VirtualizedListRowKind {
    /// Selectable item row.
    #[default]
    Item,
    /// Non-selectable section heading that groups following item rows.
    Section,
    /// Non-selectable visual separator.
    Separator,
    /// Non-selectable loading status row.
    Loading,
    /// Non-selectable empty status row.
    Empty,
    /// Non-selectable error status row.
    Error,
}

impl VirtualizedListRowKind {
    /// Returns the stable row kind label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Item => "item",
            Self::Section => "section",
            Self::Separator => "separator",
            Self::Loading => "loading",
            Self::Empty => "empty",
            Self::Error => "error",
        }
    }

    /// Returns whether the row participates in active selection and activation.
    pub const fn selectable(self) -> bool {
        matches!(self, Self::Item)
    }

    /// Returns the row accessibility role.
    pub const fn role(self) -> Role {
        match self {
            Self::Item => Role::ListBoxOption,
            Self::Section => Role::Group,
            Self::Separator => Role::Separator,
            Self::Loading => Role::ProgressIndicator,
            Self::Empty => Role::Section,
            Self::Error => Role::AlertDialog,
        }
    }
}

/// Pure descriptor for one virtualized list item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualizedListItemDescriptor {
    key: String,
    label: String,
    kind: VirtualizedListRowKind,
    disabled: bool,
    disabled_reason: Option<String>,
    secondary_text: Option<String>,
    text_value: Option<String>,
    leading_metadata: Option<String>,
    trailing_metadata: Option<String>,
    badge: Option<String>,
    status: Option<String>,
}

impl VirtualizedListItemDescriptor {
    /// Creates a new item descriptor.
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            kind: VirtualizedListRowKind::Item,
            disabled: false,
            disabled_reason: None,
            secondary_text: None,
            text_value: None,
            leading_metadata: None,
            trailing_metadata: None,
            badge: None,
            status: None,
        }
    }

    /// Creates a selectable item descriptor.
    pub fn item(key: impl Into<String>, primary_text: impl Into<String>) -> Self {
        Self::new(key, primary_text)
    }

    /// Creates a non-selectable section row.
    pub fn section(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(key, label).with_kind(VirtualizedListRowKind::Section)
    }

    /// Creates a non-selectable separator row.
    pub fn separator(key: impl Into<String>) -> Self {
        Self::new(key, "").with_kind(VirtualizedListRowKind::Separator)
    }

    /// Creates a non-selectable loading status row.
    pub fn loading(key: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(key, message).with_kind(VirtualizedListRowKind::Loading)
    }

    /// Creates a non-selectable empty status row.
    pub fn empty(key: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(key, message).with_kind(VirtualizedListRowKind::Empty)
    }

    /// Creates a non-selectable error status row.
    pub fn error(key: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(key, message).with_kind(VirtualizedListRowKind::Error)
    }

    /// Marks the item as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks the item as disabled and records the reason exposed in snapshots.
    pub fn disabled_reason(mut self, reason: impl Into<String>) -> Self {
        self.disabled = true;
        self.disabled_reason = Some(reason.into());
        self
    }

    /// Applies secondary row text.
    pub fn secondary_text(mut self, secondary_text: impl Into<String>) -> Self {
        self.secondary_text = Some(secondary_text.into());
        self
    }

    /// Applies explicit text used by typeahead and activation payloads.
    pub fn with_text_value(mut self, text_value: impl Into<String>) -> Self {
        self.text_value = Some(text_value.into());
        self
    }

    /// Applies leading metadata text.
    pub fn leading_metadata(mut self, metadata: impl Into<String>) -> Self {
        self.leading_metadata = Some(metadata.into());
        self
    }

    /// Applies trailing metadata text.
    pub fn trailing_metadata(mut self, metadata: impl Into<String>) -> Self {
        self.trailing_metadata = Some(metadata.into());
        self
    }

    /// Applies compact badge text.
    pub fn badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// Applies status text.
    pub fn status(mut self, status: impl Into<String>) -> Self {
        self.status = Some(status.into());
        self
    }

    /// Returns the stable item key.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns the visible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the primary row text.
    pub fn primary_text(&self) -> &str {
        &self.label
    }

    /// Returns the secondary row text.
    pub fn secondary_text_ref(&self) -> Option<&str> {
        self.secondary_text.as_deref()
    }

    /// Returns the text value used by typeahead and accessibility.
    pub fn text_value(&self) -> &str {
        self.text_value.as_deref().unwrap_or(&self.label)
    }

    /// Returns the row kind.
    pub const fn kind(&self) -> VirtualizedListRowKind {
        self.kind
    }

    /// Returns whether the item is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }

    /// Returns the disabled reason.
    pub fn disabled_reason_ref(&self) -> Option<&str> {
        self.disabled_reason.as_deref()
    }

    /// Returns leading metadata text.
    pub fn leading_metadata_ref(&self) -> Option<&str> {
        self.leading_metadata.as_deref()
    }

    /// Returns trailing metadata text.
    pub fn trailing_metadata_ref(&self) -> Option<&str> {
        self.trailing_metadata.as_deref()
    }

    /// Returns badge text.
    pub fn badge_ref(&self) -> Option<&str> {
        self.badge.as_deref()
    }

    /// Returns status text.
    pub fn status_ref(&self) -> Option<&str> {
        self.status.as_deref()
    }

    /// Returns whether the row participates in active selection and activation.
    pub const fn selectable(&self) -> bool {
        self.kind.selectable() && !self.disabled
    }

    /// Returns the renderer-neutral state item for this descriptor.
    pub fn state_item(&self) -> VirtualizedListStateItem {
        VirtualizedListStateItem::new(self.key(), self.text_value())
            .row_kind(self.kind)
            .disabled(self.disabled)
    }

    fn with_kind(mut self, kind: VirtualizedListRowKind) -> Self {
        self.kind = kind;
        self.disabled = !kind.selectable();
        self
    }
}
