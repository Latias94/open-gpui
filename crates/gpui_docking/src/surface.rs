use crate::{
    DockActionApplyError, DockActionOutcome, DockClassId, DockController, DockControllerBuilder,
    DockHost, DockHostOptions, DockItemId, DockLayout, DockLayoutValidationError, DockPanel,
    DockPanelCloseOutcome, DockPanelDescriptor, DockPanelOpenOutcome, DockPanelPlacement,
    DockPolicy, DockPolicyError, DockSpaceId, DockViewportCloseOutcome, DockViewportClosePolicy,
    DockViewportCloseStatus, DockViewportOpenOutcome, DockViewportOpenStatus,
    DockViewportPlacementLayout, DockViewportPlacementValidationError,
    DockViewportRestoreReadiness, DockViewportRuntimeHandle, DockViewportShouldCloseOutcome,
    DockViewportShouldCloseStatus, EditorDockLayoutSpec,
};
use open_gpui::{
    AnyView, AnyWindowHandle, App, AppContext as _, Bounds, Context, Entity, Pixels,
    Result as GpuiResult, WindowBounds, WindowHandle, WindowId, WindowOptions,
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

/// Facade-level outcome of a platform window should-close query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockSurfaceViewportShouldCloseOutcome {
    space: Option<DockSpaceId>,
    window_id: WindowId,
    status: DockSurfaceViewportShouldCloseStatus,
}

/// Facade-level status for a platform window should-close query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockSurfaceViewportShouldCloseStatus {
    /// The surface allows GPUI to continue closing the platform window.
    Allowed,
    /// The surface vetoed the platform close before the window closed.
    Vetoed,
    /// The window id is not registered with this surface's viewport runtime.
    UnknownWindow,
}

/// Facade-level outcome of a platform viewport close notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockSurfaceViewportCloseOutcome {
    space: Option<DockSpaceId>,
    window_id: WindowId,
    status: DockSurfaceViewportCloseStatus,
    merge_target_space: Option<DockSpaceId>,
}

/// Facade-level status for a platform viewport close notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockSurfaceViewportCloseStatus {
    /// The runtime window mapping was removed while logical layout remained available.
    Closed,
    /// The runtime window mapping was removed and dock contents moved to fallback space.
    MergedBack,
    /// The runtime window mapping was removed, but merge-back could not commit.
    MergeBackFailed,
    /// The window id is not registered with this surface's viewport runtime.
    UnknownWindow,
}

/// Facade-level request for opening one logical dock space in a platform viewport window.
///
/// This keeps ordinary applications on the `DockSurface` API surface: callers provide a logical
/// space plus GPUI window options, and can apply serialized placement data without importing the
/// lower-level runtime tier.
#[derive(Debug)]
pub struct DockSurfaceViewportSpec {
    space: DockSpaceId,
    options: WindowOptions,
}

/// Error returned while preparing a facade viewport spec before opening a window.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DockSurfaceViewportSpecError {
    /// Serialized viewport placement data failed validation.
    #[error("saved dock viewport placement is invalid: {message}")]
    InvalidPlacement {
        /// Validation message from the placement validator.
        message: String,
    },
}

