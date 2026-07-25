use super::{DockSurface, DockSurfaceViewportSpec, DockSurfaceViewportUnavailable};
use crate::{
    DockPolicyError, DockSpaceId, DockViewportInputStatus, DockViewportLifecycleRecord,
    DockViewportPlacementLayout, DockViewportPlacementValidationError,
    DockViewportRestoreReadiness, DockViewportRouteStatus, DockViewportStaleStatusReason,
};
use open_gpui::{
    App, AppContext as _, PlatformViewportCapabilities, PlatformViewportFlagCapabilities,
    WindowBackgroundAppearance, WindowId, WindowOptions,
};

/// Facade-level readiness report for opening one platform viewport window.
///
/// This report is intentionally safe for common application code: it exposes the policy/backend
/// blocker that would prevent opening, plus platform and lifecycle facts needed to disable UI or
/// show an unsupported-state message without importing the lower-level runtime tier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockSurfaceViewportReadiness {
    space: DockSpaceId,
    status: DockSurfaceViewportReadinessStatus,
    platform: DockSurfaceViewportPlatformReadiness,
    restore: Option<DockViewportRestoreReadiness>,
    lifecycle: Vec<DockSurfaceViewportLifecycleReadiness>,
}

/// Ordered facade-level readiness report for saved-placement restore flows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockSurfaceViewportReadinessReport {
    entries: Vec<DockSurfaceViewportReadiness>,
}

/// Whether a facade viewport request can attempt to open a platform window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DockSurfaceViewportReadinessStatus {
    /// The request passed facade policy, backend, and placement preflight.
    Openable,
    /// Application policy disabled platform viewport windows.
    PolicyDisabled(DockPolicyError),
    /// The active GPUI backend cannot open independent platform viewport windows.
    BackendUnsupported,
    /// The request asks for platform viewport flags the active backend cannot honor.
    FlagUnsupported {
        /// Unsupported platform viewport flags requested by the spec.
        flags: Vec<DockSurfaceViewportUnsupportedFlag>,
    },
    /// Serialized viewport placement data failed validation before opening.
    InvalidPlacement {
        /// Structured validation error from the placement validator.
        error: DockViewportPlacementValidationError,
    },
}

/// Facade-level platform viewport flag that is unsupported for a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DockSurfaceViewportUnsupportedFlag {
    /// The request needs a native no-input/click-through viewport window.
    NoInputWindow,
    /// The request needs native alpha/transparent viewport-window support.
    AlphaWindow,
}

/// Platform facts and non-blocking warnings relevant to viewport opening.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockSurfaceViewportPlatformReadiness {
    capabilities: DockSurfaceViewportPlatformCapabilities,
    flag_capabilities: DockSurfaceViewportFlagCapabilities,
    warnings: Vec<DockSurfaceViewportFlagWarning>,
}

/// Platform viewport capability snapshot exposed through the DockSurface facade.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DockSurfaceViewportPlatformCapabilities {
    /// Independent application viewport windows can be opened for docking tear-off.
    pub platform_viewport_windows: bool,
    /// Window bounds are reported in a shared desktop coordinate space.
    pub global_window_bounds: bool,
    /// The platform can report application windows in front-to-back order.
    pub window_stack: bool,
    /// Display visible bounds exclude system-reserved work areas.
    pub display_work_area: bool,
    /// Per-window DPI scale facts are reliable for placement decisions.
    pub dpi_scale: bool,
    /// Already-open windows can be moved or resized programmatically.
    pub live_window_move: bool,
    /// Native no-input/click-through windows are supported.
    pub no_input_windows: bool,
    /// Hovered-window queries pass through native no-input/click-through application windows.
    pub hovered_window_ignores_no_input: bool,
}

