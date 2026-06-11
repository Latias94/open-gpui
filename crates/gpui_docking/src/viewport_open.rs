use crate::DockSpaceId;
use open_gpui::AnyWindowHandle;

/// Runtime result of opening or reopening a platform viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockViewportOpenOutcome {
    /// Logical dock space rendered by the window.
    space: DockSpaceId,
    /// GPUI window that renders the logical dock space.
    window: AnyWindowHandle,
    /// Whether the runtime opened, reused, or replaced a window.
    status: DockViewportOpenStatus,
}

impl DockViewportOpenOutcome {
    pub(crate) fn new(
        space: DockSpaceId,
        window: AnyWindowHandle,
        status: DockViewportOpenStatus,
    ) -> Self {
        Self {
            space,
            window,
            status,
        }
    }

    /// Logical dock space rendered by the window.
    pub fn space(&self) -> &DockSpaceId {
        &self.space
    }

    /// GPUI window that renders the logical dock space.
    pub fn window(&self) -> AnyWindowHandle {
        self.window
    }

    /// Whether the runtime opened, reused, or replaced a window.
    pub fn status(&self) -> DockViewportOpenStatus {
        self.status
    }
}

/// How an open or reopen request resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportOpenStatus {
    /// A new GPUI window was opened and registered.
    Opened,
    /// An existing live GPUI window was reused.
    Reused,
    /// A stale or superseded mapping was replaced by a new window.
    Replaced,
}
