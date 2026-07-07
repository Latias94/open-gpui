use crate::{
    DockActionApplyError, DockPolicy, DockPolicyError, DockViewportRouteSelectionSource,
    DockViewportTargetContext, DockViewportTargetHit,
};
use open_gpui::{Pixels, Point, WindowId};

/// Runtime route for a rendered drag release before workspace mutation.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockViewportDropRoute {
    /// The release is still in the source viewport, so the source host should commit locally.
    Local {
        /// Local host position for the release.
        host_position: Point<Pixels>,
        /// GPUI window that rendered the local source viewport.
        window_id: WindowId,
        /// Route-facts generation that was current for the local source viewport.
        facts_generation: u64,
        /// Source that selected the local source viewport.
        source: DockViewportRouteSelectionSource,
    },
    /// The release landed inside another registered viewport.
    KnownViewport {
        /// Destination viewport hit and its owning runtime window.
        target: DockViewportTargetHit,
        /// Source that selected the destination viewport.
        source: DockViewportRouteSelectionSource,
    },
    /// The release landed outside all registered viewports and may open a new platform viewport.
    TearOff,
    /// The release landed in a registered viewport that has no current dock target.
    Unavailable,
    /// The release landed outside all registered viewports, but policy forbids opening one.
    Rejected(DockPolicyError),
}

/// Why a route resolved to `DockViewportDropRoute::Unavailable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportDropRouteUnavailableReason {
    /// Platform viewport windows are disabled by the current backend capability contract.
    PlatformViewportWindowsUnsupported,
    /// The pointer is inside a registered viewport window, but that window cannot currently provide
    /// a host target. The release must not borrow an underlay preview through this opaque window.
    BlockedByViewportWindow,
    /// A viewport window or host target was present, but no current backend route selection chose
    /// a route-capable target.
    NoViewportRouteSelection,
    /// The backend explicitly reported hovered=None for this snapshot. This must be treated as an
    /// authoritative unavailable state for hovered-host releases.
    TrustedHoveredNone,
}

/// A viewport drop route plus internal routing diagnostics used by release-time delivery policy.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportDropRouteResolution {
    route: DockViewportDropRoute,
    unavailable_reason: Option<DockViewportDropRouteUnavailableReason>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum DockViewportDropRoutePlan {
    Route(DockViewportDropRoute),
    Unavailable(DockViewportDropRouteUnavailableReason),
    OutsideRegisteredViewport,
}

impl DockViewportDropRoutePlan {
    pub(super) fn route(route: DockViewportDropRoute) -> Self {
        Self::Route(route)
    }

    pub(super) fn unavailable(reason: DockViewportDropRouteUnavailableReason) -> Self {
        Self::Unavailable(reason)
    }

    pub(super) fn into_resolution(
        self,
        policy: &DockPolicy,
        supports_platform_viewport_windows: bool,
    ) -> DockViewportDropRouteResolution {
        match self {
            Self::Route(route) => DockViewportDropRouteResolution::route(route),
            Self::Unavailable(reason) => DockViewportDropRouteResolution::unavailable(reason),
            Self::OutsideRegisteredViewport => match policy.validate_platform_viewports() {
                Ok(()) if supports_platform_viewport_windows => {
                    DockViewportDropRouteResolution::route(DockViewportDropRoute::TearOff)
                }
                Ok(()) => DockViewportDropRouteResolution::unavailable(
                    DockViewportDropRouteUnavailableReason::PlatformViewportWindowsUnsupported,
                ),
                Err(reason) => {
                    DockViewportDropRouteResolution::route(DockViewportDropRoute::Rejected(reason))
                }
            },
        }
    }
}

impl DockViewportDropRouteResolution {
    pub(super) fn route(route: DockViewportDropRoute) -> Self {
        Self {
            route,
            unavailable_reason: None,
        }
    }

    pub(super) fn unavailable(reason: DockViewportDropRouteUnavailableReason) -> Self {
        Self {
            route: DockViewportDropRoute::Unavailable,
            unavailable_reason: Some(reason),
        }
    }

    #[cfg(test)]
    pub(crate) fn route_ref(&self) -> &DockViewportDropRoute {
        &self.route
    }

    pub(crate) fn into_route(self) -> DockViewportDropRoute {
        self.route
    }

    pub(crate) fn unavailable_reason(&self) -> Option<DockViewportDropRouteUnavailableReason> {
        self.unavailable_reason
    }
}

impl DockViewportDropRoute {
    pub(crate) fn delivery_error(&self) -> DockActionApplyError {
        match self {
            Self::Rejected(error) => DockActionApplyError::Policy(error.clone()),
            Self::Unavailable | Self::Local { .. } | Self::KnownViewport { .. } | Self::TearOff => {
                DockActionApplyError::DropTargetUnavailable
            }
        }
    }
}

pub(super) fn unavailable_route_selection_reason(
    target_context: &DockViewportTargetContext,
) -> DockViewportDropRouteUnavailableReason {
    match target_context.trusted_hovered_signal() {
        crate::DockViewportTrustedHoveredSignal::TrustedNone => {
            DockViewportDropRouteUnavailableReason::TrustedHoveredNone
        }
        crate::DockViewportTrustedHoveredSignal::Unavailable
        | crate::DockViewportTrustedHoveredSignal::Trusted(_) => {
            DockViewportDropRouteUnavailableReason::NoViewportRouteSelection
        }
    }
}
