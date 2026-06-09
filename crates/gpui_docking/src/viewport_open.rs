use crate::{DockController, DockHost, DockSpaceId, DockViewportAdapter};
use open_gpui::{AnyWindowHandle, App, AppContext as _, Entity, Result, WindowOptions};

/// Runtime result of opening or reopening a platform viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockViewportOpenOutcome {
    /// Logical dock space rendered by the window.
    pub space: DockSpaceId,
    /// GPUI window that renders the logical dock space.
    pub window: AnyWindowHandle,
    /// Whether the runtime opened, reused, or replaced a window.
    pub status: DockViewportOpenStatus,
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
    /// Opens or reuses a GPUI window that renders a logical dock space.
    ///
    /// This is the low-level window mapping primitive. It registers the viewport window and mounts
    /// a controller-backed [`DockHost`], but it does not install the should-close hook required for
    /// [`DockViewportClosePolicy::Prevent`](crate::DockViewportClosePolicy::Prevent). Applications
    /// that need product close semantics should open windows through
    /// [`DockViewportRuntime`](crate::DockViewportRuntime) or
    /// [`DockViewportRuntimeHandle`](crate::DockViewportRuntimeHandle).
    ///
    /// The returned window root is a controller-backed [`DockHost`]. If the dock space already has
    /// a live registered window, that window is activated and reused. If the existing mapping is
    /// stale, it is removed before opening a replacement window.
    pub fn open_viewport(
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
                return Ok(DockViewportOpenOutcome {
                    space,
                    window,
                    status: DockViewportOpenStatus::Reused,
                });
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

        Ok(DockViewportOpenOutcome {
            space,
            window,
            status,
        })
    }
}
