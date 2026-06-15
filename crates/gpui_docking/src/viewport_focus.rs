use crate::DockItemId;

/// Explicit viewport focus request tracked by activation and render state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DockViewportFocusRequest {
    /// Focus a concrete dock item.
    Panel(DockItemId),
    /// Clear focus from dock panels without falling back to another item.
    NoPanelFocus,
    /// Restore the most recently focused visible dock item, or clear focus if none remains.
    RestoreLastFocused,
}

impl DockViewportFocusRequest {
    /// Requests focus for a concrete dock item after viewport activation.
    pub fn panel(item: impl Into<DockItemId>) -> Self {
        Self::Panel(item.into())
    }

    /// Requests focus restoration from the runtime's recorded visible panel history.
    pub fn restore_last_focused() -> Self {
        Self::RestoreLastFocused
    }

    /// Requests clearing dock panel focus without restoring another panel.
    pub fn no_panel_focus() -> Self {
        Self::NoPanelFocus
    }

    pub(crate) fn panel_or_no_panel_focus(item: Option<DockItemId>) -> Self {
        item.map_or_else(Self::no_panel_focus, Self::panel)
    }
}
