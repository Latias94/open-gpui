use super::payload_recovery::{DockPayloadRecoveryAuthority, DockPayloadRecoveryCommitReceipt};
use super::window_session::{DockSurfaceWindowSessionDependencyId, DockSurfaceWindowSessionLease};
use crate::{
    DockViewportProvisionalOpenAttemptCompletion, host::DockHostLivePresentationCleanupReceipt,
    native_captured_drag::DockNativeCapturedDragTransportRetirementReceipt,
};
use open_gpui::{
    AnyWindowHandle, FocusHandle, PlatformPhysicalDisplayObservation, WindowId,
    WindowProvisionalPlacementOutcome, WindowProvisionalPlacementPurpose,
    WindowProvisionalPlacementSnapshot, WindowProvisionalRevealOutcome,
    WindowProvisionalRevealSnapshot, WindowProvisionalRevealZOrder,
    WindowProvisionalSemanticsOutcome, WindowProvisionalSemanticsSnapshot,
    WindowProvisionalSession, WindowProvisionalSessionPhase,
    retained_visual::TicketIdentity,
    view_presentation_window::{
        Invalidation, LeaseBatch, RehostDestinationExposure, RehostDestinationPresentation,
        RehostProjection, RehostSourceProxyCommit,
    },
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct DockLiveUndockDragGeneration(u64);

impl DockLiveUndockDragGeneration {
    pub(crate) const fn new(generation: u64) -> Option<Self> {
        if generation == 0 {
            None
        } else {
            Some(Self(generation))
        }
    }

    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct DockLiveUndockPresentationLeaseGeneration(u64);

impl DockLiveUndockPresentationLeaseGeneration {
    pub(crate) const fn new(generation: u64) -> Option<Self> {
        if generation == 0 {
            None
        } else {
            Some(Self(generation))
        }
    }

    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct DockLiveUndockPlacementGeneration(u64);

impl DockLiveUndockPlacementGeneration {
    pub(crate) const fn new(generation: u64) -> Option<Self> {
        if generation == 0 {
            None
        } else {
            Some(Self(generation))
        }
    }

    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct DockLiveUndockRouteGeneration(u64);

impl DockLiveUndockRouteGeneration {
    pub(crate) const fn new(generation: u64) -> Option<Self> {
        if generation == 0 {
            None
        } else {
            Some(Self(generation))
        }
    }

    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct DockLiveUndockOpeningKey {
    lease: DockSurfaceWindowSessionLease,
    generation: u64,
}

impl DockLiveUndockOpeningKey {
    pub(crate) const fn lease(self) -> DockSurfaceWindowSessionLease {
        self.lease
    }

    pub(crate) const fn generation(self) -> u64 {
        self.generation
    }

    #[cfg(test)]
    pub(crate) const fn for_test(lease: DockSurfaceWindowSessionLease, generation: u64) -> Self {
        Self { lease, generation }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct DockLiveUndockIdentity {
    opening: DockLiveUndockOpeningKey,
    drag_generation: DockLiveUndockDragGeneration,
}

impl DockLiveUndockIdentity {
    pub(crate) const fn opening(self) -> DockLiveUndockOpeningKey {
        self.opening
    }

    pub(crate) const fn drag_generation(self) -> DockLiveUndockDragGeneration {
        self.drag_generation
    }

    #[cfg(test)]
    pub(crate) const fn for_test(
        lease: DockSurfaceWindowSessionLease,
        opening_generation: u64,
        drag_generation: u64,
    ) -> Self {
        Self {
            opening: DockLiveUndockOpeningKey::for_test(lease, opening_generation),
            drag_generation: match DockLiveUndockDragGeneration::new(drag_generation) {
                Some(generation) => generation,
                None => panic!("test drag generation must be non-zero"),
            },
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DockLiveUndockOpenRequest {
    identity: DockLiveUndockIdentity,
    provisional_session: WindowProvisionalSession,
}

impl DockLiveUndockOpenRequest {
    pub(crate) const fn identity(&self) -> DockLiveUndockIdentity {
        self.identity
    }

    pub(crate) const fn key(&self) -> DockLiveUndockOpeningKey {
        self.identity.opening
    }

    pub(crate) fn provisional_session(&self) -> &WindowProvisionalSession {
        &self.provisional_session
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockOpenReturnOutcome {
    Admit {
        lease: DockSurfaceWindowSessionLease,
    },
    RuntimeRegistrationRejected {
        lease: DockSurfaceWindowSessionLease,
    },
    Retire {
        lease: DockSurfaceWindowSessionLease,
        dependency: Option<DockSurfaceWindowSessionDependencyId>,
        binding_valid: bool,
    },
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockOpenFailureOutcome {
    Cleared,
    SettleDependency {
        lease: DockSurfaceWindowSessionLease,
        dependency: DockSurfaceWindowSessionDependencyId,
    },
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockShutdownSnapshot {
    identity: DockLiveUndockIdentity,
    dependency: DockSurfaceWindowSessionDependencyId,
    window: Option<AnyWindowHandle>,
}

impl DockLiveUndockShutdownSnapshot {
    pub(crate) const fn identity(self) -> DockLiveUndockIdentity {
        self.identity
    }

    pub(crate) const fn dependency(self) -> DockSurfaceWindowSessionDependencyId {
        self.dependency
    }

    pub(crate) const fn window(self) -> Option<AnyWindowHandle> {
        self.window
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockWindowTerminalOutcome {
    lease: DockSurfaceWindowSessionLease,
    dependency: Option<DockSurfaceWindowSessionDependencyId>,
}

impl DockLiveUndockWindowTerminalOutcome {
    pub(crate) const fn lease(self) -> DockSurfaceWindowSessionLease {
        self.lease
    }

    pub(crate) const fn dependency(self) -> Option<DockSurfaceWindowSessionDependencyId> {
        self.dependency
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockPhase {
    Idle,
    Opening,
    Bound,
    Compensating,
    Restoring,
    RecoveringOrphan,
    WaitingForPromotionCommit,
    ShutdownCleanupFailed,
    RecoveringCommittedDestination,
    Retiring,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockPhysicalPoint {
    x: i32,
    y: i32,
}

impl DockLiveUndockPhysicalPoint {
    pub(crate) const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub(crate) const fn x(self) -> i32 {
        self.x
    }

    pub(crate) const fn y(self) -> i32 {
        self.y
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockPhysicalBounds {
    origin: DockLiveUndockPhysicalPoint,
    width: u32,
    height: u32,
    target_display: PlatformPhysicalDisplayObservation,
}

impl DockLiveUndockPhysicalBounds {
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn new(
        origin: DockLiveUndockPhysicalPoint,
        width: u32,
        height: u32,
    ) -> Option<Self> {
        Self::for_test(origin, width, height)
    }

    pub(crate) fn for_display(
        origin: DockLiveUndockPhysicalPoint,
        width: u32,
        height: u32,
        target_display: PlatformPhysicalDisplayObservation,
    ) -> Option<Self> {
        if width == 0 || height == 0 {
            return None;
        }
        origin.x.checked_add(i32::try_from(width).ok()?)?;
        origin.y.checked_add(i32::try_from(height).ok()?)?;
        Some(Self {
            origin,
            width,
            height,
            target_display,
        })
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn for_test(
        origin: DockLiveUndockPhysicalPoint,
        width: u32,
        height: u32,
    ) -> Option<Self> {
        let target_display = PlatformPhysicalDisplayObservation::try_new(
            1,
            open_gpui::DisplayId::from(1),
            open_gpui::Bounds::new(
                open_gpui::point(
                    open_gpui::DevicePixels(-100_000),
                    open_gpui::DevicePixels(-100_000),
                ),
                open_gpui::size(
                    open_gpui::DevicePixels(200_000),
                    open_gpui::DevicePixels(200_000),
                ),
            ),
            open_gpui::Bounds::new(
                open_gpui::point(
                    open_gpui::DevicePixels(-100_000),
                    open_gpui::DevicePixels(-100_000),
                ),
                open_gpui::size(
                    open_gpui::DevicePixels(200_000),
                    open_gpui::DevicePixels(200_000),
                ),
            ),
            1.0,
        )
        .expect("the synthetic Dock display observation must be valid");
        Self::for_display(origin, width, height, target_display)
    }

    pub(crate) const fn origin(self) -> DockLiveUndockPhysicalPoint {
        self.origin
    }

    pub(crate) const fn width(self) -> u32 {
        self.width
    }

    pub(crate) const fn height(self) -> u32 {
        self.height
    }

    pub(crate) const fn target_display(self) -> PlatformPhysicalDisplayObservation {
        self.target_display
    }

    pub(crate) const fn contains(self, point: DockLiveUndockPhysicalPoint) -> bool {
        let left = self.origin.x as i64;
        let top = self.origin.y as i64;
        let right = left + self.width as i64;
        let bottom = top + self.height as i64;
        let x = point.x as i64;
        let y = point.y as i64;
        x >= left && x < right && y >= top && y < bottom
    }

    pub(crate) fn contains_target_point(self, point: DockLiveUndockPhysicalPoint) -> bool {
        self.contains(point)
            && self.target_display.contains(open_gpui::point(
                open_gpui::DevicePixels(point.x),
                open_gpui::DevicePixels(point.y),
            ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockHostTarget {
    window_id: WindowId,
    host_scene_generation: u64,
}

impl DockLiveUndockHostTarget {
    pub(crate) const fn new(window_id: WindowId, host_scene_generation: u64) -> Self {
        Self {
            window_id,
            host_scene_generation,
        }
    }

    pub(crate) const fn window_id(self) -> WindowId {
        self.window_id
    }

    pub(crate) const fn host_scene_generation(self) -> u64 {
        self.host_scene_generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockRouteFeedback {
    Host(DockLiveUndockHostTarget),
    ForeignSurface { window_id: WindowId },
    Desktop,
    OpaqueBarrier,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockReleaseLock {
    point: DockLiveUndockPhysicalPoint,
    hit: DockLiveUndockRouteFeedback,
    desired_bounds: DockLiveUndockPhysicalBounds,
    placement_generation: DockLiveUndockPlacementGeneration,
}

impl DockLiveUndockReleaseLock {
    pub(crate) fn new(
        point: DockLiveUndockPhysicalPoint,
        hit: DockLiveUndockRouteFeedback,
        desired_bounds: DockLiveUndockPhysicalBounds,
        placement_generation: DockLiveUndockPlacementGeneration,
    ) -> Option<Self> {
        if !desired_bounds.contains_target_point(point) {
            return None;
        }
        Some(Self {
            point,
            hit,
            desired_bounds,
            placement_generation,
        })
    }

    pub(crate) const fn point(self) -> DockLiveUndockPhysicalPoint {
        self.point
    }

    pub(crate) const fn hit(self) -> DockLiveUndockRouteFeedback {
        self.hit
    }

    pub(crate) const fn desired_bounds(self) -> DockLiveUndockPhysicalBounds {
        self.desired_bounds
    }

    pub(crate) const fn placement_generation(self) -> DockLiveUndockPlacementGeneration {
        self.placement_generation
    }

    pub(crate) const fn route_generation(self) -> DockLiveUndockRouteGeneration {
        DockLiveUndockRouteGeneration(self.placement_generation.get())
    }
}

/// Identity of the exact published Dock host scene that triggered live undock.
///
/// The long-lived effect executor retains the registration key, payload identity, graph snapshot,
/// and owner revision. This small value only identifies the route-scene domain inside the reducer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockSourceSnapshot {
    window_id: WindowId,
    scene_generation: u64,
}

/// Exact focused payload descendant captured before source presentation authority is revoked.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockSourceFocusSnapshot {
    focus_handle: FocusHandle,
    claim_revision: u64,
}

impl DockLiveUndockSourceFocusSnapshot {
    pub(crate) const fn new(focus_handle: FocusHandle, claim_revision: u64) -> Self {
        Self {
            focus_handle,
            claim_revision,
        }
    }

    pub(crate) const fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    pub(crate) const fn claim_revision(&self) -> u64 {
        self.claim_revision
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockTrigger {
    drag_generation: DockLiveUndockDragGeneration,
    source: DockLiveUndockSourceSnapshot,
    initial_route_generation: DockLiveUndockRouteGeneration,
    initial_route: DockLiveUndockRouteFeedback,
    initial_point: DockLiveUndockPhysicalPoint,
    initial_bounds: DockLiveUndockPhysicalBounds,
}

impl DockLiveUndockTrigger {
    pub(crate) fn new(
        drag_generation: DockLiveUndockDragGeneration,
        source: DockLiveUndockSourceSnapshot,
        initial_route_generation: DockLiveUndockRouteGeneration,
        initial_route: DockLiveUndockRouteFeedback,
        initial_point: DockLiveUndockPhysicalPoint,
        initial_bounds: DockLiveUndockPhysicalBounds,
    ) -> Option<Self> {
        if !matches!(
            initial_route,
            DockLiveUndockRouteFeedback::Desktop | DockLiveUndockRouteFeedback::OpaqueBarrier
        ) || !initial_bounds.contains_target_point(initial_point)
        {
            return None;
        }
        Some(Self {
            drag_generation,
            source,
            initial_route_generation,
            initial_route,
            initial_point,
            initial_bounds,
        })
    }

    pub(crate) const fn drag_generation(self) -> DockLiveUndockDragGeneration {
        self.drag_generation
    }

    pub(crate) const fn source(self) -> DockLiveUndockSourceSnapshot {
        self.source
    }

    pub(crate) const fn initial_route_generation(self) -> DockLiveUndockRouteGeneration {
        self.initial_route_generation
    }

    pub(crate) const fn initial_route(self) -> DockLiveUndockRouteFeedback {
        self.initial_route
    }

    pub(crate) const fn initial_point(self) -> DockLiveUndockPhysicalPoint {
        self.initial_point
    }

    pub(crate) const fn initial_bounds(self) -> DockLiveUndockPhysicalBounds {
        self.initial_bounds
    }
}

impl DockLiveUndockSourceSnapshot {
    pub(crate) const fn new(window_id: WindowId, scene_generation: u64) -> Self {
        Self {
            window_id,
            scene_generation,
        }
    }

    pub(crate) const fn window_id(self) -> WindowId {
        self.window_id
    }

    pub(crate) const fn scene_generation(self) -> u64 {
        self.scene_generation
    }
}

/// Exact evidence that the source's native window generation reached terminal state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockSourceNativeTerminalReceipt {
    identity: DockLiveUndockIdentity,
    source: DockLiveUndockSourceSnapshot,
}

impl DockLiveUndockSourceNativeTerminalReceipt {
    pub(super) fn from_native_terminal(
        identity: DockLiveUndockIdentity,
        source: DockLiveUndockSourceSnapshot,
        terminal_window: WindowId,
    ) -> Option<Self> {
        if source.window_id() != terminal_window {
            return None;
        }
        Some(Self { identity, source })
    }

    const fn identity(self) -> DockLiveUndockIdentity {
        self.identity
    }

    const fn source(self) -> DockLiveUndockSourceSnapshot {
        self.source
    }
}

/// Why ordinary source-presentation authority can no longer complete restoration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockPresentationAuthorityLoss {
    /// GPUI released the prepared view-presentation generation.
    ViewPresentation(Invalidation),
    /// The exact source DockHost presentation generation was replaced or released.
    SourceHostPresentationLost,
}

/// Exact evidence that ordinary source-presentation authority was released.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockPresentationAuthorityLossReceipt {
    payload_lease: DockLiveUndockPayloadLeaseReceipt,
    cause: DockLiveUndockPresentationAuthorityLoss,
}

impl DockLiveUndockPresentationAuthorityLossReceipt {
    pub(crate) fn from_invalidation(
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        projection: &RehostProjection,
        invalidation: Invalidation,
    ) -> Option<Self> {
        if !payload_lease.matches_projection(projection) {
            return None;
        }
        Some(Self {
            payload_lease,
            cause: DockLiveUndockPresentationAuthorityLoss::ViewPresentation(invalidation),
        })
    }

    pub(crate) fn from_source_host_loss(
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        projection: &RehostProjection,
    ) -> Option<Self> {
        if !payload_lease.matches_projection(projection) {
            return None;
        }
        Some(Self {
            payload_lease,
            cause: DockLiveUndockPresentationAuthorityLoss::SourceHostPresentationLost,
        })
    }

    pub(crate) const fn identity(self) -> DockLiveUndockIdentity {
        self.payload_lease.identity()
    }

    pub(crate) const fn source(self) -> DockLiveUndockSourceSnapshot {
        self.payload_lease.source()
    }

    pub(crate) const fn cause(self) -> DockLiveUndockPresentationAuthorityLoss {
        self.cause
    }

    #[cfg(test)]
    pub(super) const fn for_test(
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        cause: DockLiveUndockPresentationAuthorityLoss,
    ) -> Self {
        Self {
            payload_lease,
            cause,
        }
    }
}

/// Exact lease that binds one live-undock generation to its immutable source proof and GPUI rehost.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockPayloadLeaseReceipt {
    identity: DockLiveUndockIdentity,
    source: DockLiveUndockSourceSnapshot,
    surface_revision: u64,
    lease_generation: DockLiveUndockPresentationLeaseGeneration,
    retained_visual: Option<TicketIdentity>,
    rehost_generation: u64,
    destination_window: WindowId,
    provisional_session_generation: u64,
}

impl DockLiveUndockPayloadLeaseReceipt {
    pub(crate) fn new(
        identity: DockLiveUndockIdentity,
        source: DockLiveUndockSourceSnapshot,
        surface_revision: u64,
        retained_visual: TicketIdentity,
        projection: &RehostProjection,
        provisional_session: &WindowProvisionalSession,
    ) -> Option<Self> {
        let session = provisional_session.snapshot();
        let destination_lease_generation = projection.destination().leases().first()?.generation();
        if retained_visual.source_window() != source.window_id
            || projection.source().window_id() != source.window_id
            || projection.source().leases().is_empty()
            || projection.destination().leases().is_empty()
            || projection.generation() == 0
            || destination_lease_generation != projection.generation()
            || projection
                .destination()
                .leases()
                .iter()
                .any(|lease| lease.generation() != destination_lease_generation)
            || session.window_id() != Some(projection.destination().window_id())
            || session.phase() != WindowProvisionalSessionPhase::Gated
        {
            return None;
        }
        let lease_generation =
            DockLiveUndockPresentationLeaseGeneration::new(destination_lease_generation)?;
        Some(Self {
            identity,
            source,
            surface_revision,
            lease_generation,
            retained_visual: Some(retained_visual),
            rehost_generation: projection.generation(),
            destination_window: projection.destination().window_id(),
            provisional_session_generation: session.generation(),
        })
    }

    fn matches_projection(self, projection: &RehostProjection) -> bool {
        projection.generation() == self.rehost_generation
            && projection.source().window_id() == self.source.window_id
            && projection.destination().window_id() == self.destination_window
            && !projection.source().leases().is_empty()
            && !projection.destination().leases().is_empty()
            && projection
                .destination()
                .leases()
                .iter()
                .all(|lease| lease.generation() == self.lease_generation.get())
    }

    pub(crate) const fn identity(self) -> DockLiveUndockIdentity {
        self.identity
    }

    pub(crate) const fn source(self) -> DockLiveUndockSourceSnapshot {
        self.source
    }

    pub(crate) const fn surface_revision(self) -> u64 {
        self.surface_revision
    }

    pub(crate) const fn lease_generation(self) -> DockLiveUndockPresentationLeaseGeneration {
        self.lease_generation
    }

    pub(crate) const fn retained_visual(self) -> Option<TicketIdentity> {
        self.retained_visual
    }

    pub(crate) const fn rehost_generation(self) -> u64 {
        self.rehost_generation
    }

    pub(crate) const fn destination_window(self) -> WindowId {
        self.destination_window
    }

    pub(crate) const fn provisional_session_generation(self) -> u64 {
        self.provisional_session_generation
    }

    #[cfg(test)]
    pub(super) fn for_test(
        identity: DockLiveUndockIdentity,
        source: DockLiveUndockSourceSnapshot,
        lease_generation: DockLiveUndockPresentationLeaseGeneration,
        destination_window: WindowId,
    ) -> Self {
        Self {
            identity,
            source,
            surface_revision: source.scene_generation,
            lease_generation,
            retained_visual: None,
            rehost_generation: lease_generation.get(),
            destination_window,
            provisional_session_generation: identity.opening.generation,
        }
    }
}

/// Exact proof that source presentation compensation reached its authoritative boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockSourceRestorationReceipt {
    payload_lease: DockLiveUndockPayloadLeaseReceipt,
    evidence: DockLiveUndockSourceRestorationEvidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockLiveUndockSourceRestorationEvidence {
    /// The prepared rehost was cancelled before the source-release barrier committed.
    Unchanged,
    /// Fresh source leases were finalized and then mounted in one later accepted frame.
    AfterRelease {
        lease_generation: u64,
        root_count: usize,
        frame_generation: u64,
    },
}

impl DockLiveUndockSourceRestorationReceipt {
    /// Proves that cancellation completed while the original source batch remained authoritative.
    pub(crate) fn source_unchanged(
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        projection: &RehostProjection,
        source_leases: &LeaseBatch,
    ) -> Option<Self> {
        let exact_source_batch = projection.source().window_id() == source_leases.window_id()
            && projection.source().leases() == source_leases.leases();
        if !payload_lease.matches_projection(projection)
            || !exact_source_batch
            || source_leases.window_id() != payload_lease.source().window_id()
            || source_leases.leases().is_empty()
            || source_leases
                .leases()
                .iter()
                .any(|lease| lease.window_id() != source_leases.window_id())
        {
            return None;
        }
        Some(Self {
            payload_lease,
            evidence: DockLiveUndockSourceRestorationEvidence::Unchanged,
        })
    }

    /// Proves that fresh source leases became stable in one accepted post-restore frame.
    pub(crate) fn source_presented_after_release(
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        projection: &RehostProjection,
        source_leases: &LeaseBatch,
        accepted_frame: u64,
    ) -> Option<Self> {
        let source_generation = projection.source().leases().first()?.generation();
        let restored_generation = source_leases.leases().first()?.generation();
        if !payload_lease.matches_projection(projection)
            || source_leases.window_id() != payload_lease.source().window_id()
            || source_leases.leases().is_empty()
            || accepted_frame == 0
            || restored_generation == source_generation
            || source_leases.leases().iter().any(|lease| {
                lease.window_id() != source_leases.window_id()
                    || lease.generation() != restored_generation
            })
        {
            return None;
        }
        Some(Self {
            payload_lease,
            evidence: DockLiveUndockSourceRestorationEvidence::AfterRelease {
                lease_generation: restored_generation,
                root_count: source_leases.leases().len(),
                frame_generation: accepted_frame,
            },
        })
    }

    pub(crate) const fn identity(self) -> DockLiveUndockIdentity {
        self.payload_lease.identity()
    }

    pub(crate) const fn source(self) -> DockLiveUndockSourceSnapshot {
        self.payload_lease.source()
    }

    pub(crate) const fn payload_lease(self) -> DockLiveUndockPayloadLeaseReceipt {
        self.payload_lease
    }

    const fn proves_source_unchanged(self) -> bool {
        matches!(
            self.evidence,
            DockLiveUndockSourceRestorationEvidence::Unchanged
        )
    }

    const fn proves_source_presented_after_release(self) -> bool {
        matches!(
            self.evidence,
            DockLiveUndockSourceRestorationEvidence::AfterRelease { .. }
        )
    }

    #[cfg(test)]
    pub(super) const fn source_unchanged_for_test(
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
    ) -> Self {
        Self {
            payload_lease,
            evidence: DockLiveUndockSourceRestorationEvidence::Unchanged,
        }
    }

    #[cfg(test)]
    pub(super) fn source_presented_after_release_for_test(
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        frame_generation: u64,
    ) -> Option<Self> {
        let lease_generation = payload_lease.lease_generation().get().checked_add(1)?;
        if frame_generation == 0 {
            return None;
        }
        Some(Self {
            payload_lease,
            evidence: DockLiveUndockSourceRestorationEvidence::AfterRelease {
                lease_generation,
                root_count: 1,
                frame_generation,
            },
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DockLiveUndockRehostCleanupEvidence {
    Abandoned {
        generation: u64,
        source_window: WindowId,
        destination_window: WindowId,
    },
    AlreadyAbsent {
        generation: u64,
        source_window: WindowId,
        destination_window: WindowId,
    },
    SourceCommitted {
        generation: u64,
        source_window: WindowId,
        destination_window: WindowId,
    },
}

impl DockLiveUndockRehostCleanupEvidence {
    pub(super) const fn abandoned(
        generation: u64,
        source_window: WindowId,
        destination_window: WindowId,
    ) -> Self {
        Self::Abandoned {
            generation,
            source_window,
            destination_window,
        }
    }

    pub(super) const fn already_absent(
        generation: u64,
        source_window: WindowId,
        destination_window: WindowId,
    ) -> Self {
        Self::AlreadyAbsent {
            generation,
            source_window,
            destination_window,
        }
    }

    pub(super) const fn source_committed(
        generation: u64,
        source_window: WindowId,
        destination_window: WindowId,
    ) -> Self {
        Self::SourceCommitted {
            generation,
            source_window,
            destination_window,
        }
    }

    pub(super) const fn authority(self) -> (u64, WindowId, WindowId) {
        match self {
            Self::Abandoned {
                generation,
                source_window,
                destination_window,
            }
            | Self::AlreadyAbsent {
                generation,
                source_window,
                destination_window,
            }
            | Self::SourceCommitted {
                generation,
                source_window,
                destination_window,
            } => (generation, source_window, destination_window),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DockLiveUndockHostCleanupEvidence {
    Abandoned(DockHostLivePresentationCleanupReceipt),
    AlreadyAbsent(WindowId),
    HostUnavailable(WindowId),
}

impl DockLiveUndockHostCleanupEvidence {
    pub(super) const fn abandoned(receipt: DockHostLivePresentationCleanupReceipt) -> Self {
        Self::Abandoned(receipt)
    }

    pub(super) const fn already_absent(window_id: WindowId) -> Self {
        Self::AlreadyAbsent(window_id)
    }

    pub(super) const fn host_unavailable(window_id: WindowId) -> Self {
        Self::HostUnavailable(window_id)
    }

    fn accepts(
        self,
        identity: DockLiveUndockIdentity,
        rehost_generation: u64,
        window_id: WindowId,
    ) -> bool {
        match self {
            Self::Abandoned(receipt) => {
                let key = receipt.key();
                key.identity() == identity
                    && key.rehost_generation() == rehost_generation
                    && key.binding().window_id() == window_id
            }
            Self::AlreadyAbsent(observed) | Self::HostUnavailable(observed) => {
                observed == window_id
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DockLiveUndockRetainedVisualCleanupEvidence {
    Released(TicketIdentity),
    AlreadyReleased(TicketIdentity),
    AuthorityAbsent(TicketIdentity),
    WindowUnavailable(TicketIdentity),
}

impl DockLiveUndockRetainedVisualCleanupEvidence {
    pub(super) const fn ticket(self) -> Option<TicketIdentity> {
        match self {
            Self::Released(ticket)
            | Self::AlreadyReleased(ticket)
            | Self::AuthorityAbsent(ticket)
            | Self::WindowUnavailable(ticket) => Some(ticket),
        }
    }
}

/// Aggregate proof that every pre-commit presentation side effect reached an exact terminal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockOrphanCleanupReceipt {
    identity: DockLiveUndockIdentity,
    payload_lease: DockLiveUndockPayloadLeaseReceipt,
    rehost: Option<DockLiveUndockRehostCleanupEvidence>,
    source_host: Option<DockLiveUndockHostCleanupEvidence>,
    destination_host: Option<DockLiveUndockHostCleanupEvidence>,
    retained_visual: Option<DockLiveUndockRetainedVisualCleanupEvidence>,
    transport: Option<DockNativeCapturedDragTransportRetirementReceipt>,
}

impl DockLiveUndockOrphanCleanupReceipt {
    pub(super) fn new(
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        rehost: DockLiveUndockRehostCleanupEvidence,
        source_host: DockLiveUndockHostCleanupEvidence,
        destination_host: DockLiveUndockHostCleanupEvidence,
        retained_visual: DockLiveUndockRetainedVisualCleanupEvidence,
        transport: DockNativeCapturedDragTransportRetirementReceipt,
    ) -> Option<Self> {
        let identity = payload_lease.identity();
        let (generation, source_window, destination_window) = rehost.authority();
        if generation != payload_lease.rehost_generation()
            || source_window != payload_lease.source().window_id()
            || destination_window != payload_lease.destination_window()
            || !source_host.accepts(identity, generation, source_window)
            || !destination_host.accepts(identity, generation, destination_window)
            || retained_visual.ticket() != payload_lease.retained_visual()
            || transport.key().source_window() != source_window
        {
            return None;
        }
        Some(Self {
            identity,
            payload_lease,
            rehost: Some(rehost),
            source_host: Some(source_host),
            destination_host: Some(destination_host),
            retained_visual: Some(retained_visual),
            transport: Some(transport),
        })
    }

    pub(crate) const fn identity(self) -> DockLiveUndockIdentity {
        self.identity
    }

    pub(crate) const fn payload_lease(self) -> DockLiveUndockPayloadLeaseReceipt {
        self.payload_lease
    }

    #[cfg(test)]
    pub(super) const fn for_test(payload_lease: DockLiveUndockPayloadLeaseReceipt) -> Self {
        let identity = payload_lease.identity();
        Self {
            identity,
            payload_lease,
            rehost: None,
            source_host: None,
            destination_host: None,
            retained_visual: None,
            transport: None,
        }
    }
}

/// Exact acknowledgement of one orphan-topology recovery attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockOrphanRecoveryReceipt {
    identity: DockLiveUndockIdentity,
    payload_lease: DockLiveUndockPayloadLeaseReceipt,
    recovery: DockPayloadRecoveryCommitReceipt,
    cleanup: DockLiveUndockOrphanCleanupReceipt,
}

impl DockLiveUndockOrphanRecoveryReceipt {
    pub(crate) fn new(
        recovery: DockPayloadRecoveryCommitReceipt,
        cleanup: DockLiveUndockOrphanCleanupReceipt,
    ) -> Option<Self> {
        let payload_lease = recovery.authority().presentation()?;
        if cleanup.identity() != recovery.live_identity()
            || cleanup.payload_lease() != payload_lease
        {
            return None;
        }
        Some(Self {
            identity: recovery.live_identity(),
            payload_lease,
            recovery,
            cleanup,
        })
    }

    #[cfg(test)]
    pub(super) fn for_test(recovery: DockPayloadRecoveryCommitReceipt) -> Option<Self> {
        let payload_lease = recovery.authority().presentation()?;
        Some(Self {
            identity: recovery.live_identity(),
            payload_lease,
            recovery,
            cleanup: DockLiveUndockOrphanCleanupReceipt::for_test(payload_lease),
        })
    }

    pub(crate) const fn identity(self) -> DockLiveUndockIdentity {
        self.identity
    }

    pub(crate) const fn payload_lease(self) -> DockLiveUndockPayloadLeaseReceipt {
        self.payload_lease
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockOrphanCleanupFailure {
    PreparationRejected,
    PreflightRejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockCommittedDestinationRecoveryFailure {
    PreparationRejected,
    PreflightRejected,
    Panicked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockShutdownFailure {
    OrphanCleanup(DockLiveUndockOrphanCleanupFailure),
    CommittedDestinationRecovery(DockLiveUndockCommittedDestinationRecoveryFailure),
}

/// Exact acknowledgement that a payload which already crossed the durable promotion boundary
/// was recorded in the surface-owned recovery registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockCommittedDestinationRecoveryReceipt {
    identity: DockLiveUndockIdentity,
    token: DockLiveUndockPromotionToken,
    destination: DockLiveUndockPromotionDestination,
    recovery: DockPayloadRecoveryCommitReceipt,
    same_window_terminal_required: bool,
}

impl DockLiveUndockCommittedDestinationRecoveryReceipt {
    pub(crate) fn new(
        recovery: DockPayloadRecoveryCommitReceipt,
        same_window_terminal_required: bool,
    ) -> Option<Self> {
        let (identity, token, destination) = recovery.authority().promotion()?;
        if same_window_terminal_required
            && !matches!(
                destination,
                DockLiveUndockPromotionDestination::SameWindowDesktop { .. }
            )
        {
            return None;
        }
        Some(Self {
            identity,
            token,
            destination,
            recovery,
            same_window_terminal_required,
        })
    }

    pub(crate) const fn identity(self) -> DockLiveUndockIdentity {
        self.identity
    }

    pub(crate) const fn authority(self) -> DockPayloadRecoveryAuthority {
        self.recovery.authority()
    }

    pub(crate) const fn token(self) -> DockLiveUndockPromotionToken {
        self.token
    }

    pub(crate) const fn destination(self) -> DockLiveUndockPromotionDestination {
        self.destination
    }

    pub(crate) const fn same_window_terminal_required(self) -> bool {
        self.same_window_terminal_required
    }
}

/// Exact accepted source-proxy commit for one payload lease.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockSourceProxyReceipt {
    lease: DockLiveUndockPayloadLeaseReceipt,
    proxy_frame_generation: u64,
}

impl DockLiveUndockSourceProxyReceipt {
    pub(crate) fn new(
        lease: DockLiveUndockPayloadLeaseReceipt,
        rehost: RehostSourceProxyCommit,
    ) -> Option<Self> {
        let replay = rehost.retained_visual_replay()?;
        if Some(replay.ticket()) != lease.retained_visual || rehost.frame_generation() == 0 {
            return None;
        }
        Some(Self {
            lease,
            proxy_frame_generation: rehost.frame_generation(),
        })
    }

    pub(crate) const fn lease(self) -> DockLiveUndockPayloadLeaseReceipt {
        self.lease
    }

    pub(crate) const fn proxy_frame_generation(self) -> u64 {
        self.proxy_frame_generation
    }

    #[cfg(test)]
    pub(super) fn for_test(
        lease: DockLiveUndockPayloadLeaseReceipt,
        proxy_frame_generation: u64,
    ) -> Option<Self> {
        if proxy_frame_generation == 0 {
            return None;
        }
        Some(Self {
            lease,
            proxy_frame_generation,
        })
    }
}

/// Exact destination mount and exposure that consumed one source-proxy receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockPayloadMountReceipt {
    proxy: DockLiveUndockSourceProxyReceipt,
    mount_frame_generation: u64,
    destination_lease_generation: u64,
    root_count: usize,
}

impl DockLiveUndockPayloadMountReceipt {
    pub(crate) fn new(
        proxy: DockLiveUndockSourceProxyReceipt,
        rehost: &RehostDestinationExposure,
    ) -> Option<Self> {
        let batch = rehost.batch();
        let destination_lease_generation = batch.leases().first()?.generation();
        let root_count = batch.leases().len();
        if batch.window_id() != proxy.lease.destination_window
            || destination_lease_generation != proxy.lease.lease_generation.get()
            || rehost.frame_generation() == 0
            || root_count == 0
            || batch.leases().iter().any(|lease| {
                lease.window_id() != batch.window_id()
                    || lease.generation() != destination_lease_generation
            })
        {
            return None;
        }
        Some(Self {
            proxy,
            mount_frame_generation: rehost.frame_generation(),
            destination_lease_generation,
            root_count,
        })
    }

    pub(crate) const fn proxy(self) -> DockLiveUndockSourceProxyReceipt {
        self.proxy
    }

    pub(crate) const fn window_id(self) -> WindowId {
        self.proxy.lease.destination_window
    }

    pub(crate) const fn mount_frame_generation(self) -> u64 {
        self.mount_frame_generation
    }

    pub(crate) const fn destination_lease_generation(self) -> u64 {
        self.destination_lease_generation
    }

    pub(crate) const fn root_count(self) -> usize {
        self.root_count
    }

    #[cfg(test)]
    pub(super) fn for_test(
        proxy: DockLiveUndockSourceProxyReceipt,
        mount_frame_generation: u64,
    ) -> Option<Self> {
        if mount_frame_generation == 0 {
            return None;
        }
        Some(Self {
            proxy,
            mount_frame_generation,
            destination_lease_generation: proxy.lease.lease_generation.get(),
            root_count: 1,
        })
    }
}

/// Exact visible candidate frame for the exposed payload batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockPayloadPresentationReceipt {
    mount: DockLiveUndockPayloadMountReceipt,
    frame_generation: u64,
    rehost: Option<RehostDestinationPresentation>,
}

impl DockLiveUndockPayloadPresentationReceipt {
    pub(crate) fn new(
        mount: DockLiveUndockPayloadMountReceipt,
        rehost: RehostDestinationPresentation,
    ) -> Option<Self> {
        if rehost.window_id() != mount.window_id()
            || rehost.lease_generation() != mount.destination_lease_generation
            || rehost.root_count() != mount.root_count
            || rehost.frame_generation() == 0
        {
            return None;
        }
        Some(Self {
            mount,
            frame_generation: rehost.frame_generation(),
            rehost: Some(rehost),
        })
    }

    pub(crate) const fn mount(self) -> DockLiveUndockPayloadMountReceipt {
        self.mount
    }

    pub(crate) const fn window_id(self) -> WindowId {
        self.mount.window_id()
    }

    pub(crate) const fn frame_generation(self) -> u64 {
        self.frame_generation
    }

    pub(crate) const fn rehost_presentation(self) -> Option<RehostDestinationPresentation> {
        self.rehost
    }

    #[cfg(test)]
    pub(super) fn for_test(
        mount: DockLiveUndockPayloadMountReceipt,
        frame_generation: u64,
    ) -> Option<Self> {
        if frame_generation == 0 {
            return None;
        }
        Some(Self {
            mount,
            frame_generation,
            rehost: None,
        })
    }
}

/// Exact native reveal joined to an exact payload presentation in the same submitted frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockRevealReceipt {
    preflight: DockLiveUndockPayloadPresentationReceipt,
    reveal_frame: DockLiveUndockPayloadPresentationReceipt,
    provisional: Option<WindowProvisionalRevealSnapshot>,
}

impl DockLiveUndockRevealReceipt {
    pub(crate) fn new(
        preflight: DockLiveUndockPayloadPresentationReceipt,
        reveal_frame: DockLiveUndockPayloadPresentationReceipt,
        provisional: WindowProvisionalRevealSnapshot,
    ) -> Option<Self> {
        let native = provisional.native_facts()?;
        if preflight.mount != reveal_frame.mount
            || reveal_frame.frame_generation <= preflight.frame_generation
            || provisional.window_id() != reveal_frame.window_id()
            || provisional.session_generation()
                != reveal_frame
                    .mount
                    .proxy
                    .lease
                    .provisional_session_generation
            || provisional.presentation_generation() != Some(reveal_frame.frame_generation)
            || provisional.outcome() != WindowProvisionalRevealOutcome::Revealed
            || !native.accepts_reveal()
            || native.z_order() == WindowProvisionalRevealZOrder::Unavailable
        {
            return None;
        }
        Some(Self {
            preflight,
            reveal_frame,
            provisional: Some(provisional),
        })
    }

    pub(crate) const fn preflight(self) -> DockLiveUndockPayloadPresentationReceipt {
        self.preflight
    }

    pub(crate) const fn reveal_frame(self) -> DockLiveUndockPayloadPresentationReceipt {
        self.reveal_frame
    }

    #[cfg(test)]
    pub(super) fn for_test(
        preflight: DockLiveUndockPayloadPresentationReceipt,
        reveal_frame: DockLiveUndockPayloadPresentationReceipt,
    ) -> Option<Self> {
        if preflight.mount != reveal_frame.mount
            || reveal_frame.frame_generation <= preflight.frame_generation
        {
            return None;
        }
        Some(Self {
            preflight,
            reveal_frame,
            provisional: None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockRevealObservation {
    Visible(DockLiveUndockRevealReceipt),
    Failed {
        presentation: DockLiveUndockPayloadPresentationReceipt,
        outcome: DockLiveUndockRevealOutcome,
    },
}

impl DockLiveUndockRevealObservation {
    pub(crate) const fn presentation(self) -> DockLiveUndockPayloadPresentationReceipt {
        match self {
            Self::Visible(receipt) => receipt.reveal_frame,
            Self::Failed { presentation, .. } => presentation,
        }
    }

    pub(crate) const fn failed(
        presentation: DockLiveUndockPayloadPresentationReceipt,
        outcome: DockLiveUndockRevealOutcome,
    ) -> Self {
        Self::Failed {
            presentation,
            outcome,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockOpeningBinding {
    ExactGated,
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockPlacementOutcome {
    Exact,
    Adjusted,
    Superseded,
    Rejected,
    WindowClosed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockRoutePlacementOutcome {
    Exact,
    Adjusted,
    Superseded,
    Rejected,
    Unsupported,
    WindowClosed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockFinalPlacementReceipt {
    window_id: WindowId,
    session_generation: u64,
    generation: DockLiveUndockPlacementGeneration,
    point: DockLiveUndockPhysicalPoint,
    bounds: DockLiveUndockPhysicalBounds,
    z_order: WindowProvisionalRevealZOrder,
}

impl DockLiveUndockFinalPlacementReceipt {
    #[cfg(test)]
    pub(crate) fn for_test(
        identity: DockLiveUndockIdentity,
        window_id: WindowId,
        release: DockLiveUndockReleaseLock,
    ) -> Self {
        Self {
            window_id,
            session_generation: identity.opening().generation(),
            generation: release.placement_generation,
            point: release.point,
            bounds: release.desired_bounds,
            z_order: WindowProvisionalRevealZOrder::Exact,
        }
    }

    pub(crate) fn new(snapshot: WindowProvisionalPlacementSnapshot) -> Option<Self> {
        let native = snapshot.native_facts()?;
        if snapshot.purpose() != WindowProvisionalPlacementPurpose::FinalRelease
            || snapshot.outcome() != WindowProvisionalPlacementOutcome::Settled
            || !native.accepts_placement()
        {
            return None;
        }
        let client_bounds = snapshot.client_bounds();
        let width = u32::try_from(client_bounds.size.width.0).ok()?;
        let height = u32::try_from(client_bounds.size.height.0).ok()?;
        Some(Self {
            window_id: snapshot.window_id(),
            session_generation: snapshot.session_generation(),
            generation: DockLiveUndockPlacementGeneration::new(snapshot.placement_generation())?,
            point: DockLiveUndockPhysicalPoint::new(
                snapshot.anchor_point().x.0,
                snapshot.anchor_point().y.0,
            ),
            bounds: DockLiveUndockPhysicalBounds::for_display(
                DockLiveUndockPhysicalPoint::new(
                    client_bounds.origin.x.0,
                    client_bounds.origin.y.0,
                ),
                width,
                height,
                snapshot.target_display(),
            )?,
            z_order: native.z_order(),
        })
    }

    pub(crate) fn matches(
        self,
        identity: DockLiveUndockIdentity,
        window_id: WindowId,
        release: DockLiveUndockReleaseLock,
    ) -> bool {
        self.window_id == window_id
            && self.session_generation == identity.opening().generation()
            && self.generation == release.placement_generation
            && self.point == release.point
            && self.bounds == release.desired_bounds
            && !matches!(self.z_order, WindowProvisionalRevealZOrder::Unavailable)
    }
}

impl DockLiveUndockPlacementOutcome {
    const fn admits_promotion(self) -> bool {
        matches!(self, Self::Exact | Self::Adjusted)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockRevealOutcome {
    Rejected,
    NativeObservationMissing,
    ObservationDeadlineExpired,
    Stale,
    WindowTerminal,
}

/// Fallible presentation-pipeline stage that failed before the durable Dock swap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockPresentationFailure {
    PayloadLeaseClaim,
    RetainedVisualTicket,
    RehostPreparation,
    SourceProxyReplay {
        lease: DockLiveUndockPayloadLeaseReceipt,
    },
    DestinationExposureFinish {
        proxy: DockLiveUndockSourceProxyReceipt,
    },
    PayloadPresentationObservation {
        mount: DockLiveUndockPayloadMountReceipt,
    },
    ExactRevealTicket {
        presentation: DockLiveUndockPayloadPresentationReceipt,
    },
}

/// A recoverable failure while returning the live payload to its source presentation.
///
/// None of these failures prove that the source window is terminal. The runtime must retain the
/// restoration obligation and retry until it either commits an exact restoration receipt or
/// receives an exact native-window terminal receipt for the source generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockSourceRestorationFailure {
    ExecutionAuthorityUnavailable,
    PresentationTransitionRejected,
    RestorationReceiptUnavailable,
    SourcePresentationMutationRejected,
    RetainedVisualReplayRejected,
    AwaitingSourceNativeTerminal,
}

impl DockLiveUndockSourceRestorationFailure {
    pub(crate) const fn schedules_timer_retry(self) -> bool {
        !matches!(self, Self::AwaitingSourceNativeTerminal)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockCancelReason {
    Escape,
    CaptureLost,
    SourceClosed,
    PayloadClosed,
    SourceDeactivated,
}

impl DockLiveUndockCancelReason {
    const fn restore_focus(self) -> bool {
        !matches!(self, Self::SourceClosed | Self::SourceDeactivated)
    }

    const fn aborts_after_release_before_commit(self) -> bool {
        matches!(
            self,
            Self::SourceClosed | Self::PayloadClosed | Self::SourceDeactivated
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockRestoreReason {
    OpeningBindingInvalid,
    RuntimeRegistrationRejected,
    PresentationFailed(DockLiveUndockPresentationFailure),
    RevealFailed(DockLiveUndockRevealOutcome),
    RoutePlacementFailed(DockLiveUndockRoutePlacementOutcome),
    PlacementFailed(DockLiveUndockPlacementOutcome),
    ReleaseDeadlineExpired,
    PromotionPreparationFailed,
    DestinationTerminalBeforeCommit,
    ProvisionalTerminal,
    ForeignSurface,
    RouteUnavailable,
    Cancelled(DockLiveUndockCancelReason),
    Shutdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockPromotionDestination {
    SameWindowDesktop { window_id: WindowId },
    Host(DockLiveUndockHostTarget),
}

impl DockLiveUndockPromotionDestination {
    pub(crate) const fn window_id(self) -> WindowId {
        match self {
            Self::SameWindowDesktop { window_id } => window_id,
            Self::Host(target) => target.window_id,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct DockLiveUndockPromotionToken(u64);

impl DockLiveUndockPromotionToken {
    pub(crate) const fn new(token: u64) -> Option<Self> {
        if token == 0 { None } else { Some(Self(token)) }
    }

    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

/// Runtime evidence used to linearize a surface shutdown against a promotion commit.
///
/// A reversible commit may be claimed for rollback exactly once. A forward-only commit must keep
/// the reducer waiting until the runtime publishes a durable outcome; only an already durable
/// runtime execution is committed-loss authority during shutdown.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockPromotionCommitDisposition {
    RollbackAllowed,
    AbortClaimed,
    ForwardOnly {
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
    },
    Durable {
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
    },
}

impl DockLiveUndockPromotionCommitDisposition {
    fn durable_for(
        self,
        identity: DockLiveUndockIdentity,
    ) -> Option<(
        DockLiveUndockPromotionToken,
        DockLiveUndockPromotionDestination,
    )> {
        match self {
            Self::Durable {
                identity: current,
                token,
                destination,
            } if current == identity => Some((token, destination)),
            Self::RollbackAllowed
            | Self::AbortClaimed
            | Self::ForwardOnly { .. }
            | Self::Durable { .. } => None,
        }
    }

    fn forward_only_for(
        self,
        identity: DockLiveUndockIdentity,
    ) -> Option<(
        DockLiveUndockPromotionToken,
        DockLiveUndockPromotionDestination,
    )> {
        match self {
            Self::ForwardOnly {
                identity: current,
                token,
                destination,
            } if current == identity => Some((token, destination)),
            Self::RollbackAllowed
            | Self::AbortClaimed
            | Self::ForwardOnly { .. }
            | Self::Durable { .. } => None,
        }
    }
}

/// Exact destination-semantics proof for one durable promotion.
///
/// Same-window receipts retain the renderer-submitted provisional frame. Host receipts retain the
/// exact already-published host scene selected by the locked drop route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockDestinationSemanticsReceipt {
    identity: DockLiveUndockIdentity,
    token: DockLiveUndockPromotionToken,
    destination: DockLiveUndockPromotionDestination,
    reveal: Option<DockLiveUndockRevealReceipt>,
    provisional: Option<WindowProvisionalSemanticsSnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockPreparedDestinationSemanticsReceipt {
    identity: DockLiveUndockIdentity,
    token: DockLiveUndockPromotionToken,
    reveal: DockLiveUndockRevealReceipt,
    prior: WindowProvisionalSemanticsSnapshot,
    frame_generation: u64,
}

impl DockLiveUndockDestinationSemanticsReceipt {
    pub(crate) fn new_host(
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
        target: DockLiveUndockHostTarget,
    ) -> Option<Self> {
        (target.host_scene_generation() != 0).then_some(Self {
            identity,
            token,
            destination: DockLiveUndockPromotionDestination::Host(target),
            reveal: None,
            provisional: None,
        })
    }

    pub(crate) fn prepare_same_window(
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
        reveal: DockLiveUndockRevealReceipt,
        provisional: WindowProvisionalSemanticsSnapshot,
        frame_generation: u64,
    ) -> Option<DockLiveUndockPreparedDestinationSemanticsReceipt> {
        let payload_lease = reveal.reveal_frame().mount().proxy().lease();
        let window_id = reveal.reveal_frame().window_id();
        let accepts_first_frame = provisional.outcome()
            == WindowProvisionalSemanticsOutcome::Pending
            && provisional.accepted_frame_generation().is_none();
        let replaces_unsubmitted_frame = provisional.outcome()
            == WindowProvisionalSemanticsOutcome::Accepted
            && provisional
                .accepted_frame_generation()
                .is_some_and(|accepted| frame_generation > accepted);
        if payload_lease.identity() != identity
            || provisional.window_id() != window_id
            || provisional.session_generation() != payload_lease.provisional_session_generation()
            || provisional.destination_generation() != token.get()
            || !(accepts_first_frame || replaces_unsubmitted_frame)
            || provisional.submitted_frame_generation().is_some()
            || frame_generation < provisional.minimum_frame_generation()
            || frame_generation <= reveal.reveal_frame().frame_generation()
        {
            return None;
        }
        Some(DockLiveUndockPreparedDestinationSemanticsReceipt {
            identity,
            token,
            reveal,
            prior: provisional,
            frame_generation,
        })
    }

    pub(crate) fn new_same_window_submitted(
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
        reveal: DockLiveUndockRevealReceipt,
        submitted: WindowProvisionalSemanticsSnapshot,
    ) -> Option<Self> {
        let payload_lease = reveal.reveal_frame().mount().proxy().lease();
        let submitted_frame_generation = submitted.submitted_frame_generation()?;
        if payload_lease.identity() != identity
            || submitted.window_id() != reveal.reveal_frame().window_id()
            || submitted.session_generation() != payload_lease.provisional_session_generation()
            || submitted.destination_generation() != token.get()
            || submitted.outcome() != WindowProvisionalSemanticsOutcome::Submitted
            || submitted.accepted_frame_generation() != Some(submitted_frame_generation)
            || submitted_frame_generation < submitted.minimum_frame_generation()
            || submitted_frame_generation <= reveal.reveal_frame().frame_generation()
        {
            return None;
        }
        Some(Self {
            identity,
            token,
            destination: DockLiveUndockPromotionDestination::SameWindowDesktop {
                window_id: submitted.window_id(),
            },
            reveal: Some(reveal),
            provisional: Some(submitted),
        })
    }

    pub(crate) const fn identity(self) -> DockLiveUndockIdentity {
        self.identity
    }

    pub(crate) const fn token(self) -> DockLiveUndockPromotionToken {
        self.token
    }

    pub(crate) const fn destination(self) -> DockLiveUndockPromotionDestination {
        self.destination
    }

    pub(crate) const fn payload_lease(self) -> Option<DockLiveUndockPayloadLeaseReceipt> {
        match self.reveal {
            Some(reveal) => Some(reveal.reveal_frame.mount.proxy.lease),
            None => None,
        }
    }

    pub(crate) const fn submitted_frame_generation(self) -> Option<u64> {
        match self.provisional {
            Some(provisional) => provisional.submitted_frame_generation(),
            None => None,
        }
    }

    #[cfg(test)]
    pub(super) const fn for_test(
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
    ) -> Self {
        Self {
            identity,
            token,
            destination,
            reveal: None,
            provisional: None,
        }
    }
}

impl DockLiveUndockPreparedDestinationSemanticsReceipt {
    pub(crate) fn accepts(self, accepted: WindowProvisionalSemanticsSnapshot) -> bool {
        self.reveal
            .reveal_frame()
            .mount()
            .proxy()
            .lease()
            .identity()
            == self.identity
            && accepted.window_id() == self.reveal.reveal_frame().window_id()
            && accepted.window_id() == self.prior.window_id()
            && accepted.session_generation() == self.prior.session_generation()
            && accepted.destination_generation() == self.prior.destination_generation()
            && accepted.destination_generation() == self.token.get()
            && accepted.minimum_frame_generation() == self.prior.minimum_frame_generation()
            && accepted.placement_mutation_generation()
                == self.prior.placement_mutation_generation()
            && accepted.accepted_frame_generation() == Some(self.frame_generation)
            && accepted.submitted_frame_generation().is_none()
            && accepted.outcome() == WindowProvisionalSemanticsOutcome::Accepted
    }
}

/// Exact proof that the destination interaction gate opened for submitted semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockLiveUndockDestinationInteractionReceipt {
    semantics: DockLiveUndockDestinationSemanticsReceipt,
    admitted_session_generation: Option<u64>,
}

impl DockLiveUndockDestinationInteractionReceipt {
    pub(crate) fn new_host(semantics: DockLiveUndockDestinationSemanticsReceipt) -> Option<Self> {
        matches!(
            semantics.destination,
            DockLiveUndockPromotionDestination::Host(_)
        )
        .then_some(Self {
            semantics,
            admitted_session_generation: None,
        })
    }

    pub(crate) fn new_same_window(
        semantics: DockLiveUndockDestinationSemanticsReceipt,
        provisional_session: &WindowProvisionalSession,
    ) -> Option<Self> {
        if !matches!(
            semantics.destination,
            DockLiveUndockPromotionDestination::SameWindowDesktop { .. }
        ) {
            return None;
        }
        let projected = semantics.provisional?;
        let session = provisional_session.snapshot();
        if session.window_id() != Some(semantics.destination.window_id())
            || session.generation() != projected.session_generation()
            || session.phase() != WindowProvisionalSessionPhase::Promoted
            || projected.outcome() != WindowProvisionalSemanticsOutcome::Submitted
            || projected.accepted_frame_generation() != projected.submitted_frame_generation()
            || projected.submitted_frame_generation().is_none()
        {
            return None;
        }
        Some(Self {
            semantics,
            admitted_session_generation: Some(session.generation()),
        })
    }

    pub(crate) const fn semantics(self) -> DockLiveUndockDestinationSemanticsReceipt {
        self.semantics
    }

    pub(crate) const fn admitted_session_generation(self) -> Option<u64> {
        self.admitted_session_generation
    }

    #[cfg(test)]
    pub(super) const fn for_test(semantics: DockLiveUndockDestinationSemanticsReceipt) -> Self {
        Self {
            semantics,
            admitted_session_generation: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockTerminalResult {
    Restored(DockLiveUndockRestoreReason),
    SourceLostBeforeCommit,
    PresentationAuthorityLostBeforeCommit(DockLiveUndockPresentationAuthorityLoss),
    Committed(DockLiveUndockPromotionDestination),
    DestinationLostAfterCommit(DockLiveUndockPromotionDestination),
    ShutdownCleanupFailed(DockLiveUndockShutdownFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockRetirementReason {
    OpeningBindingInvalid,
    RuntimeRegistrationRejected,
    SourceRestored(DockLiveUndockRestoreReason),
    SourceLost,
    PresentationAuthorityLost,
    HostCommitted,
    HostDestinationSelected,
    PresentationUnavailable,
    Shutdown,
}

#[derive(Clone, Debug)]
pub(crate) enum DockLiveUndockFact {
    Trigger {
        lease: DockSurfaceWindowSessionLease,
        trigger: DockLiveUndockTrigger,
    },
    RouteObserved {
        identity: DockLiveUndockIdentity,
        generation: DockLiveUndockRouteGeneration,
        route: DockLiveUndockRouteFeedback,
        point: DockLiveUndockPhysicalPoint,
        bounds: DockLiveUndockPhysicalBounds,
    },
    OpeningReturned {
        identity: DockLiveUndockIdentity,
        window: AnyWindowHandle,
        binding: DockLiveUndockOpeningBinding,
        runtime: DockViewportProvisionalOpenAttemptCompletion,
    },
    OpeningFailed {
        identity: DockLiveUndockIdentity,
    },
    PresentationStageFailed {
        identity: DockLiveUndockIdentity,
        failure: DockLiveUndockPresentationFailure,
    },
    PresentationLeaseActivated {
        identity: DockLiveUndockIdentity,
        receipt: DockLiveUndockPayloadLeaseReceipt,
    },
    SourceProxyCommitted {
        identity: DockLiveUndockIdentity,
        receipt: DockLiveUndockSourceProxyReceipt,
    },
    PayloadMounted {
        identity: DockLiveUndockIdentity,
        receipt: DockLiveUndockPayloadMountReceipt,
    },
    PayloadPresented {
        identity: DockLiveUndockIdentity,
        receipt: DockLiveUndockPayloadPresentationReceipt,
    },
    InitialPresentationReady {
        identity: DockLiveUndockIdentity,
        presentation: DockLiveUndockPayloadPresentationReceipt,
    },
    RevealObserved {
        identity: DockLiveUndockIdentity,
        observation: DockLiveUndockRevealObservation,
    },
    PlacementObserved {
        identity: DockLiveUndockIdentity,
        window_id: WindowId,
        generation: DockLiveUndockPlacementGeneration,
        outcome: DockLiveUndockPlacementOutcome,
        final_placement: Option<DockLiveUndockFinalPlacementReceipt>,
    },
    RoutePlacementObserved {
        identity: DockLiveUndockIdentity,
        window_id: WindowId,
        generation: DockLiveUndockRouteGeneration,
        outcome: DockLiveUndockRoutePlacementOutcome,
    },
    ReleaseLocked {
        identity: DockLiveUndockIdentity,
        release: DockLiveUndockReleaseLock,
    },
    ReleaseDeadlineExpired {
        identity: DockLiveUndockIdentity,
        placement_generation: DockLiveUndockPlacementGeneration,
    },
    PromotionPrepared {
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
    },
    PromotionPreparationFailed {
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
    },
    DurableSwapCommitted {
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
    },
    CommittedDestinationRecoveryRequired {
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
    },
    DestinationSemanticsSubmitted {
        identity: DockLiveUndockIdentity,
        receipt: DockLiveUndockDestinationSemanticsReceipt,
    },
    DestinationSemanticsSubmissionFailed {
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
    },
    DestinationInteractionAdmitted {
        identity: DockLiveUndockIdentity,
        receipt: DockLiveUndockDestinationInteractionReceipt,
    },
    DestinationInteractionAdmissionFailed {
        identity: DockLiveUndockIdentity,
        semantics: DockLiveUndockDestinationSemanticsReceipt,
    },
    SourceRestorationCommitted {
        identity: DockLiveUndockIdentity,
        receipt: DockLiveUndockSourceRestorationReceipt,
    },
    SourceRestorationDeferred {
        identity: DockLiveUndockIdentity,
        source: DockLiveUndockSourceSnapshot,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
    },
    SourceRestorationRetryElapsed {
        identity: DockLiveUndockIdentity,
        source: DockLiveUndockSourceSnapshot,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
    },
    SourceWindowNativeTerminal {
        receipt: DockLiveUndockSourceNativeTerminalReceipt,
    },
    PresentationAuthorityLost {
        receipt: DockLiveUndockPresentationAuthorityLossReceipt,
    },
    OrphanRecoveryCommitted {
        identity: DockLiveUndockIdentity,
        receipt: DockLiveUndockOrphanRecoveryReceipt,
    },
    OrphanRecoveryFailed {
        identity: DockLiveUndockIdentity,
        receipt: DockLiveUndockOrphanRecoveryReceipt,
    },
    ShutdownOrphanCleanupCompleted {
        receipt: DockLiveUndockOrphanCleanupReceipt,
    },
    ShutdownOrphanCleanupFailed {
        identity: DockLiveUndockIdentity,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        failure: DockLiveUndockOrphanCleanupFailure,
    },
    CommittedDestinationRecoveryCommitted {
        identity: DockLiveUndockIdentity,
        receipt: DockLiveUndockCommittedDestinationRecoveryReceipt,
    },
    ShutdownCommittedDestinationRecoveryFailed {
        identity: DockLiveUndockIdentity,
        authority: DockPayloadRecoveryAuthority,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
        failure: DockLiveUndockCommittedDestinationRecoveryFailure,
    },
    DestinationTerminal {
        identity: DockLiveUndockIdentity,
        window_id: WindowId,
    },
    Cancel {
        identity: DockLiveUndockIdentity,
        reason: DockLiveUndockCancelReason,
    },
    ShutdownRequested {
        lease: DockSurfaceWindowSessionLease,
    },
    ShutdownDependencyTransferred {
        identity: DockLiveUndockIdentity,
        dependency: DockSurfaceWindowSessionDependencyId,
    },
    WindowTerminal {
        identity: DockLiveUndockIdentity,
        window_id: WindowId,
    },
}

impl DockLiveUndockFact {
    const fn identity(&self) -> Option<DockLiveUndockIdentity> {
        match self {
            Self::Trigger { .. } | Self::ShutdownRequested { .. } => None,
            Self::RouteObserved { identity, .. }
            | Self::OpeningReturned { identity, .. }
            | Self::OpeningFailed { identity }
            | Self::PresentationStageFailed { identity, .. }
            | Self::PresentationLeaseActivated { identity, .. }
            | Self::SourceProxyCommitted { identity, .. }
            | Self::PayloadMounted { identity, .. }
            | Self::PayloadPresented { identity, .. }
            | Self::InitialPresentationReady { identity, .. }
            | Self::RevealObserved { identity, .. }
            | Self::PlacementObserved { identity, .. }
            | Self::RoutePlacementObserved { identity, .. }
            | Self::ReleaseLocked { identity, .. }
            | Self::ReleaseDeadlineExpired { identity, .. }
            | Self::PromotionPrepared { identity, .. }
            | Self::PromotionPreparationFailed { identity, .. }
            | Self::DurableSwapCommitted { identity, .. }
            | Self::CommittedDestinationRecoveryRequired { identity, .. }
            | Self::DestinationSemanticsSubmitted { identity, .. }
            | Self::DestinationSemanticsSubmissionFailed { identity, .. }
            | Self::DestinationInteractionAdmitted { identity, .. }
            | Self::DestinationInteractionAdmissionFailed { identity, .. }
            | Self::SourceRestorationCommitted { identity, .. }
            | Self::SourceRestorationDeferred { identity, .. }
            | Self::SourceRestorationRetryElapsed { identity, .. }
            | Self::OrphanRecoveryCommitted { identity, .. }
            | Self::OrphanRecoveryFailed { identity, .. }
            | Self::ShutdownOrphanCleanupFailed { identity, .. }
            | Self::CommittedDestinationRecoveryCommitted { identity, .. }
            | Self::ShutdownCommittedDestinationRecoveryFailed { identity, .. }
            | Self::DestinationTerminal { identity, .. }
            | Self::Cancel { identity, .. }
            | Self::ShutdownDependencyTransferred { identity, .. }
            | Self::WindowTerminal { identity, .. } => Some(*identity),
            Self::SourceWindowNativeTerminal { receipt } => Some(receipt.identity()),
            Self::PresentationAuthorityLost { receipt } => Some(receipt.identity()),
            Self::ShutdownOrphanCleanupCompleted { receipt } => Some(receipt.identity()),
        }
    }
}

/// Pure intents returned after the reducer has released all internal state borrows.
#[derive(Clone, Debug)]
pub(crate) enum DockLiveUndockEffect {
    /// The generation was not consumed. Retry only while that drag transport is still moving;
    /// after its terminal fact, the established locked-drop path remains authoritative.
    TriggerDeferred {
        drag_generation: DockLiveUndockDragGeneration,
    },
    OpenProvisional {
        identity: DockLiveUndockIdentity,
        request: DockLiveUndockOpenRequest,
    },
    RouteFeedbackChanged {
        identity: DockLiveUndockIdentity,
        route: DockLiveUndockRouteFeedback,
    },
    ProvisionalAdmitted {
        identity: DockLiveUndockIdentity,
        window: AnyWindowHandle,
        lease: DockSurfaceWindowSessionLease,
    },
    OpeningFailed {
        identity: DockLiveUndockIdentity,
        dependency: Option<DockSurfaceWindowSessionDependencyId>,
    },
    ProvisionalRetirementRequired {
        identity: DockLiveUndockIdentity,
        window: Option<AnyWindowHandle>,
        dependency: Option<DockSurfaceWindowSessionDependencyId>,
        binding: Option<DockLiveUndockOpeningBinding>,
        runtime: Option<DockViewportProvisionalOpenAttemptCompletion>,
        reason: DockLiveUndockRetirementReason,
    },
    CommitSourceProxy {
        identity: DockLiveUndockIdentity,
        lease: DockLiveUndockPayloadLeaseReceipt,
    },
    MountAndExposePayload {
        identity: DockLiveUndockIdentity,
        proxy: DockLiveUndockSourceProxyReceipt,
        window: AnyWindowHandle,
    },
    ObservePayloadPresentation {
        identity: DockLiveUndockIdentity,
        mount: DockLiveUndockPayloadMountReceipt,
        window: AnyWindowHandle,
    },
    ArmExactReveal {
        identity: DockLiveUndockIdentity,
        presentation: DockLiveUndockPayloadPresentationReceipt,
        window: AnyWindowHandle,
        point: DockLiveUndockPhysicalPoint,
        bounds: DockLiveUndockPhysicalBounds,
    },
    RequestRoutePlacement {
        identity: DockLiveUndockIdentity,
        window: AnyWindowHandle,
        generation: DockLiveUndockRouteGeneration,
        point: DockLiveUndockPhysicalPoint,
        bounds: DockLiveUndockPhysicalBounds,
    },
    RetireFrozenSourceVisual {
        identity: DockLiveUndockIdentity,
        reveal: DockLiveUndockRevealReceipt,
    },
    RetireSourceTransportProxy {
        identity: DockLiveUndockIdentity,
    },
    RequestReleasePlacement {
        identity: DockLiveUndockIdentity,
        window: AnyWindowHandle,
        release: DockLiveUndockReleaseLock,
    },
    PreparePromotion {
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
        release: DockLiveUndockReleaseLock,
    },
    /// The executor must perform only the already-prepared, infallible state swap.
    CommitPreparedPromotion {
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
    },
    /// Applies close/refresh work only after the reducer has recorded the Host swap as durable.
    ApplyCommittedHostWindowEffects {
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
    },
    DestinationSemanticsSubmissionRequired {
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
    },
    DestinationInteractionAdmissionRequired {
        identity: DockLiveUndockIdentity,
        semantics: DockLiveUndockDestinationSemanticsReceipt,
    },
    DestinationInteractionReady {
        identity: DockLiveUndockIdentity,
        interaction: DockLiveUndockDestinationInteractionReceipt,
        destination: DockLiveUndockPromotionDestination,
    },
    RestoreSource {
        identity: DockLiveUndockIdentity,
        source: DockLiveUndockSourceSnapshot,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        restore_focus: bool,
    },
    RestoreSourceFocus {
        identity: DockLiveUndockIdentity,
        source: DockLiveUndockSourceSnapshot,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
    },
    RecoverOrphanedPayloadTopology {
        identity: DockLiveUndockIdentity,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        provisional: Option<AnyWindowHandle>,
    },
    ShutdownSourceRestorationRequired {
        identity: DockLiveUndockIdentity,
        source: DockLiveUndockSourceSnapshot,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
    },
    ShutdownOrphanRecoveryRequired {
        identity: DockLiveUndockIdentity,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        provisional: Option<AnyWindowHandle>,
    },
    RecoverCommittedDestinationTopology {
        identity: DockLiveUndockIdentity,
        authority: DockPayloadRecoveryAuthority,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
    },
    ShutdownCommittedDestinationRecoveryRequired {
        identity: DockLiveUndockIdentity,
        authority: DockPayloadRecoveryAuthority,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
    },
    RetireCommittedSameWindowDestination {
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
        window_id: WindowId,
    },
    PublishTerminal {
        identity: DockLiveUndockIdentity,
        result: DockLiveUndockTerminalResult,
    },
    ShutdownFrozen(DockLiveUndockShutdownSnapshot),
    ShutdownDependencyTransferred {
        identity: DockLiveUndockIdentity,
        dependency: DockSurfaceWindowSessionDependencyId,
    },
    SettleShutdownDependency {
        identity: DockLiveUndockIdentity,
        dependency: DockSurfaceWindowSessionDependencyId,
    },
    FailShutdownDependency {
        identity: DockLiveUndockIdentity,
        dependency: DockSurfaceWindowSessionDependencyId,
        failure: DockLiveUndockShutdownFailure,
    },
    WindowTerminalSettled(DockLiveUndockWindowTerminalOutcome),
}

#[derive(Debug, Default)]
pub(crate) struct DockLiveUndockEffects(Vec<DockLiveUndockEffect>);

impl DockLiveUndockEffects {
    pub(super) fn single(effect: DockLiveUndockEffect) -> Self {
        Self(vec![effect])
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn as_slice(&self) -> &[DockLiveUndockEffect] {
        &self.0
    }

    fn push(&mut self, effect: DockLiveUndockEffect) {
        self.0.push(effect);
    }
}

#[derive(Debug)]
pub(crate) struct DockLiveUndockTransition<T> {
    outcome: T,
    effects: DockLiveUndockEffects,
}

impl<T> DockLiveUndockTransition<T> {
    fn new(outcome: T, effects: DockLiveUndockEffects) -> Self {
        Self { outcome, effects }
    }

    pub(crate) fn into_parts(self) -> (T, DockLiveUndockEffects) {
        (self.outcome, self.effects)
    }

    pub(crate) fn outcome(&self) -> &T {
        &self.outcome
    }
}

impl IntoIterator for DockLiveUndockEffects {
    type Item = DockLiveUndockEffect;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(Clone, Debug)]
struct DockLiveUndockOpening {
    identity: DockLiveUndockIdentity,
    provisional_session: WindowProvisionalSession,
}

impl DockLiveUndockOpening {
    fn binding(&self, window_id: WindowId) -> DockLiveUndockOpeningBinding {
        let snapshot = self.provisional_session.snapshot();
        if snapshot.generation() == self.identity.opening.generation
            && snapshot.window_id() == Some(window_id)
            && snapshot.phase() == WindowProvisionalSessionPhase::Gated
        {
            DockLiveUndockOpeningBinding::ExactGated
        } else {
            DockLiveUndockOpeningBinding::Invalid
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockLiveUndockProvisionalLifecycle {
    Opening,
    Bound {
        window: AnyWindowHandle,
        runtime: DockViewportProvisionalOpenAttemptCompletion,
    },
    Unavailable(AnyWindowHandle),
    Failed,
    Terminal(WindowId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockLiveUndockTransportState {
    Moving,
    Released(DockLiveUndockReleaseLock),
    Cancelled(DockLiveUndockCancelReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockLiveUndockPayloadState {
    Unclaimed,
    AwaitingSourceProxy(DockLiveUndockPayloadLeaseReceipt),
    AwaitingPayloadMount(DockLiveUndockSourceProxyReceipt),
    Mounted(DockLiveUndockPayloadMountReceipt),
}

impl DockLiveUndockPayloadState {
    const fn source(self) -> Option<DockLiveUndockSourceSnapshot> {
        match self {
            Self::Unclaimed => None,
            Self::AwaitingSourceProxy(receipt) => Some(receipt.source()),
            Self::AwaitingPayloadMount(receipt) => Some(receipt.lease().source()),
            Self::Mounted(receipt) => Some(receipt.proxy().lease().source()),
        }
    }

    const fn is_mounted(self) -> bool {
        matches!(self, Self::Mounted(_))
    }

    const fn lease(self) -> Option<DockLiveUndockPayloadLeaseReceipt> {
        match self {
            Self::Unclaimed => None,
            Self::AwaitingSourceProxy(receipt) => Some(receipt),
            Self::AwaitingPayloadMount(receipt) => Some(receipt.lease()),
            Self::Mounted(receipt) => Some(receipt.proxy().lease()),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DockLiveUndockPresentationObservation {
    preflight: Option<DockLiveUndockPayloadPresentationReceipt>,
    initial_presentation_ready: Option<DockLiveUndockPayloadPresentationReceipt>,
    visible: Option<DockLiveUndockRevealReceipt>,
}

impl DockLiveUndockPresentationObservation {
    fn exact_visible(self, window_id: WindowId) -> bool {
        self.visible.is_some_and(|receipt| {
            Some(receipt.preflight()) == self.preflight
                && receipt.reveal_frame().window_id() == window_id
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DockLiveUndockPlacementObservation {
    window_id: WindowId,
    generation: DockLiveUndockPlacementGeneration,
    outcome: DockLiveUndockPlacementOutcome,
    final_placement: Option<DockLiveUndockFinalPlacementReceipt>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockLiveUndockPromotionState {
    None,
    Preparing {
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
    },
    Prepared {
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
    },
    Durable {
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
    },
    RecoveryRequired {
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
    },
    SemanticsSubmitted {
        receipt: DockLiveUndockDestinationSemanticsReceipt,
    },
}

#[derive(Debug)]
struct DockLiveUndockActive {
    opening: DockLiveUndockOpening,
    source: DockLiveUndockSourceSnapshot,
    provisional: DockLiveUndockProvisionalLifecycle,
    transport: DockLiveUndockTransportState,
    source_transport_proxy_active: bool,
    route: Option<DockLiveUndockRouteFeedback>,
    route_generation: DockLiveUndockRouteGeneration,
    route_point: DockLiveUndockPhysicalPoint,
    route_bounds: DockLiveUndockPhysicalBounds,
    route_placement_request_generation: Option<DockLiveUndockRouteGeneration>,
    payload: DockLiveUndockPayloadState,
    presentation: DockLiveUndockPresentationObservation,
    placement: Option<DockLiveUndockPlacementObservation>,
    placement_request_generation: Option<DockLiveUndockPlacementGeneration>,
    promotion: DockLiveUndockPromotionState,
}

impl DockLiveUndockActive {
    const fn identity(&self) -> DockLiveUndockIdentity {
        self.opening.identity
    }

    const fn bound_window(&self) -> Option<AnyWindowHandle> {
        if let DockLiveUndockProvisionalLifecycle::Bound { window, .. } = self.provisional {
            Some(window)
        } else {
            None
        }
    }

    const fn owned_window(&self) -> Option<AnyWindowHandle> {
        match self.provisional {
            DockLiveUndockProvisionalLifecycle::Bound { window, .. }
            | DockLiveUndockProvisionalLifecycle::Unavailable(window) => Some(window),
            DockLiveUndockProvisionalLifecycle::Opening
            | DockLiveUndockProvisionalLifecycle::Failed
            | DockLiveUndockProvisionalLifecycle::Terminal(_) => None,
        }
    }

    fn accepts_source(&self, source: DockLiveUndockSourceSnapshot) -> bool {
        self.source == source
    }

    fn retire_source_transport_proxy(&mut self, effects: &mut DockLiveUndockEffects) {
        if self.source_transport_proxy_active {
            self.source_transport_proxy_active = false;
            effects.push(DockLiveUndockEffect::RetireSourceTransportProxy {
                identity: self.identity(),
            });
        }
    }

    const fn may_commit_host_without_provisional(&self) -> bool {
        matches!(
            self.transport,
            DockLiveUndockTransportState::Moving
                | DockLiveUndockTransportState::Released(DockLiveUndockReleaseLock {
                    hit: DockLiveUndockRouteFeedback::Host(_),
                    ..
                })
        )
    }

    const fn accepts_presentation_facts(&self) -> bool {
        matches!(self.promotion, DockLiveUndockPromotionState::None)
    }

    fn accepts_presentation_failure(&self, failure: DockLiveUndockPresentationFailure) -> bool {
        if !matches!(
            self.provisional,
            DockLiveUndockProvisionalLifecycle::Bound { .. }
        ) {
            return false;
        }
        match failure {
            DockLiveUndockPresentationFailure::PayloadLeaseClaim
            | DockLiveUndockPresentationFailure::RetainedVisualTicket
            | DockLiveUndockPresentationFailure::RehostPreparation => {
                matches!(self.payload, DockLiveUndockPayloadState::Unclaimed)
            }
            DockLiveUndockPresentationFailure::SourceProxyReplay { lease } => {
                matches!(
                    self.payload,
                    DockLiveUndockPayloadState::AwaitingSourceProxy(current) if current == lease
                )
            }
            DockLiveUndockPresentationFailure::DestinationExposureFinish { proxy } => {
                matches!(
                    self.payload,
                    DockLiveUndockPayloadState::AwaitingPayloadMount(current) if current == proxy
                )
            }
            DockLiveUndockPresentationFailure::PayloadPresentationObservation { mount } => {
                matches!(self.payload, DockLiveUndockPayloadState::Mounted(current) if current == mount)
                    && self.presentation.preflight.is_none()
            }
            DockLiveUndockPresentationFailure::ExactRevealTicket { presentation } => {
                matches!(self.payload, DockLiveUndockPayloadState::Mounted(current) if current == presentation.mount())
                    && self.presentation.preflight == Some(presentation)
                    && self.presentation.visible.is_none()
            }
        }
    }

    const fn host_destination_selected(&self) -> bool {
        matches!(
            self.promotion,
            DockLiveUndockPromotionState::Preparing {
                destination: DockLiveUndockPromotionDestination::Host(_),
                ..
            } | DockLiveUndockPromotionState::Prepared {
                destination: DockLiveUndockPromotionDestination::Host(_),
                ..
            } | DockLiveUndockPromotionState::Durable {
                destination: DockLiveUndockPromotionDestination::Host(_),
                ..
            } | DockLiveUndockPromotionState::RecoveryRequired {
                destination: DockLiveUndockPromotionDestination::Host(_),
                ..
            } | DockLiveUndockPromotionState::SemanticsSubmitted {
                receipt: DockLiveUndockDestinationSemanticsReceipt {
                    destination: DockLiveUndockPromotionDestination::Host(_),
                    ..
                },
            }
        )
    }

    const fn committed_destination(&self) -> Option<DockLiveUndockPromotionDestination> {
        match self.committed_destination_recovery_promotion() {
            Some((_, destination)) => Some(destination),
            None => None,
        }
    }

    const fn durable_promotion(
        &self,
    ) -> Option<(
        DockLiveUndockPromotionToken,
        DockLiveUndockPromotionDestination,
    )> {
        match self.promotion {
            DockLiveUndockPromotionState::Durable { token, destination } => {
                Some((token, destination))
            }
            DockLiveUndockPromotionState::SemanticsSubmitted { receipt } => {
                Some((receipt.token(), receipt.destination()))
            }
            DockLiveUndockPromotionState::None
            | DockLiveUndockPromotionState::Preparing { .. }
            | DockLiveUndockPromotionState::Prepared { .. }
            | DockLiveUndockPromotionState::RecoveryRequired { .. } => None,
        }
    }

    const fn committed_destination_recovery_promotion(
        &self,
    ) -> Option<(
        DockLiveUndockPromotionToken,
        DockLiveUndockPromotionDestination,
    )> {
        match self.promotion {
            DockLiveUndockPromotionState::Durable { token, destination }
            | DockLiveUndockPromotionState::RecoveryRequired { token, destination } => {
                Some((token, destination))
            }
            DockLiveUndockPromotionState::SemanticsSubmitted { receipt } => {
                Some((receipt.token(), receipt.destination()))
            }
            DockLiveUndockPromotionState::None
            | DockLiveUndockPromotionState::Preparing { .. }
            | DockLiveUndockPromotionState::Prepared { .. } => None,
        }
    }

    fn unproven_promotion_matches(
        &self,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
    ) -> bool {
        matches!(
            self.promotion,
            DockLiveUndockPromotionState::Preparing {
                token: current_token,
                destination: current_destination,
            } | DockLiveUndockPromotionState::Prepared {
                token: current_token,
                destination: current_destination,
            } if current_token == token && current_destination == destination
        )
    }

    fn adopt_durable_promotion(
        &mut self,
        token: DockLiveUndockPromotionToken,
        expected_destination: Option<DockLiveUndockPromotionDestination>,
    ) -> Option<DockLiveUndockPromotionDestination> {
        let destination = match self.promotion {
            DockLiveUndockPromotionState::Preparing {
                token: current,
                destination,
            }
            | DockLiveUndockPromotionState::Prepared {
                token: current,
                destination,
            }
            | DockLiveUndockPromotionState::Durable {
                token: current,
                destination,
            } if current == token => destination,
            DockLiveUndockPromotionState::SemanticsSubmitted { receipt }
                if receipt.token() == token =>
            {
                receipt.destination()
            }
            DockLiveUndockPromotionState::None
            | DockLiveUndockPromotionState::Preparing { .. }
            | DockLiveUndockPromotionState::Prepared { .. }
            | DockLiveUndockPromotionState::Durable { .. }
            | DockLiveUndockPromotionState::RecoveryRequired { .. }
            | DockLiveUndockPromotionState::SemanticsSubmitted { .. } => return None,
        };
        if expected_destination.is_some_and(|expected| expected != destination) {
            return None;
        }
        if matches!(
            self.promotion,
            DockLiveUndockPromotionState::Preparing { .. }
                | DockLiveUndockPromotionState::Prepared { .. }
        ) {
            self.promotion = DockLiveUndockPromotionState::Durable { token, destination };
        }
        Some(destination)
    }

    fn adopt_committed_destination_recovery(
        &mut self,
        token: DockLiveUndockPromotionToken,
        expected_destination: Option<DockLiveUndockPromotionDestination>,
    ) -> Option<DockLiveUndockPromotionDestination> {
        let destination = match self.promotion {
            DockLiveUndockPromotionState::Preparing {
                token: current,
                destination,
            }
            | DockLiveUndockPromotionState::Prepared {
                token: current,
                destination,
            }
            | DockLiveUndockPromotionState::RecoveryRequired {
                token: current,
                destination,
            } if current == token => destination,
            DockLiveUndockPromotionState::None
            | DockLiveUndockPromotionState::Preparing { .. }
            | DockLiveUndockPromotionState::Prepared { .. }
            | DockLiveUndockPromotionState::Durable { .. }
            | DockLiveUndockPromotionState::RecoveryRequired { .. }
            | DockLiveUndockPromotionState::SemanticsSubmitted { .. } => return None,
        };
        if expected_destination.is_some_and(|expected| expected != destination) {
            return None;
        }
        self.promotion = DockLiveUndockPromotionState::RecoveryRequired { token, destination };
        Some(destination)
    }

    fn request_release_placement_if_needed(&mut self, effects: &mut DockLiveUndockEffects) {
        let Some(window) = self.bound_window() else {
            return;
        };
        if !self.presentation.exact_visible(window.window_id()) {
            return;
        }
        let DockLiveUndockTransportState::Released(release) = self.transport else {
            return;
        };
        if !matches!(
            release.hit(),
            DockLiveUndockRouteFeedback::Desktop | DockLiveUndockRouteFeedback::OpaqueBarrier
        ) || self.placement.is_some_and(|placement| {
            placement.window_id == window.window_id()
                && placement.generation == release.placement_generation()
        }) || self.placement_request_generation == Some(release.placement_generation())
        {
            return;
        }

        self.placement_request_generation = Some(release.placement_generation());
        effects.push(DockLiveUndockEffect::RequestReleasePlacement {
            identity: self.identity(),
            window,
            release,
        });
    }

    fn request_route_placement_if_visible(&mut self, effects: &mut DockLiveUndockEffects) {
        let Some(window) = self.bound_window() else {
            return;
        };
        if self.transport != DockLiveUndockTransportState::Moving
            || !self.presentation.exact_visible(window.window_id())
            || self.route_placement_request_generation == Some(self.route_generation)
        {
            return;
        }

        self.route_placement_request_generation = Some(self.route_generation);
        effects.push(DockLiveUndockEffect::RequestRoutePlacement {
            identity: self.identity(),
            window,
            generation: self.route_generation,
            point: self.route_point,
            bounds: self.route_bounds,
        });
    }
}

#[derive(Debug)]
struct DockLiveUndockRetiring {
    opening: DockLiveUndockOpening,
    window: Option<AnyWindowHandle>,
    shutdown_dependency: DockLiveUndockShutdownDependency,
    reason: DockLiveUndockRetirementReason,
}

#[derive(Debug)]
struct DockLiveUndockSourceRestoration {
    active: DockLiveUndockActive,
    reason: DockLiveUndockRestoreReason,
    restore_focus: bool,
    shutdown_dependency: DockLiveUndockShutdownDependency,
}

#[derive(Debug)]
struct DockLiveUndockOrphanRecovery {
    active: DockLiveUndockActive,
    payload_lease: DockLiveUndockPayloadLeaseReceipt,
    cause: DockLiveUndockPayloadRecoveryCause,
    shutdown_dependency: DockLiveUndockShutdownDependency,
}

#[derive(Debug)]
struct DockLiveUndockCommittedDestinationRecovery {
    active: DockLiveUndockActive,
    authority: DockPayloadRecoveryAuthority,
    token: DockLiveUndockPromotionToken,
    destination: DockLiveUndockPromotionDestination,
    recovery_receipt: Option<DockLiveUndockCommittedDestinationRecoveryReceipt>,
    cleanup: DockLiveUndockCommittedDestinationCleanup,
    shutdown_dependency: DockLiveUndockShutdownDependency,
}

#[derive(Debug)]
struct DockLiveUndockShutdownPromotionCommitWait {
    active: DockLiveUndockActive,
    token: DockLiveUndockPromotionToken,
    destination: DockLiveUndockPromotionDestination,
    shutdown_dependency: DockLiveUndockShutdownDependency,
}

impl DockLiveUndockShutdownPromotionCommitWait {
    const fn identity(&self) -> DockLiveUndockIdentity {
        self.active.identity()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockLiveUndockCommittedDestinationCleanup {
    SameWindow {
        window_id: WindowId,
        terminal: bool,
        retirement_requested: bool,
    },
    Host {
        target: DockLiveUndockHostTarget,
        recovery_committed: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockLiveUndockPayloadRecoveryCause {
    SourceNativeTerminal,
    PresentationAuthorityLost(DockLiveUndockPresentationAuthorityLoss),
}

impl DockLiveUndockSourceRestoration {
    const fn identity(&self) -> DockLiveUndockIdentity {
        self.active.identity()
    }

    fn accepts(
        &self,
        source: DockLiveUndockSourceSnapshot,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
    ) -> bool {
        payload_lease.identity() == self.identity()
            && payload_lease.source() == source
            && self.active.payload.source() == Some(source)
            && self.active.payload.lease() == Some(payload_lease)
    }

    fn accepts_receipt(&self, receipt: DockLiveUndockSourceRestorationReceipt) -> bool {
        receipt.identity() == self.identity()
            && self.accepts(receipt.source(), receipt.payload_lease())
    }
}

impl DockLiveUndockOrphanRecovery {
    const fn identity(&self) -> DockLiveUndockIdentity {
        self.active.identity()
    }

    fn accepts(&self, receipt: DockLiveUndockOrphanRecoveryReceipt) -> bool {
        receipt.identity() == self.identity()
            && receipt.payload_lease() == self.payload_lease
            && self.active.payload.lease() == Some(self.payload_lease)
    }

    fn accepts_cleanup(&self, receipt: DockLiveUndockOrphanCleanupReceipt) -> bool {
        receipt.identity() == self.identity()
            && receipt.payload_lease() == self.payload_lease
            && self.active.payload.lease() == Some(self.payload_lease)
    }
}

impl DockLiveUndockCommittedDestinationRecovery {
    const fn identity(&self) -> DockLiveUndockIdentity {
        self.active.identity()
    }

    fn accepts(&self, receipt: DockLiveUndockCommittedDestinationRecoveryReceipt) -> bool {
        receipt.identity() == self.identity()
            && receipt.authority() == self.authority
            && receipt.token() == self.token
            && receipt.destination() == self.destination
            && self.active.committed_destination_recovery_promotion()
                == Some((self.token, self.destination))
    }
}

impl DockLiveUndockRetiring {
    const fn identity(&self) -> DockLiveUndockIdentity {
        self.opening.identity
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockLiveUndockShutdownDependency {
    Unclaimed,
    Claimed(DockSurfaceWindowSessionDependencyId),
    Transferred,
}

impl DockLiveUndockShutdownDependency {
    const fn claimed(self) -> Option<DockSurfaceWindowSessionDependencyId> {
        if let Self::Claimed(dependency) = self {
            Some(dependency)
        } else {
            None
        }
    }
}

#[derive(Debug)]
enum DockLiveUndockState {
    Idle,
    Active(DockLiveUndockActive),
    Compensating(DockLiveUndockSourceRestoration),
    Restoring(DockLiveUndockSourceRestoration),
    RecoveringOrphan(DockLiveUndockOrphanRecovery),
    WaitingForPromotionCommit(DockLiveUndockShutdownPromotionCommitWait),
    RecoveringCommittedDestination(DockLiveUndockCommittedDestinationRecovery),
    ShutdownFailed {
        identity: DockLiveUndockIdentity,
        failure: DockLiveUndockShutdownFailure,
    },
    Retiring(DockLiveUndockRetiring),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockLiveUndockSettlement {
    Restore(DockLiveUndockRestoreReason),
    SourceLostBeforeCommit,
    PresentationAuthorityLostBeforeCommit(DockLiveUndockPresentationAuthorityLoss),
    Complete(DockLiveUndockPromotionDestination),
    DestinationLost(DockLiveUndockPromotionDestination),
}

#[derive(Debug)]
pub(crate) struct DockLiveUndockSession {
    last_opening_generation: u64,
    last_triggered_drag_generation: u64,
    last_promotion_token: u64,
    last_terminal_drag_generation: u64,
    state: DockLiveUndockState,
}

impl DockLiveUndockSession {
    pub(crate) const fn new() -> Self {
        Self {
            last_opening_generation: 0,
            last_triggered_drag_generation: 0,
            last_promotion_token: 0,
            last_terminal_drag_generation: 0,
            state: DockLiveUndockState::Idle,
        }
    }

    /// Reduces one immutable fact and returns borrow-free intents for the caller to execute.
    pub(crate) fn apply(&mut self, fact: DockLiveUndockFact) -> DockLiveUndockEffects {
        match fact {
            DockLiveUndockFact::Trigger { lease, trigger } => self.apply_trigger(lease, trigger),
            DockLiveUndockFact::ShutdownRequested { lease } => self.apply_shutdown(
                lease,
                DockLiveUndockPromotionCommitDisposition::RollbackAllowed,
            ),
            fact => self.apply_generation_fact(fact),
        }
    }

    fn apply_trigger(
        &mut self,
        lease: DockSurfaceWindowSessionLease,
        trigger: DockLiveUndockTrigger,
    ) -> DockLiveUndockEffects {
        let mut effects = DockLiveUndockEffects::default();
        let drag_generation = trigger.drag_generation();
        if drag_generation.get() <= self.last_triggered_drag_generation {
            return effects;
        }
        if !matches!(self.state, DockLiveUndockState::Idle) {
            effects.push(DockLiveUndockEffect::TriggerDeferred { drag_generation });
            return effects;
        }
        let Some(opening_generation) = self.last_opening_generation.checked_add(1) else {
            return effects;
        };
        let Ok(provisional_session) = WindowProvisionalSession::new(opening_generation) else {
            return effects;
        };
        self.last_triggered_drag_generation = drag_generation.get();
        self.last_opening_generation = opening_generation;
        let identity = DockLiveUndockIdentity {
            opening: DockLiveUndockOpeningKey {
                lease,
                generation: opening_generation,
            },
            drag_generation,
        };
        let request = DockLiveUndockOpenRequest {
            identity,
            provisional_session: provisional_session.clone(),
        };
        self.state = DockLiveUndockState::Active(DockLiveUndockActive {
            opening: DockLiveUndockOpening {
                identity,
                provisional_session,
            },
            source: trigger.source(),
            provisional: DockLiveUndockProvisionalLifecycle::Opening,
            transport: DockLiveUndockTransportState::Moving,
            source_transport_proxy_active: true,
            route: Some(trigger.initial_route()),
            route_generation: trigger.initial_route_generation(),
            route_point: trigger.initial_point(),
            route_bounds: trigger.initial_bounds(),
            route_placement_request_generation: None,
            payload: DockLiveUndockPayloadState::Unclaimed,
            presentation: DockLiveUndockPresentationObservation::default(),
            placement: None,
            placement_request_generation: None,
            promotion: DockLiveUndockPromotionState::None,
        });
        effects.push(DockLiveUndockEffect::OpenProvisional { identity, request });
        effects.push(DockLiveUndockEffect::RouteFeedbackChanged {
            identity,
            route: trigger.initial_route(),
        });
        effects
    }

    fn apply_generation_fact(&mut self, fact: DockLiveUndockFact) -> DockLiveUndockEffects {
        let Some(identity) = fact.identity() else {
            return DockLiveUndockEffects::default();
        };
        let state = std::mem::replace(&mut self.state, DockLiveUndockState::Idle);
        let (state, effects) = match state {
            DockLiveUndockState::Active(mut active) if active.identity() == identity => {
                let mut effects = DockLiveUndockEffects::default();
                let settlement = self.reduce_active(&mut active, fact, &mut effects);
                let state = if let Some(settlement) = settlement {
                    self.settle_active(active, settlement, &mut effects)
                } else {
                    DockLiveUndockState::Active(active)
                };
                (state, effects)
            }
            DockLiveUndockState::Compensating(restoration)
                if restoration.identity() == identity =>
            {
                self.reduce_source_restoration(restoration, fact, false)
            }
            DockLiveUndockState::Restoring(restoration) if restoration.identity() == identity => {
                self.reduce_source_restoration(restoration, fact, true)
            }
            DockLiveUndockState::RecoveringOrphan(recovery) if recovery.identity() == identity => {
                self.reduce_orphan_recovery(recovery, fact)
            }
            DockLiveUndockState::WaitingForPromotionCommit(waiting)
                if waiting.identity() == identity =>
            {
                self.reduce_shutdown_promotion_commit_wait(waiting, fact)
            }
            DockLiveUndockState::RecoveringCommittedDestination(recovery)
                if recovery.identity() == identity =>
            {
                self.reduce_committed_destination_recovery(recovery, fact)
            }
            DockLiveUndockState::ShutdownFailed {
                identity: current, ..
            } if current == identity => (state, DockLiveUndockEffects::default()),
            DockLiveUndockState::Retiring(retiring) if retiring.identity() == identity => {
                self.reduce_retiring(retiring, fact)
            }
            state => (state, DockLiveUndockEffects::default()),
        };
        self.state = state;
        effects
    }

    fn reduce_active(
        &mut self,
        active: &mut DockLiveUndockActive,
        fact: DockLiveUndockFact,
        effects: &mut DockLiveUndockEffects,
    ) -> Option<DockLiveUndockSettlement> {
        let identity = active.identity();
        if !active.accepts_presentation_facts()
            && matches!(
                &fact,
                DockLiveUndockFact::PresentationStageFailed { .. }
                    | DockLiveUndockFact::PresentationLeaseActivated { .. }
                    | DockLiveUndockFact::SourceProxyCommitted { .. }
                    | DockLiveUndockFact::PayloadMounted { .. }
                    | DockLiveUndockFact::PayloadPresented { .. }
                    | DockLiveUndockFact::InitialPresentationReady { .. }
                    | DockLiveUndockFact::RevealObserved { .. }
                    | DockLiveUndockFact::PlacementObserved { .. }
                    | DockLiveUndockFact::RoutePlacementObserved { .. }
            )
        {
            return None;
        }
        match fact {
            DockLiveUndockFact::RouteObserved {
                generation,
                route,
                point,
                bounds,
                ..
            } => {
                let route = if matches!(
                    active.provisional,
                    DockLiveUndockProvisionalLifecycle::Unavailable(_)
                        | DockLiveUndockProvisionalLifecycle::Failed
                        | DockLiveUndockProvisionalLifecycle::Terminal(_)
                ) && !matches!(route, DockLiveUndockRouteFeedback::Host(_))
                {
                    DockLiveUndockRouteFeedback::Unavailable
                } else {
                    route
                };
                if active.transport == DockLiveUndockTransportState::Moving
                    && generation > active.route_generation
                    && bounds.contains_target_point(point)
                {
                    active.route_generation = generation;
                    active.route_point = point;
                    active.route_bounds = bounds;
                    if active.route != Some(route) {
                        active.route = Some(route);
                        effects
                            .push(DockLiveUndockEffect::RouteFeedbackChanged { identity, route });
                    }
                    active.request_route_placement_if_visible(effects);
                }
            }
            DockLiveUndockFact::OpeningReturned {
                window,
                binding,
                runtime,
                ..
            } if active.provisional == DockLiveUndockProvisionalLifecycle::Opening => {
                if active.host_destination_selected() {
                    active.provisional = DockLiveUndockProvisionalLifecycle::Unavailable(window);
                    effects.push(DockLiveUndockEffect::ProvisionalRetirementRequired {
                        identity,
                        window: Some(window),
                        dependency: None,
                        binding: Some(binding),
                        runtime: Some(runtime),
                        reason: DockLiveUndockRetirementReason::HostDestinationSelected,
                    });
                } else if binding == DockLiveUndockOpeningBinding::ExactGated
                    && runtime.is_admitted()
                {
                    active.provisional =
                        DockLiveUndockProvisionalLifecycle::Bound { window, runtime };
                    effects.push(DockLiveUndockEffect::ProvisionalAdmitted {
                        identity,
                        window,
                        lease: identity.opening.lease,
                    });
                    active.request_release_placement_if_needed(effects);
                } else {
                    active.provisional = DockLiveUndockProvisionalLifecycle::Unavailable(window);
                    let (restore_reason, retirement_reason) =
                        if binding == DockLiveUndockOpeningBinding::ExactGated {
                            (
                                DockLiveUndockRestoreReason::RuntimeRegistrationRejected,
                                DockLiveUndockRetirementReason::RuntimeRegistrationRejected,
                            )
                        } else {
                            (
                                DockLiveUndockRestoreReason::OpeningBindingInvalid,
                                DockLiveUndockRetirementReason::OpeningBindingInvalid,
                            )
                        };
                    effects.push(DockLiveUndockEffect::ProvisionalRetirementRequired {
                        identity,
                        window: Some(window),
                        dependency: None,
                        binding: Some(binding),
                        runtime: Some(runtime),
                        reason: retirement_reason,
                    });
                    if !active.may_commit_host_without_provisional() {
                        return Some(DockLiveUndockSettlement::Restore(restore_reason));
                    }
                    if active.transport == DockLiveUndockTransportState::Moving {
                        active.route = Some(DockLiveUndockRouteFeedback::Unavailable);
                        effects.push(DockLiveUndockEffect::RouteFeedbackChanged {
                            identity,
                            route: DockLiveUndockRouteFeedback::Unavailable,
                        });
                    }
                }
            }
            DockLiveUndockFact::OpeningFailed { .. }
                if active.provisional == DockLiveUndockProvisionalLifecycle::Opening =>
            {
                active.provisional = DockLiveUndockProvisionalLifecycle::Failed;
                effects.push(DockLiveUndockEffect::OpeningFailed {
                    identity,
                    dependency: None,
                });
                if active.transport == DockLiveUndockTransportState::Moving {
                    active.route = Some(DockLiveUndockRouteFeedback::Unavailable);
                    effects.push(DockLiveUndockEffect::RouteFeedbackChanged {
                        identity,
                        route: DockLiveUndockRouteFeedback::Unavailable,
                    });
                }
            }
            DockLiveUndockFact::PresentationStageFailed { failure, .. } => {
                if active.accepts_presentation_failure(failure) {
                    return Some(DockLiveUndockSettlement::Restore(
                        DockLiveUndockRestoreReason::PresentationFailed(failure),
                    ));
                }
            }
            DockLiveUndockFact::PresentationLeaseActivated { receipt, .. }
                if matches!(
                    active.provisional,
                    DockLiveUndockProvisionalLifecycle::Bound { .. }
                ) && active.payload == DockLiveUndockPayloadState::Unclaimed
                    && receipt.identity() == identity
                    && active.accepts_source(receipt.source())
                    && active.bound_window().is_some_and(|window| {
                        window.window_id() == receipt.destination_window()
                            && receipt.provisional_session_generation()
                                == active.opening.identity.opening.generation
                    }) =>
            {
                active.payload = DockLiveUndockPayloadState::AwaitingSourceProxy(receipt);
                effects.push(DockLiveUndockEffect::CommitSourceProxy {
                    identity,
                    lease: receipt,
                });
            }
            DockLiveUndockFact::SourceProxyCommitted { receipt, .. } => {
                if let DockLiveUndockPayloadState::AwaitingSourceProxy(current) = active.payload
                    && current == receipt.lease()
                    && let Some(window) = active.bound_window()
                {
                    active.payload = DockLiveUndockPayloadState::AwaitingPayloadMount(receipt);
                    effects.push(DockLiveUndockEffect::MountAndExposePayload {
                        identity,
                        proxy: receipt,
                        window,
                    });
                }
            }
            DockLiveUndockFact::PayloadMounted { receipt, .. } => {
                if let DockLiveUndockPayloadState::AwaitingPayloadMount(current) = active.payload
                    && current == receipt.proxy()
                    && let Some(window) = active.bound_window()
                {
                    active.payload = DockLiveUndockPayloadState::Mounted(receipt);
                    effects.push(DockLiveUndockEffect::ObservePayloadPresentation {
                        identity,
                        mount: receipt,
                        window,
                    });
                }
            }
            DockLiveUndockFact::PayloadPresented { receipt, .. }
                if matches!(active.payload, DockLiveUndockPayloadState::Mounted(current) if current == receipt.mount())
                    && active.bound_window().map(|window| window.window_id())
                        == Some(receipt.window_id()) =>
            {
                if active
                    .presentation
                    .preflight
                    .is_none_or(|current| receipt.frame_generation() > current.frame_generation())
                {
                    active.presentation.preflight = Some(receipt);
                    active.presentation.initial_presentation_ready = None;
                    active.presentation.visible = None;
                    if let Some(window) = active.bound_window() {
                        active.route_placement_request_generation = Some(active.route_generation);
                        effects.push(DockLiveUndockEffect::ArmExactReveal {
                            identity,
                            presentation: receipt,
                            window,
                            point: active.route_point,
                            bounds: active.route_bounds,
                        });
                    }
                }
            }
            DockLiveUndockFact::InitialPresentationReady { presentation, .. }
                if active.presentation.preflight == Some(presentation)
                    && active.presentation.initial_presentation_ready != Some(presentation)
                    && active.presentation.visible.is_none()
                    && active.bound_window().map(|window| window.window_id())
                        == Some(presentation.window_id()) =>
            {
                active.presentation.initial_presentation_ready = Some(presentation);
                if let Some(window) = active.bound_window() {
                    active.route_placement_request_generation = Some(active.route_generation);
                    effects.push(DockLiveUndockEffect::ArmExactReveal {
                        identity,
                        presentation,
                        window,
                        point: active.route_point,
                        bounds: active.route_bounds,
                    });
                }
            }
            DockLiveUndockFact::RevealObserved { observation, .. }
                if active.presentation.preflight.is_some_and(|preflight| {
                    preflight.mount() == observation.presentation().mount()
                        && observation.presentation().frame_generation()
                            >= preflight.frame_generation()
                }) && active.bound_window().map(|window| window.window_id())
                    == Some(observation.presentation().window_id()) =>
            {
                match observation {
                    DockLiveUndockRevealObservation::Visible(receipt) => {
                        if active.presentation.preflight == Some(receipt.preflight()) {
                            active.presentation.visible = Some(receipt);
                            effects.push(DockLiveUndockEffect::RetireFrozenSourceVisual {
                                identity,
                                reveal: receipt,
                            });
                            active.request_route_placement_if_visible(effects);
                            active.request_release_placement_if_needed(effects);
                        }
                    }
                    DockLiveUndockRevealObservation::Failed { outcome, .. } => {
                        #[cfg(feature = "test-support")]
                        eprintln!(
                            "OPEN_GPUI_DOCK_REVEAL_FAILED identity={identity:?} outcome={outcome:?}"
                        );
                        if active.may_commit_host_without_provisional() {
                            if let DockLiveUndockProvisionalLifecycle::Bound { window, runtime } =
                                active.provisional
                            {
                                active.provisional =
                                    DockLiveUndockProvisionalLifecycle::Unavailable(window);
                                effects.push(DockLiveUndockEffect::ProvisionalRetirementRequired {
                                    identity,
                                    window: Some(window),
                                    dependency: None,
                                    binding: None,
                                    runtime: Some(runtime),
                                    reason: DockLiveUndockRetirementReason::PresentationUnavailable,
                                });
                            }
                            if active.transport == DockLiveUndockTransportState::Moving {
                                active.route = Some(DockLiveUndockRouteFeedback::Unavailable);
                                effects.push(DockLiveUndockEffect::RouteFeedbackChanged {
                                    identity,
                                    route: DockLiveUndockRouteFeedback::Unavailable,
                                });
                            }
                        } else {
                            return Some(DockLiveUndockSettlement::Restore(
                                DockLiveUndockRestoreReason::RevealFailed(outcome),
                            ));
                        }
                    }
                }
            }
            DockLiveUndockFact::PlacementObserved {
                window_id,
                generation,
                outcome,
                final_placement,
                ..
            } if active.bound_window().map(|window| window.window_id()) == Some(window_id)
                && active
                    .placement
                    .is_none_or(|current| generation > current.generation) =>
            {
                active.placement = Some(DockLiveUndockPlacementObservation {
                    window_id,
                    generation,
                    outcome,
                    final_placement,
                });
            }
            DockLiveUndockFact::RoutePlacementObserved {
                window_id,
                generation,
                outcome,
                ..
            } if active.bound_window().map(|window| window.window_id()) == Some(window_id) => {
                if outcome == DockLiveUndockRoutePlacementOutcome::WindowClosed {
                    return Some(DockLiveUndockSettlement::Restore(
                        DockLiveUndockRestoreReason::ProvisionalTerminal,
                    ));
                }
                if active.transport == DockLiveUndockTransportState::Moving
                    && generation == active.route_generation
                    && !matches!(
                        outcome,
                        DockLiveUndockRoutePlacementOutcome::Exact
                            | DockLiveUndockRoutePlacementOutcome::Adjusted
                    )
                {
                    return Some(DockLiveUndockSettlement::Restore(
                        DockLiveUndockRestoreReason::RoutePlacementFailed(outcome),
                    ));
                }
            }
            DockLiveUndockFact::ReleaseLocked { release, .. }
                if active.transport == DockLiveUndockTransportState::Moving =>
            {
                active.route_generation = release.route_generation();
                active.route = Some(release.hit());
                active.route_point = release.point();
                active.route_bounds = release.desired_bounds();
                active.retire_source_transport_proxy(effects);
                active.transport = DockLiveUndockTransportState::Released(release);
                active.request_release_placement_if_needed(effects);
            }
            DockLiveUndockFact::ReleaseDeadlineExpired {
                placement_generation,
                ..
            } => {
                if let DockLiveUndockTransportState::Released(release) = active.transport
                    && release.placement_generation == placement_generation
                    && active.promotion == DockLiveUndockPromotionState::None
                {
                    return Some(DockLiveUndockSettlement::Restore(
                        DockLiveUndockRestoreReason::ReleaseDeadlineExpired,
                    ));
                }
            }
            DockLiveUndockFact::PromotionPrepared { token, .. } => {
                if let DockLiveUndockPromotionState::Preparing {
                    token: current,
                    destination,
                } = active.promotion
                    && current == token
                {
                    active.promotion =
                        DockLiveUndockPromotionState::Prepared { token, destination };
                    effects.push(DockLiveUndockEffect::CommitPreparedPromotion {
                        identity,
                        token,
                        destination,
                    });
                }
            }
            DockLiveUndockFact::PromotionPreparationFailed { token, .. } => {
                if matches!(
                    active.promotion,
                    DockLiveUndockPromotionState::Preparing { token: current, .. }
                        | DockLiveUndockPromotionState::Prepared { token: current, .. }
                        if current == token
                ) {
                    return Some(DockLiveUndockSettlement::Restore(
                        DockLiveUndockRestoreReason::PromotionPreparationFailed,
                    ));
                }
            }
            DockLiveUndockFact::DurableSwapCommitted { token, .. } => {
                if let DockLiveUndockPromotionState::Prepared {
                    token: current,
                    destination,
                } = active.promotion
                    && current == token
                {
                    active.promotion = DockLiveUndockPromotionState::Durable { token, destination };
                    if matches!(destination, DockLiveUndockPromotionDestination::Host(_)) {
                        effects.push(DockLiveUndockEffect::ApplyCommittedHostWindowEffects {
                            identity,
                            token,
                            destination,
                        });
                    }
                    effects.push(
                        DockLiveUndockEffect::DestinationSemanticsSubmissionRequired {
                            identity,
                            token,
                            destination,
                        },
                    );
                }
            }
            DockLiveUndockFact::CommittedDestinationRecoveryRequired {
                token, destination, ..
            } => {
                if active
                    .adopt_committed_destination_recovery(token, Some(destination))
                    .is_some()
                {
                    return Some(DockLiveUndockSettlement::DestinationLost(destination));
                }
            }
            DockLiveUndockFact::DestinationSemanticsSubmitted { receipt, .. } => {
                if let DockLiveUndockPromotionState::Durable {
                    token: current,
                    destination,
                } = active.promotion
                    && receipt.identity() == identity
                    && receipt.token() == current
                    && receipt.destination() == destination
                {
                    active.promotion = DockLiveUndockPromotionState::SemanticsSubmitted { receipt };
                    effects.push(
                        DockLiveUndockEffect::DestinationInteractionAdmissionRequired {
                            identity,
                            semantics: receipt,
                        },
                    );
                }
            }
            DockLiveUndockFact::DestinationSemanticsSubmissionFailed {
                token, destination, ..
            } => {
                if matches!(
                    active.promotion,
                    DockLiveUndockPromotionState::Durable {
                        token: current_token,
                        destination: current_destination,
                    } if current_token == token && current_destination == destination
                ) {
                    return Some(DockLiveUndockSettlement::DestinationLost(destination));
                }
            }
            DockLiveUndockFact::DestinationInteractionAdmitted { receipt, .. } => {
                if let DockLiveUndockPromotionState::SemanticsSubmitted { receipt: current } =
                    active.promotion
                    && current == receipt.semantics()
                {
                    let destination = current.destination();
                    effects.push(DockLiveUndockEffect::DestinationInteractionReady {
                        identity,
                        interaction: receipt,
                        destination,
                    });
                    return Some(DockLiveUndockSettlement::Complete(destination));
                }
            }
            DockLiveUndockFact::DestinationInteractionAdmissionFailed { semantics, .. } => {
                if let DockLiveUndockPromotionState::SemanticsSubmitted { receipt: current } =
                    active.promotion
                    && current == semantics
                {
                    let destination = current.destination();
                    return Some(DockLiveUndockSettlement::DestinationLost(destination));
                }
            }
            DockLiveUndockFact::DestinationTerminal { window_id, .. } => match active.promotion {
                DockLiveUndockPromotionState::Preparing { destination, .. }
                | DockLiveUndockPromotionState::Prepared { destination, .. }
                    if destination.window_id() == window_id =>
                {
                    return Some(DockLiveUndockSettlement::Restore(
                        DockLiveUndockRestoreReason::DestinationTerminalBeforeCommit,
                    ));
                }
                DockLiveUndockPromotionState::Durable { destination, .. }
                | DockLiveUndockPromotionState::RecoveryRequired { destination, .. }
                    if destination.window_id() == window_id =>
                {
                    return Some(DockLiveUndockSettlement::DestinationLost(destination));
                }
                DockLiveUndockPromotionState::SemanticsSubmitted { receipt }
                    if receipt.destination().window_id() == window_id =>
                {
                    let destination = receipt.destination();
                    return Some(DockLiveUndockSettlement::DestinationLost(destination));
                }
                DockLiveUndockPromotionState::None
                | DockLiveUndockPromotionState::Preparing { .. }
                | DockLiveUndockPromotionState::Prepared { .. }
                | DockLiveUndockPromotionState::Durable { .. }
                | DockLiveUndockPromotionState::RecoveryRequired { .. }
                | DockLiveUndockPromotionState::SemanticsSubmitted { .. } => {}
            },
            DockLiveUndockFact::Cancel { reason, .. } => {
                let precommit = active.committed_destination().is_none();
                let accepted = active.transport == DockLiveUndockTransportState::Moving
                    || matches!(active.transport, DockLiveUndockTransportState::Released(_))
                        && reason.aborts_after_release_before_commit();
                if !precommit || !accepted {
                    return None;
                }
                active.transport = DockLiveUndockTransportState::Cancelled(reason);
                return Some(DockLiveUndockSettlement::Restore(
                    DockLiveUndockRestoreReason::Cancelled(reason),
                ));
            }
            DockLiveUndockFact::SourceWindowNativeTerminal { receipt }
                if active.committed_destination().is_none()
                    && active.accepts_source(receipt.source()) =>
            {
                return Some(DockLiveUndockSettlement::SourceLostBeforeCommit);
            }
            DockLiveUndockFact::PresentationAuthorityLost { receipt }
                if active.committed_destination().is_none()
                    && active.payload.lease() == Some(receipt.payload_lease) =>
            {
                return Some(
                    DockLiveUndockSettlement::PresentationAuthorityLostBeforeCommit(
                        receipt.cause(),
                    ),
                );
            }
            DockLiveUndockFact::WindowTerminal { window_id, .. }
                if active.owned_window().map(|window| window.window_id()) == Some(window_id) =>
            {
                effects.push(DockLiveUndockEffect::WindowTerminalSettled(
                    DockLiveUndockWindowTerminalOutcome {
                        lease: identity.opening.lease,
                        dependency: None,
                    },
                ));
                if let Some(destination) = active.committed_destination()
                    && destination.window_id() == window_id
                {
                    active.provisional = DockLiveUndockProvisionalLifecycle::Terminal(window_id);
                    return Some(DockLiveUndockSettlement::DestinationLost(destination));
                }
                active.provisional = DockLiveUndockProvisionalLifecycle::Terminal(window_id);
                if active.may_commit_host_without_provisional() {
                    if active.transport == DockLiveUndockTransportState::Moving {
                        active.route = Some(DockLiveUndockRouteFeedback::Unavailable);
                        effects.push(DockLiveUndockEffect::RouteFeedbackChanged {
                            identity,
                            route: DockLiveUndockRouteFeedback::Unavailable,
                        });
                    }
                } else {
                    return Some(DockLiveUndockSettlement::Restore(
                        DockLiveUndockRestoreReason::ProvisionalTerminal,
                    ));
                }
            }
            DockLiveUndockFact::ShutdownDependencyTransferred { .. }
            | DockLiveUndockFact::Trigger { .. }
            | DockLiveUndockFact::ShutdownRequested { .. }
            | DockLiveUndockFact::SourceRestorationCommitted { .. }
            | DockLiveUndockFact::SourceRestorationDeferred { .. }
            | DockLiveUndockFact::SourceRestorationRetryElapsed { .. }
            | DockLiveUndockFact::SourceWindowNativeTerminal { .. }
            | DockLiveUndockFact::PresentationAuthorityLost { .. }
            | DockLiveUndockFact::OrphanRecoveryCommitted { .. }
            | DockLiveUndockFact::OrphanRecoveryFailed { .. }
            | DockLiveUndockFact::ShutdownOrphanCleanupCompleted { .. }
            | DockLiveUndockFact::ShutdownOrphanCleanupFailed { .. }
            | DockLiveUndockFact::CommittedDestinationRecoveryCommitted { .. }
            | DockLiveUndockFact::ShutdownCommittedDestinationRecoveryFailed { .. }
            | DockLiveUndockFact::OpeningReturned { .. }
            | DockLiveUndockFact::OpeningFailed { .. }
            | DockLiveUndockFact::PresentationLeaseActivated { .. }
            | DockLiveUndockFact::PayloadPresented { .. }
            | DockLiveUndockFact::InitialPresentationReady { .. }
            | DockLiveUndockFact::RevealObserved { .. }
            | DockLiveUndockFact::PlacementObserved { .. }
            | DockLiveUndockFact::RoutePlacementObserved { .. }
            | DockLiveUndockFact::ReleaseLocked { .. }
            | DockLiveUndockFact::WindowTerminal { .. } => {}
        }
        self.maybe_prepare_promotion(active, effects)
    }

    fn maybe_prepare_promotion(
        &mut self,
        active: &mut DockLiveUndockActive,
        effects: &mut DockLiveUndockEffects,
    ) -> Option<DockLiveUndockSettlement> {
        if active.promotion != DockLiveUndockPromotionState::None {
            return None;
        }
        let DockLiveUndockTransportState::Released(release) = active.transport else {
            return None;
        };
        let destination = match release.hit {
            DockLiveUndockRouteFeedback::Host(target) => {
                DockLiveUndockPromotionDestination::Host(target)
            }
            DockLiveUndockRouteFeedback::Desktop | DockLiveUndockRouteFeedback::OpaqueBarrier => {
                let Some(window) = active.bound_window() else {
                    return if matches!(
                        active.provisional,
                        DockLiveUndockProvisionalLifecycle::Unavailable(_)
                            | DockLiveUndockProvisionalLifecycle::Failed
                            | DockLiveUndockProvisionalLifecycle::Terminal(_)
                    ) {
                        Some(DockLiveUndockSettlement::Restore(
                            DockLiveUndockRestoreReason::ProvisionalTerminal,
                        ))
                    } else {
                        None
                    };
                };
                if !active.payload.is_mounted()
                    || !active.presentation.exact_visible(window.window_id())
                {
                    return None;
                }
                let Some(placement) = active.placement else {
                    return None;
                };
                if placement.window_id != window.window_id()
                    || placement.generation != release.placement_generation
                {
                    return None;
                }
                if !placement.outcome.admits_promotion() {
                    return Some(DockLiveUndockSettlement::Restore(
                        DockLiveUndockRestoreReason::PlacementFailed(placement.outcome),
                    ));
                }
                if placement.final_placement.is_none_or(|receipt| {
                    !receipt.matches(active.identity(), window.window_id(), release)
                }) {
                    return Some(DockLiveUndockSettlement::Restore(
                        DockLiveUndockRestoreReason::PlacementFailed(
                            DockLiveUndockPlacementOutcome::Rejected,
                        ),
                    ));
                }
                DockLiveUndockPromotionDestination::SameWindowDesktop {
                    window_id: window.window_id(),
                }
            }
            DockLiveUndockRouteFeedback::ForeignSurface { .. } => {
                return Some(DockLiveUndockSettlement::Restore(
                    DockLiveUndockRestoreReason::ForeignSurface,
                ));
            }
            DockLiveUndockRouteFeedback::Unavailable => {
                return Some(DockLiveUndockSettlement::Restore(
                    DockLiveUndockRestoreReason::RouteUnavailable,
                ));
            }
        };
        let Some(next) = self.last_promotion_token.checked_add(1) else {
            return Some(DockLiveUndockSettlement::Restore(
                DockLiveUndockRestoreReason::PromotionPreparationFailed,
            ));
        };
        let Some(token) = DockLiveUndockPromotionToken::new(next) else {
            return Some(DockLiveUndockSettlement::Restore(
                DockLiveUndockRestoreReason::PromotionPreparationFailed,
            ));
        };
        self.last_promotion_token = next;
        active.promotion = DockLiveUndockPromotionState::Preparing { token, destination };
        effects.push(DockLiveUndockEffect::PreparePromotion {
            identity: active.identity(),
            token,
            destination,
            release,
        });
        None
    }

    fn settle_active(
        &mut self,
        mut active: DockLiveUndockActive,
        settlement: DockLiveUndockSettlement,
        effects: &mut DockLiveUndockEffects,
    ) -> DockLiveUndockState {
        active.retire_source_transport_proxy(effects);
        match settlement {
            DockLiveUndockSettlement::Restore(reason) => {
                let restore_focus = Self::restore_focus(reason);
                if active.payload.lease().is_some() {
                    return Self::begin_source_restoration(
                        active,
                        reason,
                        restore_focus,
                        DockLiveUndockShutdownDependency::Unclaimed,
                        effects,
                    );
                }
                self.finish_active(
                    active,
                    DockLiveUndockTerminalResult::Restored(reason),
                    Some(Self::restoration_retirement_reason(reason)),
                    DockLiveUndockShutdownDependency::Unclaimed,
                    effects,
                )
            }
            DockLiveUndockSettlement::SourceLostBeforeCommit => {
                if active.payload.lease().is_some() {
                    return Self::begin_orphan_recovery(
                        active,
                        DockLiveUndockPayloadRecoveryCause::SourceNativeTerminal,
                        DockLiveUndockShutdownDependency::Unclaimed,
                        effects,
                    );
                }
                self.finish_active(
                    active,
                    DockLiveUndockTerminalResult::SourceLostBeforeCommit,
                    Some(DockLiveUndockRetirementReason::SourceLost),
                    DockLiveUndockShutdownDependency::Unclaimed,
                    effects,
                )
            }
            DockLiveUndockSettlement::PresentationAuthorityLostBeforeCommit(cause) => {
                if active.payload.lease().is_some() {
                    return Self::begin_orphan_recovery(
                        active,
                        DockLiveUndockPayloadRecoveryCause::PresentationAuthorityLost(cause),
                        DockLiveUndockShutdownDependency::Unclaimed,
                        effects,
                    );
                }
                self.finish_active(
                    active,
                    DockLiveUndockTerminalResult::PresentationAuthorityLostBeforeCommit(cause),
                    Some(DockLiveUndockRetirementReason::PresentationAuthorityLost),
                    DockLiveUndockShutdownDependency::Unclaimed,
                    effects,
                )
            }
            DockLiveUndockSettlement::Complete(destination) => self.finish_active(
                active,
                DockLiveUndockTerminalResult::Committed(destination),
                matches!(destination, DockLiveUndockPromotionDestination::Host(_))
                    .then_some(DockLiveUndockRetirementReason::HostCommitted),
                DockLiveUndockShutdownDependency::Unclaimed,
                effects,
            ),
            DockLiveUndockSettlement::DestinationLost(destination) => {
                Self::begin_committed_destination_recovery(
                    active,
                    destination,
                    DockLiveUndockShutdownDependency::Unclaimed,
                    effects,
                )
            }
        }
    }

    fn begin_orphan_recovery(
        active: DockLiveUndockActive,
        cause: DockLiveUndockPayloadRecoveryCause,
        shutdown_dependency: DockLiveUndockShutdownDependency,
        effects: &mut DockLiveUndockEffects,
    ) -> DockLiveUndockState {
        let Some(payload_lease) = active.payload.lease() else {
            return DockLiveUndockState::Active(active);
        };
        let identity = active.identity();
        let provisional = active.owned_window();
        effects.push(match shutdown_dependency {
            DockLiveUndockShutdownDependency::Claimed(_)
            | DockLiveUndockShutdownDependency::Transferred => {
                DockLiveUndockEffect::ShutdownOrphanRecoveryRequired {
                    identity,
                    payload_lease,
                    provisional,
                }
            }
            DockLiveUndockShutdownDependency::Unclaimed => {
                DockLiveUndockEffect::RecoverOrphanedPayloadTopology {
                    identity,
                    payload_lease,
                    provisional,
                }
            }
        });
        DockLiveUndockState::RecoveringOrphan(DockLiveUndockOrphanRecovery {
            active,
            payload_lease,
            cause,
            shutdown_dependency,
        })
    }

    fn begin_committed_destination_recovery(
        active: DockLiveUndockActive,
        destination: DockLiveUndockPromotionDestination,
        shutdown_dependency: DockLiveUndockShutdownDependency,
        effects: &mut DockLiveUndockEffects,
    ) -> DockLiveUndockState {
        let (token, committed_destination) = active
            .committed_destination_recovery_promotion()
            .expect("a committed destination recovery must follow committed destination authority");
        debug_assert_eq!(committed_destination, destination);
        let authority = DockPayloadRecoveryAuthority::committed_destination(
            active.identity(),
            token,
            destination,
        );
        let cleanup = match destination {
            DockLiveUndockPromotionDestination::SameWindowDesktop { window_id } => {
                DockLiveUndockCommittedDestinationCleanup::SameWindow {
                    window_id,
                    terminal: matches!(
                        active.provisional,
                        DockLiveUndockProvisionalLifecycle::Terminal(current)
                            if current == window_id
                    ),
                    retirement_requested: false,
                }
            }
            DockLiveUndockPromotionDestination::Host(target) => {
                DockLiveUndockCommittedDestinationCleanup::Host {
                    target,
                    recovery_committed: false,
                }
            }
        };
        let identity = active.identity();
        effects.push(match shutdown_dependency {
            DockLiveUndockShutdownDependency::Claimed(_)
            | DockLiveUndockShutdownDependency::Transferred => {
                DockLiveUndockEffect::ShutdownCommittedDestinationRecoveryRequired {
                    identity,
                    authority,
                    token,
                    destination,
                }
            }
            DockLiveUndockShutdownDependency::Unclaimed => {
                DockLiveUndockEffect::RecoverCommittedDestinationTopology {
                    identity,
                    authority,
                    token,
                    destination,
                }
            }
        });
        DockLiveUndockState::RecoveringCommittedDestination(
            DockLiveUndockCommittedDestinationRecovery {
                active,
                authority,
                token,
                destination,
                recovery_receipt: None,
                cleanup,
                shutdown_dependency,
            },
        )
    }

    const fn restore_focus(reason: DockLiveUndockRestoreReason) -> bool {
        match reason {
            DockLiveUndockRestoreReason::Cancelled(reason) => reason.restore_focus(),
            DockLiveUndockRestoreReason::Shutdown => false,
            DockLiveUndockRestoreReason::OpeningBindingInvalid
            | DockLiveUndockRestoreReason::RuntimeRegistrationRejected
            | DockLiveUndockRestoreReason::PresentationFailed(_)
            | DockLiveUndockRestoreReason::RevealFailed(_)
            | DockLiveUndockRestoreReason::RoutePlacementFailed(_)
            | DockLiveUndockRestoreReason::PlacementFailed(_)
            | DockLiveUndockRestoreReason::ReleaseDeadlineExpired
            | DockLiveUndockRestoreReason::PromotionPreparationFailed
            | DockLiveUndockRestoreReason::DestinationTerminalBeforeCommit
            | DockLiveUndockRestoreReason::ProvisionalTerminal
            | DockLiveUndockRestoreReason::ForeignSurface
            | DockLiveUndockRestoreReason::RouteUnavailable => true,
        }
    }

    const fn restoration_retirement_reason(
        reason: DockLiveUndockRestoreReason,
    ) -> DockLiveUndockRetirementReason {
        match reason {
            DockLiveUndockRestoreReason::OpeningBindingInvalid => {
                DockLiveUndockRetirementReason::OpeningBindingInvalid
            }
            DockLiveUndockRestoreReason::RuntimeRegistrationRejected => {
                DockLiveUndockRetirementReason::RuntimeRegistrationRejected
            }
            DockLiveUndockRestoreReason::Shutdown => DockLiveUndockRetirementReason::Shutdown,
            reason => DockLiveUndockRetirementReason::SourceRestored(reason),
        }
    }

    fn begin_source_restoration(
        active: DockLiveUndockActive,
        reason: DockLiveUndockRestoreReason,
        restore_focus: bool,
        shutdown_dependency: DockLiveUndockShutdownDependency,
        effects: &mut DockLiveUndockEffects,
    ) -> DockLiveUndockState {
        let identity = active.identity();
        let (payload_lease, source_released) = match active.payload {
            DockLiveUndockPayloadState::AwaitingSourceProxy(payload_lease) => {
                (payload_lease, false)
            }
            DockLiveUndockPayloadState::AwaitingPayloadMount(proxy) => (proxy.lease(), true),
            DockLiveUndockPayloadState::Mounted(mount) => (mount.proxy().lease(), true),
            DockLiveUndockPayloadState::Unclaimed => {
                return DockLiveUndockState::Active(active);
            }
        };
        effects.push(DockLiveUndockEffect::RestoreSource {
            identity,
            source: payload_lease.source(),
            payload_lease,
            restore_focus,
        });
        let restoration = DockLiveUndockSourceRestoration {
            active,
            reason,
            restore_focus,
            shutdown_dependency,
        };
        if source_released {
            DockLiveUndockState::Restoring(restoration)
        } else {
            DockLiveUndockState::Compensating(restoration)
        }
    }

    fn finish_active(
        &mut self,
        active: DockLiveUndockActive,
        terminal: DockLiveUndockTerminalResult,
        retirement_reason: Option<DockLiveUndockRetirementReason>,
        shutdown_dependency: DockLiveUndockShutdownDependency,
        effects: &mut DockLiveUndockEffects,
    ) -> DockLiveUndockState {
        let identity = active.identity();

        let next = if let Some(reason) = retirement_reason {
            match active.provisional {
                DockLiveUndockProvisionalLifecycle::Opening => {
                    effects.push(DockLiveUndockEffect::ProvisionalRetirementRequired {
                        identity,
                        window: None,
                        dependency: shutdown_dependency.claimed(),
                        binding: None,
                        runtime: None,
                        reason,
                    });
                    DockLiveUndockState::Retiring(DockLiveUndockRetiring {
                        opening: active.opening,
                        window: None,
                        shutdown_dependency,
                        reason,
                    })
                }
                DockLiveUndockProvisionalLifecycle::Bound { window, runtime } => {
                    effects.push(DockLiveUndockEffect::ProvisionalRetirementRequired {
                        identity,
                        window: Some(window),
                        dependency: shutdown_dependency.claimed(),
                        binding: Some(DockLiveUndockOpeningBinding::ExactGated),
                        runtime: Some(runtime),
                        reason,
                    });
                    DockLiveUndockState::Retiring(DockLiveUndockRetiring {
                        opening: active.opening,
                        window: Some(window),
                        shutdown_dependency,
                        reason,
                    })
                }
                DockLiveUndockProvisionalLifecycle::Unavailable(window) => {
                    DockLiveUndockState::Retiring(DockLiveUndockRetiring {
                        opening: active.opening,
                        window: Some(window),
                        shutdown_dependency,
                        reason,
                    })
                }
                DockLiveUndockProvisionalLifecycle::Failed
                | DockLiveUndockProvisionalLifecycle::Terminal(_) => DockLiveUndockState::Idle,
            }
        } else {
            DockLiveUndockState::Idle
        };
        if matches!(next, DockLiveUndockState::Idle)
            && let DockLiveUndockShutdownDependency::Claimed(dependency) = shutdown_dependency
        {
            effects.push(DockLiveUndockEffect::SettleShutdownDependency {
                identity,
                dependency,
            });
        }
        self.publish_terminal_once(identity, terminal, effects);
        next
    }

    fn finish_shutdown_cleanup_failure(
        &mut self,
        active: DockLiveUndockActive,
        failure: DockLiveUndockShutdownFailure,
        shutdown_dependency: DockLiveUndockShutdownDependency,
        effects: &mut DockLiveUndockEffects,
    ) -> DockLiveUndockState {
        let identity = active.identity();
        let claimed_dependency = shutdown_dependency.claimed();
        let _ = self.finish_active(
            active,
            DockLiveUndockTerminalResult::ShutdownCleanupFailed(failure),
            Some(DockLiveUndockRetirementReason::Shutdown),
            DockLiveUndockShutdownDependency::Transferred,
            effects,
        );
        if let Some(dependency) = claimed_dependency {
            effects.push(DockLiveUndockEffect::FailShutdownDependency {
                identity,
                dependency,
                failure,
            });
        }
        DockLiveUndockState::ShutdownFailed { identity, failure }
    }

    fn reduce_shutdown_promotion_commit_wait(
        &mut self,
        mut waiting: DockLiveUndockShutdownPromotionCommitWait,
        fact: DockLiveUndockFact,
    ) -> (DockLiveUndockState, DockLiveUndockEffects) {
        let mut effects = DockLiveUndockEffects::default();
        match fact {
            DockLiveUndockFact::DurableSwapCommitted { token, .. } if token == waiting.token => {
                let Some(destination) = waiting
                    .active
                    .adopt_durable_promotion(token, Some(waiting.destination))
                else {
                    return (
                        DockLiveUndockState::WaitingForPromotionCommit(waiting),
                        effects,
                    );
                };
                let state = Self::begin_committed_destination_recovery(
                    waiting.active,
                    destination,
                    waiting.shutdown_dependency,
                    &mut effects,
                );
                (state, effects)
            }
            DockLiveUndockFact::CommittedDestinationRecoveryRequired {
                token, destination, ..
            } if token == waiting.token && destination == waiting.destination => {
                let Some(destination) = waiting
                    .active
                    .adopt_committed_destination_recovery(token, Some(destination))
                else {
                    return (
                        DockLiveUndockState::WaitingForPromotionCommit(waiting),
                        effects,
                    );
                };
                let state = Self::begin_committed_destination_recovery(
                    waiting.active,
                    destination,
                    waiting.shutdown_dependency,
                    &mut effects,
                );
                (state, effects)
            }
            DockLiveUndockFact::PromotionPreparationFailed { token, .. }
                if token == waiting.token =>
            {
                waiting.active.promotion = DockLiveUndockPromotionState::None;
                let state = if waiting.active.payload.lease().is_some() {
                    Self::begin_source_restoration(
                        waiting.active,
                        DockLiveUndockRestoreReason::Shutdown,
                        false,
                        waiting.shutdown_dependency,
                        &mut effects,
                    )
                } else {
                    self.finish_active(
                        waiting.active,
                        DockLiveUndockTerminalResult::Restored(
                            DockLiveUndockRestoreReason::Shutdown,
                        ),
                        Some(DockLiveUndockRetirementReason::Shutdown),
                        waiting.shutdown_dependency,
                        &mut effects,
                    )
                };
                (state, effects)
            }
            DockLiveUndockFact::WindowTerminal { window_id, .. }
                if waiting
                    .active
                    .owned_window()
                    .map(|window| window.window_id())
                    == Some(window_id) =>
            {
                effects.push(DockLiveUndockEffect::WindowTerminalSettled(
                    DockLiveUndockWindowTerminalOutcome {
                        lease: waiting.identity().opening.lease,
                        dependency: None,
                    },
                ));
                waiting.active.provisional =
                    DockLiveUndockProvisionalLifecycle::Terminal(window_id);
                (
                    DockLiveUndockState::WaitingForPromotionCommit(waiting),
                    effects,
                )
            }
            _ => (
                DockLiveUndockState::WaitingForPromotionCommit(waiting),
                effects,
            ),
        }
    }

    fn reduce_source_restoration(
        &mut self,
        mut restoration: DockLiveUndockSourceRestoration,
        fact: DockLiveUndockFact,
        source_released: bool,
    ) -> (DockLiveUndockState, DockLiveUndockEffects) {
        let identity = restoration.identity();
        let mut effects = DockLiveUndockEffects::default();
        match fact {
            DockLiveUndockFact::DurableSwapCommitted { token, .. } => {
                let Some(destination) = restoration.active.adopt_durable_promotion(token, None)
                else {
                    return (
                        Self::source_restoration_state(restoration, source_released),
                        effects,
                    );
                };
                let shutdown_dependency = restoration.shutdown_dependency;
                let state = Self::begin_committed_destination_recovery(
                    restoration.active,
                    destination,
                    shutdown_dependency,
                    &mut effects,
                );
                (state, effects)
            }
            DockLiveUndockFact::CommittedDestinationRecoveryRequired {
                token, destination, ..
            } => {
                let Some(destination) = restoration
                    .active
                    .adopt_committed_destination_recovery(token, Some(destination))
                else {
                    return (
                        Self::source_restoration_state(restoration, source_released),
                        effects,
                    );
                };
                let shutdown_dependency = restoration.shutdown_dependency;
                let state = Self::begin_committed_destination_recovery(
                    restoration.active,
                    destination,
                    shutdown_dependency,
                    &mut effects,
                );
                (state, effects)
            }
            DockLiveUndockFact::SourceRestorationCommitted { receipt, .. }
                if restoration.accepts_receipt(receipt)
                    && if source_released {
                        receipt.proves_source_presented_after_release()
                    } else {
                        receipt.proves_source_unchanged()
                    } =>
            {
                if restoration.restore_focus {
                    effects.push(DockLiveUndockEffect::RestoreSourceFocus {
                        identity,
                        source: receipt.source(),
                        payload_lease: receipt.payload_lease(),
                    });
                }
                let retirement_reason = Self::restoration_retirement_reason(restoration.reason);
                let terminal = DockLiveUndockTerminalResult::Restored(restoration.reason);
                let state = self.finish_active(
                    restoration.active,
                    terminal,
                    Some(retirement_reason),
                    restoration.shutdown_dependency,
                    &mut effects,
                );
                (state, effects)
            }
            DockLiveUndockFact::SourceRestorationDeferred {
                source,
                payload_lease,
                ..
            } if restoration.accepts(source, payload_lease) => (
                Self::source_restoration_state(restoration, source_released),
                effects,
            ),
            DockLiveUndockFact::SourceRestorationRetryElapsed {
                source,
                payload_lease,
                ..
            } if restoration.accepts(source, payload_lease) => {
                effects.push(DockLiveUndockEffect::RestoreSource {
                    identity,
                    source,
                    payload_lease,
                    restore_focus: restoration.restore_focus,
                });
                (
                    Self::source_restoration_state(restoration, source_released),
                    effects,
                )
            }
            DockLiveUndockFact::SourceWindowNativeTerminal { receipt }
                if restoration.active.accepts_source(receipt.source()) =>
            {
                let state = Self::begin_orphan_recovery(
                    restoration.active,
                    DockLiveUndockPayloadRecoveryCause::SourceNativeTerminal,
                    restoration.shutdown_dependency,
                    &mut effects,
                );
                (state, effects)
            }
            DockLiveUndockFact::PresentationAuthorityLost { receipt }
                if restoration.accepts(receipt.source(), receipt.payload_lease) =>
            {
                let state = Self::begin_orphan_recovery(
                    restoration.active,
                    DockLiveUndockPayloadRecoveryCause::PresentationAuthorityLost(receipt.cause()),
                    restoration.shutdown_dependency,
                    &mut effects,
                );
                (state, effects)
            }
            DockLiveUndockFact::WindowTerminal { window_id, .. }
                if restoration
                    .active
                    .owned_window()
                    .map(|window| window.window_id())
                    == Some(window_id) =>
            {
                effects.push(DockLiveUndockEffect::WindowTerminalSettled(
                    DockLiveUndockWindowTerminalOutcome {
                        lease: identity.opening.lease,
                        dependency: None,
                    },
                ));
                restoration.active.provisional =
                    DockLiveUndockProvisionalLifecycle::Terminal(window_id);
                (
                    Self::source_restoration_state(restoration, source_released),
                    effects,
                )
            }
            _ => (
                Self::source_restoration_state(restoration, source_released),
                effects,
            ),
        }
    }

    fn source_restoration_state(
        restoration: DockLiveUndockSourceRestoration,
        source_released: bool,
    ) -> DockLiveUndockState {
        if source_released {
            DockLiveUndockState::Restoring(restoration)
        } else {
            DockLiveUndockState::Compensating(restoration)
        }
    }

    fn reduce_orphan_recovery(
        &mut self,
        mut recovery: DockLiveUndockOrphanRecovery,
        fact: DockLiveUndockFact,
    ) -> (DockLiveUndockState, DockLiveUndockEffects) {
        let identity = recovery.identity();
        let mut effects = DockLiveUndockEffects::default();
        match fact {
            DockLiveUndockFact::DurableSwapCommitted { token, .. } => {
                let Some(destination) = recovery.active.adopt_durable_promotion(token, None) else {
                    return (DockLiveUndockState::RecoveringOrphan(recovery), effects);
                };
                let shutdown_dependency = recovery.shutdown_dependency;
                let state = Self::begin_committed_destination_recovery(
                    recovery.active,
                    destination,
                    shutdown_dependency,
                    &mut effects,
                );
                (state, effects)
            }
            DockLiveUndockFact::CommittedDestinationRecoveryRequired {
                token, destination, ..
            } => {
                let Some(destination) = recovery
                    .active
                    .adopt_committed_destination_recovery(token, Some(destination))
                else {
                    return (DockLiveUndockState::RecoveringOrphan(recovery), effects);
                };
                let shutdown_dependency = recovery.shutdown_dependency;
                let state = Self::begin_committed_destination_recovery(
                    recovery.active,
                    destination,
                    shutdown_dependency,
                    &mut effects,
                );
                (state, effects)
            }
            DockLiveUndockFact::OrphanRecoveryCommitted { receipt, .. }
            | DockLiveUndockFact::OrphanRecoveryFailed { receipt, .. }
                if recovery.accepts(receipt) =>
            {
                let state = self.finish_orphan_recovery(recovery, &mut effects);
                (state, effects)
            }
            DockLiveUndockFact::ShutdownOrphanCleanupCompleted { receipt }
                if recovery.accepts_cleanup(receipt)
                    && !matches!(
                        recovery.shutdown_dependency,
                        DockLiveUndockShutdownDependency::Unclaimed
                    ) =>
            {
                let state = self.finish_orphan_recovery(recovery, &mut effects);
                (state, effects)
            }
            DockLiveUndockFact::ShutdownOrphanCleanupFailed {
                identity: failed_identity,
                payload_lease,
                failure,
            } if failed_identity == identity
                && recovery.payload_lease == payload_lease
                && recovery.active.payload.lease() == Some(payload_lease)
                && !matches!(
                    recovery.shutdown_dependency,
                    DockLiveUndockShutdownDependency::Unclaimed
                ) =>
            {
                let state = self.finish_shutdown_cleanup_failure(
                    recovery.active,
                    DockLiveUndockShutdownFailure::OrphanCleanup(failure),
                    recovery.shutdown_dependency,
                    &mut effects,
                );
                (state, effects)
            }
            DockLiveUndockFact::WindowTerminal { window_id, .. }
                if recovery
                    .active
                    .owned_window()
                    .map(|window| window.window_id())
                    == Some(window_id) =>
            {
                effects.push(DockLiveUndockEffect::WindowTerminalSettled(
                    DockLiveUndockWindowTerminalOutcome {
                        lease: identity.opening.lease,
                        dependency: None,
                    },
                ));
                recovery.active.provisional =
                    DockLiveUndockProvisionalLifecycle::Terminal(window_id);
                (DockLiveUndockState::RecoveringOrphan(recovery), effects)
            }
            _ => (DockLiveUndockState::RecoveringOrphan(recovery), effects),
        }
    }

    fn finish_orphan_recovery(
        &mut self,
        recovery: DockLiveUndockOrphanRecovery,
        effects: &mut DockLiveUndockEffects,
    ) -> DockLiveUndockState {
        let (terminal, retirement) = match recovery.cause {
            DockLiveUndockPayloadRecoveryCause::SourceNativeTerminal => (
                DockLiveUndockTerminalResult::SourceLostBeforeCommit,
                DockLiveUndockRetirementReason::SourceLost,
            ),
            DockLiveUndockPayloadRecoveryCause::PresentationAuthorityLost(cause) => (
                DockLiveUndockTerminalResult::PresentationAuthorityLostBeforeCommit(cause),
                DockLiveUndockRetirementReason::PresentationAuthorityLost,
            ),
        };
        self.finish_active(
            recovery.active,
            terminal,
            Some(retirement),
            recovery.shutdown_dependency,
            effects,
        )
    }

    fn reduce_committed_destination_recovery(
        &mut self,
        mut recovery: DockLiveUndockCommittedDestinationRecovery,
        fact: DockLiveUndockFact,
    ) -> (DockLiveUndockState, DockLiveUndockEffects) {
        let identity = recovery.identity();
        let mut effects = DockLiveUndockEffects::default();
        match fact {
            DockLiveUndockFact::CommittedDestinationRecoveryCommitted { receipt, .. }
                if recovery.accepts(receipt) =>
            {
                if recovery.recovery_receipt.is_none() {
                    recovery.recovery_receipt = Some(receipt);
                }
                if let DockLiveUndockCommittedDestinationCleanup::Host {
                    target,
                    recovery_committed,
                } = &mut recovery.cleanup
                {
                    debug_assert_eq!(recovery.destination.window_id(), target.window_id());
                    *recovery_committed = true;
                }
                if let DockLiveUndockCommittedDestinationCleanup::SameWindow {
                    window_id,
                    terminal: false,
                    retirement_requested,
                } = &mut recovery.cleanup
                {
                    if receipt.same_window_terminal_required() {
                        if !*retirement_requested {
                            *retirement_requested = true;
                            effects.push(
                                DockLiveUndockEffect::RetireCommittedSameWindowDestination {
                                    identity,
                                    token: recovery.token,
                                    window_id: *window_id,
                                },
                            );
                        }
                    } else {
                        recovery.cleanup = DockLiveUndockCommittedDestinationCleanup::SameWindow {
                            window_id: *window_id,
                            terminal: true,
                            retirement_requested: *retirement_requested,
                        };
                    }
                }
            }
            DockLiveUndockFact::ShutdownCommittedDestinationRecoveryFailed {
                authority,
                token,
                destination,
                failure,
                ..
            } if recovery.authority == authority
                && recovery.token == token
                && recovery.destination == destination
                && recovery.active.committed_destination_recovery_promotion()
                    == Some((token, destination))
                && !matches!(
                    recovery.shutdown_dependency,
                    DockLiveUndockShutdownDependency::Unclaimed
                ) =>
            {
                let state = self.finish_shutdown_cleanup_failure(
                    recovery.active,
                    DockLiveUndockShutdownFailure::CommittedDestinationRecovery(failure),
                    recovery.shutdown_dependency,
                    &mut effects,
                );
                return (state, effects);
            }
            DockLiveUndockFact::WindowTerminal { window_id, .. }
                if matches!(
                    recovery.cleanup,
                    DockLiveUndockCommittedDestinationCleanup::SameWindow {
                        window_id: current,
                        terminal: false,
                        ..
                    } if current == window_id
                ) =>
            {
                effects.push(DockLiveUndockEffect::WindowTerminalSettled(
                    DockLiveUndockWindowTerminalOutcome {
                        lease: identity.opening.lease,
                        dependency: None,
                    },
                ));
                recovery.active.provisional =
                    DockLiveUndockProvisionalLifecycle::Terminal(window_id);
                let DockLiveUndockCommittedDestinationCleanup::SameWindow { terminal, .. } =
                    &mut recovery.cleanup
                else {
                    unreachable!("the exact same-window terminal guard must preserve its variant");
                };
                *terminal = true;
            }
            _ => {}
        }

        let cleanup_complete = matches!(
            recovery.cleanup,
            DockLiveUndockCommittedDestinationCleanup::SameWindow { terminal: true, .. }
                | DockLiveUndockCommittedDestinationCleanup::Host {
                    recovery_committed: true,
                    ..
                }
        );
        if recovery.recovery_receipt.is_some() && cleanup_complete {
            let destination = recovery.destination;
            let retirement = match recovery.shutdown_dependency {
                DockLiveUndockShutdownDependency::Unclaimed => {
                    matches!(destination, DockLiveUndockPromotionDestination::Host(_))
                        .then_some(DockLiveUndockRetirementReason::HostCommitted)
                }
                DockLiveUndockShutdownDependency::Claimed(_)
                | DockLiveUndockShutdownDependency::Transferred => {
                    Some(DockLiveUndockRetirementReason::Shutdown)
                }
            };
            let state = self.finish_active(
                recovery.active,
                DockLiveUndockTerminalResult::DestinationLostAfterCommit(destination),
                retirement,
                recovery.shutdown_dependency,
                &mut effects,
            );
            (state, effects)
        } else {
            (
                DockLiveUndockState::RecoveringCommittedDestination(recovery),
                effects,
            )
        }
    }

    fn publish_terminal_once(
        &mut self,
        identity: DockLiveUndockIdentity,
        result: DockLiveUndockTerminalResult,
        effects: &mut DockLiveUndockEffects,
    ) {
        let generation = identity.drag_generation.get();
        if generation <= self.last_terminal_drag_generation {
            return;
        }
        self.last_terminal_drag_generation = generation;
        effects.push(DockLiveUndockEffect::PublishTerminal { identity, result });
    }

    fn reduce_retiring(
        &mut self,
        mut retiring: DockLiveUndockRetiring,
        fact: DockLiveUndockFact,
    ) -> (DockLiveUndockState, DockLiveUndockEffects) {
        let identity = retiring.identity();
        let mut effects = DockLiveUndockEffects::default();
        match fact {
            DockLiveUndockFact::OpeningReturned {
                window,
                binding,
                runtime,
                ..
            } if retiring.window.is_none() => {
                retiring.window = Some(window);
                effects.push(DockLiveUndockEffect::ProvisionalRetirementRequired {
                    identity,
                    window: Some(window),
                    dependency: retiring.shutdown_dependency.claimed(),
                    binding: Some(binding),
                    runtime: Some(runtime),
                    reason: retiring.reason,
                });
                (DockLiveUndockState::Retiring(retiring), effects)
            }
            DockLiveUndockFact::OpeningFailed { .. } if retiring.window.is_none() => {
                effects.push(DockLiveUndockEffect::OpeningFailed {
                    identity,
                    dependency: retiring.shutdown_dependency.claimed(),
                });
                (DockLiveUndockState::Idle, effects)
            }
            DockLiveUndockFact::ShutdownDependencyTransferred { dependency, .. }
                if retiring.window.is_some()
                    && retiring.shutdown_dependency
                        == DockLiveUndockShutdownDependency::Claimed(dependency) =>
            {
                retiring.shutdown_dependency = DockLiveUndockShutdownDependency::Transferred;
                effects.push(DockLiveUndockEffect::ShutdownDependencyTransferred {
                    identity,
                    dependency,
                });
                (DockLiveUndockState::Retiring(retiring), effects)
            }
            DockLiveUndockFact::WindowTerminal { window_id, .. }
                if retiring.window.map(|window| window.window_id()) == Some(window_id) =>
            {
                effects.push(DockLiveUndockEffect::WindowTerminalSettled(
                    DockLiveUndockWindowTerminalOutcome {
                        lease: identity.opening.lease,
                        dependency: retiring.shutdown_dependency.claimed(),
                    },
                ));
                (DockLiveUndockState::Idle, effects)
            }
            _ => (DockLiveUndockState::Retiring(retiring), effects),
        }
    }

    fn apply_shutdown(
        &mut self,
        lease: DockSurfaceWindowSessionLease,
        promotion_commit: DockLiveUndockPromotionCommitDisposition,
    ) -> DockLiveUndockEffects {
        let state = std::mem::replace(&mut self.state, DockLiveUndockState::Idle);
        let mut effects = DockLiveUndockEffects::default();
        self.state = match state {
            DockLiveUndockState::Active(mut active) if active.identity().opening.lease == lease => {
                let identity = active.identity();
                let mut shutdown_dependency = DockLiveUndockShutdownDependency::Unclaimed;
                Self::freeze_active_shutdown_dependency(
                    &active,
                    &mut shutdown_dependency,
                    &mut effects,
                );
                if let Some((token, destination)) = promotion_commit.forward_only_for(identity)
                    && active.unproven_promotion_matches(token, destination)
                {
                    effects.push(DockLiveUndockEffect::CommitPreparedPromotion {
                        identity,
                        token,
                        destination,
                    });
                    DockLiveUndockState::WaitingForPromotionCommit(
                        DockLiveUndockShutdownPromotionCommitWait {
                            active,
                            token,
                            destination,
                            shutdown_dependency,
                        },
                    )
                } else {
                    let runtime_durable = promotion_commit.durable_for(identity);
                    let reducer_durable = active.durable_promotion();
                    let committed = runtime_durable.or(reducer_durable);
                    if let Some((token, destination)) = committed {
                        active
                            .adopt_durable_promotion(token, Some(destination))
                            .expect("runtime promotion commit must match the reducer promotion");
                        Self::begin_committed_destination_recovery(
                            active,
                            destination,
                            shutdown_dependency,
                            &mut effects,
                        )
                    } else if active.payload.lease().is_some() {
                        Self::begin_source_restoration(
                            active,
                            DockLiveUndockRestoreReason::Shutdown,
                            false,
                            shutdown_dependency,
                            &mut effects,
                        )
                    } else {
                        self.finish_active(
                            active,
                            DockLiveUndockTerminalResult::Restored(
                                DockLiveUndockRestoreReason::Shutdown,
                            ),
                            Some(DockLiveUndockRetirementReason::Shutdown),
                            shutdown_dependency,
                            &mut effects,
                        )
                    }
                }
            }
            DockLiveUndockState::Compensating(mut restoration)
                if restoration.identity().opening.lease == lease =>
            {
                if let Some((token, destination)) =
                    promotion_commit.forward_only_for(restoration.identity())
                    && restoration
                        .active
                        .unproven_promotion_matches(token, destination)
                {
                    Self::freeze_active_shutdown_dependency(
                        &restoration.active,
                        &mut restoration.shutdown_dependency,
                        &mut effects,
                    );
                    effects.push(DockLiveUndockEffect::CommitPreparedPromotion {
                        identity: restoration.identity(),
                        token,
                        destination,
                    });
                    DockLiveUndockState::WaitingForPromotionCommit(
                        DockLiveUndockShutdownPromotionCommitWait {
                            active: restoration.active,
                            token,
                            destination,
                            shutdown_dependency: restoration.shutdown_dependency,
                        },
                    )
                } else if let Some((token, destination)) =
                    promotion_commit.durable_for(restoration.identity())
                {
                    Self::freeze_active_shutdown_dependency(
                        &restoration.active,
                        &mut restoration.shutdown_dependency,
                        &mut effects,
                    );
                    restoration
                        .active
                        .adopt_durable_promotion(token, Some(destination))
                        .expect("runtime promotion commit must match source compensation");
                    let shutdown_dependency = restoration.shutdown_dependency;
                    Self::begin_committed_destination_recovery(
                        restoration.active,
                        destination,
                        shutdown_dependency,
                        &mut effects,
                    )
                } else {
                    Self::freeze_source_restoration_for_shutdown(&mut restoration, &mut effects);
                    DockLiveUndockState::Compensating(restoration)
                }
            }
            DockLiveUndockState::Restoring(mut restoration)
                if restoration.identity().opening.lease == lease =>
            {
                if let Some((token, destination)) =
                    promotion_commit.forward_only_for(restoration.identity())
                    && restoration
                        .active
                        .unproven_promotion_matches(token, destination)
                {
                    Self::freeze_active_shutdown_dependency(
                        &restoration.active,
                        &mut restoration.shutdown_dependency,
                        &mut effects,
                    );
                    effects.push(DockLiveUndockEffect::CommitPreparedPromotion {
                        identity: restoration.identity(),
                        token,
                        destination,
                    });
                    DockLiveUndockState::WaitingForPromotionCommit(
                        DockLiveUndockShutdownPromotionCommitWait {
                            active: restoration.active,
                            token,
                            destination,
                            shutdown_dependency: restoration.shutdown_dependency,
                        },
                    )
                } else if let Some((token, destination)) =
                    promotion_commit.durable_for(restoration.identity())
                {
                    Self::freeze_active_shutdown_dependency(
                        &restoration.active,
                        &mut restoration.shutdown_dependency,
                        &mut effects,
                    );
                    restoration
                        .active
                        .adopt_durable_promotion(token, Some(destination))
                        .expect("runtime promotion commit must match source restoration");
                    let shutdown_dependency = restoration.shutdown_dependency;
                    Self::begin_committed_destination_recovery(
                        restoration.active,
                        destination,
                        shutdown_dependency,
                        &mut effects,
                    )
                } else {
                    Self::freeze_source_restoration_for_shutdown(&mut restoration, &mut effects);
                    DockLiveUndockState::Restoring(restoration)
                }
            }
            DockLiveUndockState::RecoveringOrphan(mut recovery)
                if recovery.identity().opening.lease == lease =>
            {
                if let Some((token, destination)) =
                    promotion_commit.forward_only_for(recovery.identity())
                    && recovery
                        .active
                        .unproven_promotion_matches(token, destination)
                {
                    Self::freeze_active_shutdown_dependency(
                        &recovery.active,
                        &mut recovery.shutdown_dependency,
                        &mut effects,
                    );
                    effects.push(DockLiveUndockEffect::CommitPreparedPromotion {
                        identity: recovery.identity(),
                        token,
                        destination,
                    });
                    DockLiveUndockState::WaitingForPromotionCommit(
                        DockLiveUndockShutdownPromotionCommitWait {
                            active: recovery.active,
                            token,
                            destination,
                            shutdown_dependency: recovery.shutdown_dependency,
                        },
                    )
                } else if let Some((token, destination)) =
                    promotion_commit.durable_for(recovery.identity())
                {
                    Self::freeze_active_shutdown_dependency(
                        &recovery.active,
                        &mut recovery.shutdown_dependency,
                        &mut effects,
                    );
                    recovery
                        .active
                        .adopt_durable_promotion(token, Some(destination))
                        .expect("runtime promotion commit must match orphan recovery");
                    let shutdown_dependency = recovery.shutdown_dependency;
                    Self::begin_committed_destination_recovery(
                        recovery.active,
                        destination,
                        shutdown_dependency,
                        &mut effects,
                    )
                } else {
                    Self::freeze_orphan_recovery_for_shutdown(&mut recovery, &mut effects);
                    DockLiveUndockState::RecoveringOrphan(recovery)
                }
            }
            DockLiveUndockState::RecoveringCommittedDestination(mut recovery)
                if recovery.identity().opening.lease == lease =>
            {
                Self::freeze_committed_destination_recovery_for_shutdown(
                    &mut recovery,
                    &mut effects,
                );
                DockLiveUndockState::RecoveringCommittedDestination(recovery)
            }
            DockLiveUndockState::WaitingForPromotionCommit(mut waiting)
                if waiting.identity().opening.lease == lease =>
            {
                Self::freeze_active_shutdown_dependency(
                    &waiting.active,
                    &mut waiting.shutdown_dependency,
                    &mut effects,
                );
                DockLiveUndockState::WaitingForPromotionCommit(waiting)
            }
            DockLiveUndockState::Retiring(mut retiring)
                if retiring.identity().opening.lease == lease =>
            {
                let identity = retiring.identity();
                match retiring.shutdown_dependency {
                    DockLiveUndockShutdownDependency::Unclaimed => {
                        let dependency = DockSurfaceWindowSessionDependencyId::live_undock(
                            identity.opening.generation,
                        );
                        retiring.shutdown_dependency =
                            DockLiveUndockShutdownDependency::Claimed(dependency);
                        retiring.reason = DockLiveUndockRetirementReason::Shutdown;
                        effects.push(DockLiveUndockEffect::ShutdownFrozen(
                            DockLiveUndockShutdownSnapshot {
                                identity,
                                dependency,
                                window: retiring.window,
                            },
                        ));
                    }
                    DockLiveUndockShutdownDependency::Claimed(dependency) => {
                        effects.push(DockLiveUndockEffect::ShutdownFrozen(
                            DockLiveUndockShutdownSnapshot {
                                identity,
                                dependency,
                                window: retiring.window,
                            },
                        ));
                    }
                    DockLiveUndockShutdownDependency::Transferred => {}
                }
                DockLiveUndockState::Retiring(retiring)
            }
            state => state,
        };
        effects
    }

    fn freeze_active_shutdown_dependency(
        active: &DockLiveUndockActive,
        shutdown_dependency: &mut DockLiveUndockShutdownDependency,
        effects: &mut DockLiveUndockEffects,
    ) {
        let identity = active.identity();
        let dependency = match *shutdown_dependency {
            DockLiveUndockShutdownDependency::Unclaimed => {
                let dependency =
                    DockSurfaceWindowSessionDependencyId::live_undock(identity.opening.generation);
                *shutdown_dependency = DockLiveUndockShutdownDependency::Claimed(dependency);
                dependency
            }
            DockLiveUndockShutdownDependency::Claimed(dependency) => dependency,
            DockLiveUndockShutdownDependency::Transferred => return,
        };
        effects.push(DockLiveUndockEffect::ShutdownFrozen(
            DockLiveUndockShutdownSnapshot {
                identity,
                dependency,
                window: active.owned_window(),
            },
        ));
    }

    fn freeze_source_restoration_for_shutdown(
        restoration: &mut DockLiveUndockSourceRestoration,
        effects: &mut DockLiveUndockEffects,
    ) {
        restoration.reason = DockLiveUndockRestoreReason::Shutdown;
        restoration.restore_focus = false;
        let identity = restoration.identity();
        let dependency = match restoration.shutdown_dependency {
            DockLiveUndockShutdownDependency::Unclaimed => {
                let dependency =
                    DockSurfaceWindowSessionDependencyId::live_undock(identity.opening.generation);
                restoration.shutdown_dependency =
                    DockLiveUndockShutdownDependency::Claimed(dependency);
                dependency
            }
            DockLiveUndockShutdownDependency::Claimed(dependency) => dependency,
            DockLiveUndockShutdownDependency::Transferred => return,
        };
        effects.push(DockLiveUndockEffect::ShutdownFrozen(
            DockLiveUndockShutdownSnapshot {
                identity,
                dependency,
                window: restoration.active.owned_window(),
            },
        ));
        let payload_lease = restoration
            .active
            .payload
            .lease()
            .expect("source restoration must retain its exact payload lease");
        effects.push(DockLiveUndockEffect::ShutdownSourceRestorationRequired {
            identity,
            source: payload_lease.source(),
            payload_lease,
        });
    }

    fn freeze_orphan_recovery_for_shutdown(
        recovery: &mut DockLiveUndockOrphanRecovery,
        effects: &mut DockLiveUndockEffects,
    ) {
        let identity = recovery.identity();
        let dependency = match recovery.shutdown_dependency {
            DockLiveUndockShutdownDependency::Unclaimed => {
                let dependency =
                    DockSurfaceWindowSessionDependencyId::live_undock(identity.opening.generation);
                recovery.shutdown_dependency =
                    DockLiveUndockShutdownDependency::Claimed(dependency);
                dependency
            }
            DockLiveUndockShutdownDependency::Claimed(dependency) => dependency,
            DockLiveUndockShutdownDependency::Transferred => return,
        };
        effects.push(DockLiveUndockEffect::ShutdownFrozen(
            DockLiveUndockShutdownSnapshot {
                identity,
                dependency,
                window: recovery.active.owned_window(),
            },
        ));
        effects.push(DockLiveUndockEffect::ShutdownOrphanRecoveryRequired {
            identity,
            payload_lease: recovery.payload_lease,
            provisional: recovery.active.owned_window(),
        });
    }

    fn freeze_committed_destination_recovery_for_shutdown(
        recovery: &mut DockLiveUndockCommittedDestinationRecovery,
        effects: &mut DockLiveUndockEffects,
    ) {
        let identity = recovery.identity();
        let dependency = match recovery.shutdown_dependency {
            DockLiveUndockShutdownDependency::Unclaimed => {
                let dependency =
                    DockSurfaceWindowSessionDependencyId::live_undock(identity.opening.generation);
                recovery.shutdown_dependency =
                    DockLiveUndockShutdownDependency::Claimed(dependency);
                dependency
            }
            DockLiveUndockShutdownDependency::Claimed(dependency) => dependency,
            DockLiveUndockShutdownDependency::Transferred => return,
        };
        effects.push(DockLiveUndockEffect::ShutdownFrozen(
            DockLiveUndockShutdownSnapshot {
                identity,
                dependency,
                window: recovery.active.owned_window(),
            },
        ));
        effects.push(
            DockLiveUndockEffect::ShutdownCommittedDestinationRecoveryRequired {
                identity,
                authority: recovery.authority,
                token: recovery.token,
                destination: recovery.destination,
            },
        );
    }

    pub(crate) fn freeze_for_shutdown(
        &mut self,
        lease: DockSurfaceWindowSessionLease,
        promotion_commit: DockLiveUndockPromotionCommitDisposition,
    ) -> DockLiveUndockTransition<Option<DockLiveUndockShutdownSnapshot>> {
        let effects = self.apply_shutdown(lease, promotion_commit);
        let outcome = effects.as_slice().iter().find_map(|effect| match effect {
            DockLiveUndockEffect::ShutdownFrozen(snapshot) => Some(*snapshot),
            _ => None,
        });
        DockLiveUndockTransition::new(outcome, effects)
    }

    pub(crate) fn shutdown_snapshot(
        &self,
        lease: DockSurfaceWindowSessionLease,
    ) -> Option<DockLiveUndockShutdownSnapshot> {
        match &self.state {
            DockLiveUndockState::Active(active) if active.identity().opening.lease == lease => {
                let identity = active.identity();
                Some(DockLiveUndockShutdownSnapshot {
                    identity,
                    dependency: DockSurfaceWindowSessionDependencyId::live_undock(
                        identity.opening.generation,
                    ),
                    window: active.owned_window(),
                })
            }
            DockLiveUndockState::Compensating(restoration)
            | DockLiveUndockState::Restoring(restoration)
                if restoration.identity().opening.lease == lease =>
            {
                let identity = restoration.identity();
                let dependency = match restoration.shutdown_dependency {
                    DockLiveUndockShutdownDependency::Unclaimed => {
                        DockSurfaceWindowSessionDependencyId::live_undock(
                            identity.opening.generation,
                        )
                    }
                    DockLiveUndockShutdownDependency::Claimed(dependency) => dependency,
                    DockLiveUndockShutdownDependency::Transferred => return None,
                };
                Some(DockLiveUndockShutdownSnapshot {
                    identity,
                    dependency,
                    window: restoration.active.owned_window(),
                })
            }
            DockLiveUndockState::RecoveringOrphan(recovery)
                if recovery.identity().opening.lease == lease =>
            {
                let identity = recovery.identity();
                let dependency = match recovery.shutdown_dependency {
                    DockLiveUndockShutdownDependency::Unclaimed => {
                        DockSurfaceWindowSessionDependencyId::live_undock(
                            identity.opening.generation,
                        )
                    }
                    DockLiveUndockShutdownDependency::Claimed(dependency) => dependency,
                    DockLiveUndockShutdownDependency::Transferred => return None,
                };
                Some(DockLiveUndockShutdownSnapshot {
                    identity,
                    dependency,
                    window: recovery.active.owned_window(),
                })
            }
            DockLiveUndockState::WaitingForPromotionCommit(waiting)
                if waiting.identity().opening.lease == lease =>
            {
                Some(DockLiveUndockShutdownSnapshot {
                    identity: waiting.identity(),
                    dependency: waiting
                        .shutdown_dependency
                        .claimed()
                        .expect("promotion commit wait must retain its shutdown dependency"),
                    window: waiting.active.owned_window(),
                })
            }
            DockLiveUndockState::RecoveringCommittedDestination(recovery)
                if recovery.identity().opening.lease == lease =>
            {
                let identity = recovery.identity();
                let dependency = match recovery.shutdown_dependency {
                    DockLiveUndockShutdownDependency::Unclaimed => {
                        DockSurfaceWindowSessionDependencyId::live_undock(
                            identity.opening.generation,
                        )
                    }
                    DockLiveUndockShutdownDependency::Claimed(dependency) => dependency,
                    DockLiveUndockShutdownDependency::Transferred => return None,
                };
                Some(DockLiveUndockShutdownSnapshot {
                    identity,
                    dependency,
                    window: recovery.active.owned_window(),
                })
            }
            DockLiveUndockState::Retiring(retiring)
                if retiring.identity().opening.lease == lease =>
            {
                let identity = retiring.identity();
                let dependency = match retiring.shutdown_dependency {
                    DockLiveUndockShutdownDependency::Unclaimed => {
                        DockSurfaceWindowSessionDependencyId::live_undock(
                            identity.opening.generation,
                        )
                    }
                    DockLiveUndockShutdownDependency::Claimed(dependency) => dependency,
                    DockLiveUndockShutdownDependency::Transferred => return None,
                };
                Some(DockLiveUndockShutdownSnapshot {
                    identity,
                    dependency,
                    window: retiring.window,
                })
            }
            DockLiveUndockState::Idle
            | DockLiveUndockState::Active(_)
            | DockLiveUndockState::Compensating(_)
            | DockLiveUndockState::Restoring(_)
            | DockLiveUndockState::RecoveringOrphan(_)
            | DockLiveUndockState::WaitingForPromotionCommit(_)
            | DockLiveUndockState::RecoveringCommittedDestination(_)
            | DockLiveUndockState::ShutdownFailed { .. }
            | DockLiveUndockState::Retiring(_) => None,
        }
    }

    pub(crate) fn complete_opening(
        &mut self,
        key: DockLiveUndockOpeningKey,
        window: AnyWindowHandle,
        runtime: DockViewportProvisionalOpenAttemptCompletion,
    ) -> DockLiveUndockTransition<DockLiveUndockOpenReturnOutcome> {
        let Some((identity, binding)) = self.identity_and_binding(key, window.window_id()) else {
            return DockLiveUndockTransition::new(
                DockLiveUndockOpenReturnOutcome::Stale,
                DockLiveUndockEffects::default(),
            );
        };
        let effects = self.apply(DockLiveUndockFact::OpeningReturned {
            identity,
            window,
            binding,
            runtime,
        });
        let outcome = effects
            .as_slice()
            .iter()
            .find_map(|effect| match effect {
                DockLiveUndockEffect::ProvisionalAdmitted { lease, .. } => {
                    Some(DockLiveUndockOpenReturnOutcome::Admit { lease: *lease })
                }
                DockLiveUndockEffect::ProvisionalRetirementRequired {
                    reason: DockLiveUndockRetirementReason::RuntimeRegistrationRejected,
                    ..
                } => Some(
                    DockLiveUndockOpenReturnOutcome::RuntimeRegistrationRejected {
                        lease: key.lease,
                    },
                ),
                DockLiveUndockEffect::ProvisionalRetirementRequired {
                    dependency,
                    binding,
                    ..
                } => Some(DockLiveUndockOpenReturnOutcome::Retire {
                    lease: key.lease,
                    dependency: *dependency,
                    binding_valid: *binding == Some(DockLiveUndockOpeningBinding::ExactGated),
                }),
                _ => None,
            })
            .unwrap_or(DockLiveUndockOpenReturnOutcome::Stale);
        DockLiveUndockTransition::new(outcome, effects)
    }

    fn identity_and_binding(
        &self,
        key: DockLiveUndockOpeningKey,
        window_id: WindowId,
    ) -> Option<(DockLiveUndockIdentity, DockLiveUndockOpeningBinding)> {
        match &self.state {
            DockLiveUndockState::Active(active)
                if active.identity().opening == key
                    && active.provisional == DockLiveUndockProvisionalLifecycle::Opening =>
            {
                Some((active.identity(), active.opening.binding(window_id)))
            }
            DockLiveUndockState::Retiring(retiring)
                if retiring.identity().opening == key && retiring.window.is_none() =>
            {
                Some((retiring.identity(), retiring.opening.binding(window_id)))
            }
            DockLiveUndockState::Idle
            | DockLiveUndockState::Active(_)
            | DockLiveUndockState::Compensating(_)
            | DockLiveUndockState::Restoring(_)
            | DockLiveUndockState::RecoveringOrphan(_)
            | DockLiveUndockState::WaitingForPromotionCommit(_)
            | DockLiveUndockState::RecoveringCommittedDestination(_)
            | DockLiveUndockState::ShutdownFailed { .. }
            | DockLiveUndockState::Retiring(_) => None,
        }
    }

    pub(crate) fn can_admit_open_return(
        &self,
        key: DockLiveUndockOpeningKey,
        window_id: WindowId,
    ) -> bool {
        matches!(
            &self.state,
            DockLiveUndockState::Active(active)
                if active.identity().opening == key
                    && active.provisional == DockLiveUndockProvisionalLifecycle::Opening
                    && active.opening.binding(window_id)
                        == DockLiveUndockOpeningBinding::ExactGated
        )
    }

    pub(crate) fn fail_opening(
        &mut self,
        key: DockLiveUndockOpeningKey,
    ) -> DockLiveUndockTransition<DockLiveUndockOpenFailureOutcome> {
        let identity = match &self.state {
            DockLiveUndockState::Active(active) if active.identity().opening == key => {
                active.identity()
            }
            DockLiveUndockState::Retiring(retiring) if retiring.identity().opening == key => {
                retiring.identity()
            }
            DockLiveUndockState::Idle
            | DockLiveUndockState::Active(_)
            | DockLiveUndockState::Compensating(_)
            | DockLiveUndockState::Restoring(_)
            | DockLiveUndockState::RecoveringOrphan(_)
            | DockLiveUndockState::WaitingForPromotionCommit(_)
            | DockLiveUndockState::RecoveringCommittedDestination(_)
            | DockLiveUndockState::ShutdownFailed { .. }
            | DockLiveUndockState::Retiring(_) => {
                return DockLiveUndockTransition::new(
                    DockLiveUndockOpenFailureOutcome::Stale,
                    DockLiveUndockEffects::default(),
                );
            }
        };
        let effects = self.apply(DockLiveUndockFact::OpeningFailed { identity });
        let outcome = effects
            .as_slice()
            .iter()
            .find_map(|effect| match effect {
                DockLiveUndockEffect::OpeningFailed {
                    dependency: Some(dependency),
                    ..
                } => Some(DockLiveUndockOpenFailureOutcome::SettleDependency {
                    lease: key.lease,
                    dependency: *dependency,
                }),
                DockLiveUndockEffect::OpeningFailed {
                    dependency: None, ..
                } => Some(DockLiveUndockOpenFailureOutcome::Cleared),
                _ => None,
            })
            .unwrap_or(DockLiveUndockOpenFailureOutcome::Stale);
        DockLiveUndockTransition::new(outcome, effects)
    }

    pub(crate) fn transfer_shutdown_dependency_to_window(
        &mut self,
        key: DockLiveUndockOpeningKey,
        dependency: DockSurfaceWindowSessionDependencyId,
    ) -> bool {
        let identity = match &self.state {
            DockLiveUndockState::Retiring(retiring)
                if retiring.identity().opening == key
                    && retiring.window.is_some()
                    && retiring.shutdown_dependency
                        == DockLiveUndockShutdownDependency::Claimed(dependency) =>
            {
                retiring.identity()
            }
            _ => return false,
        };
        self.apply(DockLiveUndockFact::ShutdownDependencyTransferred {
            identity,
            dependency,
        })
        .into_iter()
        .any(|effect| {
            matches!(
                effect,
                DockLiveUndockEffect::ShutdownDependencyTransferred {
                    identity: current,
                    dependency: current_dependency,
                } if current == identity && current_dependency == dependency
            )
        })
    }

    pub(crate) fn has_shutdown_dependency(
        &self,
        key: DockLiveUndockOpeningKey,
        dependency: DockSurfaceWindowSessionDependencyId,
    ) -> bool {
        matches!(
            &self.state,
            DockLiveUndockState::Retiring(retiring)
                if retiring.identity().opening == key
                    && retiring.window.is_some()
                    && retiring.shutdown_dependency
                        == DockLiveUndockShutdownDependency::Claimed(dependency)
        )
    }

    pub(crate) fn lease_for_window(
        &self,
        window_id: WindowId,
    ) -> Option<DockSurfaceWindowSessionLease> {
        match &self.state {
            DockLiveUndockState::Active(active)
                if active.owned_window().map(|window| window.window_id()) == Some(window_id) =>
            {
                Some(active.identity().opening.lease)
            }
            DockLiveUndockState::Compensating(restoration)
            | DockLiveUndockState::Restoring(restoration)
                if restoration
                    .active
                    .owned_window()
                    .map(|window| window.window_id())
                    == Some(window_id) =>
            {
                Some(restoration.identity().opening.lease)
            }
            DockLiveUndockState::RecoveringOrphan(recovery)
                if recovery
                    .active
                    .owned_window()
                    .map(|window| window.window_id())
                    == Some(window_id) =>
            {
                Some(recovery.identity().opening.lease)
            }
            DockLiveUndockState::WaitingForPromotionCommit(waiting)
                if waiting
                    .active
                    .owned_window()
                    .map(|window| window.window_id())
                    == Some(window_id) =>
            {
                Some(waiting.identity().opening.lease)
            }
            DockLiveUndockState::RecoveringCommittedDestination(recovery)
                if recovery
                    .active
                    .owned_window()
                    .map(|window| window.window_id())
                    == Some(window_id) =>
            {
                Some(recovery.identity().opening.lease)
            }
            DockLiveUndockState::Retiring(retiring)
                if retiring.window.map(|window| window.window_id()) == Some(window_id) =>
            {
                Some(retiring.identity().opening.lease)
            }
            DockLiveUndockState::Idle
            | DockLiveUndockState::Active(_)
            | DockLiveUndockState::Compensating(_)
            | DockLiveUndockState::Restoring(_)
            | DockLiveUndockState::RecoveringOrphan(_)
            | DockLiveUndockState::WaitingForPromotionCommit(_)
            | DockLiveUndockState::RecoveringCommittedDestination(_)
            | DockLiveUndockState::ShutdownFailed { .. }
            | DockLiveUndockState::Retiring(_) => None,
        }
    }

    pub(crate) fn settle_window_terminal(
        &mut self,
        window_id: WindowId,
    ) -> DockLiveUndockTransition<Option<DockLiveUndockWindowTerminalOutcome>> {
        let identity = match &self.state {
            DockLiveUndockState::Active(active)
                if active.owned_window().map(|window| window.window_id()) == Some(window_id) =>
            {
                active.identity()
            }
            DockLiveUndockState::Compensating(restoration)
            | DockLiveUndockState::Restoring(restoration)
                if restoration
                    .active
                    .owned_window()
                    .map(|window| window.window_id())
                    == Some(window_id) =>
            {
                restoration.identity()
            }
            DockLiveUndockState::RecoveringOrphan(recovery)
                if recovery
                    .active
                    .owned_window()
                    .map(|window| window.window_id())
                    == Some(window_id) =>
            {
                recovery.identity()
            }
            DockLiveUndockState::WaitingForPromotionCommit(waiting)
                if waiting
                    .active
                    .owned_window()
                    .map(|window| window.window_id())
                    == Some(window_id) =>
            {
                waiting.identity()
            }
            DockLiveUndockState::RecoveringCommittedDestination(recovery)
                if recovery
                    .active
                    .owned_window()
                    .map(|window| window.window_id())
                    == Some(window_id) =>
            {
                recovery.identity()
            }
            DockLiveUndockState::Retiring(retiring)
                if retiring.window.map(|window| window.window_id()) == Some(window_id) =>
            {
                retiring.identity()
            }
            DockLiveUndockState::Idle
            | DockLiveUndockState::Active(_)
            | DockLiveUndockState::Compensating(_)
            | DockLiveUndockState::Restoring(_)
            | DockLiveUndockState::RecoveringOrphan(_)
            | DockLiveUndockState::WaitingForPromotionCommit(_)
            | DockLiveUndockState::RecoveringCommittedDestination(_)
            | DockLiveUndockState::ShutdownFailed { .. }
            | DockLiveUndockState::Retiring(_) => {
                return DockLiveUndockTransition::new(None, DockLiveUndockEffects::default());
            }
        };
        let effects = self.apply(DockLiveUndockFact::WindowTerminal {
            identity,
            window_id,
        });
        let outcome = effects.as_slice().iter().find_map(|effect| match effect {
            DockLiveUndockEffect::WindowTerminalSettled(outcome) => Some(*outcome),
            _ => None,
        });
        DockLiveUndockTransition::new(outcome, effects)
    }

    pub(crate) fn phase(&self) -> DockLiveUndockPhase {
        match &self.state {
            DockLiveUndockState::Idle => DockLiveUndockPhase::Idle,
            DockLiveUndockState::Active(active)
                if active.provisional == DockLiveUndockProvisionalLifecycle::Opening =>
            {
                DockLiveUndockPhase::Opening
            }
            DockLiveUndockState::Active(_) => DockLiveUndockPhase::Bound,
            DockLiveUndockState::Compensating(_) => DockLiveUndockPhase::Compensating,
            DockLiveUndockState::Restoring(_) => DockLiveUndockPhase::Restoring,
            DockLiveUndockState::RecoveringOrphan(_) => DockLiveUndockPhase::RecoveringOrphan,
            DockLiveUndockState::WaitingForPromotionCommit(_) => {
                DockLiveUndockPhase::WaitingForPromotionCommit
            }
            DockLiveUndockState::RecoveringCommittedDestination(_) => {
                DockLiveUndockPhase::RecoveringCommittedDestination
            }
            DockLiveUndockState::ShutdownFailed { failure, .. } => {
                let _ = failure;
                DockLiveUndockPhase::ShutdownCleanupFailed
            }
            DockLiveUndockState::Retiring(_) => DockLiveUndockPhase::Retiring,
        }
    }

    pub(crate) const fn current_identity(&self) -> Option<DockLiveUndockIdentity> {
        match &self.state {
            DockLiveUndockState::Active(active) => Some(active.identity()),
            DockLiveUndockState::Compensating(restoration)
            | DockLiveUndockState::Restoring(restoration) => Some(restoration.identity()),
            DockLiveUndockState::RecoveringOrphan(recovery) => Some(recovery.identity()),
            DockLiveUndockState::WaitingForPromotionCommit(waiting) => Some(waiting.identity()),
            DockLiveUndockState::RecoveringCommittedDestination(recovery) => {
                Some(recovery.identity())
            }
            DockLiveUndockState::ShutdownFailed { identity, .. } => Some(*identity),
            DockLiveUndockState::Retiring(retiring) => Some(retiring.identity()),
            DockLiveUndockState::Idle => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn current_presentation_failure_for_test(&self) -> Option<DockLiveUndockFact> {
        let DockLiveUndockState::Active(active) = &self.state else {
            return None;
        };
        if active.transport != DockLiveUndockTransportState::Moving {
            return None;
        }

        let failure =
            if let Some(presentation) = active.presentation.preflight {
                active.presentation.visible.is_none().then_some(
                    DockLiveUndockPresentationFailure::ExactRevealTicket { presentation },
                )?
            } else {
                match active.payload {
                    DockLiveUndockPayloadState::Unclaimed => {
                        DockLiveUndockPresentationFailure::PayloadLeaseClaim
                    }
                    DockLiveUndockPayloadState::AwaitingSourceProxy(lease) => {
                        DockLiveUndockPresentationFailure::SourceProxyReplay { lease }
                    }
                    DockLiveUndockPayloadState::AwaitingPayloadMount(proxy) => {
                        DockLiveUndockPresentationFailure::DestinationExposureFinish { proxy }
                    }
                    DockLiveUndockPayloadState::Mounted(mount) => {
                        DockLiveUndockPresentationFailure::PayloadPresentationObservation { mount }
                    }
                }
            };
        active.accepts_presentation_failure(failure).then_some(
            DockLiveUndockFact::PresentationStageFailed {
                identity: active.identity(),
                failure,
            },
        )
    }
}

impl Default for DockLiveUndockSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod promotion_commit_wait_tests {
    use super::*;
    use open_gpui::{Empty, EntityId, WindowHandle};

    fn prepared_session() -> (
        DockLiveUndockSession,
        DockLiveUndockIdentity,
        DockSurfaceWindowSessionLease,
        DockLiveUndockPromotionToken,
        DockLiveUndockPromotionDestination,
    ) {
        let mut window_session =
            super::super::window_session::DockSurfaceWindowSession::new(EntityId::from(41));
        let opening = window_session.reserve_opening().expect("G1 should reserve");
        let lease = window_session
            .commit_opening(opening, WindowId::from(42))
            .expect("G1 should activate");
        let identity = DockLiveUndockIdentity {
            opening: DockLiveUndockOpeningKey {
                lease,
                generation: 1,
            },
            drag_generation: DockLiveUndockDragGeneration::new(1)
                .expect("test drag generation is non-zero"),
        };
        let token = DockLiveUndockPromotionToken::new(1).expect("test promotion token is non-zero");
        let destination = DockLiveUndockPromotionDestination::SameWindowDesktop {
            window_id: WindowId::from(43),
        };
        let active = DockLiveUndockActive {
            opening: DockLiveUndockOpening {
                identity,
                provisional_session: WindowProvisionalSession::new(1)
                    .expect("test provisional generation is valid"),
            },
            source: DockLiveUndockSourceSnapshot::new(WindowId::from(44), 1),
            provisional: DockLiveUndockProvisionalLifecycle::Opening,
            transport: DockLiveUndockTransportState::Released(
                DockLiveUndockReleaseLock::new(
                    DockLiveUndockPhysicalPoint::new(10, 10),
                    DockLiveUndockRouteFeedback::Desktop,
                    DockLiveUndockPhysicalBounds::new(
                        DockLiveUndockPhysicalPoint::new(0, 0),
                        100,
                        100,
                    )
                    .expect("test release bounds are non-empty"),
                    DockLiveUndockPlacementGeneration::new(1)
                        .expect("test placement generation is non-zero"),
                )
                .expect("test release point must remain inside its physical bounds"),
            ),
            source_transport_proxy_active: false,
            route: Some(DockLiveUndockRouteFeedback::Desktop),
            route_generation: DockLiveUndockRouteGeneration::new(1)
                .expect("test route generation is non-zero"),
            route_point: DockLiveUndockPhysicalPoint::new(10, 10),
            route_bounds: DockLiveUndockPhysicalBounds::new(
                DockLiveUndockPhysicalPoint::new(0, 0),
                100,
                100,
            )
            .expect("test route bounds are non-empty"),
            route_placement_request_generation: None,
            payload: DockLiveUndockPayloadState::Unclaimed,
            presentation: DockLiveUndockPresentationObservation::default(),
            placement: None,
            placement_request_generation: None,
            promotion: DockLiveUndockPromotionState::Prepared { token, destination },
        };
        let session = DockLiveUndockSession {
            last_opening_generation: 1,
            last_triggered_drag_generation: 1,
            last_promotion_token: 1,
            last_terminal_drag_generation: 0,
            state: DockLiveUndockState::Active(active),
        };
        (session, identity, lease, token, destination)
    }

    fn bound_prepared_session() -> (
        DockLiveUndockSession,
        DockLiveUndockIdentity,
        DockSurfaceWindowSessionLease,
        DockLiveUndockPromotionToken,
        DockLiveUndockPromotionDestination,
    ) {
        let (mut session, identity, lease, token, destination) = prepared_session();
        let window: AnyWindowHandle = WindowHandle::<Empty>::new(destination.window_id()).into();
        let DockLiveUndockState::Active(active) = &mut session.state else {
            unreachable!("the prepared test session must be active");
        };
        active.provisional = DockLiveUndockProvisionalLifecycle::Bound {
            window,
            runtime: DockViewportProvisionalOpenAttemptCompletion::admitted_for_test(
                window.window_id(),
                identity.opening(),
            ),
        };
        (session, identity, lease, token, destination)
    }

    fn freeze_in_flight(
        session: &mut DockLiveUndockSession,
        identity: DockLiveUndockIdentity,
        lease: DockSurfaceWindowSessionLease,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
    ) -> DockLiveUndockEffects {
        session
            .freeze_for_shutdown(
                lease,
                DockLiveUndockPromotionCommitDisposition::ForwardOnly {
                    identity,
                    token,
                    destination,
                },
            )
            .into_parts()
            .1
    }

    #[test]
    fn active_shutdown_in_flight_only_freezes_and_commits() {
        let (mut session, identity, lease, token, destination) = prepared_session();
        let effects = freeze_in_flight(&mut session, identity, lease, token, destination);

        assert!(matches!(
            effects.as_slice(),
            [
                DockLiveUndockEffect::ShutdownFrozen(_),
                DockLiveUndockEffect::CommitPreparedPromotion {
                    identity: current_identity,
                    token: current_token,
                    destination: current_destination,
                },
            ] if *current_identity == identity
                && *current_token == token
                && *current_destination == destination
        ));
        assert_eq!(
            session.phase(),
            DockLiveUndockPhase::WaitingForPromotionCommit
        );
    }

    #[test]
    fn repeated_shutdown_freeze_replays_snapshot_without_restarting_commit() {
        let (mut session, identity, lease, token, destination) = prepared_session();
        let (first_snapshot, first_effects) = session
            .freeze_for_shutdown(
                lease,
                DockLiveUndockPromotionCommitDisposition::ForwardOnly {
                    identity,
                    token,
                    destination,
                },
            )
            .into_parts();
        assert!(
            first_effects.as_slice().iter().any(|effect| matches!(
                effect,
                DockLiveUndockEffect::CommitPreparedPromotion { .. }
            ))
        );

        let (repeated_snapshot, repeated_effects) = session
            .freeze_for_shutdown(
                lease,
                DockLiveUndockPromotionCommitDisposition::ForwardOnly {
                    identity,
                    token,
                    destination,
                },
            )
            .into_parts();

        assert_eq!(repeated_snapshot, first_snapshot);
        assert!(matches!(
            repeated_effects.as_slice(),
            [DockLiveUndockEffect::ShutdownFrozen(snapshot)]
                if Some(*snapshot) == first_snapshot
        ));
        assert_eq!(
            session.phase(),
            DockLiveUndockPhase::WaitingForPromotionCommit
        );
    }

    #[test]
    fn terminal_window_before_durable_is_preserved_for_committed_recovery() {
        let (mut session, identity, lease, token, destination) = bound_prepared_session();
        freeze_in_flight(&mut session, identity, lease, token, destination);

        let (terminal, terminal_effects) = session
            .settle_window_terminal(destination.window_id())
            .into_parts();
        assert!(terminal.is_some_and(|terminal| {
            terminal.lease() == lease && terminal.dependency().is_none()
        }));
        assert!(matches!(
            terminal_effects.as_slice(),
            [DockLiveUndockEffect::WindowTerminalSettled(_)]
        ));
        assert_eq!(
            session.phase(),
            DockLiveUndockPhase::WaitingForPromotionCommit
        );

        let effects = session.apply(DockLiveUndockFact::DurableSwapCommitted { identity, token });

        assert!(effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ShutdownCommittedDestinationRecoveryRequired { .. }
        )));
        assert!(!effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::RetireCommittedSameWindowDestination { .. }
                | DockLiveUndockEffect::WindowTerminalSettled(_)
        )));
        let DockLiveUndockState::RecoveringCommittedDestination(recovery) = &session.state else {
            panic!("durable promotion must begin committed destination recovery");
        };
        assert!(matches!(
            recovery.cleanup,
            DockLiveUndockCommittedDestinationCleanup::SameWindow {
                window_id,
                terminal: true,
                retirement_requested: false,
            } if window_id == destination.window_id()
        ));
    }

    #[test]
    fn terminal_window_before_forward_recovery_preserves_shutdown_dependency() {
        let (mut session, identity, lease, token, destination) = bound_prepared_session();
        freeze_in_flight(&mut session, identity, lease, token, destination);
        let (terminal, _) = session
            .settle_window_terminal(destination.window_id())
            .into_parts();
        assert!(terminal.is_some());

        let effects = session.apply(DockLiveUndockFact::CommittedDestinationRecoveryRequired {
            identity,
            token,
            destination,
        });

        assert!(effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ShutdownCommittedDestinationRecoveryRequired {
                identity: current_identity,
                token: current_token,
                destination: current_destination,
                ..
            } if *current_identity == identity
                && *current_token == token
                && *current_destination == destination
        )));
        assert!(!effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ProvisionalRetirementRequired { .. }
                | DockLiveUndockEffect::WindowTerminalSettled(_)
        )));
        assert_eq!(
            session.phase(),
            DockLiveUndockPhase::RecoveringCommittedDestination
        );
        let DockLiveUndockState::RecoveringCommittedDestination(recovery) = &session.state else {
            panic!("forward-only promotion must begin committed destination recovery");
        };
        assert!(matches!(
            recovery.cleanup,
            DockLiveUndockCommittedDestinationCleanup::SameWindow {
                window_id,
                terminal: true,
                retirement_requested: false,
            } if window_id == destination.window_id()
        ));
    }

    #[test]
    fn terminal_window_before_preparation_failure_settles_shutdown_without_retirement() {
        let (mut session, identity, lease, token, destination) = bound_prepared_session();
        freeze_in_flight(&mut session, identity, lease, token, destination);
        let (terminal, _) = session
            .settle_window_terminal(destination.window_id())
            .into_parts();
        assert!(terminal.is_some());

        let effects =
            session.apply(DockLiveUndockFact::PromotionPreparationFailed { identity, token });

        assert!(effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::SettleShutdownDependency { .. }
        )));
        assert!(effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                result: DockLiveUndockTerminalResult::Restored(
                    DockLiveUndockRestoreReason::Shutdown,
                ),
                ..
            }
        )));
        assert!(!effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ProvisionalRetirementRequired { .. }
                | DockLiveUndockEffect::WindowTerminalSettled(_)
        )));
        assert_eq!(session.phase(), DockLiveUndockPhase::Idle);
    }

    #[test]
    fn durable_fact_pivots_wait_to_committed_destination_recovery() {
        let (mut session, identity, lease, token, destination) = prepared_session();
        freeze_in_flight(&mut session, identity, lease, token, destination);

        let effects = session.apply(DockLiveUndockFact::DurableSwapCommitted { identity, token });

        assert!(effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ShutdownCommittedDestinationRecoveryRequired {
                identity: current_identity,
                token: current_token,
                destination: current_destination,
                ..
            } if *current_identity == identity
                && *current_token == token
                && *current_destination == destination
        )));
        assert!(!effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::RestoreSource { .. }
                | DockLiveUndockEffect::RecoverOrphanedPayloadTopology { .. }
        )));
        assert_eq!(
            session.phase(),
            DockLiveUndockPhase::RecoveringCommittedDestination
        );
    }

    #[test]
    fn forward_recovery_pivots_wait_without_source_restore() {
        let (mut session, identity, lease, token, destination) = prepared_session();
        freeze_in_flight(&mut session, identity, lease, token, destination);

        let effects = session.apply(DockLiveUndockFact::CommittedDestinationRecoveryRequired {
            identity,
            token,
            destination,
        });

        assert!(effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ShutdownCommittedDestinationRecoveryRequired {
                identity: current_identity,
                token: current_token,
                destination: current_destination,
                ..
            } if *current_identity == identity
                && *current_token == token
                && *current_destination == destination
        )));
        assert!(!effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::RestoreSource { .. }
                | DockLiveUndockEffect::ShutdownSourceRestorationRequired { .. }
                | DockLiveUndockEffect::RecoverOrphanedPayloadTopology { .. }
                | DockLiveUndockEffect::PublishTerminal { .. }
                | DockLiveUndockEffect::FailShutdownDependency { .. }
        )));
        assert_eq!(
            session.phase(),
            DockLiveUndockPhase::RecoveringCommittedDestination
        );
    }

    #[test]
    fn stale_token_does_not_resolve_promotion_commit_wait() {
        let (mut session, identity, lease, token, destination) = prepared_session();
        freeze_in_flight(&mut session, identity, lease, token, destination);
        let stale_token = DockLiveUndockPromotionToken::new(token.get() + 1)
            .expect("stale test token is non-zero");

        assert!(
            session
                .apply(DockLiveUndockFact::DurableSwapCommitted {
                    identity,
                    token: stale_token,
                })
                .is_empty()
        );
        assert!(
            session
                .apply(DockLiveUndockFact::CommittedDestinationRecoveryRequired {
                    identity,
                    token: stale_token,
                    destination,
                })
                .is_empty()
        );
        assert_eq!(
            session.phase(),
            DockLiveUndockPhase::WaitingForPromotionCommit
        );
    }

    #[test]
    fn active_forward_recovery_begins_committed_destination_recovery() {
        let (mut session, identity, _, token, destination) = prepared_session();

        let effects = session.apply(DockLiveUndockFact::CommittedDestinationRecoveryRequired {
            identity,
            token,
            destination,
        });

        assert!(effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::RecoverCommittedDestinationTopology {
                identity: current_identity,
                token: current_token,
                destination: current_destination,
                ..
            } if *current_identity == identity
                && *current_token == token
                && *current_destination == destination
        )));
        assert!(!effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::RestoreSource { .. }
                | DockLiveUndockEffect::RecoverOrphanedPayloadTopology { .. }
                | DockLiveUndockEffect::PublishTerminal { .. }
        )));
        assert_eq!(
            session.phase(),
            DockLiveUndockPhase::RecoveringCommittedDestination
        );
    }

    #[test]
    fn source_restoration_pivots_to_committed_destination_recovery() {
        let (mut session, identity, _, token, destination) = prepared_session();
        let DockLiveUndockState::Active(active) =
            std::mem::replace(&mut session.state, DockLiveUndockState::Idle)
        else {
            unreachable!("the prepared test session must be active");
        };
        session.state = DockLiveUndockState::Restoring(DockLiveUndockSourceRestoration {
            active,
            reason: DockLiveUndockRestoreReason::Cancelled(DockLiveUndockCancelReason::Escape),
            restore_focus: false,
            shutdown_dependency: DockLiveUndockShutdownDependency::Unclaimed,
        });

        let effects = session.apply(DockLiveUndockFact::CommittedDestinationRecoveryRequired {
            identity,
            token,
            destination,
        });

        assert!(effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::RecoverCommittedDestinationTopology {
                identity: current_identity,
                token: current_token,
                destination: current_destination,
                ..
            } if *current_identity == identity
                && *current_token == token
                && *current_destination == destination
        )));
        assert!(!effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::RestoreSource { .. }
                | DockLiveUndockEffect::RecoverOrphanedPayloadTopology { .. }
                | DockLiveUndockEffect::PublishTerminal { .. }
        )));
        assert_eq!(
            session.phase(),
            DockLiveUndockPhase::RecoveringCommittedDestination
        );
    }

    #[test]
    fn shutdown_orphan_recovery_pivots_to_committed_destination_recovery() {
        let (mut session, identity, _, token, destination) = prepared_session();
        let DockLiveUndockState::Active(mut active) =
            std::mem::replace(&mut session.state, DockLiveUndockState::Idle)
        else {
            unreachable!("the prepared test session must be active");
        };
        let payload_lease = DockLiveUndockPayloadLeaseReceipt::for_test(
            identity,
            active.source,
            DockLiveUndockPresentationLeaseGeneration::new(1)
                .expect("test presentation generation is non-zero"),
            destination.window_id(),
        );
        active.payload = DockLiveUndockPayloadState::AwaitingSourceProxy(payload_lease);
        let dependency = DockSurfaceWindowSessionDependencyId::live_undock(7);
        session.state = DockLiveUndockState::RecoveringOrphan(DockLiveUndockOrphanRecovery {
            active,
            payload_lease,
            cause: DockLiveUndockPayloadRecoveryCause::SourceNativeTerminal,
            shutdown_dependency: DockLiveUndockShutdownDependency::Claimed(dependency),
        });

        let effects = session.apply(DockLiveUndockFact::CommittedDestinationRecoveryRequired {
            identity,
            token,
            destination,
        });

        assert!(effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ShutdownCommittedDestinationRecoveryRequired {
                identity: current_identity,
                token: current_token,
                destination: current_destination,
                ..
            } if *current_identity == identity
                && *current_token == token
                && *current_destination == destination
        )));
        assert!(!effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::FailShutdownDependency { .. }
                | DockLiveUndockEffect::PublishTerminal { .. }
        )));
        let DockLiveUndockState::RecoveringCommittedDestination(recovery) = &session.state else {
            panic!("forward-only promotion must replace orphan recovery");
        };
        assert_eq!(
            recovery.shutdown_dependency,
            DockLiveUndockShutdownDependency::Claimed(dependency)
        );
        assert_eq!(
            session.phase(),
            DockLiveUndockPhase::RecoveringCommittedDestination
        );
    }
}
