use crate::DockSpaceId;
use open_gpui::WindowId;

/// Runtime identity for one logical viewport binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportIdentity {
    space: DockSpaceId,
    window_id: WindowId,
}

impl DockViewportIdentity {
    /// Creates a viewport identity from the logical space and platform window id.
    pub(crate) fn new(space: impl Into<DockSpaceId>, window_id: WindowId) -> Self {
        Self {
            space: space.into(),
            window_id,
        }
    }

    /// Returns the logical dock space.
    pub(crate) fn space(&self) -> &DockSpaceId {
        &self.space
    }

    /// Returns the platform window id.
    pub(crate) fn window_id(&self) -> WindowId {
        self.window_id
    }

    /// Reports whether these facts still describe the same runtime viewport binding.
    pub(crate) fn matches(&self, space: &DockSpaceId, window_id: WindowId) -> bool {
        self.space() == space && self.window_id() == window_id
    }
}