/// Platform support for ImGui-style viewport window flags exposed through the facade.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DockSurfaceViewportFlagCapabilities {
    /// Native no-focus-on-appearing viewport windows are supported.
    pub no_focus_on_appearing_windows: bool,
    /// Native no-focus-on-click viewport windows are supported.
    pub no_focus_on_click_windows: bool,
    /// Native alpha/transparent viewport windows are supported.
    pub alpha_windows: bool,
    /// Native always-on-top viewport windows are supported.
    pub topmost_windows: bool,
    /// Native taskbar-hidden viewport windows are supported.
    pub no_taskbar_windows: bool,
}

/// Non-blocking platform flag warning for a viewport request.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockSurfaceViewportFlagWarning {
    /// The request asked for pointer pass-through, but the backend cannot apply native no-input.
    PointerInputPassThroughUnsupported,
}

/// Facade-level lifecycle status for one registered viewport window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockSurfaceViewportLifecycleReadiness {
    /// Logical dock space rendered by the viewport window.
    pub space: DockSpaceId,
    /// GPUI window currently bound to the logical dock space.
    pub window_id: WindowId,
    /// Route-facts status derived from the viewport lifecycle machine.
    pub route_status: DockSurfaceViewportRouteStatus,
    /// Platform input-mask state observed for the viewport window.
    pub input_status: DockSurfaceViewportInputStatus,
    /// The platform has requested that this viewport should close.
    pub close_requested: bool,
    /// The platform has requested or reported an authoritative resize.
    pub resize_requested: bool,
    /// Generation of the latest platform/host route facts.
    pub facts_generation: u64,
}

/// Facade-level route readiness for a registered platform viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockSurfaceViewportRouteStatus {
    /// The space/window binding exists, but no rendered host scene has published route facts yet.
    RegisteredNotReady,
    /// The latest rendered host scene and platform window facts can be used for routing.
    RouteReady,
    /// Previously published route facts were invalidated and need a fresh rendered host scene.
    Stale {
        /// Reason the viewport was demoted from route-ready to stale.
        reason: DockSurfaceViewportStaleReason,
    },
    /// The latest platform facts say the window is minimized.
    Minimized,
    /// The lifecycle state was ready, but one of the required route fact snapshots is absent.
    MissingRouteFacts,
}

/// Facade-level reason that a registered viewport route became stale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockSurfaceViewportStaleReason {
    /// GPUI reported platform window facts changed after the last rendered host scene.
    WindowFactsChanged,
}

/// Facade-level input-mask status for a registered viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockSurfaceViewportInputStatus {
    /// The window receives pointer input.
    ReceivesInput,
    /// The window is minimized.
    Minimized,
    /// The window is native no-input/click-through.
    NoInputPassThrough,
}

impl DockSurfaceViewportReadiness {
    pub(crate) fn check_open(
        surface: &DockSurface,
        spec: &DockSurfaceViewportSpec,
        restore: Option<DockViewportRestoreReadiness>,
        cx: &mut App,
    ) -> Self {
        let controller = surface.controller(cx);
        let status = match cx.read_entity(&controller, |controller, _| {
            controller.policy().validate_platform_viewports()
        }) {
            Ok(()) if cx.viewport_capabilities().platform_viewport_windows => {
                let unsupported_flags = unsupported_viewport_flags(
                    spec.window_options(),
                    cx.viewport_capabilities(),
                    cx.viewport_flag_capabilities(),
                );
                if unsupported_flags.is_empty() {
                    DockSurfaceViewportReadinessStatus::Openable
                } else {
                    DockSurfaceViewportReadinessStatus::FlagUnsupported {
                        flags: unsupported_flags,
                    }
                }
            }
            Ok(()) => DockSurfaceViewportReadinessStatus::BackendUnsupported,
            Err(error) => DockSurfaceViewportReadinessStatus::PolicyDisabled(error),
        };
        Self::from_status(
            surface,
            spec.space().clone(),
            spec.window_options(),
            restore,
            status,
            cx,
        )
    }

