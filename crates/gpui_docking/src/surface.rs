mod builder;
mod panel;
mod viewport;

pub use builder::{DockSurfaceBuildError, DockSurfaceBuilder};
pub use panel::{DockSurfaceChange, DockSurfacePanelError, DockSurfacePanelOutcome};
pub use viewport::{
    DockSurfaceViewportOpenOutcome, DockSurfaceViewportOpenReport, DockSurfaceViewportOpenStatus,
    DockSurfaceViewportOpened, DockSurfaceViewportSpec, DockSurfaceViewportSpecError,
    DockSurfaceViewportUnavailable,
};

use crate::{
    DockController, DockHost, DockSpaceId, DockViewportClosePolicy, DockViewportRuntimeHandle,
};
use open_gpui::{
    App, AppContext as _, Bounds, Context, Entity, Pixels, Result as GpuiResult, WindowBounds,
    WindowHandle, WindowOptions,
};

/// Application-level owner for one docked workspace and its viewport runtime.
///
/// `DockSurface` is the common app seam for docking. It keeps controller state, host creation, and
/// viewport runtime wiring together so ordinary applications do not need to assemble
/// [`DockHost`] and [`DockViewportRuntimeHandle`] directly.
#[derive(Clone, Debug)]
pub struct DockSurface {
    controller: Entity<DockController>,
    primary_space: DockSpaceId,
    viewport_runtime: DockViewportRuntimeHandle,
}

impl DockSurface {
    /// Starts a facade-first docking surface builder for a logical dock space.
    pub fn builder(space: impl Into<DockSpaceId>) -> DockSurfaceBuilder {
        DockSurfaceBuilder::new(space)
    }

    /// Wraps an existing controller entity with the default viewport close policy.
    pub fn from_controller(controller: Entity<DockController>, cx: &App) -> Self {
        Self::from_controller_with_close_policy(controller, DockViewportClosePolicy::default(), cx)
    }

    /// Wraps an existing controller entity with an explicit viewport close policy.
    pub fn from_controller_with_close_policy(
        controller: Entity<DockController>,
        close_policy: DockViewportClosePolicy,
        cx: &App,
    ) -> Self {
        let primary_space = cx.read_entity(&controller, |controller, _| controller.space().clone());
        let viewport_runtime =
            DockViewportRuntimeHandle::with_close_policy(controller.clone(), close_policy);
        Self {
            controller,
            primary_space,
            viewport_runtime,
        }
    }

    /// Returns the controller entity owned by this surface.
    pub fn controller(&self) -> Entity<DockController> {
        self.controller.clone()
    }

    /// Returns the default logical dock space for primary host windows.
    pub fn primary_space(&self) -> &DockSpaceId {
        &self.primary_space
    }

    /// Returns the close policy used by facade-opened platform viewport windows.
    pub fn viewport_close_policy(&self) -> DockViewportClosePolicy {
        self.viewport_runtime.close_policy()
    }

    /// Returns true when a facade-opened platform viewport is registered for the dock space.
    pub fn is_viewport_open(&self, space: &DockSpaceId) -> bool {
        self.viewport_runtime.is_viewport_open(space)
    }

    /// Creates a host view for the surface's primary dock space.
    pub fn primary_host(&self, cx: &mut Context<DockHost>) -> DockHost {
        self.host(self.primary_space.clone(), cx)
    }

    /// Creates a host view for one dock space.
    pub fn host(&self, space: impl Into<DockSpaceId>, cx: &mut Context<DockHost>) -> DockHost {
        DockHost::from_controller(
            self.controller.clone(),
            space,
            self.viewport_runtime.clone(),
            cx,
        )
    }

    /// Opens a normal GPUI window that renders the primary dock host.
    ///
    /// This is for the main application window and does not require platform viewport-window
    /// capability. Detached platform viewports are opened through the viewport-runtime path.
    pub fn open_primary_window(
        &self,
        options: WindowOptions,
        cx: &mut App,
    ) -> GpuiResult<WindowHandle<DockHost>> {
        let surface = self.clone();
        cx.open_window(options, move |_, cx| {
            cx.new(move |cx| surface.primary_host(cx))
        })
    }

    /// Returns default window options for a centered primary dock host.
    pub fn primary_window_options(bounds: Bounds<Pixels>) -> WindowOptions {
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        }
    }
}