/// Facade-level report for opening a batch of platform viewport windows.
#[derive(Debug)]
pub struct DockSurfaceViewportOpenReport {
    outcomes: Vec<DockSurfaceViewportOpenOutcome>,
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

impl From<DockViewportShouldCloseStatus> for DockSurfaceViewportShouldCloseStatus {
    fn from(status: DockViewportShouldCloseStatus) -> Self {
        match status {
            DockViewportShouldCloseStatus::Allowed => Self::Allowed,
            DockViewportShouldCloseStatus::Vetoed => Self::Vetoed,
            DockViewportShouldCloseStatus::UnknownWindow => Self::UnknownWindow,
        }
    }
}

impl From<DockViewportShouldCloseOutcome> for DockSurfaceViewportShouldCloseOutcome {
    fn from(outcome: DockViewportShouldCloseOutcome) -> Self {
        Self {
            space: outcome.space,
            window_id: outcome.window_id,
            status: outcome.status.into(),
        }
    }
}

impl From<DockViewportCloseStatus> for DockSurfaceViewportCloseStatus {
    fn from(status: DockViewportCloseStatus) -> Self {
        match status {
            DockViewportCloseStatus::Closed => Self::Closed,
            DockViewportCloseStatus::MergedBack => Self::MergedBack,
            DockViewportCloseStatus::MergeBackFailed => Self::MergeBackFailed,
            DockViewportCloseStatus::UnknownWindow => Self::UnknownWindow,
        }
    }
}

impl From<DockViewportCloseOutcome> for DockSurfaceViewportCloseOutcome {
    fn from(outcome: DockViewportCloseOutcome) -> Self {
        Self {
            space: outcome.space().cloned(),
            window_id: outcome.window_id(),
            status: outcome.status().into(),
            merge_target_space: outcome.merge_target_space().cloned(),
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

impl DockSurfaceViewportShouldCloseOutcome {
    /// Logical dock space associated with the queried window, when known.
    pub fn space(&self) -> Option<&DockSpaceId> {
        self.space.as_ref()
    }

    /// GPUI window id received from the should-close callback.
    pub fn window_id(&self) -> WindowId {
        self.window_id
    }

    /// How the should-close query resolved.
    pub fn status(&self) -> DockSurfaceViewportShouldCloseStatus {
        self.status
    }

    /// Returns true when GPUI should continue closing the platform window.
    pub fn allows_close(&self) -> bool {
        !matches!(self.status, DockSurfaceViewportShouldCloseStatus::Vetoed)
    }
}

impl DockSurfaceViewportCloseOutcome {
    /// Logical dock space that was associated with the closed window, when known.
    pub fn space(&self) -> Option<&DockSpaceId> {
        self.space.as_ref()
    }

    /// GPUI window id received from the close callback.
    pub fn window_id(&self) -> WindowId {
        self.window_id
    }

    /// How the close notification resolved.
    pub fn status(&self) -> DockSurfaceViewportCloseStatus {
        self.status
    }

    /// Fallback space that received the closed viewport contents, when merge-back committed.
    pub fn merge_target_space(&self) -> Option<&DockSpaceId> {
        self.merge_target_space.as_ref()
    }
}

impl DockSurfaceViewportSpec {
    /// Creates a viewport-open request for one logical dock space.
    pub fn new(space: impl Into<DockSpaceId>, options: WindowOptions) -> Self {
        Self {
            space: space.into(),
            options,
        }
    }

    /// Logical dock space that should be rendered by the opened viewport window.
    pub fn space(&self) -> &DockSpaceId {
        &self.space
    }

    /// GPUI window options that will be used when the viewport window is opened.
    pub fn window_options(&self) -> &WindowOptions {
        &self.options
    }

    /// Mutable access for app code that wants to fill less common GPUI window fields.
    pub fn window_options_mut(&mut self) -> &mut WindowOptions {
        &mut self.options
    }

    /// Applies saved platform-window placement to the spec's fallback GPUI window options.
    pub fn with_saved_placement(
        mut self,
        placement: &DockViewportPlacementLayout,
    ) -> Result<Self, DockSurfaceViewportSpecError> {
        self.options = placement
            .window_options_for_space(&self.space, self.options)
            .map_err(|error| DockSurfaceViewportSpecError::InvalidPlacement {
                message: error.to_string(),
            })?;
        Ok(self)
    }

    /// Consumes the spec into its logical space and GPUI window options.
    pub fn into_parts(self) -> (DockSpaceId, WindowOptions) {
        (self.space, self.options)
    }
}

impl DockSurfaceViewportOpenReport {
    fn new(outcomes: Vec<DockSurfaceViewportOpenOutcome>) -> Self {
        Self { outcomes }
    }

    /// Outcomes in the same order as the requested viewport specs.
    pub fn outcomes(&self) -> &[DockSurfaceViewportOpenOutcome] {
        &self.outcomes
    }

    /// Consumes the report into the ordered outcomes.
    pub fn into_outcomes(self) -> Vec<DockSurfaceViewportOpenOutcome> {
        self.outcomes
    }

    /// Number of viewport requests in the report.
    pub fn len(&self) -> usize {
        self.outcomes.len()
    }

    /// Returns true when no viewport requests were submitted.
    pub fn is_empty(&self) -> bool {
        self.outcomes.is_empty()
    }

    /// Number of requests that opened, reused, or replaced a platform viewport.
    pub fn opened_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|outcome| outcome.opened())
            .count()
    }

    /// Number of requests that could not open a platform viewport.
    pub fn unavailable_count(&self) -> usize {
        self.len() - self.opened_count()
    }

    /// Returns true when every requested viewport opened, reused, or replaced a window.
    pub fn all_opened(&self) -> bool {
        self.outcomes
            .iter()
            .all(DockSurfaceViewportOpenOutcome::opened)
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

    /// Replaces the close policy used by facade-opened platform viewport windows.
    pub fn set_viewport_close_policy(&self, close_policy: DockViewportClosePolicy) {
        self.viewport_runtime.set_close_policy(close_policy);
    }

    /// Returns registered platform viewport spaces in stable lexical order.
    pub fn registered_viewport_spaces(&self) -> Vec<DockSpaceId> {
        self.viewport_runtime.registered_viewport_spaces()
    }

    /// Returns true when a facade-opened platform viewport is registered for the dock space.
    pub fn is_viewport_open(&self, space: &DockSpaceId) -> bool {
        self.viewport_runtime.is_viewport_open(space)
    }

    /// Exports serializable platform-window placement snapshots for facade-opened viewports.
    pub fn export_viewport_placement(&self) -> DockViewportPlacementLayout {
        self.viewport_runtime.export_placement()
    }

    /// Checks saved placement snapshots against currently registered facade viewport windows.
    ///
    /// This does not open, move, or resize platform windows. Use
    /// [`DockSurfaceViewportSpec::with_saved_placement`] before opening viewports with saved
    /// placement hints.
    pub fn check_viewport_placement_restore(
        &self,
        placement: &DockViewportPlacementLayout,
    ) -> Result<DockViewportRestoreReadiness, DockViewportPlacementValidationError> {
        self.viewport_runtime.check_placement_restore(placement)
    }

    /// Handles a GPUI window should-close callback for a facade-opened viewport window.
    pub fn handle_viewport_window_should_close(
        &self,
        window_id: WindowId,
        cx: &mut App,
    ) -> DockSurfaceViewportShouldCloseOutcome {
        self.viewport_runtime
            .handle_window_should_close_with_app(window_id, cx)
            .into()
    }

    /// Handles a GPUI window closed callback for a facade-opened viewport window.
    pub fn handle_viewport_window_closed(
        &self,
        window_id: WindowId,
        cx: &mut App,
    ) -> DockSurfaceViewportCloseOutcome {
        self.viewport_runtime
            .handle_window_closed_with_app(window_id, cx)
            .into()
    }

    /// Cancels a previously accepted platform close request when the platform kept the window open.
    pub fn cancel_viewport_window_close(
        &self,
        window_id: WindowId,
        cx: &mut App,
    ) -> DockSurfaceChange {
        if self
            .viewport_runtime
            .cancel_window_close_request_with_app(window_id, cx)
        {
            DockSurfaceChange::Changed
        } else {
            DockSurfaceChange::Unchanged
        }
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
        self.open_viewport_spec(DockSurfaceViewportSpec::new(space, options), cx)
    }

    /// Opens or reuses a controller-backed platform viewport from a facade request.
    pub fn open_viewport_spec(
        &self,
        spec: DockSurfaceViewportSpec,
        cx: &mut App,
    ) -> DockSurfaceViewportOpenOutcome {
        let (space, options) = spec.into_parts();
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

        match self
            .viewport_runtime
            .open_viewport_unchecked_policy(space, options, cx)
        {
            Ok(outcome) => DockSurfaceViewportOpenOutcome::Opened(outcome.into()),
            Err(error) => DockSurfaceViewportOpenOutcome::Unavailable(
                DockSurfaceViewportUnavailable::OpenFailed(error.to_string()),
            ),
        }
    }

    /// Opens a batch of facade viewport requests and returns ordered outcomes.
    pub fn open_viewports(
        &self,
        specs: impl IntoIterator<Item = DockSurfaceViewportSpec>,
        cx: &mut App,
    ) -> DockSurfaceViewportOpenReport {
        DockSurfaceViewportOpenReport::new(
            specs
                .into_iter()
                .map(|spec| self.open_viewport_spec(spec, cx))
                .collect(),
        )
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