    pub(crate) fn invalid_placement(
        surface: &DockSurface,
        space: DockSpaceId,
        options: &WindowOptions,
        error: DockViewportPlacementValidationError,
        cx: &mut App,
    ) -> Self {
        Self::from_status(
            surface,
            space,
            options,
            None,
            DockSurfaceViewportReadinessStatus::InvalidPlacement { error },
            cx,
        )
    }

    fn from_status(
        surface: &DockSurface,
        space: DockSpaceId,
        options: &WindowOptions,
        restore: Option<DockViewportRestoreReadiness>,
        status: DockSurfaceViewportReadinessStatus,
        cx: &mut App,
    ) -> Self {
        let platform = DockSurfaceViewportPlatformReadiness::from_window_options(
            options,
            cx.viewport_capabilities(),
            cx.viewport_flag_capabilities(),
        );
        let lifecycle = surface
            .viewport_runtime(cx)
            .runtime_status()
            .viewport_lifecycle
            .into_iter()
            .map(DockSurfaceViewportLifecycleReadiness::from)
            .collect();
        Self {
            space,
            status,
            platform,
            restore,
            lifecycle,
        }
    }

    /// Logical dock space that this readiness report describes.
    pub fn space(&self) -> &DockSpaceId {
        &self.space
    }

    /// Blocking readiness status for the viewport request.
    pub fn status(&self) -> &DockSurfaceViewportReadinessStatus {
        &self.status
    }

    /// Returns true when the facade preflight allows opening a platform viewport window.
    pub fn is_openable(&self) -> bool {
        self.status.is_openable()
    }

    /// Returns true when the facade preflight allows opening a platform viewport window.
    pub fn ready(&self) -> bool {
        self.is_openable()
    }

    /// Returns true when application policy rejects platform viewport windows.
    pub fn is_policy_disabled(&self) -> bool {
        self.status.is_policy_disabled()
    }

    /// Returns true when the active platform backend lacks viewport-window support.
    pub fn is_backend_unsupported(&self) -> bool {
        self.status.is_backend_unsupported()
    }

    /// Returns true when requested platform viewport flags are unsupported.
    pub fn is_flag_unsupported(&self) -> bool {
        self.status.is_flag_unsupported()
    }

    /// Returns true when serialized placement data failed validation.
    pub fn is_invalid_placement(&self) -> bool {
        self.status.is_invalid_placement()
    }

    /// Returns true when a platform viewport is already registered for this dock space.
    pub fn registered(&self) -> bool {
        self.lifecycle.iter().any(|entry| entry.space == self.space)
    }

    /// Registered platform viewport spaces visible at the time of the check.
    pub fn registered_spaces(&self) -> Vec<DockSpaceId> {
        self.lifecycle
            .iter()
            .map(|entry| entry.space.clone())
            .collect()
    }

    /// Platform capability facts and non-blocking warnings for this request.
    pub fn platform(&self) -> &DockSurfaceViewportPlatformReadiness {
        &self.platform
    }

    /// Platform viewport capabilities sampled from the active GPUI backend.
    pub fn platform_capabilities(&self) -> DockSurfaceViewportPlatformCapabilities {
        self.platform.capabilities()
    }

    /// ImGui-style viewport flag capabilities sampled from the active GPUI backend.
    pub fn platform_flag_capabilities(&self) -> DockSurfaceViewportFlagCapabilities {
        self.platform.flag_capabilities()
    }

    /// Unsupported platform viewport flags requested by the spec.
    pub fn unsupported_flags(&self) -> &[DockSurfaceViewportUnsupportedFlag] {
        self.status.unsupported_flags()
    }

    /// Structured placement validation details when saved placement data was invalid.
    pub fn placement_validation_error(&self) -> Option<&DockViewportPlacementValidationError> {
        self.status.placement_validation_error()
    }

