use crate::{
    DockActionApplyError, DockPolicy, DockPolicyError, DockViewportRouteProof,
    DockViewportRouteSelectionSource, DockViewportTargetContext, DockViewportTargetHit,
};
use open_gpui::{Pixels, Point};

/// Runtime route for a rendered drag release before workspace mutation.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockViewportDropRoute {
    /// The release is still in the source viewport, so the source host should commit locally.
    Local {
        /// Local host position for the release.
        host_position: Point<Pixels>,
        /// Exact viewport registration and route-facts generation that selected this target.
        route_proof: DockViewportRouteProof,
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
    /// The release target is known, but route authority forbids committing there.
    Rejected(DockViewportDropRouteRejectionReason),
}

/// Typed reason a resolved viewport route is visible but ineligible for commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DockViewportDropRouteRejectionReason {
    /// Workspace policy rejected the otherwise current route.
    Policy(DockPolicyError),
    /// The target host belongs to another independent Dock surface.
    ForeignSurface,
}

impl From<DockPolicyError> for DockViewportDropRouteRejectionReason {
    fn from(reason: DockPolicyError) -> Self {
        Self::Policy(reason)
    }
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
                Err(reason) => DockViewportDropRouteResolution::route(
                    DockViewportDropRoute::Rejected(reason.into()),
                ),
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
    #[cfg(test)]
    pub(crate) fn rejected_by_policy(reason: DockPolicyError) -> Self {
        Self::Rejected(DockViewportDropRouteRejectionReason::Policy(reason))
    }

    #[cfg(test)]
    pub(crate) fn local_for_registration_test(
        registration_key: crate::viewport_registry::DockViewportRegistrationKey,
        host_position: Point<Pixels>,
        facts_generation: u64,
        source: DockViewportRouteSelectionSource,
    ) -> Self {
        Self::Local {
            host_position,
            route_proof: DockViewportRouteProof::new(registration_key, facts_generation),
            source,
        }
    }

    #[cfg(test)]
    pub(crate) fn local_for_test(
        space: impl Into<crate::DockSpaceId>,
        window_id: open_gpui::WindowId,
        host_position: Point<Pixels>,
        facts_generation: u64,
        source: DockViewportRouteSelectionSource,
    ) -> Self {
        Self::Local {
            host_position,
            route_proof: DockViewportRouteProof::for_test_registration_generation(
                space.into(),
                window_id,
                1,
                facts_generation,
            ),
            source,
        }
    }

    pub(crate) fn route_proof(&self) -> Option<&DockViewportRouteProof> {
        match self {
            Self::Local { route_proof, .. } => Some(route_proof),
            Self::KnownViewport { target, .. } => Some(target.route_proof()),
            Self::TearOff | Self::Unavailable | Self::Rejected(_) => None,
        }
    }

    pub(crate) fn delivery_error(&self) -> DockActionApplyError {
        match self {
            Self::Rejected(DockViewportDropRouteRejectionReason::Policy(error)) => {
                DockActionApplyError::Policy(error.clone())
            }
            Self::Rejected(DockViewportDropRouteRejectionReason::ForeignSurface) => {
                DockActionApplyError::DropTargetUnavailable
            }
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
