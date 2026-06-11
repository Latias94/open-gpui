use crate::{DockController, DockHost, DockSpaceId, DockViewportAdapter};
use open_gpui::{AnyWindowHandle, App, AppContext as _, Entity, Result, WindowOptions};

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

impl DockViewportAdapter {
    pub(crate) fn open_viewport(
        &mut self,
        controller: Entity<DockController>,
        space: impl Into<DockSpaceId>,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<DockViewportOpenOutcome> {
        let space = space.into();
        let mut status = DockViewportOpenStatus::Opened;

        if let Some(window) = self.window_for_space(&space) {
            if window
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
            {
                return Ok(DockViewportOpenOutcome::new(
                    space,
                    window,
                    DockViewportOpenStatus::Reused,
                ));
            }

            self.unregister_space(&space);
            status = DockViewportOpenStatus::Replaced;
        }

        let host_space = space.clone();
        let window = cx
            .open_window(options, move |_, cx| {
                cx.new(move |cx| DockHost::from_controller(controller, host_space, cx))
            })?
            .into();
        self.register_viewport(space.clone(), window);

        Ok(DockViewportOpenOutcome::new(space, window, status))
    }
}