    /// Saved-placement match summary attached to restore readiness checks.
    pub fn restore(&self) -> Option<DockViewportRestoreReadiness> {
        self.restore
    }

    /// Registered viewport lifecycle facts visible at the time of the check.
    pub fn lifecycle(&self) -> &[DockSurfaceViewportLifecycleReadiness] {
        &self.lifecycle
    }

    /// Converts a blocking readiness state into the matching open-unavailable reason.
    pub(crate) fn unavailable_reason(&self) -> Option<DockSurfaceViewportUnavailable> {
        self.status.to_unavailable()
    }

    /// Converts a blocking readiness state into the matching open-unavailable reason.
    pub fn unavailable(&self) -> Option<DockSurfaceViewportUnavailable> {
        self.unavailable_reason()
    }
}

impl DockSurfaceViewportReadinessReport {
    pub(crate) fn new(entries: Vec<DockSurfaceViewportReadiness>) -> Self {
        Self { entries }
    }

    pub(crate) fn check_restore(
        surface: &DockSurface,
        placement: &DockViewportPlacementLayout,
        mut fallback_options: impl FnMut(&DockSpaceId) -> WindowOptions,
        cx: &mut App,
    ) -> Self {
        let default_options = WindowOptions::default();
        let restore = match surface.check_viewport_placement_restore(placement, cx) {
            Ok(restore) => restore,
            Err(error) => {
                return Self::new(
                    placement
                        .viewports
                        .iter()
                        .map(|viewport| {
                            DockSurfaceViewportReadiness::invalid_placement(
                                surface,
                                viewport.space.clone(),
                                &default_options,
                                error.clone(),
                                cx,
                            )
                        })
                        .collect(),
                );
            }
        };

        Self::new(
            placement
                .viewports
                .iter()
                .map(|viewport| {
                    let space = viewport.space.clone();
                    match DockSurfaceViewportSpec::new(space.clone(), fallback_options(&space))
                        .with_saved_placement(placement)
                    {
                        Ok(spec) => DockSurfaceViewportReadiness::check_open(
                            surface,
                            &spec,
                            Some(restore),
                            cx,
                        ),
                        Err(error) => match error {
                            super::DockSurfaceViewportSpecError::InvalidPlacement { error } => {
                                DockSurfaceViewportReadiness::invalid_placement(
                                    surface,
                                    space,
                                    &default_options,
                                    error,
                                    cx,
                                )
                            }
                        },
                    }
                })
                .collect(),
        )
    }

    /// Ordered readiness entries keyed by saved-placement viewport order.
    pub fn entries(&self) -> &[DockSurfaceViewportReadiness] {
        &self.entries
    }

    /// Ordered readiness entries keyed by saved-placement viewport order.
    pub fn readiness(&self) -> &[DockSurfaceViewportReadiness] {
        self.entries()
    }

    /// Consumes the report and returns ordered readiness entries.
    pub fn into_entries(self) -> Vec<DockSurfaceViewportReadiness> {
        self.entries
    }

    /// Consumes the report and returns ordered readiness entries.
    pub fn into_readiness(self) -> Vec<DockSurfaceViewportReadiness> {
        self.into_entries()
    }

    /// Returns the first readiness entry for a logical dock space, when present.
    pub fn readiness_for_space(
        &self,
        space: &DockSpaceId,
    ) -> Option<&DockSurfaceViewportReadiness> {
        self.entries.iter().find(|entry| entry.space() == space)
    }

    /// Number of readiness entries in the report.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true when no readiness entries were produced.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Number of entries whose facade preflight allows opening.
    pub fn openable_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.is_openable())
            .count()
    }

    /// Number of entries whose facade preflight allows opening.
    pub fn ready_count(&self) -> usize {
        self.openable_count()
    }

    /// Number of entries blocked by policy, backend, or placement validation.
    pub fn unavailable_count(&self) -> usize {
        self.len().saturating_sub(self.openable_count())
    }

    /// Returns true when every entry is openable.
    pub fn all_openable(&self) -> bool {
        self.entries
            .iter()
            .all(DockSurfaceViewportReadiness::is_openable)
    }

    /// Returns true when every entry is openable.
    pub fn all_ready(&self) -> bool {
        self.all_openable()
    }
}

