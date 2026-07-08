use super::DockSurface;
use crate::{
    DockPolicyError, DockSpaceId, DockViewportOpenOutcome, DockViewportOpenStatus,
    DockViewportPlacementLayout,
};
use open_gpui::{AnyWindowHandle, App, AppContext as _, WindowOptions};
use thiserror::Error;

/// Facade-level outcome of a platform viewport open request.
#[derive(Debug)]
pub enum DockSurfaceViewportOpenOutcome {
    /// The runtime opened, reused, or replaced a platform viewport window.
    Opened(DockSurfaceViewportOpened),
    /// The surface rejected the request before opening a window.
    Unavailable(DockSurfaceViewportUnavailable),
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

/// Typed reason a facade-level platform viewport request did not open a window.
#[derive(Debug)]
pub enum DockSurfaceViewportUnavailable {
    /// The app's docking policy does not allow platform viewport windows.
    PolicyDisabled(DockPolicyError),
    /// The active backend does not support independent platform viewport windows.
    BackendUnsupported,
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
        match self.viewport_runtime.open_viewport(space, options, cx) {
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
}
