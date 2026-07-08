use super::{DockSurface, DockSurfaceChange};
use crate::{
    DockPolicyError, DockSpaceId, DockViewportCloseOutcome, DockViewportClosePolicy,
    DockViewportCloseStatus, DockViewportOpenOutcome, DockViewportOpenStatus,
    DockViewportPlacementLayout, DockViewportPlacementValidationError,
    DockViewportRestoreReadiness, DockViewportShouldCloseOutcome, DockViewportShouldCloseStatus,
};
use open_gpui::{AnyWindowHandle, App, AppContext as _, WindowId, WindowOptions};
use thiserror::Error;

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
    #[error("saved dock viewport placement is invalid: {error}")]
    InvalidPlacement {
        /// Structured validation error from the placement validator.
        error: DockViewportPlacementValidationError,
    },
}

/// Facade-level report for opening a batch of platform viewport windows.
#[derive(Debug)]
pub struct DockSurfaceViewportOpenReport {
    outcomes: Vec<DockSurfaceViewportOpenOutcome>,
}

/// Facade-level report for restoring platform viewport windows from saved placement data.
#[derive(Debug)]
pub struct DockSurfaceViewportRestoreReport {
    outcomes: Vec<DockSurfaceViewportRestoreOutcome>,
}

/// Facade-level outcome keyed by the logical dock space requested by saved placement data.
#[derive(Debug)]
pub struct DockSurfaceViewportRestoreOutcome {
    space: DockSpaceId,
    outcome: DockSurfaceViewportOpenOutcome,
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

/// Typed reason a facade-level platform viewport request did not open a window.
#[derive(Debug)]
pub enum DockSurfaceViewportUnavailable {
    /// The app's docking policy does not allow platform viewport windows.
    PolicyDisabled(DockPolicyError),
    /// The active backend does not support independent platform viewport windows.
    BackendUnsupported,
    /// Serialized viewport placement data failed validation before a window was opened.
    InvalidPlacement {
        /// Structured validation error from the placement validator.
        error: DockViewportPlacementValidationError,
    },
    /// The request reached GPUI window opening but the backend returned an error.
    OpenFailed(String),
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
            .map_err(|error| DockSurfaceViewportSpecError::InvalidPlacement { error })?;
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

impl DockSurfaceViewportRestoreReport {
    fn new(outcomes: Vec<DockSurfaceViewportRestoreOutcome>) -> Self {
        Self { outcomes }
    }

    /// Outcomes keyed by saved logical dock space in placement order.
    pub fn outcomes(&self) -> &[DockSurfaceViewportRestoreOutcome] {
        &self.outcomes
    }

    /// Returns the first outcome for a logical dock space, when present.
    pub fn outcome_for_space(
        &self,
        space: &DockSpaceId,
    ) -> Option<&DockSurfaceViewportOpenOutcome> {
        self.outcomes
            .iter()
            .find(|outcome| outcome.space() == space)
            .map(DockSurfaceViewportRestoreOutcome::outcome)
    }

    /// Consumes the report into keyed outcomes.
    pub fn into_outcomes(self) -> Vec<DockSurfaceViewportRestoreOutcome> {
        self.outcomes
    }

    /// Number of saved viewport requests in the report.
    pub fn len(&self) -> usize {
        self.outcomes.len()
    }

    /// Returns true when the placement did not request any viewport restores.
    pub fn is_empty(&self) -> bool {
        self.outcomes.is_empty()
    }

    /// Number of saved viewport requests that opened, reused, or replaced a platform viewport.
    pub fn opened_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|outcome| outcome.outcome().opened())
            .count()
    }

    /// Number of saved viewport requests that could not open a platform viewport.
    pub fn unavailable_count(&self) -> usize {
        self.len() - self.opened_count()
    }

    /// Returns true when every saved viewport opened, reused, or replaced a window.
    pub fn all_opened(&self) -> bool {
        self.outcomes
            .iter()
            .all(|outcome| outcome.outcome().opened())
    }
}

impl DockSurfaceViewportRestoreOutcome {
    fn new(space: DockSpaceId, outcome: DockSurfaceViewportOpenOutcome) -> Self {
        Self { space, outcome }
    }

    /// Logical dock space requested by saved placement data.
    pub fn space(&self) -> &DockSpaceId {
        &self.space
    }