impl DockSurfaceViewportReadinessStatus {
    /// Returns true when this status permits a platform viewport open attempt.
    pub fn is_openable(&self) -> bool {
        matches!(self, Self::Openable)
    }

    /// Returns true when this status was blocked by the active backend capability.
    pub fn is_backend_unsupported(&self) -> bool {
        matches!(self, Self::BackendUnsupported)
    }

    /// Returns true when this status was blocked by application policy.
    pub fn is_policy_disabled(&self) -> bool {
        matches!(self, Self::PolicyDisabled(_))
    }

    /// Returns true when this status was blocked by saved-placement validation.
    pub fn is_invalid_placement(&self) -> bool {
        matches!(self, Self::InvalidPlacement { .. })
    }

    /// Returns true when this status was blocked by unsupported platform viewport flags.
    pub fn is_flag_unsupported(&self) -> bool {
        matches!(self, Self::FlagUnsupported { .. })
    }

    /// Unsupported platform viewport flags requested by the spec.
    pub fn unsupported_flags(&self) -> &[DockSurfaceViewportUnsupportedFlag] {
        match self {
            Self::FlagUnsupported { flags } => flags,
            _ => &[],
        }
    }

    /// Structured placement validation details when saved placement data was invalid.
    pub fn placement_validation_error(&self) -> Option<&DockViewportPlacementValidationError> {
        match self {
            Self::InvalidPlacement { error } => Some(error),
            _ => None,
        }
    }

    fn to_unavailable(&self) -> Option<DockSurfaceViewportUnavailable> {
        match self {
            Self::Openable => None,
            Self::PolicyDisabled(error) => Some(DockSurfaceViewportUnavailable::PolicyDisabled(
                error.clone(),
            )),
            Self::BackendUnsupported => Some(DockSurfaceViewportUnavailable::BackendUnsupported),
            Self::FlagUnsupported { flags } => {
                Some(DockSurfaceViewportUnavailable::FlagUnsupported {
                    flags: flags.clone(),
                })
            }
            Self::InvalidPlacement { error } => {
                Some(DockSurfaceViewportUnavailable::InvalidPlacement {
                    error: error.clone(),
                })
            }
        }
    }
}

fn unsupported_viewport_flags(
    options: &WindowOptions,
    platform_capabilities: PlatformViewportCapabilities,
    flag_capabilities: PlatformViewportFlagCapabilities,
) -> Vec<DockSurfaceViewportUnsupportedFlag> {
    let mut flags = Vec::new();
    if !options.accepts_pointer_input && !platform_capabilities.no_input_windows {
        flags.push(DockSurfaceViewportUnsupportedFlag::NoInputWindow);
    }
    if matches!(
        options.window_background,
        WindowBackgroundAppearance::Transparent | WindowBackgroundAppearance::Blurred
    ) && !flag_capabilities.alpha_windows
    {
        flags.push(DockSurfaceViewportUnsupportedFlag::AlphaWindow);
    }
    flags
}

impl DockSurfaceViewportPlatformReadiness {
    fn from_window_options(
        options: &WindowOptions,
        capabilities: PlatformViewportCapabilities,
        flag_capabilities: PlatformViewportFlagCapabilities,
    ) -> Self {
        let capabilities = DockSurfaceViewportPlatformCapabilities::from(capabilities);
        let mut warnings = Vec::new();
        if !options.accepts_pointer_input && !capabilities.no_input_windows {
            warnings.push(DockSurfaceViewportFlagWarning::PointerInputPassThroughUnsupported);
        }
        Self {
            capabilities,
            flag_capabilities: DockSurfaceViewportFlagCapabilities::from(flag_capabilities),
            warnings,
        }
    }

