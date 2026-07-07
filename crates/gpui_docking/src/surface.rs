use crate::{
    DockActionApplyError, DockActionOutcome, DockClassId, DockController, DockControllerBuilder,
    DockHost, DockHostOptions, DockItemId, DockLayout, DockLayoutValidationError, DockPanel,
    DockPanelCloseOutcome, DockPanelDescriptor, DockPanelOpenOutcome, DockPanelPlacement,
    DockPolicy, DockPolicyError, DockSpaceId, DockViewportClosePolicy, DockViewportOpenOutcome,
    DockViewportOpenStatus, DockViewportRuntimeHandle, EditorDockLayoutSpec,
};
use open_gpui::{
    AnyView, AnyWindowHandle, App, AppContext as _, Bounds, Context, Entity, Pixels,
    Result as GpuiResult, WindowBounds, WindowHandle, WindowOptions,
};
use thiserror::Error;

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

/// Builder for [`DockSurface`].
#[derive(Debug)]
pub struct DockSurfaceBuilder {
    controller: DockControllerBuilder,
    close_policy: DockViewportClosePolicy,
}

/// Error returned when a facade docking surface cannot be built.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DockSurfaceBuildError {
    /// The configured layout graph did not validate.
    #[error("dock surface layout is invalid: {message}")]
    InvalidLayout {
        /// Validation message from the low-level model validator.
        message: String,
    },
}

/// Facade-level outcome of a platform viewport open request.
#[derive(Debug)]
pub enum DockSurfaceViewportOpenOutcome {
    /// The runtime opened, reused, or replaced a platform viewport window.
    Opened(DockSurfaceViewportOpened),
    /// The surface rejected the request before opening a window.
    Unavailable(DockSurfaceViewportUnavailable),
}

/// Facade-level result for a successfully opened or reused platform viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockSurfaceViewportOpened {
    space: DockSpaceId,
    window: AnyWindowHandle,
    status: DockSurfaceViewportOpenStatus,
}

/// Facade-level status for a platform viewport open request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockSurfaceViewportOpenStatus {
    /// A new GPUI window was opened and registered.
    Opened,
    /// An existing live GPUI window was reused.
    Reused,
    /// A stale or superseded mapping was replaced by a new window.
    Replaced,
}

/// Facade-level outcome of a product panel operation.
#[derive(Debug, Clone, PartialEq)]
pub enum DockSurfacePanelOutcome {
    /// A panel was opened or reopened into graph state.
    Opened(DockPanelOpenOutcome),
    /// A panel was closed while its registration stayed available.
    Closed(DockPanelCloseOutcome),
    /// A panel was moved into an in-window floating container.
    Floated(DockSurfaceChange),
    /// A floating panel was moved back into the dock layout.
    Docked(DockSurfaceChange),
}

/// App-level change flag for facade panel operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockSurfaceChange {
    /// The operation changed docking state.
    Changed,
    /// The operation was valid but left docking state unchanged.
    Unchanged,
}

impl DockSurfacePanelOutcome {
    /// Returns true when the operation changed docking graph state.
    pub fn changed(&self) -> bool {
        match self {
            Self::Opened(outcome) => outcome.changed(),
            Self::Closed(outcome) => outcome.changed(),
            Self::Floated(change) | Self::Docked(change) => change.changed(),
        }
    }
}

impl DockSurfaceChange {
    /// Returns true when the operation changed docking state.
    pub fn changed(self) -> bool {
        matches!(self, Self::Changed)
    }
}

impl From<DockActionOutcome> for DockSurfaceChange {
    fn from(outcome: DockActionOutcome) -> Self {
        if outcome.changed() {
            Self::Changed
        } else {
            Self::Unchanged
        }
    }
}

impl From<DockViewportOpenStatus> for DockSurfaceViewportOpenStatus {
    fn from(status: DockViewportOpenStatus) -> Self {
        match status {
            DockViewportOpenStatus::Opened => Self::Opened,
            DockViewportOpenStatus::Reused => Self::Reused,
            DockViewportOpenStatus::Replaced => Self::Replaced,
        }
    }
}

impl From<DockViewportOpenOutcome> for DockSurfaceViewportOpened {
    fn from(outcome: DockViewportOpenOutcome) -> Self {
        Self {
            space: outcome.space().clone(),
            window: outcome.window(),
            status: outcome.status().into(),
        }
    }
}

impl DockSurfaceViewportOpened {
    /// Logical dock space rendered by the viewport window.
    pub fn space(&self) -> &DockSpaceId {
        &self.space
    }

    /// GPUI window that renders the logical dock space.
    pub fn window(&self) -> AnyWindowHandle {
        self.window
    }

