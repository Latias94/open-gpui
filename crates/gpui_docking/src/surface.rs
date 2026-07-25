mod activation;
mod builder;
mod owner;
mod panel;
mod state;
mod viewport;
mod viewport_readiness;

pub use activation::{DockSurfaceActivationOutcome, DockSurfaceActivationRequestId};
pub use builder::{DockSurfaceBuildError, DockSurfaceBuilder};
pub use owner::{DockSurfaceChangeCategory, DockSurfaceChangeEvent};
pub use panel::{
    DockSurfaceChange, DockSurfaceFloatingPanelSnapshot, DockSurfacePanelError,
    DockSurfacePanelLocation, DockSurfacePanelLocationKind, DockSurfacePanelOutcome,
    DockSurfacePanelSnapshot,
};
pub use state::DockSurfaceSnapshot;
pub use viewport::{
    DockSurfaceViewportCloseOutcome, DockSurfaceViewportCloseStatus,
    DockSurfaceViewportOpenOutcome, DockSurfaceViewportOpenReport, DockSurfaceViewportOpenStatus,
    DockSurfaceViewportOpened, DockSurfaceViewportRestoreOutcome, DockSurfaceViewportRestoreReport,
    DockSurfaceViewportSession, DockSurfaceViewportShouldCloseOutcome,
    DockSurfaceViewportShouldCloseStatus, DockSurfaceViewportSpec, DockSurfaceViewportSpecError,
    DockSurfaceViewportUnavailable,
};
pub use viewport_readiness::{
    DockSurfaceViewportFlagCapabilities, DockSurfaceViewportFlagWarning,
    DockSurfaceViewportInputStatus, DockSurfaceViewportLifecycleReadiness,
    DockSurfaceViewportPlatformCapabilities, DockSurfaceViewportPlatformReadiness,
    DockSurfaceViewportReadiness, DockSurfaceViewportReadinessReport,
    DockSurfaceViewportReadinessStatus, DockSurfaceViewportRouteStatus,
    DockSurfaceViewportStaleReason, DockSurfaceViewportUnsupportedFlag,
};

use crate::{
    DockController, DockHost, DockSpaceId, DockViewportClosePolicy, DockViewportRuntimeHandle,
    DockVisualStyleResolver,
};
pub(crate) use activation::{
    DockSurfaceActivationBinding, DockSurfaceActivationHostRegistration,
    DockSurfaceActivationHostRegistrationStatus, DockSurfaceActivationSettlements,
    DockSurfaceActivationState,
};
#[cfg(test)]
pub(crate) use activation::{DockSurfaceActivationDispatch, DockSurfaceActivationHostLookup};
use open_gpui::{
    AnyView, AnyWindowHandle, App, AppContext, Bounds, Context, Entity, Pixels,
    Result as GpuiResult, Subscription, WindowBounds, WindowOptions,
};
pub(crate) use owner::{
    DockSurfaceOwner, DockSurfaceTransactionId, with_detached_root_transaction,
    with_root_transaction,
};

/// Application-level owner for one docked workspace and its viewport runtime.
///
/// `DockSurface` is the common app seam for docking. It keeps controller state, host creation, and
/// viewport runtime wiring together so ordinary applications do not need to assemble
/// [`runtime::DockHost`](crate::runtime::DockHost) and
/// [`runtime::DockViewportRuntimeHandle`](crate::runtime::DockViewportRuntimeHandle) directly.
#[derive(Clone, Debug)]
pub struct DockSurface {
    owner: Entity<DockSurfaceOwner>,
    primary_space: DockSpaceId,
}

impl DockSurface {
    /// Starts a facade-first docking surface builder for a logical dock space.
    pub fn builder(space: impl Into<DockSpaceId>) -> DockSurfaceBuilder {
        DockSurfaceBuilder::new(space)
    }

    #[cfg(test)]
    pub(crate) fn from_controller(controller: Entity<DockController>, cx: &mut App) -> Self {
        Self::from_controller_with_close_policy_and_visual_style_resolver(
            controller,
            DockViewportClosePolicy::default(),
            None,
            cx,
        )
    }