    /// Outcome for this logical dock space.
    pub fn outcome(&self) -> &DockSurfaceViewportOpenOutcome {
        &self.outcome
    }

    /// Consumes this keyed outcome into its space and open outcome.
    pub fn into_parts(self) -> (DockSpaceId, DockSurfaceViewportOpenOutcome) {
        (self.space, self.outcome)
    }
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

    /// Returns true when serialized placement data failed validation.
    pub fn is_invalid_placement(&self) -> bool {
        matches!(self, Self::InvalidPlacement { .. })
    }

    /// Returns structured placement validation details when serialized placement data was invalid.
    pub fn placement_validation_error(&self) -> Option<&DockViewportPlacementValidationError> {
        match self {
            Self::InvalidPlacement { error } => Some(error),
            _ => None,
        }
    }
}

impl DockSurface {
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

        self.open_viewport_spec_without_preflight(spec, cx)
    }

    fn open_viewport_spec_without_preflight(
        &self,
        spec: DockSurfaceViewportSpec,
        cx: &mut App,
    ) -> DockSurfaceViewportOpenOutcome {
        let (space, options) = spec.into_parts();
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
        let specs = specs.into_iter();
        let policy_result = cx.read_entity(&self.controller, |controller, _| {
            controller.policy().validate_platform_viewports()
        });
        if let Err(error) = policy_result {
            return DockSurfaceViewportOpenReport::new(
                specs
                    .map(|_| {
                        DockSurfaceViewportOpenOutcome::Unavailable(
                            DockSurfaceViewportUnavailable::PolicyDisabled(error.clone()),
                        )
                    })
                    .collect(),
            );
        }

        if !cx.viewport_capabilities().platform_viewport_windows {
            return DockSurfaceViewportOpenReport::new(
                specs
                    .map(|_| {
                        DockSurfaceViewportOpenOutcome::Unavailable(
                            DockSurfaceViewportUnavailable::BackendUnsupported,
                        )
                    })
                    .collect(),
            );
        }

        DockSurfaceViewportOpenReport::new(
            specs
                .map(|spec| self.open_viewport_spec_without_preflight(spec, cx))
                .collect(),
        )
    }

    /// Opens saved platform viewport windows from placement data and keys outcomes by dock space.
    ///
    /// `fallback_options` is called once per saved dock space. Saved placement hints are layered on
    /// top of those fallback options before the viewport is opened.
    pub fn open_viewports_from_saved_placement(
        &self,
        placement: &DockViewportPlacementLayout,
        mut fallback_options: impl FnMut(&DockSpaceId) -> WindowOptions,
        cx: &mut App,
    ) -> DockSurfaceViewportRestoreReport {
        if let Err(error) = placement.validate() {
            return DockSurfaceViewportRestoreReport::new(
                placement
                    .viewports
                    .iter()
                    .map(|viewport| {
                        DockSurfaceViewportRestoreOutcome::new(
                            viewport.space.clone(),
                            DockSurfaceViewportOpenOutcome::Unavailable(
                                DockSurfaceViewportUnavailable::InvalidPlacement {
                                    error: error.clone(),
                                },
                            ),
                        )
                    })
                    .collect(),
            );
        }

        DockSurfaceViewportRestoreReport::new(
            placement
                .viewports
                .iter()
                .map(|viewport| {
                    let space = viewport.space.clone();
                    let spec =
                        DockSurfaceViewportSpec::new(space.clone(), fallback_options(&space))
                            .with_saved_placement(placement);
                    let outcome = match spec {
                        Ok(spec) => self.open_viewport_spec(spec, cx),
                        Err(error) => DockSurfaceViewportOpenOutcome::Unavailable(
                            DockSurfaceViewportUnavailable::from(error),
                        ),
                    };
                    DockSurfaceViewportRestoreOutcome::new(space, outcome)
                })
                .collect(),
        )
    }
}

impl From<DockSurfaceViewportSpecError> for DockSurfaceViewportUnavailable {
    fn from(error: DockSurfaceViewportSpecError) -> Self {
        match error {
            DockSurfaceViewportSpecError::InvalidPlacement { error } => {
                Self::InvalidPlacement { error }
            }
        }
    }
}