    /// Whether the surface opened, reused, or replaced a viewport window.
    pub fn status(&self) -> DockSurfaceViewportOpenStatus {
        self.status
    }
}

/// Error returned when a facade panel operation cannot be applied.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DockSurfacePanelError {
    /// The panel has no registered metadata.
    #[error("dock item {item} has no registered panel")]
    PanelNotRegistered {
        /// The item that was requested.
        item: DockItemId,
    },
    /// The panel is registered but not closable.
    #[error("dock item {item} is not closable")]
    PanelNotClosable {
        /// The item that was requested.
        item: DockItemId,
    },
    /// The requested panel could not be found at the target location.
    #[error("dock item {item} is not available in the requested dock location")]
    PanelUnavailable {
        /// The item that was requested.
        item: DockItemId,
    },
    /// The requested dock target is unavailable.
    #[error("dock target is not currently available")]
    DropTargetUnavailable,
    /// The operation was rejected by docking policy.
    #[error(transparent)]
    Policy(#[from] DockPolicyError),
    /// The low-level docking model rejected the operation.
    #[error("dock model rejected the operation: {message}")]
    Model {
        /// Error message from the low-level model.
        message: String,
    },
    /// The runtime could not complete the operation.
    #[error("dock runtime could not complete the operation: {message}")]
    Runtime {
        /// Runtime failure message.
        message: String,
    },
}

impl From<DockActionApplyError> for DockSurfacePanelError {
    fn from(error: DockActionApplyError) -> Self {
        match error {
            DockActionApplyError::ItemNotInTabs { item, .. } => Self::PanelUnavailable { item },
            DockActionApplyError::PanelNotRegistered { item } => Self::PanelNotRegistered { item },
            DockActionApplyError::PanelNotClosable { item } => Self::PanelNotClosable { item },
            DockActionApplyError::Graph(error) => Self::Model {
                message: error.to_string(),
            },
            DockActionApplyError::Policy(error) => Self::Policy(error),
            DockActionApplyError::DropTargetUnavailable => Self::DropTargetUnavailable,
            DockActionApplyError::TearOffViewportOpenFailed { message } => {
                Self::Runtime { message }
            }
            DockActionApplyError::DropDragSessionStale { .. }
            | DockActionApplyError::DropDragSessionMissing
            | DockActionApplyError::TearOffViewportPlacementUnavailable
            | DockActionApplyError::DropPayloadMismatch { .. } => Self::Runtime {
                message: error.to_string(),
            },
        }
    }
}

/// Typed reason a facade-level platform viewport request did not open a window.
#[derive(Debug)]
pub enum DockSurfaceViewportUnavailable {
    /// The app's docking policy does not allow platform viewport windows.
    PolicyDisabled(crate::DockPolicyError),
    /// The active backend does not support independent platform viewport windows.
    BackendUnsupported,
    /// The request reached GPUI window opening but the backend returned an error.
    OpenFailed(String),
}

impl DockSurfaceViewportOpenOutcome {
    /// Returns true when this request opened or reused a viewport window.
    pub fn opened(&self) -> bool {
        matches!(self, Self::Opened(_))
    }

    /// Returns the successful runtime outcome when present.
    pub fn open_outcome(&self) -> Option<&DockSurfaceViewportOpened> {
        match self {
            Self::Opened(outcome) => Some(outcome),
            Self::Unavailable(_) => None,
        }
    }

    /// Returns the unavailable reason when the request failed before producing a viewport.
    pub fn unavailable(&self) -> Option<&DockSurfaceViewportUnavailable> {
        match self {
            Self::Opened(_) => None,
            Self::Unavailable(reason) => Some(reason),
        }
    }
}

impl DockSurfaceViewportUnavailable {
    /// Returns true when the active platform backend does not expose platform viewport windows.
    pub fn is_backend_unsupported(&self) -> bool {
        matches!(self, Self::BackendUnsupported)
    }

    /// Returns true when application policy rejected the viewport request.
    pub fn is_policy_disabled(&self) -> bool {
        matches!(self, Self::PolicyDisabled(_))
    }
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

