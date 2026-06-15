use crate::DockItemId;

/// Explicit viewport focus request tracked by activation and render state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DockViewportFocusRequest {
    /// Focus a concrete dock item.
    Panel(DockItemId),
    /// Restore the most recently focused visible dock item.
    RestoreLastFocused,
}

impl DockViewportFocusRequest {
    pub(crate) fn panel(item: impl Into<DockItemId>) -> Self {
        Self::Panel(item.into())
    }

    pub(crate) fn restore_last_focused() -> Self {
        Self::RestoreLastFocused
    }

    pub(crate) fn panel_or_restore_last_focused(item: Option<DockItemId>) -> Self {
        item.map_or_else(Self::restore_last_focused, Self::panel)
    }

    pub(crate) fn panel_item(&self) -> Option<&DockItemId> {
        match self {
            Self::Panel(item) => Some(item),
            Self::RestoreLastFocused => None,
        }
    }
}