    /// Platform viewport capabilities sampled from the active GPUI backend.
    pub fn capabilities(&self) -> DockSurfaceViewportPlatformCapabilities {
        self.capabilities
    }

    /// ImGui-style viewport flag capabilities sampled from the active GPUI backend.
    pub fn flag_capabilities(&self) -> DockSurfaceViewportFlagCapabilities {
        self.flag_capabilities
    }

    /// Non-blocking platform flag warnings for the request.
    pub fn warnings(&self) -> &[DockSurfaceViewportFlagWarning] {
        &self.warnings
    }

    /// Returns true when this request has platform warnings but no blocking readiness error.
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

impl From<PlatformViewportCapabilities> for DockSurfaceViewportPlatformCapabilities {
    fn from(capabilities: PlatformViewportCapabilities) -> Self {
        Self {
            platform_viewport_windows: capabilities.platform_viewport_windows,
            global_window_bounds: capabilities.global_window_bounds,
            window_stack: capabilities.window_stack,
            display_work_area: capabilities.display_work_area,
            dpi_scale: capabilities.dpi_scale,
            live_window_move: capabilities.live_window_move,
            no_input_windows: capabilities.no_input_windows,
            hovered_window_ignores_no_input: capabilities.hovered_window_ignores_no_input,
        }
    }
}

impl From<PlatformViewportFlagCapabilities> for DockSurfaceViewportFlagCapabilities {
    fn from(capabilities: PlatformViewportFlagCapabilities) -> Self {
        Self {
            no_focus_on_appearing_windows: capabilities.no_focus_on_appearing_windows,
            no_focus_on_click_windows: capabilities.no_focus_on_click_windows,
            alpha_windows: capabilities.alpha_windows,
            topmost_windows: capabilities.topmost_windows,
            no_taskbar_windows: capabilities.no_taskbar_windows,
        }
    }
}

impl From<DockViewportLifecycleRecord> for DockSurfaceViewportLifecycleReadiness {
    fn from(record: DockViewportLifecycleRecord) -> Self {
        Self {
            space: record.space,
            window_id: record.window_id,
            route_status: DockSurfaceViewportRouteStatus::from(record.route_status),
            input_status: DockSurfaceViewportInputStatus::from(record.input_status),
            close_requested: record.platform_request_status.close_requested,
            resize_requested: record.platform_request_status.resize_requested,
            facts_generation: record.facts_generation,
        }
    }
}

impl From<DockViewportRouteStatus> for DockSurfaceViewportRouteStatus {
    fn from(status: DockViewportRouteStatus) -> Self {
        match status {
            DockViewportRouteStatus::RegisteredNotReady => Self::RegisteredNotReady,
            DockViewportRouteStatus::RouteReady => Self::RouteReady,
            DockViewportRouteStatus::Stale { reason } => Self::Stale {
                reason: DockSurfaceViewportStaleReason::from(reason),
            },
            DockViewportRouteStatus::Minimized => Self::Minimized,
            DockViewportRouteStatus::MissingRouteFacts => Self::MissingRouteFacts,
        }
    }
}

impl From<DockViewportStaleStatusReason> for DockSurfaceViewportStaleReason {
    fn from(reason: DockViewportStaleStatusReason) -> Self {
        match reason {
            DockViewportStaleStatusReason::WindowFactsChanged => Self::WindowFactsChanged,
        }
    }
}

impl From<DockViewportInputStatus> for DockSurfaceViewportInputStatus {
    fn from(status: DockViewportInputStatus) -> Self {
        match status {
            DockViewportInputStatus::ReceivesInput => Self::ReceivesInput,
            DockViewportInputStatus::Minimized => Self::Minimized,
            DockViewportInputStatus::NoInputPassThrough => Self::NoInputPassThrough,
        }
    }
}