    /// Opens or reuses a controller-backed platform viewport window for one dock space.
    ///
    /// The facade reports policy and backend capability failures before delegating to the runtime,
    /// so unsupported platforms do not create windows or runtime registrations.
    pub fn open_viewport(
        &self,
        space: impl Into<DockSpaceId>,
        options: WindowOptions,
        cx: &mut App,
    ) -> DockSurfaceViewportOpenOutcome {
        let policy_result = cx.read_entity(&self.controller, |controller, _| {
            controller.policy().validate_platform_viewports()
        });
        if let Err(error) = policy_result {
            return DockSurfaceViewportOpenOutcome::Unavailable(
                DockSurfaceViewportUnavailable::PolicyDisabled(error),
            );
        }

        if !cx.viewport_capabilities().platform_viewport_windows {
            return DockSurfaceViewportOpenOutcome::Unavailable(
                DockSurfaceViewportUnavailable::BackendUnsupported,
            );
        }

        match self.viewport_runtime.open_viewport(space, options, cx) {
            Ok(outcome) => DockSurfaceViewportOpenOutcome::Opened(outcome.into()),
            Err(error) => DockSurfaceViewportOpenOutcome::Unavailable(
                DockSurfaceViewportUnavailable::OpenFailed(error.to_string()),
            ),
        }
    }

    /// Opens a registered panel using descriptor last-known or default placement.
    pub fn open_panel(
        &self,
        item: impl Into<DockItemId>,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        let space = self.primary_space.clone();
        self.open_panel_in_space(space, item, cx)
    }

    /// Opens a registered panel in one dock space using descriptor placement metadata.
    pub fn open_panel_in_space(
        &self,
        space: impl Into<DockSpaceId>,
        item: impl Into<DockItemId>,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        self.update_controller(cx, |controller| {
            controller
                .reopen_panel(space, item)
                .map(DockSurfacePanelOutcome::Opened)
        })
    }

    /// Opens a registered panel at an explicit product placement in the primary dock space.
    pub fn open_panel_at(
        &self,
        placement: DockPanelPlacement,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        let space = self.primary_space.clone();
        self.open_panel_at_in_space(space, placement, cx)
    }

    /// Opens a registered panel at an explicit product placement in one dock space.
    pub fn open_panel_at_in_space(
        &self,
        space: impl Into<DockSpaceId>,
        placement: DockPanelPlacement,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        self.update_controller(cx, |controller| {
            controller
                .open_panel_at_placement(space, placement)
                .map(DockSurfacePanelOutcome::Opened)
        })
    }

    /// Docks a floating panel back into the primary dock space at an explicit product placement.
    pub fn dock_panel_at(
        &self,
        placement: DockPanelPlacement,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        let space = self.primary_space.clone();
        self.dock_panel_at_in_space(space, placement, cx)
    }

    /// Docks a floating panel back into one dock space at an explicit product placement.
    pub fn dock_panel_at_in_space(
        &self,
        space: impl Into<DockSpaceId>,
        placement: DockPanelPlacement,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        let space = space.into();
        self.update_controller(cx, |controller| {
            let target_tabs = controller
                .graph()
                .target_tabs_for_panel_placement(&space, &placement)
                .ok_or(DockActionApplyError::DropTargetUnavailable)?;
            let floating = controller
                .graph()
                .floating_containers(&space)
                .iter()
                .find(|container| {
                    controller
                        .graph()
                        .collect_items_in_subtree(container.node)
                        .iter()
                        .any(|item| item == placement.item())
                })
                .map(|container| container.node)
                .ok_or_else(|| DockActionApplyError::PanelNotRegistered {
                    item: placement.item().clone(),
                })?;

            controller
                .merge_floating_into(space, floating, target_tabs)
                .map(DockSurfaceChange::from)
                .map(DockSurfacePanelOutcome::Docked)
        })
    }

    /// Closes a registered panel in the primary dock space.
    pub fn close_panel(
        &self,
        item: impl Into<DockItemId>,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        let space = self.primary_space.clone();
        self.close_panel_in_space(space, item, cx)
    }

    /// Closes a registered panel in one dock space.
    pub fn close_panel_in_space(
        &self,
        space: impl Into<DockSpaceId>,
        item: impl Into<DockItemId>,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        self.update_controller(cx, |controller| {
            controller
                .close_panel(space, item)
                .map(DockSurfacePanelOutcome::Closed)
        })
    }

    /// Moves a panel from the primary dock space into an in-window floating container.
    pub fn float_panel_in_window(
        &self,
        item: impl Into<DockItemId>,
        bounds: Bounds<Pixels>,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        let space = self.primary_space.clone();
        self.float_panel_between_spaces(space.clone(), item, space, bounds, cx)
    }

    /// Moves a panel into an in-window floating container, optionally across dock spaces.
    pub fn float_panel_between_spaces(
        &self,
        source_space: impl Into<DockSpaceId>,
        item: impl Into<DockItemId>,
        target_space: impl Into<DockSpaceId>,
        bounds: Bounds<Pixels>,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        self.update_controller(cx, |controller| {
            controller
                .float_item_in_window(source_space, item, target_space, bounds)
                .map(DockSurfaceChange::from)
                .map(DockSurfacePanelOutcome::Floated)
        })
    }