    pub(crate) fn from_controller_with_close_policy_and_visual_style_resolver(
        controller: Entity<DockController>,
        close_policy: DockViewportClosePolicy,
        visual_style_resolver: Option<DockVisualStyleResolver>,
        cx: &mut App,
    ) -> Self {
        let primary_space = cx.read_entity(&controller, |controller, _| controller.space().clone());
        let viewport_runtime = match visual_style_resolver {
            Some(resolver) => {
                DockViewportRuntimeHandle::with_close_policy_and_visual_style_resolver(
                    controller.clone(),
                    close_policy,
                    resolver,
                )
            }
            None => DockViewportRuntimeHandle::with_close_policy(controller.clone(), close_policy),
        };
        let owner =
            cx.new(|_| DockSurfaceOwner::new(controller, viewport_runtime, primary_space.clone()));
        let weak_owner = owner.downgrade();
        let runtime = cx.read_entity(&owner, |owner, _| owner.runtime());
        runtime.install_surface_owner(owner.downgrade());
        runtime.install_surface_commit_sink(move |transaction, categories, cx| {
            let Some(owner) = weak_owner.upgrade() else {
                return;
            };
            cx.update_entity(&owner, |owner, owner_cx| {
                if let Some(transaction) = transaction {
                    owner.record_changes(transaction, categories.iter().copied());
                } else {
                    let transaction = owner.begin_root_transaction();
                    owner.record_changes(transaction, categories.iter().copied());
                    owner.finish_root_transaction(transaction, owner_cx);
                }
            });
        });
        let primary_space = cx.read_entity(&owner, |owner, _| owner.primary_space().clone());
        let activation_owner = owner.downgrade();
        cx.on_window_closed(move |cx, window_id| {
            let Some(owner) = activation_owner.upgrade() else {
                return;
            };
            let settlements = cx.update_entity(&owner, |owner, owner_cx| {
                let settlements = owner.activation_mut().window_closed(window_id);
                owner_cx.notify();
                settlements
            });
            settlements.deliver(cx);
        })
        .detach();
        Self {
            owner,
            primary_space,
        }
    }

    pub(crate) fn controller<C: AppContext>(&self, cx: &C) -> Entity<DockController> {
        cx.read_entity(&self.owner, |owner, _| owner.controller())
    }

    pub(crate) fn viewport_runtime<C: AppContext>(&self, cx: &C) -> DockViewportRuntimeHandle {
        cx.read_entity(&self.owner, |owner, _| owner.runtime())
    }

    pub(crate) fn owner(&self) -> &Entity<DockSurfaceOwner> {
        &self.owner
    }

    /// Returns the default logical dock space for primary host windows.
    pub fn primary_space(&self) -> &DockSpaceId {
        &self.primary_space
    }

    pub(crate) fn primary_host(&self, cx: &mut Context<DockHost>) -> DockHost {
        self.host(self.primary_space.clone(), cx)
    }

    pub(crate) fn host(
        &self,
        space: impl Into<DockSpaceId>,
        cx: &mut Context<DockHost>,
    ) -> DockHost {
        let controller = cx.read_entity(&self.owner, |owner, _| owner.controller());
        let viewport_runtime = cx.read_entity(&self.owner, |owner, _| owner.runtime());
        DockHost::from_surface_owner(controller, space, viewport_runtime, &self.owner, cx)
    }

    /// Returns the latest committed persistence revision shared by all surface clones.
    pub fn revision(&self, cx: &App) -> u64 {
        cx.read_entity(&self.owner, |owner, _| owner.revision())
    }

    /// Subscribes to lightweight metadata for committed surface changes.
    ///
    /// Applications own debounce, snapshot export, storage, and file-I/O policy. Dropping the
    /// returned subscription only stops observation.
    pub fn subscribe_changes(
        &self,
        cx: &mut App,
        on_event: impl FnMut(&DockSurfaceChangeEvent, &mut App) + 'static,
    ) -> Subscription {
        owner::subscribe(&self.owner, cx, on_event)
    }

    /// Creates an erased GPUI view that renders the primary dock space inside an existing window.
    pub fn host_view(&self, cx: &mut App) -> AnyView {
        self.host_view_for_space(self.primary_space.clone(), cx)
    }

    /// Creates an erased GPUI view that renders one logical dock space inside an existing window.
    pub fn host_view_for_space(&self, space: impl Into<DockSpaceId>, cx: &mut App) -> AnyView {
        let surface = self.clone();
        let space = space.into();
        cx.new(move |cx| surface.host(space, cx)).into()
    }

    /// Opens a normal GPUI window that renders the primary dock host.
    ///
    /// This is for the main application window and does not require platform viewport-window
    /// capability. Detached platform viewports are opened through the viewport-runtime path.
    pub fn open_primary_window(
        &self,
        options: WindowOptions,
        cx: &mut App,
    ) -> GpuiResult<AnyWindowHandle> {
        let surface = self.clone();
        cx.open_window(options, move |_, cx| {
            cx.new(move |cx| surface.primary_host(cx))
        })
        .map(Into::into)
    }

    /// Returns default window options for a centered primary dock host.
    pub fn primary_window_options(bounds: Bounds<Pixels>) -> WindowOptions {
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        }
    }
}