    fn update_controller<T>(
        &self,
        cx: &mut App,
        update: impl FnOnce(&mut DockController) -> Result<T, DockActionApplyError>,
    ) -> Result<T, DockSurfacePanelError>
    where
        T: SurfaceChanged,
    {
        let controller = self.controller.clone();
        cx.update_entity(&controller, |controller, cx| {
            let outcome = update(controller);
            if outcome
                .as_ref()
                .map(SurfaceChanged::changed)
                .unwrap_or(false)
            {
                cx.notify();
            }
            outcome
        })
        .map_err(DockSurfacePanelError::from)
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

trait SurfaceChanged {
    fn changed(&self) -> bool;
}

impl SurfaceChanged for DockSurfacePanelOutcome {
    fn changed(&self) -> bool {
        DockSurfacePanelOutcome::changed(self)
    }
}

impl DockSurfaceBuilder {
    /// Creates a builder from the existing controller builder.
    pub fn new(space: impl Into<DockSpaceId>) -> Self {
        Self {
            controller: DockController::builder(space),
            close_policy: DockViewportClosePolicy::default(),
        }
    }

    /// Restores the durable layout graph from serialized dock layout data.
    pub fn try_layout(
        mut self,
        layout: &DockLayout,
    ) -> std::result::Result<Self, DockLayoutValidationError> {
        self.controller = self.controller.try_layout(layout)?;
        Ok(self)
    }

    /// Replaces the durable layout graph with the common editor-style layout.
    pub fn default_editor_layout(mut self, spec: EditorDockLayoutSpec) -> Self {
        self.controller = self.controller.default_editor_layout(spec);
        self
    }

    /// Replaces the durable layout graph with product-level panel placements.
    pub fn panel_placements(
        mut self,
        placements: impl IntoIterator<Item = DockPanelPlacement>,
    ) -> Self {
        self.controller = self.controller.panel_placements(placements);
        self
    }

    /// Registers descriptor-only panel metadata.
    pub fn panel_descriptor(
        mut self,
        item: impl Into<DockItemId>,
        descriptor: DockPanelDescriptor,
    ) -> Self {
        self.controller = self.controller.panel_descriptor(item, descriptor);
        self
    }

    /// Registers a prepared panel.
    pub fn panel(mut self, item: impl Into<DockItemId>, panel: DockPanel) -> Self {
        self.controller = self.controller.panel(item, panel);
        self
    }

    /// Registers a lazy GPUI view factory as panel content.
    pub fn panel_factory(
        mut self,
        item: impl Into<DockItemId>,
        title: impl Into<String>,
        factory: impl Fn(&mut App) -> AnyView + 'static,
    ) -> Self {
        self.controller = self.controller.panel_factory(item, title, factory);
        self
    }

    /// Replaces static host rendering options.
    pub fn options(mut self, options: DockHostOptions) -> Self {
        self.controller = self.controller.options(options);
        self
    }

    /// Replaces the docking interaction policy.
    pub fn policy(mut self, policy: DockPolicy) -> Self {
        self.controller = self.controller.policy(policy);
        self
    }

    /// Enables or disables in-window floating interactions.
    pub fn allow_floating(mut self, allowed: bool) -> Self {
        self.controller = self.controller.allow_floating(allowed);
        self
    }

    /// Enables or disables platform viewport interactions.
    pub fn allow_platform_viewports(mut self, allowed: bool) -> Self {
        self.controller = self.controller.allow_platform_viewports(allowed);
        self
    }

    /// Enables or disables restoring dock-panel focus when a platform window gains focus.
    pub fn platform_focus_sets_dock_focus(mut self, enabled: bool) -> Self {
        self.controller = self.controller.platform_focus_sets_dock_focus(enabled);
        self
    }

    /// Allows one dock class to be dropped into the given dock space.
    pub fn allow_dock_class_in_space(
        mut self,
        space: impl Into<DockSpaceId>,
        dock_class: impl Into<DockClassId>,
    ) -> Self {
        self.controller = self.controller.allow_dock_class_in_space(space, dock_class);
        self
    }

    /// Replaces the close policy for runtime-opened viewport windows.
    pub fn close_policy(mut self, close_policy: DockViewportClosePolicy) -> Self {
        self.close_policy = close_policy;
        self
    }

    /// Builds the surface after validating controller graph state.
    pub fn build(self, cx: &mut App) -> Result<DockSurface, DockSurfaceBuildError> {
        let controller =
            self.controller
                .try_build()
                .map_err(|error| DockSurfaceBuildError::InvalidLayout {
                    message: error.to_string(),
                })?;
        let controller = cx.new(|_| controller);
        Ok(DockSurface::from_controller_with_close_policy(
            controller,
            self.close_policy,
            cx,
        ))
    }
}
