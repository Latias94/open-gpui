use super::{
    DockSurfaceOwner,
    live_undock::{
        DockLiveUndockCommittedDestinationRecoveryFailure,
        DockLiveUndockCommittedDestinationRecoveryReceipt,
        DockLiveUndockDestinationInteractionReceipt, DockLiveUndockDestinationSemanticsReceipt,
        DockLiveUndockEffect, DockLiveUndockEffects, DockLiveUndockFact,
        DockLiveUndockHostCleanupEvidence, DockLiveUndockIdentity, DockLiveUndockOpenRequest,
        DockLiveUndockOrphanCleanupFailure, DockLiveUndockOrphanCleanupReceipt,
        DockLiveUndockOrphanRecoveryReceipt, DockLiveUndockPayloadLeaseReceipt,
        DockLiveUndockPayloadPresentationReceipt, DockLiveUndockPlacementGeneration,
        DockLiveUndockPresentationAuthorityLossReceipt, DockLiveUndockPresentationFailure,
        DockLiveUndockPromotionDestination, DockLiveUndockPromotionToken,
        DockLiveUndockRehostCleanupEvidence, DockLiveUndockReleaseLock,
        DockLiveUndockRetainedVisualCleanupEvidence, DockLiveUndockRevealObservation,
        DockLiveUndockRevealOutcome, DockLiveUndockRevealReceipt,
        DockLiveUndockSourceFocusSnapshot, DockLiveUndockSourceNativeTerminalReceipt,
        DockLiveUndockSourceRestorationFailure, DockLiveUndockSourceRestorationReceipt,
        DockLiveUndockSourceSnapshot, DockLiveUndockTrigger,
    },
    live_undock_pump::{
        DockLiveUndockDrainPermit, DockLiveUndockEffectPump, DockLiveUndockEnqueueResult,
        DockLiveUndockPumpCommand,
    },
    payload_recovery::{
        DockPayloadRecoveryAuthority, DockPayloadRecoveryCommitReceipt,
        DockPayloadRecoveryDisposition, DockPayloadRecoveryFocus, DockPayloadRecoveryPrepareError,
        DockPayloadRecoveryPrepared, DockPayloadRecoveryPresentationOrigin,
        DockPayloadRecoveryReason,
    },
};
use crate::{
    DockController, DockGraph, DockHost, DockHostWindowBinding, DockSpaceId,
    DockViewportDropPayload, DockViewportLockedDropRoute,
    DockViewportPreflightedLiveUndockHostDrop, DockViewportPreparedLiveUndockHostDrop,
    DockViewportRuntimeHandle, DockViewportRuntimeWorkContext, DockViewportTearOffRequest,
    DockViewportWindowFacts,
    drag::DockDragPayload,
    host::{
        DockHostLiveDestinationSemantics, DockHostLivePresentationKey,
        DockHostLiveSourceRestorationInstallOutcome, DockHostPreparedLiveDestinationPromotion,
        DockHostPreparedLivePresentationAbandonment, DockHostPreparedLiveSourceRetirement,
        DockHostPreparedLiveSourceSemanticRetirement,
    },
    host_render_session::DockHostPresentationSession,
    interaction::DockRuntimeDragSession,
    presentation_scene::DockPresentationScene,
    surface::live_payload_carrier::{DockLivePayloadCarrier, resolve_live_payload_carrier},
    viewport_drop_scene::DockViewportHostSceneFrame,
    viewport_runtime::DockViewportPreparedLiveUndockPromotion,
    viewport_tear_off_move::{DockViewportTearOffMovePlan, lock_tear_off_move},
};
use open_gpui::{
    AnyWindowHandle, App, AppContext, Bounds, Entity, SharedString, Subscription, WeakEntity,
    Window, WindowBounds, WindowHandle, WindowId, WindowInitialPresentationStatus,
    WindowMutationDispatch, WindowMutationOutcome, WindowOptions, WindowPlacementRequest,
    WindowProvisionalSemanticsOutcome, WindowProvisionalSemanticsTicket, WindowProvisionalSession,
    WindowProvisionalSessionPhase, point, px,
    retained_visual::{self, Ticket},
    size,
    view_presentation_window::{
        self, PreparedRehostTerminal, RehostProjection, RehostSession, RehostTerminalPreparation,
    },
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
    time::Duration,
};

const LIVE_UNDOCK_RELEASE_DEADLINE: Duration = Duration::from_millis(500);
const LIVE_UNDOCK_RETRY_CAP: Duration = Duration::from_millis(250);

enum DockLiveUndockQueuedFact {
    Reduce(DockLiveUndockFact),
    AdoptRelease {
        identity: DockLiveUndockIdentity,
        release: DockLiveUndockReleaseLock,
        finalizer: DockPayloadDragFinalizer,
        runtime: DockViewportRuntimeHandle,
        work_context: DockViewportRuntimeWorkContext,
        session: DockRuntimeDragSession,
    },
    Start {
        lease: super::window_session::DockSurfaceWindowSessionLease,
        trigger: DockLiveUndockTrigger,
        seed: DockLiveUndockPreparedSeed,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockLiveUndockRevealArmOutcome {
    Armed,
    WaitingForInitialPresentation,
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockPayloadDragFinalizerAuthority {
    Route,
    PendingLiveUndock(DockLiveUndockIdentity),
    LiveUndock(DockLiveUndockIdentity),
    Finalizing,
    SurfaceShutdown(super::window_session::DockSurfaceWindowSessionLease),
    Finalized,
}

#[derive(Clone, Debug)]
pub(crate) struct DockPayloadDragFinalizer {
    authority: Rc<Cell<DockPayloadDragFinalizerAuthority>>,
}

impl DockPayloadDragFinalizer {
    pub(crate) fn new() -> Self {
        Self {
            authority: Rc::new(Cell::new(DockPayloadDragFinalizerAuthority::Route)),
        }
    }

    pub(crate) fn same_token(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.authority, &other.authority)
    }

    fn begin_live_undock(&self, identity: DockLiveUndockIdentity) -> bool {
        if self.authority.get() != DockPayloadDragFinalizerAuthority::Route {
            return false;
        }
        self.authority
            .set(DockPayloadDragFinalizerAuthority::PendingLiveUndock(
                identity,
            ));
        true
    }

    fn commit_live_undock(&self, identity: DockLiveUndockIdentity) -> bool {
        if self.authority.get() != DockPayloadDragFinalizerAuthority::PendingLiveUndock(identity) {
            return false;
        }
        self.authority
            .set(DockPayloadDragFinalizerAuthority::LiveUndock(identity));
        true
    }

    fn rollback_live_undock(&self, identity: DockLiveUndockIdentity) -> bool {
        if self.authority.get() != DockPayloadDragFinalizerAuthority::PendingLiveUndock(identity) {
            return false;
        }
        self.authority.set(DockPayloadDragFinalizerAuthority::Route);
        true
    }

    pub(crate) fn claim_route(&self) -> Option<DockPayloadDragFinalizerClaim> {
        self.claim(DockPayloadDragFinalizerAuthority::Route)
    }

    fn claim_pending_live_undock(
        &self,
        identity: DockLiveUndockIdentity,
    ) -> Option<DockPayloadDragFinalizerClaim> {
        self.claim(DockPayloadDragFinalizerAuthority::PendingLiveUndock(
            identity,
        ))
    }

    fn claim_live_undock(
        &self,
        identity: DockLiveUndockIdentity,
    ) -> Option<DockPayloadDragFinalizerClaim> {
        self.claim(DockPayloadDragFinalizerAuthority::LiveUndock(identity))
    }

    fn claim_release_adoption(
        &self,
        identity: DockLiveUndockIdentity,
    ) -> Option<DockPayloadDragFinalizerClaim> {
        self.claim_pending_live_undock(identity)
            .or_else(|| self.claim_live_undock(identity))
    }

    fn transfer_to_surface_shutdown(
        &self,
        lease: super::window_session::DockSurfaceWindowSessionLease,
    ) -> Option<DockPayloadDragSurfaceShutdownFinalizer> {
        if self.authority.get() != DockPayloadDragFinalizerAuthority::Finalizing {
            return None;
        }
        self.authority
            .set(DockPayloadDragFinalizerAuthority::SurfaceShutdown(lease));
        Some(DockPayloadDragSurfaceShutdownFinalizer {
            finalizer: self.clone(),
            lease,
        })
    }

    fn claim(
        &self,
        expected: DockPayloadDragFinalizerAuthority,
    ) -> Option<DockPayloadDragFinalizerClaim> {
        if self.authority.get() != expected {
            return None;
        }
        self.authority
            .set(DockPayloadDragFinalizerAuthority::Finalizing);
        Some(DockPayloadDragFinalizerClaim {
            finalizer: self.clone(),
            prior: expected,
            completed: false,
        })
    }
}

pub(crate) struct DockPayloadDragFinalizerClaim {
    finalizer: DockPayloadDragFinalizer,
    prior: DockPayloadDragFinalizerAuthority,
    completed: bool,
}

impl DockPayloadDragFinalizerClaim {
    pub(crate) fn complete(mut self) {
        self.finalizer
            .authority
            .set(DockPayloadDragFinalizerAuthority::Finalized);
        self.completed = true;
    }

    pub(crate) fn transfer_to_surface_shutdown(
        mut self,
        lease: super::window_session::DockSurfaceWindowSessionLease,
    ) -> DockPayloadDragSurfaceShutdownFinalizer {
        let finalizer = self
            .finalizer
            .transfer_to_surface_shutdown(lease)
            .expect("a claimed payload finalizer must remain finalizing during shutdown transfer");
        self.completed = true;
        finalizer
    }
}

impl Drop for DockPayloadDragFinalizerClaim {
    fn drop(&mut self) {
        if !self.completed
            && self.finalizer.authority.get() == DockPayloadDragFinalizerAuthority::Finalizing
        {
            self.finalizer.authority.set(self.prior);
        }
    }
}

pub(crate) struct DockPayloadDragSurfaceShutdownFinalizer {
    finalizer: DockPayloadDragFinalizer,
    lease: super::window_session::DockSurfaceWindowSessionLease,
}

impl DockPayloadDragSurfaceShutdownFinalizer {
    pub(crate) fn same_token(&self, other: &Self) -> bool {
        self.lease == other.lease && self.finalizer.same_token(&other.finalizer)
    }

    pub(crate) fn complete(self) -> bool {
        if self.finalizer.authority.get()
            != DockPayloadDragFinalizerAuthority::SurfaceShutdown(self.lease)
        {
            return false;
        }
        self.finalizer
            .authority
            .set(DockPayloadDragFinalizerAuthority::Finalized);
        true
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DockLiveUndockReleaseDeadline {
    placement_generation: Option<DockLiveUndockPlacementGeneration>,
}

impl DockLiveUndockReleaseDeadline {
    fn arm(&mut self, placement_generation: DockLiveUndockPlacementGeneration) {
        self.placement_generation = Some(placement_generation);
    }

    fn claim_expiration(
        &mut self,
        placement_generation: DockLiveUndockPlacementGeneration,
    ) -> bool {
        if self.placement_generation != Some(placement_generation) {
            return false;
        }
        self.placement_generation = None;
        true
    }

    fn clear(&mut self) {
        self.placement_generation = None;
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DockLiveUndockRetryBackoff {
    generation: u64,
    attempts: u32,
    armed: bool,
}

impl DockLiveUndockRetryBackoff {
    fn arm_if_idle(&mut self) -> Option<(u64, Duration)> {
        if self.armed {
            return None;
        }
        self.generation = self
            .generation
            .checked_add(1)
            .expect("orphan-recovery retry generation space exhausted");
        self.attempts = self.attempts.saturating_add(1);
        self.armed = true;
        let shift = self.attempts.saturating_sub(1).min(4);
        let delay = Duration::from_millis(16_u64 << shift).min(LIVE_UNDOCK_RETRY_CAP);
        Some((self.generation, delay))
    }

    fn claim(&mut self, generation: u64) -> bool {
        if self.generation != generation || !self.armed {
            return false;
        }
        self.armed = false;
        true
    }

    fn clear(&mut self) {
        self.attempts = 0;
        self.armed = false;
    }
}

#[derive(Clone)]
pub(crate) struct DockLiveUndockExecutionSeed {
    runtime: DockViewportRuntimeHandle,
    work_context: DockViewportRuntimeWorkContext,
    session: DockRuntimeDragSession,
    payload: DockDragPayload,
    source_window: AnyWindowHandle,
    source_host: WeakEntity<DockHost>,
    source_binding: DockHostWindowBinding,
    source_transport: crate::native_captured_drag::DockNativeCapturedDragTransportLease,
    source_focus: Option<DockLiveUndockSourceFocusSnapshot>,
    source_frame: DockViewportHostSceneFrame,
    source_presentation_scene: DockPresentationScene,
    suggested_window_bounds: Option<WindowBounds>,
    identity_slot: Rc<Cell<Option<DockLiveUndockIdentity>>>,
    payload_finalizer: DockPayloadDragFinalizer,
}

impl DockLiveUndockExecutionSeed {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        runtime: DockViewportRuntimeHandle,
        work_context: DockViewportRuntimeWorkContext,
        session: DockRuntimeDragSession,
        payload: DockDragPayload,
        source_window: AnyWindowHandle,
        source_host: WeakEntity<DockHost>,
        source_binding: DockHostWindowBinding,
        source_transport: crate::native_captured_drag::DockNativeCapturedDragTransportLease,
        source_focus: Option<DockLiveUndockSourceFocusSnapshot>,
        source_frame: DockViewportHostSceneFrame,
        source_presentation_scene: DockPresentationScene,
        suggested_window_bounds: Option<WindowBounds>,
        identity_slot: Rc<Cell<Option<DockLiveUndockIdentity>>>,
        payload_finalizer: DockPayloadDragFinalizer,
    ) -> Self {
        Self {
            runtime,
            work_context,
            session,
            payload,
            source_window,
            source_host,
            source_binding,
            source_transport,
            source_focus,
            source_frame,
            source_presentation_scene,
            suggested_window_bounds,
            identity_slot,
            payload_finalizer,
        }
    }
}

pub(crate) struct DockLiveUndockHostReleaseAuthority {
    locked_drop: DockViewportLockedDropRoute,
    target_window: WindowHandle<DockHost>,
    target_host: WeakEntity<DockHost>,
    target_binding: DockHostWindowBinding,
    target_space: DockSpaceId,
    target_frame: DockViewportHostSceneFrame,
}

impl DockLiveUndockHostReleaseAuthority {
    pub(crate) fn try_new(
        locked_drop: DockViewportLockedDropRoute,
        target_window: WindowHandle<DockHost>,
        target_host: WeakEntity<DockHost>,
        target_binding: DockHostWindowBinding,
        target_space: DockSpaceId,
        target_frame: DockViewportHostSceneFrame,
    ) -> Result<Self, DockViewportLockedDropRoute> {
        if !locked_drop.is_workspace()
            || target_window.window_id() != target_binding.window_id()
            || !target_frame.matches_viewport(&target_space, target_window.window_id())
        {
            return Err(locked_drop);
        }
        Ok(Self {
            locked_drop,
            target_window,
            target_host,
            target_binding,
            target_space,
            target_frame,
        })
    }

    fn matches_release(
        &self,
        target: super::live_undock::DockLiveUndockHostTarget,
        session: &DockRuntimeDragSession,
    ) -> bool {
        self.locked_drop.drag_session() == session
            && self.target_window.window_id() == target.window_id()
            && self.target_frame.generation() == target.host_scene_generation()
    }

    pub(crate) fn into_locked_drop(self) -> DockViewportLockedDropRoute {
        self.locked_drop
    }
}

pub(crate) enum DockLiveUndockReleaseAdoption {
    Adopted,
    Rejected(Option<DockLiveUndockHostReleaseAuthority>),
}

struct DockLiveUndockExecution {
    seed: DockLiveUndockPreparedSeed,
    request: DockLiveUndockOpenRequest,
    surface_revision: u64,
    release_deadline: DockLiveUndockReleaseDeadline,
    release_placement: Option<DockLiveUndockReleasePlacementExecution>,
    observed_release_placement: Option<DockLiveUndockObservedReleasePlacement>,
    destination_host: Option<WindowHandle<DockHost>>,
    host_release: Option<DockLiveUndockHostReleaseAuthority>,
    presentation: Option<DockLiveUndockPresentationExecution>,
    promotion: Option<DockLiveUndockPromotionExecution>,
    source_restoration_retry: DockLiveUndockRetryBackoff,
    orphan_recovery_retry: DockLiveUndockRetryBackoff,
    committed_destination_recovery_retry: DockLiveUndockRetryBackoff,
}

struct DockLiveUndockReleasePlacementExecution {
    window_id: WindowId,
    generation: DockLiveUndockPlacementGeneration,
    subscription: Option<Subscription>,
}

#[derive(Clone, Copy)]
struct DockLiveUndockObservedReleasePlacement {
    window_id: WindowId,
    generation: DockLiveUndockPlacementGeneration,
    facts: DockViewportWindowFacts,
}

struct DockLiveUndockPresentationExecution {
    carrier: DockLivePayloadCarrier,
    retained: Ticket,
    projection: RehostProjection,
    session: DockLiveUndockRehostSessionState,
    lease: DockLiveUndockPayloadLeaseReceipt,
    source_key: Option<DockHostLivePresentationKey>,
    destination_key: Option<DockHostLivePresentationKey>,
    reveal: Option<DockLiveUndockRevealReceipt>,
    retained_released: bool,
    source_restoration_batch: Option<view_presentation_window::LeaseBatch>,
    source_restoration_receipt: Option<DockLiveUndockSourceRestorationReceipt>,
    restore_focus: bool,
    source_focus_restored: bool,
}

enum DockLiveUndockRehostSessionState {
    Active(RehostSession),
    CheckedOut,
    Retired,
}

impl DockLiveUndockRehostSessionState {
    fn active(&self) -> Option<&RehostSession> {
        match self {
            Self::Active(session) => Some(session),
            Self::CheckedOut | Self::Retired => None,
        }
    }

    fn checkout(&mut self) -> Option<RehostSession> {
        let current = std::mem::replace(self, Self::CheckedOut);
        match current {
            Self::Active(session) => Some(session),
            Self::CheckedOut | Self::Retired => {
                *self = current;
                None
            }
        }
    }

    fn restore(&mut self, session: RehostSession, projection: &RehostProjection) {
        assert!(
            matches!(self, Self::CheckedOut) && projection.matches_exactly(&session.projection()),
            "checked-out live-undock rehost authority must return to its exact execution"
        );
        *self = Self::Active(session);
    }

    fn retire_terminal(&mut self) -> bool {
        let Self::Active(session) = self else {
            return false;
        };
        if !session.is_terminal() {
            return false;
        }
        *self = Self::Retired;
        true
    }

    fn is_active(&self) -> bool {
        matches!(self, Self::Active(_))
    }

    fn is_checked_out(&self) -> bool {
        matches!(self, Self::CheckedOut)
    }
}

#[derive(Clone)]
struct DockLiveUndockPresentationSnapshot {
    carrier: DockLivePayloadCarrier,
    retained: Ticket,
    projection: RehostProjection,
}

impl DockLiveUndockPresentationExecution {
    fn snapshot(&self) -> DockLiveUndockPresentationSnapshot {
        DockLiveUndockPresentationSnapshot {
            carrier: self.carrier.clone(),
            retained: self.retained,
            projection: self.projection.clone(),
        }
    }

    fn checkout_session(&mut self) -> Option<RehostSession> {
        self.session.checkout()
    }

    fn restore_session(&mut self, session: RehostSession) {
        self.session.restore(session, &self.projection);
    }
}

#[derive(Clone)]
struct DockLiveUndockSourceRestorationExecution {
    identity: DockLiveUndockIdentity,
    source: DockLiveUndockSourceSnapshot,
    payload_lease: DockLiveUndockPayloadLeaseReceipt,
    source_window: AnyWindowHandle,
    source_host: WeakEntity<DockHost>,
    source_key: Option<DockHostLivePresentationKey>,
    restore_session: DockHostPresentationSession,
    destination_host: Option<WindowHandle<DockHost>>,
    destination_key: Option<DockHostLivePresentationKey>,
    projection: RehostProjection,
    retained: Ticket,
    retained_released: bool,
    source_restoration_batch: Option<view_presentation_window::LeaseBatch>,
    source_restoration_receipt: Option<DockLiveUndockSourceRestorationReceipt>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockSourceFinishOutcome {
    Finished,
    AuthorityLossSubmitted,
    Retry,
}

enum DockLiveUndockPreparedHostPresentationAbandonment {
    Exact {
        host: Entity<DockHost>,
        prepared: DockHostPreparedLivePresentationAbandonment,
    },
    AlreadyAbsent {
        host: Entity<DockHost>,
        identity: DockLiveUndockIdentity,
        window_id: WindowId,
    },
    HostUnavailable {
        window_id: WindowId,
    },
}

enum DockLiveUndockPreparedPresentationCleanup {
    Exact(PreparedRehostTerminal),
    AlreadyTerminal(DockLiveUndockRehostCleanupEvidence),
}

enum DockLiveUndockPreparedRetainedVisualCleanup {
    AlreadyReleased(Ticket),
    Exact {
        source_window: AnyWindowHandle,
        ticket: Ticket,
        prepared: retained_visual::PreparedRelease,
    },
    AuthorityAbsent {
        source_window: AnyWindowHandle,
        ticket: Ticket,
    },
    WindowUnavailable {
        source_window: AnyWindowHandle,
        ticket: Ticket,
    },
}

struct DockLiveUndockPreparedOrphanCleanup {
    identity: DockLiveUndockIdentity,
    payload_lease: DockLiveUndockPayloadLeaseReceipt,
    runtime: DockViewportRuntimeHandle,
    presentation: DockLiveUndockPreparedPresentationCleanup,
    presentation_generation: u64,
    source_host: DockLiveUndockPreparedHostPresentationAbandonment,
    destination_host: DockLiveUndockPreparedHostPresentationAbandonment,
    retained: DockLiveUndockPreparedRetainedVisualCleanup,
    source_transport_host: WeakEntity<DockHost>,
    transport: crate::native_captured_drag::DockNativeCapturedDragTransportLease,
}

struct DockLiveUndockPreparedOrphanRecoveryExecution {
    recovery: DockLiveUndockPreparedRecoveryRecord,
    cleanup: DockLiveUndockPreparedOrphanCleanup,
}

struct DockLiveUndockPreparedCommittedDestinationRecoveryExecution {
    identity: DockLiveUndockIdentity,
    authority: DockPayloadRecoveryAuthority,
    presentation_lease: Option<DockLiveUndockPayloadLeaseReceipt>,
    token: DockLiveUndockPromotionToken,
    destination: DockLiveUndockPromotionDestination,
    runtime: DockViewportRuntimeHandle,
    recovery: DockLiveUndockPreparedRecoveryRecord,
    source_semantic: Option<DockLiveUndockPreparedSourceSemanticRetirement>,
    destination_window: WindowHandle<DockHost>,
}

enum DockLiveUndockPreparedRecoveryRecord {
    Prepared(DockPayloadRecoveryPrepared),
    AlreadyCommitted(DockPayloadRecoveryCommitReceipt),
}

enum DockLiveUndockPreparedSourceSemanticRetirement {
    Exact {
        host: Entity<DockHost>,
        prepared: DockHostPreparedLiveSourceSemanticRetirement,
    },
    AlreadyAbsent {
        host: Entity<DockHost>,
        key: DockHostLivePresentationKey,
        lease: DockLiveUndockPayloadLeaseReceipt,
    },
    HostUnavailable,
}

enum DockLiveUndockPromotionExecution {
    Prepared(DockLiveUndockPreparedPromotionExecution),
    Durable(DockLiveUndockDurablePromotionExecution),
}

enum DockLiveUndockPreparedPromotionExecution {
    SameWindow(DockLiveUndockPreparedSameWindowPromotionExecution),
    Host(DockLiveUndockPreparedHostPromotionExecution),
}

struct DockLiveUndockPreparedSameWindowPromotionExecution {
    identity: DockLiveUndockIdentity,
    token: DockLiveUndockPromotionToken,
    destination: DockLiveUndockPromotionDestination,
    release: DockLiveUndockReleaseLock,
    surface_revision: u64,
    controller: Entity<DockController>,
    move_plan: DockViewportTearOffMovePlan,
    runtime: DockViewportRuntimeHandle,
    viewport: DockViewportPreparedLiveUndockPromotion,
    source_window: AnyWindowHandle,
    retained_release: retained_visual::PreparedRelease,
    source_host: Entity<DockHost>,
    source: DockHostPreparedLiveSourceRetirement,
    destination_host: Entity<DockHost>,
    destination_host_promotion: DockHostPreparedLiveDestinationPromotion,
    presentation: RehostTerminalPreparation,
    reveal: DockLiveUndockRevealReceipt,
    provisional_session: WindowProvisionalSession,
    semantics: WindowProvisionalSemanticsTicket,
}

struct DockLiveUndockPreparedHostPromotionExecution {
    identity: DockLiveUndockIdentity,
    token: DockLiveUndockPromotionToken,
    destination: DockLiveUndockPromotionDestination,
    release: DockLiveUndockReleaseLock,
    surface_revision: u64,
    controller: Entity<DockController>,
    runtime: DockViewportRuntimeHandle,
    work_context: DockViewportRuntimeWorkContext,
    drop: DockViewportPreparedLiveUndockHostDrop,
    target_window: WindowHandle<DockHost>,
    target_host: Entity<DockHost>,
    target_binding: DockHostWindowBinding,
    target_registration: crate::viewport_registry::DockViewportRegistrationKey,
    target_frame: DockViewportHostSceneFrame,
    presentation_cleanup: Option<DockLiveUndockPreparedHostPromotionPresentationCleanup>,
}

struct DockLiveUndockPreflightedHostPromotionExecution {
    identity: DockLiveUndockIdentity,
    token: DockLiveUndockPromotionToken,
    destination: DockLiveUndockPromotionDestination,
    release: DockLiveUndockReleaseLock,
    surface_revision: u64,
    runtime: DockViewportRuntimeHandle,
    drop: DockViewportPreflightedLiveUndockHostDrop,
    target_window: WindowHandle<DockHost>,
    target_host: Entity<DockHost>,
    target_binding: DockHostWindowBinding,
    target_registration: crate::viewport_registry::DockViewportRegistrationKey,
    presentation_cleanup: Option<DockLiveUndockPreparedHostPromotionPresentationCleanup>,
}

struct DockLiveUndockPreparedHostPromotionPresentationCleanup {
    payload_lease: DockLiveUndockPayloadLeaseReceipt,
    presentation_generation: u64,
    presentation: DockLiveUndockPreparedPresentationCleanup,
    source_host: DockLiveUndockPreparedHostPresentationAbandonment,
    provisional_host: DockLiveUndockPreparedHostPresentationAbandonment,
    source_window: AnyWindowHandle,
    retained_release: Option<retained_visual::PreparedRelease>,
}

enum DockLiveUndockDurablePromotionExecution {
    SameWindow(DockLiveUndockDurableSameWindowPromotionExecution),
    Host(DockLiveUndockDurableHostPromotionExecution),
}

struct DockLiveUndockDurableSameWindowPromotionExecution {
    identity: DockLiveUndockIdentity,
    token: DockLiveUndockPromotionToken,
    destination: DockLiveUndockPromotionDestination,
    surface_revision: u64,
    destination_window: WindowHandle<DockHost>,
    destination_binding: DockHostWindowBinding,
    registration: crate::viewport_registry::DockViewportRegistrationKey,
    reveal: DockLiveUndockRevealReceipt,
    provisional_session: WindowProvisionalSession,
    semantics: WindowProvisionalSemanticsTicket,
}

struct DockLiveUndockDurableHostPromotionExecution {
    identity: DockLiveUndockIdentity,
    token: DockLiveUndockPromotionToken,
    destination: DockLiveUndockPromotionDestination,
    destination_window: WindowHandle<DockHost>,
    destination_host: WeakEntity<DockHost>,
    destination_binding: DockHostWindowBinding,
    registration: crate::viewport_registry::DockViewportRegistrationKey,
    activation: Option<crate::DockViewportActivationTransaction>,
    pending_window_effects: Option<crate::DockViewportWindowEffects>,
}

impl DockLiveUndockPreparedPromotionExecution {
    const fn identity(&self) -> DockLiveUndockIdentity {
        match self {
            Self::SameWindow(prepared) => prepared.identity,
            Self::Host(prepared) => prepared.identity,
        }
    }

    const fn token(&self) -> DockLiveUndockPromotionToken {
        match self {
            Self::SameWindow(prepared) => prepared.token,
            Self::Host(prepared) => prepared.token,
        }
    }

    const fn destination(&self) -> DockLiveUndockPromotionDestination {
        match self {
            Self::SameWindow(prepared) => prepared.destination,
            Self::Host(prepared) => prepared.destination,
        }
    }

    const fn surface_revision(&self) -> u64 {
        match self {
            Self::SameWindow(prepared) => prepared.surface_revision,
            Self::Host(prepared) => prepared.surface_revision,
        }
    }
}

impl DockLiveUndockDurablePromotionExecution {
    const fn identity(&self) -> DockLiveUndockIdentity {
        match self {
            Self::SameWindow(durable) => durable.identity,
            Self::Host(durable) => durable.identity,
        }
    }

    const fn token(&self) -> DockLiveUndockPromotionToken {
        match self {
            Self::SameWindow(durable) => durable.token,
            Self::Host(durable) => durable.token,
        }
    }

    const fn destination(&self) -> DockLiveUndockPromotionDestination {
        match self {
            Self::SameWindow(durable) => durable.destination,
            Self::Host(durable) => durable.destination,
        }
    }

    const fn destination_window(&self) -> WindowHandle<DockHost> {
        match self {
            Self::SameWindow(durable) => durable.destination_window,
            Self::Host(durable) => durable.destination_window,
        }
    }

    const fn destination_binding(&self) -> DockHostWindowBinding {
        match self {
            Self::SameWindow(durable) => durable.destination_binding,
            Self::Host(durable) => durable.destination_binding,
        }
    }

    fn registration(&self) -> &crate::viewport_registry::DockViewportRegistrationKey {
        match self {
            Self::SameWindow(durable) => &durable.registration,
            Self::Host(durable) => &durable.registration,
        }
    }
}

#[derive(Default)]
struct DockLiveUndockRuntimeState {
    owner: Option<WeakEntity<DockSurfaceOwner>>,
    executions: HashMap<DockLiveUndockIdentity, DockLiveUndockExecution>,
    #[cfg(test)]
    reject_next_destination_interaction_admission: bool,
    #[cfg(test)]
    replace_source_host_after_finish_once: bool,
    #[cfg(test)]
    reject_orphan_recovery_records: bool,
    #[cfg(test)]
    interrupt_orphan_cleanup_after_recovery_commit_once: bool,
    #[cfg(test)]
    reject_committed_destination_recovery_records: bool,
    #[cfg(test)]
    terminate_next_same_window_destination_before_semantics_ack: bool,
    #[cfg(test)]
    before_destination_interaction_activation_test_hook: Option<Box<dyn FnOnce(&mut App)>>,
}

struct DockLiveUndockPreparedSeed {
    source: DockLiveUndockExecutionSeed,
    surface_revision: u64,
    target_space: DockSpaceId,
    move_plan: DockViewportTearOffMovePlan,
    source_session: DockHostPresentationSession,
    restore_session: DockHostPresentationSession,
    payload_session: DockHostPresentationSession,
}

#[derive(Clone)]
pub(crate) struct DockLiveUndockRuntime {
    pump: DockLiveUndockEffectPump<DockLiveUndockQueuedFact, DockLiveUndockEffects>,
    state: Rc<RefCell<DockLiveUndockRuntimeState>>,
}

struct DockLiveUndockRehostSessionCheckout {
    state: Rc<RefCell<DockLiveUndockRuntimeState>>,
    identity: DockLiveUndockIdentity,
    lease: DockLiveUndockPayloadLeaseReceipt,
    session: Option<RehostSession>,
}

impl DockLiveUndockRehostSessionCheckout {
    fn session(&self) -> &RehostSession {
        self.session
            .as_ref()
            .expect("live-undock rehost checkout must retain its session")
    }

    fn session_mut(&mut self) -> &mut RehostSession {
        self.session
            .as_mut()
            .expect("live-undock rehost checkout must retain its session")
    }
}

impl Drop for DockLiveUndockRehostSessionCheckout {
    fn drop(&mut self) {
        let Some(session) = self.session.take() else {
            return;
        };
        let mut state = self.state.borrow_mut();
        let presentation = state
            .executions
            .get_mut(&self.identity)
            .and_then(|execution| execution.presentation.as_mut())
            .filter(|presentation| presentation.lease == self.lease)
            .expect("checked-out live-undock rehost execution cannot retire before return");
        presentation.restore_session(session);
    }
}

impl fmt::Debug for DockLiveUndockRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DockLiveUndockRuntime")
            .field("pump", &self.pump)
            .field("execution_count", &self.state.borrow().executions.len())
            .finish()
    }
}

pub(crate) fn live_undock_host_presentation_released(
    owner: WeakEntity<DockSurfaceOwner>,
    source_key: DockHostLivePresentationKey,
    cx: &mut App,
) {
    cx.defer(move |cx| {
        let Ok(runtime) = owner.read_with(cx, |owner, _| owner.live_undock_runtime()) else {
            return;
        };
        runtime.source_host_presentation_released(source_key, cx);
    });
}

pub(crate) fn live_undock_destination_reveal_released(
    owner: WeakEntity<DockSurfaceOwner>,
    key: DockHostLivePresentationKey,
    receipt: DockLiveUndockPayloadPresentationReceipt,
    cx: &mut App,
) {
    cx.defer(move |cx| {
        let Ok(runtime) = owner.read_with(cx, |owner, _| owner.live_undock_runtime()) else {
            return;
        };
        runtime.submit(
            DockLiveUndockFact::RevealObserved {
                identity: key.identity(),
                observation: DockLiveUndockRevealObservation::failed(
                    receipt,
                    DockLiveUndockRevealOutcome::WindowTerminal,
                ),
            },
            cx,
        );
    });
}

impl DockLiveUndockRuntime {
    pub(crate) fn new() -> Self {
        Self {
            pump: DockLiveUndockEffectPump::new(),
            state: Rc::new(RefCell::new(DockLiveUndockRuntimeState::default())),
        }
    }

    #[cfg(test)]
    pub(crate) fn execution_count_for_test(&self) -> usize {
        self.state.borrow().executions.len()
    }

    #[cfg(test)]
    pub(crate) fn current_rehost_session_is_active_for_test(&self) -> bool {
        self.state.borrow().executions.values().any(|execution| {
            execution
                .presentation
                .as_ref()
                .is_some_and(|presentation| presentation.session.is_active())
        })
    }

    #[cfg(test)]
    pub(crate) fn panic_with_current_rehost_session_checked_out_for_test(&self) {
        let (identity, lease) = self
            .state
            .borrow()
            .executions
            .iter()
            .find_map(|(identity, execution)| {
                execution
                    .presentation
                    .as_ref()
                    .filter(|presentation| presentation.session.is_active())
                    .map(|presentation| (*identity, presentation.lease))
            })
            .expect("the test requires one active live-undock rehost session");
        let _checkout = self
            .checkout_presentation_session(identity, lease)
            .expect("the active test session must check out exactly once");
        panic!("injected panic while live-undock rehost authority is checked out");
    }

    #[cfg(test)]
    pub(crate) fn reject_orphan_recovery_records_for_test(&self) {
        self.state.borrow_mut().reject_orphan_recovery_records = true;
    }

    #[cfg(test)]
    pub(crate) fn interrupt_orphan_cleanup_after_recovery_commit_once_for_test(&self) {
        self.state
            .borrow_mut()
            .interrupt_orphan_cleanup_after_recovery_commit_once = true;
    }

    #[cfg(test)]
    pub(crate) fn reject_committed_destination_recovery_records_for_test(&self) {
        self.state
            .borrow_mut()
            .reject_committed_destination_recovery_records = true;
    }

    #[cfg(test)]
    pub(crate) fn orphan_cleanup_authority_for_test(
        &self,
    ) -> Option<(
        RehostProjection,
        Ticket,
        crate::native_captured_drag::DockNativeCapturedDragTransportLease,
    )> {
        self.state
            .borrow()
            .executions
            .values()
            .find_map(|execution| {
                let presentation = execution.presentation.as_ref()?;
                Some((
                    presentation.projection.clone(),
                    presentation.retained,
                    execution.seed.source.source_transport.clone(),
                ))
            })
    }

    pub(crate) fn committed_destination_registration_for_logical_close(
        &self,
        identity: DockLiveUndockIdentity,
        window_id: open_gpui::WindowId,
    ) -> Option<crate::viewport_registry::DockViewportRegistrationKey> {
        let state = self.state.borrow();
        let execution = state.executions.get(&identity)?;
        match execution.promotion.as_ref()? {
            DockLiveUndockPromotionExecution::Durable(
                DockLiveUndockDurablePromotionExecution::SameWindow(durable),
            ) if durable.identity == identity
                && durable.destination_window.window_id() == window_id =>
            {
                Some(durable.registration.clone())
            }
            DockLiveUndockPromotionExecution::Durable(
                DockLiveUndockDurablePromotionExecution::Host(durable),
            ) if durable.identity == identity
                && durable.destination_window.window_id() == window_id =>
            {
                Some(durable.registration.clone())
            }
            DockLiveUndockPromotionExecution::Prepared(_)
            | DockLiveUndockPromotionExecution::Durable(_) => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn reject_next_destination_interaction_admission_for_test(&self) {
        self.state
            .borrow_mut()
            .reject_next_destination_interaction_admission = true;
    }

    #[cfg(test)]
    pub(crate) fn replace_source_host_after_finish_once_for_test(&self) {
        self.state
            .borrow_mut()
            .replace_source_host_after_finish_once = true;
    }

    #[cfg(test)]
    pub(crate) fn terminate_next_same_window_destination_before_semantics_ack_for_test(&self) {
        self.state
            .borrow_mut()
            .terminate_next_same_window_destination_before_semantics_ack = true;
    }

    #[cfg(test)]
    pub(crate) fn install_before_destination_interaction_activation_hook_for_test(
        &self,
        hook: impl FnOnce(&mut App) + 'static,
    ) {
        let mut state = self.state.borrow_mut();
        assert!(
            state
                .before_destination_interaction_activation_test_hook
                .is_none(),
            "dock live-undock destination-interaction activation test hook is already installed"
        );
        state.before_destination_interaction_activation_test_hook = Some(Box::new(hook));
    }

    #[cfg(test)]
    pub(crate) fn take_replace_source_host_after_finish_for_test(&self) -> bool {
        std::mem::take(
            &mut self
                .state
                .borrow_mut()
                .replace_source_host_after_finish_once,
        )
    }

    #[cfg(test)]
    pub(crate) fn fail_current_presentation_for_test(&self, cx: &mut App) -> bool {
        let owner = self.state.borrow().owner.clone();
        let Some(fact) = owner.and_then(|owner| {
            owner
                .read_with(cx, |owner, _| {
                    owner.current_live_undock_presentation_failure_for_test()
                })
                .ok()
                .flatten()
        }) else {
            return false;
        };
        self.submit(fact, cx)
    }

    pub(crate) fn bind_owner(&self, owner: WeakEntity<DockSurfaceOwner>) {
        {
            let mut state = self.state.borrow_mut();
            assert!(
                state.owner.is_none(),
                "live-undock runtime owner is already bound"
            );
            state.owner = Some(owner.clone());
        }
        self.pump.bind_owner(owner);
    }

    pub(crate) fn start(
        &self,
        lease: super::window_session::DockSurfaceWindowSessionLease,
        trigger: DockLiveUndockTrigger,
        seed: DockLiveUndockExecutionSeed,
        cx: &mut App,
    ) -> bool {
        if trigger.source().window_id() != seed.source_window.window_id()
            || trigger.source().scene_generation() != seed.source_frame.generation()
            || seed.work_context.lineage() != crate::DockViewportRuntimeLineage::Surface(lease)
            || !seed.source_transport.is_active()
            || seed.source_transport.key().source_binding() != seed.source_binding
            || seed.source_transport.key().runtime_identity() != seed.runtime.identity()
            || seed.source_frame.registration_key().lineage()
                != crate::DockViewportRuntimeLineage::Surface(lease)
            || !seed
                .source_frame
                .matches_viewport(&seed.payload.source_space, seed.source_window.window_id())
            || !seed
                .runtime
                .is_current_viewport_host_scene_frame(&seed.source_frame)
        {
            return false;
        }
        let Some(seed) = self.prepare_seed(lease, trigger, seed, cx) else {
            return false;
        };
        self.enqueue_fact(
            DockLiveUndockQueuedFact::Start {
                lease,
                trigger,
                seed,
            },
            cx,
        )
    }

    fn prepare_seed(
        &self,
        lease: super::window_session::DockSurfaceWindowSessionLease,
        trigger: DockLiveUndockTrigger,
        seed: DockLiveUndockExecutionSeed,
        cx: &mut App,
    ) -> Option<DockLiveUndockPreparedSeed> {
        let owner = self
            .state
            .borrow()
            .owner
            .as_ref()
            .and_then(WeakEntity::upgrade)?;
        let (controller, surface_revision) =
            cx.read_entity(&owner, |owner, _| (owner.controller(), owner.revision()));
        let target_space = DockSpaceId::new(format!(
            "{}::live-undock::{}::{}",
            seed.payload.source_space,
            lease.generation(),
            trigger.drag_generation().get(),
        ));
        let tear_off_geometry = seed
            .runtime
            .active_payload_drag_tear_off_geometry(Some(&seed.session));
        let request = DockViewportTearOffRequest::new(
            seed.payload.source_space.clone(),
            seed.payload.source_node,
            DockViewportDropPayload::from_drag_payload(&seed.payload),
            None,
            seed.suggested_window_bounds.clone(),
        )
        .with_drag_session(Some(seed.session.clone()))
        .with_tear_off_geometry(tear_off_geometry);
        let projection: Result<_, crate::DockActionApplyError> =
            cx.read_entity(&controller, |controller, _| {
                let workspace = controller.workspace();
                let move_plan = lock_tear_off_move(workspace, &request, &target_space)?;
                let (graph, changed) = move_plan.project_graph(workspace)?;
                if !changed {
                    return Ok(None);
                }
                let source_session = DockHostPresentationSession::live_payload_projection(
                    seed.payload.source_space.clone(),
                    &graph,
                    workspace,
                );
                let restore_session = DockHostPresentationSession::from_graph(
                    seed.payload.source_space.clone(),
                    workspace.graph(),
                    workspace,
                );
                let payload_session = DockHostPresentationSession::live_payload_projection(
                    target_space.clone(),
                    &graph,
                    workspace,
                );
                Ok(Some((
                    move_plan,
                    source_session,
                    restore_session,
                    payload_session,
                )))
            });
        let (move_plan, source_session, restore_session, payload_session) = match projection {
            Ok(Some(projection)) => projection,
            Ok(None) => return None,
            Err(error) => {
                log::debug!("live-undock projection failed closed: {error}");
                return None;
            }
        };
        if !seed
            .runtime
            .is_current_viewport_host_scene_frame(&seed.source_frame)
        {
            return None;
        }
        Some(DockLiveUndockPreparedSeed {
            source: seed,
            surface_revision,
            target_space,
            move_plan,
            source_session,
            restore_session,
            payload_session,
        })
    }

    pub(crate) fn submit(&self, fact: DockLiveUndockFact, cx: &mut App) -> bool {
        self.enqueue_fact(DockLiveUndockQueuedFact::Reduce(fact), cx)
    }

    pub(crate) fn source_window_native_terminal(
        &self,
        window_id: open_gpui::WindowId,
        cx: &mut App,
    ) {
        let receipts = self
            .state
            .borrow()
            .executions
            .iter()
            .filter_map(|(identity, execution)| {
                let source = DockLiveUndockSourceSnapshot::new(
                    execution.seed.source.source_window.window_id(),
                    execution.seed.source.source_frame.generation(),
                );
                DockLiveUndockSourceNativeTerminalReceipt::from_native_terminal(
                    *identity, source, window_id,
                )
            })
            .collect::<Vec<_>>();
        for receipt in receipts {
            let _ = self.submit(
                DockLiveUndockFact::SourceWindowNativeTerminal { receipt },
                cx,
            );
        }
    }

    pub(crate) fn adopt_release(
        &self,
        identity: DockLiveUndockIdentity,
        release: DockLiveUndockReleaseLock,
        mut host_release: Option<DockLiveUndockHostReleaseAuthority>,
        runtime: &DockViewportRuntimeHandle,
        work_context: DockViewportRuntimeWorkContext,
        session: &DockRuntimeDragSession,
        payload: &DockDragPayload,
        finalizer: &DockPayloadDragFinalizer,
        cx: &mut App,
    ) -> DockLiveUndockReleaseAdoption {
        let release_authority_matches = match release.hit() {
            super::live_undock::DockLiveUndockRouteFeedback::Host(target) => host_release
                .as_ref()
                .is_some_and(|authority| authority.matches_release(target, session)),
            _ => host_release.is_none(),
        };
        let execution_matches =
            self.state
                .borrow()
                .executions
                .get(&identity)
                .is_some_and(|execution| {
                    execution.seed.source.runtime.identity() == runtime.identity()
                        && execution.seed.source.work_context == work_context
                        && execution.seed.source.session == *session
                        && execution.seed.source.payload == *payload
                        && execution
                            .seed
                            .source
                            .payload_finalizer
                            .same_token(finalizer)
                        && execution.host_release.is_none()
                });
        if !release_authority_matches
            || !execution_matches
            || !finalizer.begin_live_undock(identity)
        {
            return DockLiveUndockReleaseAdoption::Rejected(host_release);
        }
        {
            let mut state = self.state.borrow_mut();
            let execution = state
                .executions
                .get_mut(&identity)
                .expect("validated release adoption must retain its execution");
            execution.host_release = host_release.take();
        }
        match self
            .pump
            .enqueue_fact(DockLiveUndockQueuedFact::AdoptRelease {
                identity,
                release,
                finalizer: finalizer.clone(),
                runtime: runtime.clone(),
                work_context,
                session: session.clone(),
            }) {
            DockLiveUndockEnqueueResult::Accepted { drain_permit } => {
                self.schedule(drain_permit, cx);
                DockLiveUndockReleaseAdoption::Adopted
            }
            DockLiveUndockEnqueueResult::OwnerUnavailable(DockLiveUndockPumpCommand::Fact(
                DockLiveUndockQueuedFact::AdoptRelease {
                    identity: rejected_identity,
                    finalizer: rejected_finalizer,
                    ..
                },
            )) => {
                debug_assert_eq!(rejected_identity, identity);
                debug_assert!(rejected_finalizer.same_token(finalizer));
                host_release = self
                    .state
                    .borrow_mut()
                    .executions
                    .get_mut(&identity)
                    .and_then(|execution| execution.host_release.take());
                let restored = finalizer.rollback_live_undock(identity);
                debug_assert!(
                    restored,
                    "failed release enqueue must restore route authority"
                );
                DockLiveUndockReleaseAdoption::Rejected(host_release)
            }
            DockLiveUndockEnqueueResult::OwnerUnavailable(_) => {
                unreachable!("fact enqueue must return the exact offered release-adoption fact")
            }
        }
    }

    pub(crate) fn enqueue_effects(&self, effects: DockLiveUndockEffects, cx: &mut App) {
        match self.pump.enqueue_effects(effects) {
            DockLiveUndockEnqueueResult::Accepted { drain_permit } => {
                self.schedule(drain_permit, cx);
            }
            DockLiveUndockEnqueueResult::OwnerUnavailable(DockLiveUndockPumpCommand::Effects(
                effects,
            )) => {
                debug_assert!(
                    effects.is_empty(),
                    "live-undock effects must not outlive their surface owner"
                );
            }
            DockLiveUndockEnqueueResult::OwnerUnavailable(_) => {
                unreachable!("effect enqueue must return the exact offered effect batch")
            }
        }
    }

    fn enqueue_fact(&self, fact: DockLiveUndockQueuedFact, cx: &mut App) -> bool {
        match self.pump.enqueue_fact(fact) {
            DockLiveUndockEnqueueResult::Accepted { drain_permit } => {
                self.schedule(drain_permit, cx);
                true
            }
            DockLiveUndockEnqueueResult::OwnerUnavailable(DockLiveUndockPumpCommand::Fact(
                fact,
            )) => {
                debug_assert!(
                    !matches!(fact, DockLiveUndockQueuedFact::AdoptRelease { .. }),
                    "release adoption must use its compensation-aware enqueue path"
                );
                false
            }
            DockLiveUndockEnqueueResult::OwnerUnavailable(_) => {
                unreachable!("fact enqueue must return the exact offered fact")
            }
        }
    }

    fn schedule(
        &self,
        permit: Option<DockLiveUndockDrainPermit<DockLiveUndockQueuedFact, DockLiveUndockEffects>>,
        cx: &mut App,
    ) {
        let Some(permit) = permit else {
            return;
        };
        let runtime = self.clone();
        cx.defer_shutdown_critical_before_window_registry_clear_or_run_now(move |cx| {
            runtime.drain(permit, cx)
        });
    }

    fn drain(
        &self,
        permit: DockLiveUndockDrainPermit<DockLiveUndockQueuedFact, DockLiveUndockEffects>,
        cx: &mut App,
    ) {
        let mut first_panic = None;
        permit.drain(|owner, command, _| match command {
            DockLiveUndockPumpCommand::Fact(fact) => {
                let pending_release = match &fact {
                    DockLiveUndockQueuedFact::AdoptRelease {
                        identity,
                        finalizer,
                        runtime,
                        work_context,
                        session,
                        ..
                    } => Some((
                        *identity,
                        finalizer.clone(),
                        runtime.clone(),
                        *work_context,
                        session.clone(),
                    )),
                    DockLiveUndockQueuedFact::Reduce(_)
                    | DockLiveUndockQueuedFact::Start { .. } => None,
                };
                let result = catch_unwind(AssertUnwindSafe(|| {
                    self.reduce_queued_fact(&owner, fact, cx)
                }));
                if let Err(payload) = result {
                    if let Some((identity, finalizer, runtime, work_context, session)) =
                        pending_release
                    {
                        settle_payload_drag_finalizer_claim(
                            finalizer.claim_release_adoption(identity),
                            &runtime,
                            work_context,
                            &session,
                            cx,
                        );
                    }
                    if first_panic.is_none() {
                        first_panic = Some(payload);
                    } else {
                        log::error!("suppressed a secondary live-undock reducer panic");
                    }
                }
            }
            DockLiveUndockPumpCommand::Effects(effects) => {
                for effect in effects {
                    let result =
                        catch_unwind(AssertUnwindSafe(|| self.execute_effect(&owner, effect, cx)));
                    if let Err(payload) = result {
                        if first_panic.is_none() {
                            first_panic = Some(payload);
                        } else {
                            log::error!("suppressed a secondary live-undock effect panic");
                        }
                    }
                }
            }
        });
        if let Some(payload) = first_panic {
            resume_unwind(payload);
        }
    }

    fn reduce_queued_fact(
        &self,
        owner: &open_gpui::Entity<DockSurfaceOwner>,
        queued: DockLiveUndockQueuedFact,
        cx: &mut App,
    ) {
        let (fact, seed) = match queued {
            DockLiveUndockQueuedFact::Reduce(fact) => (fact, None),
            DockLiveUndockQueuedFact::AdoptRelease {
                identity,
                release,
                finalizer,
                runtime,
                work_context,
                session,
            } => {
                self.reduce_release_adoption(
                    owner,
                    identity,
                    release,
                    finalizer,
                    runtime,
                    work_context,
                    session,
                    cx,
                );
                return;
            }
            DockLiveUndockQueuedFact::Start {
                lease,
                trigger,
                seed,
            } => (DockLiveUndockFact::Trigger { lease, trigger }, Some(seed)),
        };
        if seed.as_ref().is_some_and(|seed| {
            !seed
                .source
                .runtime
                .is_current_viewport_host_scene_frame(&seed.source.source_frame)
        }) {
            return;
        }
        let expected_revision = seed.as_ref().map(|seed| seed.surface_revision);
        let transition = cx.update_entity(owner, |owner, owner_cx| {
            if expected_revision.is_some_and(|revision| owner.revision() != revision) {
                return None;
            }
            let effects = owner.reduce_live_undock_fact(fact)?;
            let revision = owner.revision();
            if !effects.is_empty() {
                owner_cx.notify();
            }
            Some((revision, effects))
        });
        let Some((surface_revision, effects)) = transition else {
            return;
        };

        if let Some(seed) = seed {
            let opening = effects.as_slice().iter().find_map(|effect| match effect {
                DockLiveUndockEffect::OpenProvisional { identity, request } => {
                    Some((*identity, request.clone()))
                }
                _ => None,
            });
            if let Some((identity, request)) = opening {
                seed.source.identity_slot.set(Some(identity));
                let previous = self.state.borrow_mut().executions.insert(
                    identity,
                    DockLiveUndockExecution {
                        seed,
                        request,
                        surface_revision,
                        release_deadline: DockLiveUndockReleaseDeadline::default(),
                        release_placement: None,
                        observed_release_placement: None,
                        destination_host: None,
                        host_release: None,
                        presentation: None,
                        promotion: None,
                        source_restoration_retry: DockLiveUndockRetryBackoff::default(),
                        orphan_recovery_retry: DockLiveUndockRetryBackoff::default(),
                        committed_destination_recovery_retry: DockLiveUndockRetryBackoff::default(),
                    },
                );
                assert!(
                    previous.is_none(),
                    "one live-undock identity must install one execution"
                );
            }
        }

        self.enqueue_effects(effects, cx);
    }

    #[allow(clippy::too_many_arguments)]
    fn reduce_release_adoption(
        &self,
        owner: &open_gpui::Entity<DockSurfaceOwner>,
        identity: DockLiveUndockIdentity,
        release: DockLiveUndockReleaseLock,
        finalizer: DockPayloadDragFinalizer,
        runtime: DockViewportRuntimeHandle,
        work_context: DockViewportRuntimeWorkContext,
        session: DockRuntimeDragSession,
        cx: &mut App,
    ) {
        let execution_is_current = self.state.borrow().executions.contains_key(&identity);
        let effects = execution_is_current.then(|| {
            cx.update_entity(owner, |owner, owner_cx| {
                if !owner.accepts_live_undock_identity(identity) {
                    return None;
                }
                let effects = owner.reduce_live_undock_fact(DockLiveUndockFact::ReleaseLocked {
                    identity,
                    release,
                })?;
                if !effects.is_empty() {
                    owner_cx.notify();
                }
                Some(effects)
            })
        });
        let Some(effects) = effects.flatten() else {
            settle_payload_drag_finalizer_claim(
                finalizer.claim_pending_live_undock(identity),
                &runtime,
                work_context,
                &session,
                cx,
            );
            return;
        };
        if !finalizer.commit_live_undock(identity) {
            settle_payload_drag_finalizer_claim(
                finalizer.claim_pending_live_undock(identity),
                &runtime,
                work_context,
                &session,
                cx,
            );
            return;
        }
        self.arm_release_deadline(identity, release.placement_generation(), cx);
        self.enqueue_effects(effects, cx);
    }

    fn request_release_placement(
        &self,
        identity: DockLiveUndockIdentity,
        window: AnyWindowHandle,
        release: DockLiveUndockReleaseLock,
        cx: &mut App,
    ) {
        let generation = release.placement_generation();
        let claimed = {
            let mut state = self.state.borrow_mut();
            let Some(execution) = state.executions.get_mut(&identity) else {
                return;
            };
            if execution.request.key() != identity.opening()
                || execution.destination_host.map(Into::into) != Some(window)
                || execution.release_placement.is_some()
            {
                false
            } else {
                execution.release_placement = Some(DockLiveUndockReleasePlacementExecution {
                    window_id: window.window_id(),
                    generation,
                    subscription: None,
                });
                true
            }
        };
        if !claimed {
            return;
        }

        let (dispatch, unchanged_facts) = window
            .update(cx, |_, window, _| {
                let scale_factor = window.scale_factor();
                if !scale_factor.is_finite() || scale_factor <= 0.0 {
                    return (WindowMutationDispatch::Rejected, None);
                }
                let desired = release.desired_bounds();
                let bounds = Bounds::new(
                    point(
                        px(desired.origin().x() as f32 / scale_factor),
                        px(desired.origin().y() as f32 / scale_factor),
                    ),
                    size(
                        px(desired.width() as f32 / scale_factor),
                        px(desired.height() as f32 / scale_factor),
                    ),
                );
                let dispatch = window
                    .request_window_placement_request(WindowPlacementRequest::windowed(bounds));
                let facts = matches!(&dispatch, WindowMutationDispatch::Unchanged)
                    .then(|| DockViewportWindowFacts::from_platform_facts(window.platform_facts()));
                (dispatch, facts)
            })
            .unwrap_or((WindowMutationDispatch::WindowClosed, None));

        match dispatch {
            WindowMutationDispatch::Queued(ticket) => {
                let runtime = self.clone();
                let async_cx = cx.to_async();
                let subscription = ticket.subscribe(move |observation| {
                    let outcome = Self::dock_placement_outcome(observation.outcome);
                    let facts = matches!(
                        outcome,
                        super::live_undock::DockLiveUndockPlacementOutcome::Exact
                            | super::live_undock::DockLiveUndockPlacementOutcome::Adjusted
                    )
                    .then(|| DockViewportWindowFacts::from_platform_facts(&observation.facts));
                    async_cx
                        .spawn(async move |cx| {
                            cx.update(|cx| {
                                runtime.observe_release_placement(
                                    identity,
                                    window.window_id(),
                                    generation,
                                    outcome,
                                    facts,
                                    cx,
                                );
                            });
                        })
                        .detach();
                });
                let mut state = self.state.borrow_mut();
                if let Some(placement) = state
                    .executions
                    .get_mut(&identity)
                    .and_then(|execution| execution.release_placement.as_mut())
                    .filter(|placement| {
                        placement.window_id == window.window_id()
                            && placement.generation == generation
                    })
                {
                    placement.subscription = Some(subscription);
                }
            }
            WindowMutationDispatch::Unchanged => self.observe_release_placement(
                identity,
                window.window_id(),
                generation,
                super::live_undock::DockLiveUndockPlacementOutcome::Exact,
                unchanged_facts,
                cx,
            ),
            WindowMutationDispatch::Unsupported | WindowMutationDispatch::Rejected => self
                .observe_release_placement(
                    identity,
                    window.window_id(),
                    generation,
                    super::live_undock::DockLiveUndockPlacementOutcome::Rejected,
                    None,
                    cx,
                ),
            WindowMutationDispatch::WindowClosed => self.observe_release_placement(
                identity,
                window.window_id(),
                generation,
                super::live_undock::DockLiveUndockPlacementOutcome::WindowClosed,
                None,
                cx,
            ),
        }
    }

    fn observe_release_placement(
        &self,
        identity: DockLiveUndockIdentity,
        window_id: WindowId,
        generation: DockLiveUndockPlacementGeneration,
        outcome: super::live_undock::DockLiveUndockPlacementOutcome,
        facts: Option<DockViewportWindowFacts>,
        cx: &mut App,
    ) {
        let current = self
            .state
            .borrow_mut()
            .executions
            .get_mut(&identity)
            .is_some_and(|execution| {
                let current = execution
                    .release_placement
                    .as_ref()
                    .is_some_and(|placement| {
                        placement.window_id == window_id && placement.generation == generation
                    });
                if current {
                    execution.release_placement.take();
                    execution.observed_release_placement =
                        facts.map(|facts| DockLiveUndockObservedReleasePlacement {
                            window_id,
                            generation,
                            facts,
                        });
                }
                current
            });
        if current {
            let _ = self.submit(
                DockLiveUndockFact::PlacementObserved {
                    identity,
                    window_id,
                    generation,
                    outcome,
                },
                cx,
            );
        }
    }

    const fn dock_placement_outcome(
        outcome: WindowMutationOutcome,
    ) -> super::live_undock::DockLiveUndockPlacementOutcome {
        match outcome {
            WindowMutationOutcome::Exact => {
                super::live_undock::DockLiveUndockPlacementOutcome::Exact
            }
            WindowMutationOutcome::Adjusted => {
                super::live_undock::DockLiveUndockPlacementOutcome::Adjusted
            }
            WindowMutationOutcome::Superseded => {
                super::live_undock::DockLiveUndockPlacementOutcome::Superseded
            }
            WindowMutationOutcome::Rejected | WindowMutationOutcome::Unsupported => {
                super::live_undock::DockLiveUndockPlacementOutcome::Rejected
            }
            WindowMutationOutcome::WindowClosed => {
                super::live_undock::DockLiveUndockPlacementOutcome::WindowClosed
            }
        }
    }

    fn arm_release_deadline(
        &self,
        identity: DockLiveUndockIdentity,
        placement_generation: DockLiveUndockPlacementGeneration,
        cx: &mut App,
    ) {
        let armed = {
            let mut state = self.state.borrow_mut();
            let Some(execution) = state.executions.get_mut(&identity) else {
                return;
            };
            execution.release_deadline.arm(placement_generation);
            true
        };
        if !armed {
            return;
        }

        let runtime = self.clone();
        cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(LIVE_UNDOCK_RELEASE_DEADLINE)
                .await;
            cx.update(|cx| {
                runtime.expire_release_deadline(identity, placement_generation, cx);
            });
        })
        .detach();
    }

    fn expire_release_deadline(
        &self,
        identity: DockLiveUndockIdentity,
        placement_generation: DockLiveUndockPlacementGeneration,
        cx: &mut App,
    ) {
        let claimed = self
            .state
            .borrow_mut()
            .executions
            .get_mut(&identity)
            .is_some_and(|execution| {
                execution
                    .release_deadline
                    .claim_expiration(placement_generation)
            });
        if claimed {
            let _ = self.submit(
                DockLiveUndockFact::ReleaseDeadlineExpired {
                    identity,
                    placement_generation,
                },
                cx,
            );
        }
    }

    fn schedule_source_restoration_retry(
        &self,
        identity: DockLiveUndockIdentity,
        source: DockLiveUndockSourceSnapshot,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        cx: &mut App,
    ) {
        let Some((retry_generation, delay)) = self
            .state
            .borrow_mut()
            .executions
            .get_mut(&identity)
            .and_then(|execution| execution.source_restoration_retry.arm_if_idle())
        else {
            return;
        };
        let runtime = self.clone();
        cx.spawn(async move |cx| {
            cx.background_executor().timer(delay).await;
            cx.update(|cx| {
                runtime.retry_source_restoration(
                    identity,
                    source,
                    payload_lease,
                    retry_generation,
                    cx,
                );
            });
        })
        .detach();
    }

    fn retry_source_restoration(
        &self,
        identity: DockLiveUndockIdentity,
        source: DockLiveUndockSourceSnapshot,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        retry_generation: u64,
        cx: &mut App,
    ) {
        let current = self
            .state
            .borrow_mut()
            .executions
            .get_mut(&identity)
            .is_some_and(|execution| {
                execution.request.key() == identity.opening()
                    && execution.source_restoration_retry.claim(retry_generation)
            });
        if current {
            let _ = self.submit(
                DockLiveUndockFact::SourceRestorationRetryElapsed {
                    identity,
                    source,
                    payload_lease,
                },
                cx,
            );
        }
    }

    fn clear_source_restoration_retry(&self, identity: DockLiveUndockIdentity) {
        if let Some(execution) = self.state.borrow_mut().executions.get_mut(&identity) {
            execution.source_restoration_retry.clear();
        }
    }

    fn schedule_orphan_recovery_retry(
        &self,
        identity: DockLiveUndockIdentity,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        provisional: Option<AnyWindowHandle>,
        cx: &mut App,
    ) {
        let Some((retry_generation, delay)) = self
            .state
            .borrow_mut()
            .executions
            .get_mut(&identity)
            .and_then(|execution| execution.orphan_recovery_retry.arm_if_idle())
        else {
            return;
        };
        let runtime = self.clone();
        cx.spawn(async move |cx| {
            cx.background_executor().timer(delay).await;
            cx.update(|cx| {
                runtime.retry_orphan_recovery(
                    identity,
                    payload_lease,
                    provisional,
                    retry_generation,
                    cx,
                );
            });
        })
        .detach();
    }

    fn retry_orphan_recovery(
        &self,
        identity: DockLiveUndockIdentity,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        provisional: Option<AnyWindowHandle>,
        retry_generation: u64,
        cx: &mut App,
    ) {
        let current = self
            .state
            .borrow_mut()
            .executions
            .get_mut(&identity)
            .is_some_and(|execution| {
                execution.orphan_recovery_retry.claim(retry_generation)
                    && execution.request.key() == identity.opening()
            });
        if current {
            self.enqueue_effects(
                DockLiveUndockEffects::single(
                    DockLiveUndockEffect::RecoverOrphanedPayloadTopology {
                        identity,
                        payload_lease,
                        provisional,
                    },
                ),
                cx,
            );
        }
    }

    fn clear_orphan_recovery_retry(&self, identity: DockLiveUndockIdentity) {
        if let Some(execution) = self.state.borrow_mut().executions.get_mut(&identity) {
            execution.orphan_recovery_retry.clear();
        }
    }

    fn schedule_committed_destination_recovery_retry(
        &self,
        identity: DockLiveUndockIdentity,
        authority: DockPayloadRecoveryAuthority,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
        cx: &mut App,
    ) {
        let Some((retry_generation, delay)) = self
            .state
            .borrow_mut()
            .executions
            .get_mut(&identity)
            .and_then(|execution| execution.committed_destination_recovery_retry.arm_if_idle())
        else {
            return;
        };
        let runtime = self.clone();
        cx.spawn(async move |cx| {
            cx.background_executor().timer(delay).await;
            cx.update(|cx| {
                runtime.retry_committed_destination_recovery(
                    identity,
                    authority,
                    token,
                    destination,
                    retry_generation,
                    cx,
                );
            });
        })
        .detach();
    }

    fn retry_committed_destination_recovery(
        &self,
        identity: DockLiveUndockIdentity,
        authority: DockPayloadRecoveryAuthority,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
        retry_generation: u64,
        cx: &mut App,
    ) {
        let current = self
            .state
            .borrow_mut()
            .executions
            .get_mut(&identity)
            .is_some_and(|execution| {
                execution
                    .committed_destination_recovery_retry
                    .claim(retry_generation)
                    && execution.request.key() == identity.opening()
            });
        if current {
            self.enqueue_effects(
                DockLiveUndockEffects::single(
                    DockLiveUndockEffect::RecoverCommittedDestinationTopology {
                        identity,
                        authority,
                        token,
                        destination,
                    },
                ),
                cx,
            );
        }
    }

    fn clear_committed_destination_recovery_retry(&self, identity: DockLiveUndockIdentity) {
        if let Some(execution) = self.state.borrow_mut().executions.get_mut(&identity) {
            execution.committed_destination_recovery_retry.clear();
        }
    }

    fn release_retained_visual(source_window: AnyWindowHandle, retained: Ticket, cx: &mut App) {
        let _ = source_window.update(cx, |_, window, _| {
            let _ = retained_visual::release(window, &retained);
        });
    }

    fn checkout_presentation_session(
        &self,
        identity: DockLiveUndockIdentity,
        lease: DockLiveUndockPayloadLeaseReceipt,
    ) -> Option<DockLiveUndockRehostSessionCheckout> {
        let mut state = self.state.borrow_mut();
        let presentation = state.executions.get_mut(&identity)?.presentation.as_mut()?;
        if presentation.lease != lease {
            return None;
        }
        let session = presentation.checkout_session()?;
        drop(state);
        Some(DockLiveUndockRehostSessionCheckout {
            state: self.state.clone(),
            identity,
            lease,
            session: Some(session),
        })
    }

    fn committed_source_finish(
        &self,
        identity: DockLiveUndockIdentity,
        lease: DockLiveUndockPayloadLeaseReceipt,
    ) -> Option<view_presentation_window::SourcePresentationFinish> {
        self.state
            .borrow()
            .executions
            .get(&identity)?
            .presentation
            .as_ref()
            .filter(|presentation| presentation.lease == lease)?
            .session
            .active()?
            .committed_source_finish()
    }

    fn prepare_presentation_terminal(
        &self,
        identity: DockLiveUndockIdentity,
        lease: DockLiveUndockPayloadLeaseReceipt,
        intent: view_presentation_window::RehostTerminalIntent<'_>,
        cx: &App,
    ) -> Option<
        Result<
            view_presentation_window::RehostTerminalPreparation,
            view_presentation_window::TransitionError,
        >,
    > {
        let state = self.state.borrow();
        let presentation = state.executions.get(&identity)?.presentation.as_ref()?;
        if presentation.lease != lease {
            return None;
        }
        Some(presentation.session.active()?.prepare_terminal(cx, intent))
    }

    fn presentation_projection_matches(
        &self,
        identity: DockLiveUndockIdentity,
        lease: DockLiveUndockPayloadLeaseReceipt,
        projection: &RehostProjection,
    ) -> bool {
        self.state
            .borrow()
            .executions
            .get(&identity)
            .and_then(|execution| execution.presentation.as_ref())
            .filter(|presentation| presentation.lease == lease)
            .and_then(|presentation| presentation.session.active())
            .is_some_and(|session| projection.matches_exactly(&session.projection()))
    }

    fn retire_presentation_session_after_terminal_commit(
        &self,
        identity: DockLiveUndockIdentity,
        lease: DockLiveUndockPayloadLeaseReceipt,
    ) -> bool {
        let mut state = self.state.borrow_mut();
        let Some(presentation) = state
            .executions
            .get_mut(&identity)
            .and_then(|execution| execution.presentation.as_mut())
            .filter(|presentation| presentation.lease == lease)
        else {
            return false;
        };
        presentation.session.retire_terminal()
    }

    fn cancel_prepared_presentation(
        source_window: AnyWindowHandle,
        retained: Ticket,
        mut session: RehostSession,
        cx: &mut App,
    ) {
        let source_retired = matches!(
            session.settle_source(cx),
            Ok(
                view_presentation_window::SourceSettlement::RetiredToSource(_)
                    | view_presentation_window::SourceSettlement::AlreadyRetired
                    | view_presentation_window::SourceSettlement::PresentationAuthorityReleased(_)
            )
        );
        if !source_retired {
            let _ = session.abandon_after_source_loss(cx);
        }
        Self::release_retained_visual(source_window, retained, cx);
    }

    fn prepare_presentation_handoff(
        &self,
        identity: DockLiveUndockIdentity,
        effect_window: AnyWindowHandle,
        cx: &mut App,
    ) -> Result<DockLiveUndockPresentationExecution, DockLiveUndockPresentationFailure> {
        let (
            destination_host,
            source_window,
            source_host,
            source_binding,
            source_frame_generation,
            source_scene,
            source_identity,
            payload_session,
            surface_revision,
            request,
        ) = {
            let state = self.state.borrow();
            let execution = state
                .executions
                .get(&identity)
                .ok_or(DockLiveUndockPresentationFailure::PayloadLeaseClaim)?;
            let destination_host = execution
                .destination_host
                .ok_or(DockLiveUndockPresentationFailure::PayloadLeaseClaim)?;
            if destination_host.window_id() != effect_window.window_id() {
                return Err(DockLiveUndockPresentationFailure::PayloadLeaseClaim);
            }
            (
                destination_host,
                execution.seed.source.source_window,
                execution.seed.source.source_host.clone(),
                execution.seed.source.source_binding,
                execution.seed.source.source_frame.generation(),
                execution.seed.source.source_presentation_scene.clone(),
                execution.seed.move_plan.source_identity().clone(),
                execution.seed.payload_session.clone(),
                execution.surface_revision,
                execution.request.clone(),
            )
        };

        let destination_binding = destination_host
            .update(cx, |host, window, cx| {
                if window.window_handle().window_id() != effect_window.window_id() {
                    return None;
                }
                host.ensure_window_binding(window, cx);
                host.current_window_binding()
            })
            .ok()
            .flatten()
            .ok_or(DockLiveUndockPresentationFailure::PayloadLeaseClaim)?;
        if destination_binding.window_id() != effect_window.window_id() {
            return Err(DockLiveUndockPresentationFailure::PayloadLeaseClaim);
        }

        let carrier = resolve_live_payload_carrier(&source_scene, &source_identity)
            .map_err(|_| DockLiveUndockPresentationFailure::RetainedVisualTicket)?;
        let source_id = carrier.kind.retained_source_id(
            source_host.entity_id(),
            source_binding.generation(),
            &source_scene.space,
        );
        let retained = source_window
            .update(cx, |_, window, _| {
                (window.window_handle().window_id() == source_binding.window_id())
                    .then(|| retained_visual::lease_committed(window, &source_id).ok())
                    .flatten()
            })
            .ok()
            .flatten()
            .ok_or(DockLiveUndockPresentationFailure::RetainedVisualTicket)?;
        if retained.bounds() != carrier.bounds {
            Self::release_retained_visual(source_window, retained, cx);
            return Err(DockLiveUndockPresentationFailure::RetainedVisualTicket);
        }

        let roots = source_host
            .update(cx, |host, cx| {
                if !host.accepts_bound_window(Some(source_binding)) {
                    return None;
                }
                host.mounted_panel_presentation_roots(
                    &payload_session,
                    source_binding.window_id(),
                    cx,
                )
            })
            .ok()
            .flatten()
            .filter(|roots| !roots.is_empty());
        let Some(roots) = roots else {
            Self::release_retained_visual(source_window, retained, cx);
            return Err(DockLiveUndockPresentationFailure::PayloadLeaseClaim);
        };
        let source_leases = roots.iter().map(|(_, lease)| *lease).collect::<Vec<_>>();
        let session = match view_presentation_window::prepare_rehost(
            cx,
            &source_leases,
            destination_binding.window_id(),
        ) {
            Ok(prepared) => prepared,
            Err(_) => {
                Self::release_retained_visual(source_window, retained, cx);
                return Err(DockLiveUndockPresentationFailure::RehostPreparation);
            }
        };
        let projection = session.projection();
        let source =
            DockLiveUndockSourceSnapshot::new(source_binding.window_id(), source_frame_generation);
        let Some(lease) = DockLiveUndockPayloadLeaseReceipt::new(
            identity,
            source,
            surface_revision,
            retained.identity(),
            &projection,
            request.provisional_session(),
        ) else {
            Self::cancel_prepared_presentation(source_window, retained, session, cx);
            return Err(DockLiveUndockPresentationFailure::RehostPreparation);
        };

        Ok(DockLiveUndockPresentationExecution {
            carrier,
            retained,
            projection,
            session: DockLiveUndockRehostSessionState::Active(session),
            lease,
            source_key: None,
            destination_key: None,
            reveal: None,
            retained_released: false,
            source_restoration_batch: None,
            source_restoration_receipt: None,
            restore_focus: false,
            source_focus_restored: false,
        })
    }

    fn submit_presentation_failure(
        &self,
        identity: DockLiveUndockIdentity,
        failure: DockLiveUndockPresentationFailure,
        cx: &mut App,
    ) {
        let _ = self.submit(
            DockLiveUndockFact::PresentationStageFailed { identity, failure },
            cx,
        );
    }

    pub(crate) fn defer_source_restoration(
        &self,
        identity: DockLiveUndockIdentity,
        source: DockLiveUndockSourceSnapshot,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        failure: DockLiveUndockSourceRestorationFailure,
        cx: &mut App,
    ) {
        if self.submit(
            DockLiveUndockFact::SourceRestorationDeferred {
                identity,
                source,
                payload_lease,
            },
            cx,
        ) && failure.schedules_timer_retry()
        {
            self.schedule_source_restoration_retry(identity, source, payload_lease, cx);
        }
    }

    pub(crate) fn commit_source_restoration(
        &self,
        identity: DockLiveUndockIdentity,
        receipt: DockLiveUndockSourceRestorationReceipt,
        cx: &mut App,
    ) -> bool {
        let submitted = self.submit(
            DockLiveUndockFact::SourceRestorationCommitted { identity, receipt },
            cx,
        );
        if submitted {
            self.clear_source_restoration_retry(identity);
        }
        submitted
    }

    pub(crate) fn stage_source_restoration_receipt(
        &self,
        identity: DockLiveUndockIdentity,
        source_key: DockHostLivePresentationKey,
        receipt: DockLiveUndockSourceRestorationReceipt,
    ) -> bool {
        if receipt.identity() != identity {
            return false;
        }
        let mut state = self.state.borrow_mut();
        let Some(presentation) = state
            .executions
            .get_mut(&identity)
            .and_then(|execution| execution.presentation.as_mut())
            .filter(|presentation| {
                presentation.lease == receipt.payload_lease()
                    && presentation.source_key == Some(source_key)
            })
        else {
            return false;
        };
        if presentation
            .source_restoration_receipt
            .is_some_and(|current| current != receipt)
        {
            return false;
        }
        presentation.source_restoration_receipt = Some(receipt);
        true
    }

    pub(crate) fn accept_source_proxy_frame(
        &self,
        key: DockHostLivePresentationKey,
        lease: DockLiveUndockPayloadLeaseReceipt,
        accepted_frame: u64,
        _cx: &mut App,
    ) -> Option<
        Result<
            super::live_undock::DockLiveUndockSourceProxyReceipt,
            view_presentation_window::TransitionError,
        >,
    > {
        let source_is_exact = self
            .state
            .borrow()
            .executions
            .get(&key.identity())?
            .presentation
            .as_ref()
            .is_some_and(|presentation| {
                presentation.lease == lease && presentation.source_key == Some(key)
            });
        if !source_is_exact {
            return None;
        }
        let mut checkout = self.checkout_presentation_session(key.identity(), lease)?;
        let result = checkout
            .session_mut()
            .accept_source_proxy_frame(accepted_frame)
            .and_then(|accepted| {
                super::live_undock::DockLiveUndockSourceProxyReceipt::new(lease, accepted)
                    .ok_or(view_presentation_window::TransitionError::WrongSourceProxyEvidence)
            });
        Some(result)
    }

    pub(crate) fn accept_destination_frame(
        &self,
        key: DockHostLivePresentationKey,
        proxy: super::live_undock::DockLiveUndockSourceProxyReceipt,
        accepted_frame: u64,
        cx: &mut App,
    ) -> Option<
        Result<
            (
                view_presentation_window::LeaseBatch,
                super::live_undock::DockLiveUndockPayloadMountReceipt,
            ),
            view_presentation_window::TransitionError,
        >,
    > {
        let lease = proxy.lease();
        let destination_is_exact = self
            .state
            .borrow()
            .executions
            .get(&key.identity())?
            .presentation
            .as_ref()
            .is_some_and(|presentation| {
                presentation.lease == lease && presentation.destination_key == Some(key)
            });
        if !destination_is_exact {
            return None;
        }
        let mut checkout = self.checkout_presentation_session(key.identity(), lease)?;
        let result = checkout
            .session_mut()
            .accept_destination_frame(cx, accepted_frame)
            .and_then(|exposure| {
                let batch = exposure.batch().clone();
                super::live_undock::DockLiveUndockPayloadMountReceipt::new(proxy, &exposure)
                    .map(|receipt| (batch, receipt))
                    .ok_or(view_presentation_window::TransitionError::StalePrepared)
            });
        Some(result)
    }

    pub(crate) fn accept_destination_presentation_frame(
        &self,
        key: DockHostLivePresentationKey,
        mount: super::live_undock::DockLiveUndockPayloadMountReceipt,
        accepted_frame: u64,
        cx: &mut App,
    ) -> Option<
        Result<DockLiveUndockPayloadPresentationReceipt, view_presentation_window::TransitionError>,
    > {
        let lease = mount.proxy().lease();
        let destination_is_exact = self
            .state
            .borrow()
            .executions
            .get(&key.identity())?
            .presentation
            .as_ref()
            .is_some_and(|presentation| {
                presentation.lease == lease && presentation.destination_key == Some(key)
            });
        if !destination_is_exact {
            return None;
        }
        let checkout = self.checkout_presentation_session(key.identity(), lease)?;
        let result = checkout
            .session()
            .accept_destination_presentation_frame(cx, accepted_frame)
            .and_then(|accepted| {
                DockLiveUndockPayloadPresentationReceipt::new(mount, accepted)
                    .ok_or(view_presentation_window::TransitionError::StalePrepared)
            });
        Some(result)
    }

    pub(crate) fn current_destination_presentation(
        &self,
        key: DockHostLivePresentationKey,
        mount: super::live_undock::DockLiveUndockPayloadMountReceipt,
        cx: &App,
    ) -> Option<
        Result<DockLiveUndockPayloadPresentationReceipt, view_presentation_window::TransitionError>,
    > {
        let lease = mount.proxy().lease();
        let state = self.state.borrow();
        let presentation = state
            .executions
            .get(&key.identity())?
            .presentation
            .as_ref()?;
        if presentation.lease != lease || presentation.destination_key != Some(key) {
            return None;
        }
        Some(
            presentation
                .session
                .active()?
                .current_destination_presentation(cx)
                .and_then(|current| {
                    DockLiveUndockPayloadPresentationReceipt::new(mount, current)
                        .ok_or(view_presentation_window::TransitionError::StalePrepared)
                }),
        )
    }

    pub(crate) fn release_source_restoration_visual_in_frame(
        &self,
        identity: DockLiveUndockIdentity,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        window: &mut Window,
    ) -> bool {
        let retained = {
            let state = self.state.borrow();
            let Some(presentation) = state
                .executions
                .get(&identity)
                .and_then(|execution| execution.presentation.as_ref())
                .filter(|presentation| presentation.lease == payload_lease)
            else {
                return false;
            };
            if presentation.retained_released {
                return true;
            }
            if presentation.source_restoration_receipt.is_none() {
                return false;
            }
            presentation.retained
        };
        if retained_visual::release(window, &retained).is_err() {
            return false;
        }
        let mut state = self.state.borrow_mut();
        let presentation = state
            .executions
            .get_mut(&identity)
            .and_then(|execution| execution.presentation.as_mut())
            .filter(|presentation| presentation.lease == payload_lease)
            .expect("an in-frame retained release must keep its exact execution");
        presentation.retained_released = true;
        true
    }

    pub(crate) fn accept_destination_semantics_frame(
        &self,
        host: &Entity<DockHost>,
        marker: &DockHostLiveDestinationSemantics,
        frame_generation: u64,
        window: &mut open_gpui::Window,
        cx: &mut App,
    ) {
        let authority = self
            .state
            .borrow()
            .executions
            .get(&marker.identity())
            .and_then(|execution| match execution.promotion.as_ref()? {
                DockLiveUndockPromotionExecution::Durable(
                    DockLiveUndockDurablePromotionExecution::SameWindow(durable),
                ) if durable.identity == marker.identity()
                    && durable.token == marker.token()
                    && durable.destination.window_id() == window.window_handle().window_id()
                    && durable.surface_revision == marker.surface_revision()
                    && durable.destination_binding == marker.binding()
                    && &durable.registration == marker.registration()
                    && durable.destination_window.window_id()
                        == window.window_handle().window_id() =>
                {
                    Some((
                        durable.identity,
                        durable.token,
                        durable.destination,
                        durable.reveal,
                        durable.provisional_session.clone(),
                        durable.semantics.clone(),
                    ))
                }
                _ => None,
            });
        let Some((identity, token, destination, reveal, provisional_session, semantics)) =
            authority
        else {
            return;
        };
        let host_is_exact = cx.read_entity(host, |host, _| {
            host.accepts_live_destination_semantics(marker)
                && host
                    .interaction()
                    .viewport_host_scene_frame()
                    .is_some_and(|scene| {
                        scene.registration_key() == marker.registration()
                            && scene.matches_viewport(
                                marker.registration().space(),
                                marker.binding().window_id(),
                            )
                    })
        });
        if !host_is_exact {
            window.refresh();
            return;
        }
        let Some(presentation) =
            view_presentation_window::stable_batch_presentation_receipt(cx, marker.destination())
        else {
            window.refresh();
            return;
        };
        let lease_generation = marker
            .destination()
            .leases()
            .first()
            .map(|lease| lease.generation());
        if presentation.window_id() != marker.binding().window_id()
            || presentation.frame_generation() != frame_generation
            || Some(presentation.lease_generation()) != lease_generation
            || presentation.root_count() != marker.destination().leases().len()
        {
            window.refresh();
            return;
        }

        let pending = semantics.snapshot();
        let Some(prepared_receipt) = DockLiveUndockDestinationSemanticsReceipt::prepare_same_window(
            identity,
            token,
            reveal,
            pending,
            frame_generation,
        ) else {
            let retryable = pending.outcome() == WindowProvisionalSemanticsOutcome::Pending
                && pending.committed_frame_generation().is_none()
                && pending.window_id() == window.window_handle().window_id()
                && pending.session_generation() == provisional_session.snapshot().generation()
                && pending.destination_generation() == token.get()
                && (frame_generation < pending.minimum_frame_generation()
                    || frame_generation <= reveal.reveal_frame().frame_generation());
            if retryable {
                window.refresh();
            } else {
                self.enqueue_fact(
                    DockLiveUndockQueuedFact::Reduce(
                        DockLiveUndockFact::DestinationSemanticsCommitFailed {
                            identity,
                            token,
                            destination,
                        },
                    ),
                    cx,
                );
            }
            return;
        };

        let committed = match window.accept_provisional_destination_semantics_frame(
            &provisional_session,
            &semantics,
            frame_generation,
            cx,
        ) {
            Ok(committed) => committed,
            Err(_) => {
                let semantics = semantics.snapshot();
                let session = provisional_session.snapshot();
                if semantics.outcome() == WindowProvisionalSemanticsOutcome::Pending
                    && session.phase()
                        == WindowProvisionalSessionPhase::ProjectingDestinationSemantics
                {
                    window.refresh();
                } else {
                    self.enqueue_fact(
                        DockLiveUndockQueuedFact::Reduce(
                            DockLiveUndockFact::DestinationSemanticsCommitFailed {
                                identity,
                                token,
                                destination,
                            },
                        ),
                        cx,
                    );
                }
                return;
            }
        };
        let receipt = prepared_receipt.commit(committed);
        let completed = cx.update_entity(host, |host, host_cx| {
            host.complete_live_destination_semantics(marker, host_cx)
        });
        assert!(
            completed,
            "an accepted destination-semantics marker must remain exact through one callback"
        );
        let _ = self.enqueue_fact(
            DockLiveUndockQueuedFact::Reduce(DockLiveUndockFact::DestinationSemanticsCommitted {
                identity,
                receipt,
            }),
            cx,
        );
    }

    fn source_restoration_execution_for_identity(
        &self,
        identity: DockLiveUndockIdentity,
    ) -> Option<DockLiveUndockSourceRestorationExecution> {
        let state = self.state.borrow();
        let execution = state.executions.get(&identity)?;
        let presentation = execution.presentation.as_ref()?;
        Some(DockLiveUndockSourceRestorationExecution {
            identity,
            source: presentation.lease.source(),
            payload_lease: presentation.lease,
            source_window: execution.seed.source.source_window,
            source_host: execution.seed.source.source_host.clone(),
            source_key: presentation.source_key,
            restore_session: execution.seed.restore_session.clone(),
            destination_host: execution.destination_host,
            destination_key: presentation.destination_key,
            projection: presentation.projection.clone(),
            retained: presentation.retained,
            retained_released: presentation.retained_released,
            source_restoration_batch: presentation.source_restoration_batch.clone(),
            source_restoration_receipt: presentation.source_restoration_receipt,
        })
    }

    fn source_restoration_execution(
        &self,
        identity: DockLiveUndockIdentity,
        source: DockLiveUndockSourceSnapshot,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
    ) -> Option<DockLiveUndockSourceRestorationExecution> {
        let restoration = self.source_restoration_execution_for_identity(identity)?;
        (restoration.payload_lease == payload_lease
            && restoration.source == source
            && restoration.source_window.window_id() == source.window_id())
        .then_some(restoration)
    }

    fn source_host_presentation_released(
        &self,
        source_key: DockHostLivePresentationKey,
        cx: &mut App,
    ) {
        let Some(restoration) =
            self.source_restoration_execution_for_identity(source_key.identity())
        else {
            return;
        };
        if restoration.source_key != Some(source_key) {
            return;
        }
        self.submit_source_host_authority_loss(&restoration, cx);
    }

    fn record_source_restoration_focus_intent(
        &self,
        identity: DockLiveUndockIdentity,
        source: DockLiveUndockSourceSnapshot,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        restore_focus: bool,
    ) -> bool {
        let mut state = self.state.borrow_mut();
        let Some(presentation) = state
            .executions
            .get_mut(&identity)
            .filter(|execution| {
                execution.seed.source.source_window.window_id() == source.window_id()
            })
            .and_then(|execution| execution.presentation.as_mut())
            .filter(|presentation| {
                presentation.lease == payload_lease && payload_lease.source() == source
            })
        else {
            return false;
        };
        presentation.restore_focus = restore_focus;
        true
    }

    fn restore_source_focus(
        &self,
        identity: DockLiveUndockIdentity,
        source: DockLiveUndockSourceSnapshot,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        cx: &mut App,
    ) {
        let authority = {
            let state = self.state.borrow();
            let Some(execution) = state.executions.get(&identity).filter(|execution| {
                execution.seed.source.source_window.window_id() == source.window_id()
            }) else {
                return;
            };
            let Some(presentation) = execution.presentation.as_ref().filter(|presentation| {
                presentation.lease == payload_lease
                    && payload_lease.source() == source
                    && presentation.restore_focus
                    && !presentation.source_focus_restored
            }) else {
                return;
            };
            (
                execution
                    .seed
                    .source
                    .source_frame
                    .registration_key()
                    .clone(),
                execution.seed.source.source_window,
                execution
                    .seed
                    .source
                    .session
                    .focus_item()
                    .cloned()
                    .map(crate::DockViewportFocusRequest::panel)
                    .unwrap_or_else(crate::DockViewportFocusRequest::no_panel_focus),
                execution.seed.source.source_host.clone(),
                presentation.lease,
            )
        };
        let (registration, source_window, source_focus, source_host, exact_lease) = authority;
        let activation = crate::DockViewportActivationTransaction::registered_host(
            registration,
            source_window,
            source_focus,
            source_host,
        );
        let outcome =
            crate::viewport_activation::apply_viewport_activation_transaction(Some(activation), cx);
        if !matches!(
            outcome,
            crate::viewport_activation::DockViewportActivationApplyOutcome::Applied { .. }
        ) {
            return;
        }
        let mut state = self.state.borrow_mut();
        if let Some(presentation) = state
            .executions
            .get_mut(&identity)
            .and_then(|execution| execution.presentation.as_mut())
            .filter(|presentation| {
                presentation.lease == exact_lease
                    && presentation.restore_focus
                    && !presentation.source_focus_restored
            })
        {
            presentation.source_focus_restored = true;
        }
    }

    fn defer_source_restoration_execution(
        &self,
        restoration: &DockLiveUndockSourceRestorationExecution,
        failure: DockLiveUndockSourceRestorationFailure,
        cx: &mut App,
    ) {
        self.defer_source_restoration(
            restoration.identity,
            restoration.source,
            restoration.payload_lease,
            failure,
            cx,
        );
    }

    fn clear_exact_source_presentation(
        restoration: &DockLiveUndockSourceRestorationExecution,
        cx: &mut App,
    ) -> bool {
        let Some(source_key) = restoration.source_key else {
            return true;
        };
        restoration
            .source_host
            .update(cx, |host, cx| {
                !host.accepts_live_presentation_key(source_key)
                    || host.clear_live_presentation(source_key, cx)
            })
            .unwrap_or(true)
    }

    fn clear_exact_destination_presentation(
        restoration: &DockLiveUndockSourceRestorationExecution,
        cx: &mut App,
    ) -> bool {
        let Some(destination_key) = restoration.destination_key else {
            return true;
        };
        let Some(destination_host) = restoration.destination_host else {
            return true;
        };
        destination_host
            .update(cx, |host, window, cx| {
                if !host.accepts_live_presentation_key(destination_key) {
                    return true;
                }
                let cleared = host.clear_live_presentation(destination_key, cx);
                if cleared {
                    window.refresh();
                }
                cleared
            })
            .unwrap_or(true)
    }

    fn release_exact_restoration_visual(
        &self,
        restoration: &DockLiveUndockSourceRestorationExecution,
        cx: &mut App,
    ) -> bool {
        if restoration.retained_released {
            return true;
        }
        let released = restoration
            .source_window
            .update(cx, |_, window, _| {
                retained_visual::release(window, &restoration.retained).is_ok()
            })
            .unwrap_or(false);
        if released {
            let mut state = self.state.borrow_mut();
            let Some(presentation) = state
                .executions
                .get_mut(&restoration.identity)
                .and_then(|execution| execution.presentation.as_mut())
                .filter(|presentation| presentation.lease == restoration.payload_lease)
            else {
                return false;
            };
            presentation.retained_released = true;
        }
        released
    }

    fn finish_presented_source_restoration(
        &self,
        restoration: &DockLiveUndockSourceRestorationExecution,
        receipt: DockLiveUndockSourceRestorationReceipt,
        cx: &mut App,
    ) {
        if receipt.payload_lease() != restoration.payload_lease {
            self.defer_source_restoration_execution(
                restoration,
                DockLiveUndockSourceRestorationFailure::RestorationReceiptUnavailable,
                cx,
            );
            return;
        }
        if !Self::clear_exact_destination_presentation(restoration, cx)
            || !self.release_exact_restoration_visual(restoration, cx)
            || !Self::clear_exact_source_presentation(restoration, cx)
        {
            self.defer_source_restoration_execution(
                restoration,
                DockLiveUndockSourceRestorationFailure::SourcePresentationMutationRejected,
                cx,
            );
            return;
        }
        let _ = restoration
            .source_window
            .update(cx, |_, window, _| window.refresh());
        let _ = self.commit_source_restoration(restoration.identity, receipt, cx);
    }

    pub(crate) fn finish_source_restoration_checkpoint(
        &self,
        identity: DockLiveUndockIdentity,
        source_key: DockHostLivePresentationKey,
        receipt: DockLiveUndockSourceRestorationReceipt,
        cx: &mut App,
    ) {
        let Some(restoration) =
            self.source_restoration_execution(identity, receipt.source(), receipt.payload_lease())
        else {
            return;
        };
        if restoration.source_key != Some(source_key)
            || restoration.source_restoration_receipt != Some(receipt)
        {
            return;
        }
        self.finish_presented_source_restoration(&restoration, receipt, cx);
    }

    pub(crate) fn finish_source_restoration_presentation(
        &self,
        identity: DockLiveUndockIdentity,
        source_key: DockHostLivePresentationKey,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        accepted_frame: u64,
        cx: &mut App,
    ) -> DockLiveUndockSourceFinishOutcome {
        let Some(restoration) =
            self.source_restoration_execution(identity, payload_lease.source(), payload_lease)
        else {
            return DockLiveUndockSourceFinishOutcome::Retry;
        };
        let Some(source) = restoration.source_restoration_batch.clone() else {
            return DockLiveUndockSourceFinishOutcome::Retry;
        };
        if restoration.source_key != Some(source_key) {
            return DockLiveUndockSourceFinishOutcome::Retry;
        }
        let projection = restoration.projection.clone();
        let result = if let Some(outcome) = self.committed_source_finish(identity, payload_lease) {
            Ok(outcome)
        } else {
            let Some(mut checkout) = self.checkout_presentation_session(identity, payload_lease)
            else {
                return DockLiveUndockSourceFinishOutcome::Retry;
            };
            checkout
                .session_mut()
                .accept_source_restoration_frame(cx, accepted_frame)
        };
        match result {
            Ok(view_presentation_window::SourcePresentationFinish::Finished(finished)) => {
                debug_assert!(finished.matches_exactly(&source));
                DockLiveUndockSourceFinishOutcome::Finished
            }
            Ok(
                view_presentation_window::SourcePresentationFinish::PresentationAuthorityReleased(
                    invalidation,
                ),
            ) => {
                let Some(receipt) =
                    DockLiveUndockPresentationAuthorityLossReceipt::from_invalidation(
                        payload_lease,
                        &projection,
                        invalidation,
                    )
                else {
                    return DockLiveUndockSourceFinishOutcome::Retry;
                };
                self.submit(
                    DockLiveUndockFact::PresentationAuthorityLost { receipt },
                    cx,
                )
                .then_some(DockLiveUndockSourceFinishOutcome::AuthorityLossSubmitted)
                .unwrap_or(DockLiveUndockSourceFinishOutcome::Retry)
            }
            Err(_) => DockLiveUndockSourceFinishOutcome::Retry,
        }
    }

    fn finish_unchanged_source_restoration(
        &self,
        restoration: &DockLiveUndockSourceRestorationExecution,
        source_leases: &view_presentation_window::LeaseBatch,
        cx: &mut App,
    ) {
        let Some(receipt) = DockLiveUndockSourceRestorationReceipt::source_unchanged(
            restoration.payload_lease,
            &restoration.projection,
            source_leases,
        ) else {
            self.defer_source_restoration_execution(
                restoration,
                DockLiveUndockSourceRestorationFailure::RestorationReceiptUnavailable,
                cx,
            );
            return;
        };
        if !Self::clear_exact_source_presentation(restoration, cx)
            || !Self::clear_exact_destination_presentation(restoration, cx)
            || !self.release_exact_restoration_visual(restoration, cx)
        {
            self.defer_source_restoration_execution(
                restoration,
                DockLiveUndockSourceRestorationFailure::SourcePresentationMutationRejected,
                cx,
            );
            return;
        }
        let _ = restoration
            .source_window
            .update(cx, |_, window, _| window.refresh());
        let _ = self.commit_source_restoration(restoration.identity, receipt, cx);
    }

    fn record_source_restoration_batch(
        &self,
        identity: DockLiveUndockIdentity,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        source_leases: &view_presentation_window::LeaseBatch,
    ) -> bool {
        let mut state = self.state.borrow_mut();
        let Some(presentation) = state
            .executions
            .get_mut(&identity)
            .and_then(|execution| execution.presentation.as_mut())
            .filter(|presentation| presentation.lease == payload_lease)
        else {
            return false;
        };
        if presentation
            .source_restoration_batch
            .as_ref()
            .is_some_and(|current| !current.matches_exactly(source_leases))
        {
            return false;
        }
        presentation.source_restoration_batch = Some(source_leases.clone());
        true
    }

    fn settle_source_for_restoration(
        &self,
        identity: DockLiveUndockIdentity,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        cx: &mut App,
    ) -> Option<
        Result<
            view_presentation_window::SourceSettlement,
            view_presentation_window::TransitionError,
        >,
    > {
        let mut checkout = self.checkout_presentation_session(identity, payload_lease)?;
        Some(checkout.session_mut().settle_source(cx))
    }

    fn submit_source_host_authority_loss(
        &self,
        restoration: &DockLiveUndockSourceRestorationExecution,
        cx: &mut App,
    ) {
        let Some(receipt) = DockLiveUndockPresentationAuthorityLossReceipt::from_source_host_loss(
            restoration.payload_lease,
            &restoration.projection,
        ) else {
            self.defer_source_restoration_execution(
                restoration,
                DockLiveUndockSourceRestorationFailure::RestorationReceiptUnavailable,
                cx,
            );
            return;
        };
        if self.submit(
            DockLiveUndockFact::PresentationAuthorityLost { receipt },
            cx,
        ) {
            self.clear_source_restoration_retry(restoration.identity);
        } else {
            self.defer_source_restoration_execution(
                restoration,
                DockLiveUndockSourceRestorationFailure::ExecutionAuthorityUnavailable,
                cx,
            );
        }
    }

    fn install_source_restoration(
        &self,
        restoration: &DockLiveUndockSourceRestorationExecution,
        source_leases: view_presentation_window::LeaseBatch,
        cx: &mut App,
    ) {
        let Some(source_key) = restoration.source_key else {
            self.submit_source_host_authority_loss(restoration, cx);
            return;
        };
        if !Self::clear_exact_destination_presentation(restoration, cx) {
            self.defer_source_restoration_execution(
                restoration,
                DockLiveUndockSourceRestorationFailure::SourcePresentationMutationRejected,
                cx,
            );
            return;
        }
        let outcome = restoration
            .source_host
            .update(cx, |host, cx| {
                host.begin_live_source_restoration(
                    source_key,
                    restoration.restore_session.clone(),
                    source_leases.clone(),
                    cx,
                )
            })
            .unwrap_or(DockHostLiveSourceRestorationInstallOutcome::PresentationAuthorityLost);
        match outcome {
            DockHostLiveSourceRestorationInstallOutcome::Installed
            | DockHostLiveSourceRestorationInstallOutcome::AlreadyInstalled => {
                let _ = restoration
                    .source_window
                    .update(cx, |_, window, _| window.refresh());
            }
            DockHostLiveSourceRestorationInstallOutcome::PresentationAuthorityLost => {
                self.submit_source_host_authority_loss(restoration, cx);
            }
        }
    }

    fn restore_source(
        &self,
        identity: DockLiveUndockIdentity,
        source: DockLiveUndockSourceSnapshot,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        restore_focus: bool,
        cx: &mut App,
    ) {
        if !self.record_source_restoration_focus_intent(
            identity,
            source,
            payload_lease,
            restore_focus,
        ) {
            self.defer_source_restoration(
                identity,
                source,
                payload_lease,
                DockLiveUndockSourceRestorationFailure::ExecutionAuthorityUnavailable,
                cx,
            );
            return;
        }
        let Some(restoration) = self.source_restoration_execution(identity, source, payload_lease)
        else {
            self.defer_source_restoration(
                identity,
                source,
                payload_lease,
                DockLiveUndockSourceRestorationFailure::ExecutionAuthorityUnavailable,
                cx,
            );
            return;
        };

        if let Some(receipt) = restoration.source_restoration_receipt {
            self.finish_presented_source_restoration(&restoration, receipt, cx);
            return;
        }

        let Some(settlement) = self.settle_source_for_restoration(identity, payload_lease, cx)
        else {
            self.defer_source_restoration_execution(
                &restoration,
                DockLiveUndockSourceRestorationFailure::ExecutionAuthorityUnavailable,
                cx,
            );
            return;
        };
        match settlement {
            Ok(view_presentation_window::SourceSettlement::RetiredToSource(source_leases)) => {
                self.finish_unchanged_source_restoration(&restoration, &source_leases, cx);
            }
            Ok(view_presentation_window::SourceSettlement::AlreadyRetired) => {
                let source_leases = restoration.projection.source().clone();
                self.finish_unchanged_source_restoration(&restoration, &source_leases, cx);
            }
            Ok(view_presentation_window::SourceSettlement::RenderSource(source_leases)) => {
                if !self.record_source_restoration_batch(identity, payload_lease, &source_leases) {
                    self.defer_source_restoration_execution(
                        &restoration,
                        DockLiveUndockSourceRestorationFailure::ExecutionAuthorityUnavailable,
                        cx,
                    );
                    return;
                }
                self.install_source_restoration(&restoration, source_leases, cx);
            }
            Ok(view_presentation_window::SourceSettlement::AwaitingSourceNativeTerminal) => {
                self.defer_source_restoration_execution(
                    &restoration,
                    DockLiveUndockSourceRestorationFailure::AwaitingSourceNativeTerminal,
                    cx,
                );
            }
            Ok(view_presentation_window::SourceSettlement::PresentationAuthorityReleased(
                invalidation,
            )) => {
                let Some(receipt) =
                    DockLiveUndockPresentationAuthorityLossReceipt::from_invalidation(
                        restoration.payload_lease,
                        &restoration.projection,
                        invalidation,
                    )
                else {
                    self.defer_source_restoration_execution(
                        &restoration,
                        DockLiveUndockSourceRestorationFailure::RestorationReceiptUnavailable,
                        cx,
                    );
                    return;
                };
                let _ = self.submit(
                    DockLiveUndockFact::PresentationAuthorityLost { receipt },
                    cx,
                );
            }
            Err(_) => {
                self.defer_source_restoration_execution(
                    &restoration,
                    DockLiveUndockSourceRestorationFailure::PresentationTransitionRejected,
                    cx,
                );
            }
        }
    }

    fn prepare_presentation_cleanup(
        &self,
        identity: DockLiveUndockIdentity,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        cx: &App,
    ) -> Option<
        Result<
            DockLiveUndockPreparedPresentationCleanup,
            view_presentation_window::TransitionError,
        >,
    > {
        let terminal = {
            let state = self.state.borrow();
            let presentation = state.executions.get(&identity)?.presentation.as_ref()?;
            if presentation.lease != payload_lease {
                return None;
            }
            presentation
                .session
                .active()?
                .terminal_disposition()
                .map(|terminal| {
                    (
                        terminal,
                        presentation.projection.generation(),
                        presentation.projection.source().window_id(),
                        presentation.projection.destination().window_id(),
                    )
                })
        };
        match terminal {
            Some((
                view_presentation_window::RehostTerminalDisposition::SourceCommitted,
                generation,
                source_window,
                destination_window,
            )) => {
                return Some(Ok(
                    DockLiveUndockPreparedPresentationCleanup::AlreadyTerminal(
                        DockLiveUndockRehostCleanupEvidence::source_committed(
                            generation,
                            source_window,
                            destination_window,
                        ),
                    ),
                ));
            }
            Some((
                view_presentation_window::RehostTerminalDisposition::PresentationAuthorityReleased,
                generation,
                source_window,
                destination_window,
            )) => {
                return Some(Ok(
                    DockLiveUndockPreparedPresentationCleanup::AlreadyTerminal(
                        DockLiveUndockRehostCleanupEvidence::already_absent(
                            generation,
                            source_window,
                            destination_window,
                        ),
                    ),
                ));
            }
            Some((
                view_presentation_window::RehostTerminalDisposition::DestinationCommitted,
                ..,
            )) => {
                return Some(Err(
                    view_presentation_window::TransitionError::ConflictingTerminalOutcome,
                ));
            }
            Some((view_presentation_window::RehostTerminalDisposition::Abandoned, ..)) | None => {}
        }
        Some(
            match self.prepare_presentation_terminal(
                identity,
                payload_lease,
                view_presentation_window::RehostTerminalIntent::AbandonAfterSourceLoss,
                cx,
            )? {
                Ok(view_presentation_window::RehostTerminalPreparation::Prepared(prepared)) => {
                    Ok(DockLiveUndockPreparedPresentationCleanup::Exact(prepared))
                }
                Ok(view_presentation_window::RehostTerminalPreparation::AlreadyCommitted(
                    view_presentation_window::RehostTerminalOutcome::Abandoned(receipt),
                )) => Ok(DockLiveUndockPreparedPresentationCleanup::AlreadyTerminal(
                    DockLiveUndockRehostCleanupEvidence::already_absent(
                        receipt.generation(),
                        receipt.source_window(),
                        receipt.destination_window(),
                    ),
                )),
                Ok(view_presentation_window::RehostTerminalPreparation::AlreadyCommitted(
                    view_presentation_window::RehostTerminalOutcome::DestinationCommitted(_),
                )) => Err(view_presentation_window::TransitionError::ConflictingTerminalOutcome),
                Err(error) => Err(error),
            },
        )
    }

    fn can_commit_presentation_cleanup(
        prepared: &DockLiveUndockPreparedPresentationCleanup,
        cx: &App,
    ) -> bool {
        match prepared {
            DockLiveUndockPreparedPresentationCleanup::Exact(prepared) => prepared.can_commit(cx),
            DockLiveUndockPreparedPresentationCleanup::AlreadyTerminal(_) => true,
        }
    }

    fn restore_prepared_presentation_cleanup(
        &self,
        prepared: DockLiveUndockPreparedPresentationCleanup,
    ) -> bool {
        drop(prepared);
        true
    }

    fn restore_orphan_cleanup_presentation(
        &self,
        prepared: DockLiveUndockPreparedOrphanCleanup,
        cx: &mut App,
    ) -> bool {
        let _ = cx;
        self.restore_prepared_presentation_cleanup(prepared.presentation)
    }

    fn commit_presentation_cleanup(
        prepared: DockLiveUndockPreparedPresentationCleanup,
        cx: &mut App,
    ) -> DockLiveUndockRehostCleanupEvidence {
        match prepared {
            DockLiveUndockPreparedPresentationCleanup::Exact(prepared) => {
                let view_presentation_window::RehostTerminalOutcome::Abandoned(receipt) = prepared
                    .try_commit(cx)
                    .expect("preflighted source-loss abandonment must remain exact")
                else {
                    unreachable!("source-loss terminal preparation committed a destination")
                };
                DockLiveUndockRehostCleanupEvidence::abandoned(
                    receipt.generation(),
                    receipt.source_window(),
                    receipt.destination_window(),
                )
            }
            DockLiveUndockPreparedPresentationCleanup::AlreadyTerminal(evidence) => evidence,
        }
    }

    fn prepare_host_presentation_abandonment(
        host: Entity<DockHost>,
        identity: DockLiveUndockIdentity,
        rehost_generation: u64,
        window_id: WindowId,
        cx: &App,
    ) -> Option<DockLiveUndockPreparedHostPresentationAbandonment> {
        let (state, semantics) = cx.read_entity(&host, |host, _| {
            (
                host.live_presentation_state(),
                host.live_destination_semantics(),
            )
        });
        if semantics
            .as_ref()
            .is_some_and(|semantics| semantics.identity() == identity)
        {
            return None;
        }
        match state {
            Some(state)
                if state.key.identity() == identity
                    && state.key.rehost_generation() == rehost_generation
                    && state.key.binding().window_id() == window_id =>
            {
                let key = state.key;
                let prepared = cx.read_entity(&host, |host, _| {
                    host.prepare_live_presentation_abandonment(key)
                })?;
                Some(DockLiveUndockPreparedHostPresentationAbandonment::Exact { host, prepared })
            }
            Some(state) if state.key.identity() == identity => None,
            _ => Some(
                DockLiveUndockPreparedHostPresentationAbandonment::AlreadyAbsent {
                    host,
                    identity,
                    window_id,
                },
            ),
        }
    }

    fn can_commit_host_presentation_abandonment(
        prepared: &DockLiveUndockPreparedHostPresentationAbandonment,
        cx: &App,
    ) -> bool {
        match prepared {
            DockLiveUndockPreparedHostPresentationAbandonment::Exact { host, prepared } => cx
                .read_entity(host, |host, _| {
                    host.can_commit_prepared_live_presentation_abandonment(prepared)
                }),
            DockLiveUndockPreparedHostPresentationAbandonment::AlreadyAbsent {
                host,
                identity,
                ..
            } => cx.read_entity(host, |host, _| {
                host.live_presentation_state()
                    .is_none_or(|state| state.key.identity() != *identity)
                    && host
                        .live_destination_semantics()
                        .is_none_or(|semantics| semantics.identity() != *identity)
            }),
            DockLiveUndockPreparedHostPresentationAbandonment::HostUnavailable { .. } => true,
        }
    }

    fn commit_host_presentation_abandonment(
        prepared: DockLiveUndockPreparedHostPresentationAbandonment,
        cx: &mut App,
    ) -> DockLiveUndockHostCleanupEvidence {
        match prepared {
            DockLiveUndockPreparedHostPresentationAbandonment::Exact { host, prepared } => {
                let receipt = cx.update_entity(&host, |host, host_cx| {
                    host.commit_prepared_live_presentation_abandonment(prepared, host_cx)
                });
                DockLiveUndockHostCleanupEvidence::abandoned(receipt)
            }
            DockLiveUndockPreparedHostPresentationAbandonment::AlreadyAbsent {
                window_id, ..
            } => DockLiveUndockHostCleanupEvidence::already_absent(window_id),
            DockLiveUndockPreparedHostPresentationAbandonment::HostUnavailable { window_id } => {
                DockLiveUndockHostCleanupEvidence::host_unavailable(window_id)
            }
        }
    }

    fn prepare_source_semantic_retirement(
        host: WeakEntity<DockHost>,
        key: DockHostLivePresentationKey,
        lease: DockLiveUndockPayloadLeaseReceipt,
        cx: &App,
    ) -> Option<DockLiveUndockPreparedSourceSemanticRetirement> {
        let Some(host) = host.upgrade() else {
            return Some(DockLiveUndockPreparedSourceSemanticRetirement::HostUnavailable);
        };
        let prepared = cx.read_entity(&host, |host, _| {
            host.prepare_live_source_semantic_retirement(key, lease)
        });
        match prepared {
            Some(prepared) => {
                Some(DockLiveUndockPreparedSourceSemanticRetirement::Exact { host, prepared })
            }
            None => Some(
                DockLiveUndockPreparedSourceSemanticRetirement::AlreadyAbsent { host, key, lease },
            ),
        }
    }

    fn can_commit_source_semantic_retirement(
        prepared: &DockLiveUndockPreparedSourceSemanticRetirement,
        cx: &App,
    ) -> bool {
        match prepared {
            DockLiveUndockPreparedSourceSemanticRetirement::Exact { host, prepared } => cx
                .read_entity(host, |host, _| {
                    host.can_commit_prepared_live_source_semantic_retirement(prepared)
                }),
            DockLiveUndockPreparedSourceSemanticRetirement::AlreadyAbsent { host, key, lease } => {
                cx.read_entity(host, |host, _| {
                    !host.accepts_live_source_semantic_proxy(*key, *lease)
                })
            }
            DockLiveUndockPreparedSourceSemanticRetirement::HostUnavailable => true,
        }
    }

    fn commit_source_semantic_retirement(
        prepared: DockLiveUndockPreparedSourceSemanticRetirement,
        cx: &mut App,
    ) {
        if let DockLiveUndockPreparedSourceSemanticRetirement::Exact { host, prepared } = prepared {
            cx.update_entity(&host, |host, host_cx| {
                host.commit_prepared_live_source_semantic_retirement(prepared, host_cx);
            });
        }
    }

    fn prepare_retained_visual_cleanup(
        source_window: AnyWindowHandle,
        ticket: Ticket,
        already_released: bool,
        cx: &mut App,
    ) -> Option<DockLiveUndockPreparedRetainedVisualCleanup> {
        if already_released {
            return Some(DockLiveUndockPreparedRetainedVisualCleanup::AlreadyReleased(ticket));
        }
        match source_window.update(cx, |_, window, _| {
            retained_visual::prepare_release(window, &ticket)
        }) {
            Ok(Ok(prepared)) => Some(DockLiveUndockPreparedRetainedVisualCleanup::Exact {
                source_window,
                ticket,
                prepared,
            }),
            Ok(Err(
                retained_visual::Invalidation::StaleGeneration
                | retained_visual::Invalidation::WindowClosed,
            )) => Some(
                DockLiveUndockPreparedRetainedVisualCleanup::AuthorityAbsent {
                    source_window,
                    ticket,
                },
            ),
            Err(_) => Some(
                DockLiveUndockPreparedRetainedVisualCleanup::WindowUnavailable {
                    source_window,
                    ticket,
                },
            ),
            Ok(Err(_)) => None,
        }
    }

    fn can_commit_retained_visual_cleanup(
        prepared: &DockLiveUndockPreparedRetainedVisualCleanup,
        cx: &mut App,
    ) -> bool {
        match prepared {
            DockLiveUndockPreparedRetainedVisualCleanup::AlreadyReleased(_) => true,
            DockLiveUndockPreparedRetainedVisualCleanup::Exact {
                source_window,
                prepared,
                ..
            } => source_window
                .update(cx, |_, window, _| {
                    retained_visual::can_commit_prepared_release(window, prepared)
                })
                .unwrap_or(false),
            DockLiveUndockPreparedRetainedVisualCleanup::AuthorityAbsent {
                source_window,
                ticket,
            } => matches!(
                source_window.update(cx, |_, window, _| {
                    retained_visual::prepare_release(window, ticket)
                }),
                Ok(Err(retained_visual::Invalidation::StaleGeneration
                    | retained_visual::Invalidation::WindowClosed))
            ),
            DockLiveUndockPreparedRetainedVisualCleanup::WindowUnavailable {
                source_window,
                ..
            } => source_window.update(cx, |_, _, _| ()).is_err(),
        }
    }

    fn mark_orphan_retained_visual_released(
        &self,
        identity: DockLiveUndockIdentity,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
    ) {
        let marked = self
            .state
            .borrow_mut()
            .executions
            .get_mut(&identity)
            .and_then(|execution| execution.presentation.as_mut())
            .filter(|presentation| presentation.lease == payload_lease)
            .is_some_and(|presentation| {
                presentation.retained_released = true;
                true
            });
        assert!(
            marked,
            "orphan cleanup must checkpoint the exact retained-visual terminal"
        );
    }

    fn commit_retained_visual_cleanup(
        &self,
        identity: DockLiveUndockIdentity,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        prepared: DockLiveUndockPreparedRetainedVisualCleanup,
        cx: &mut App,
    ) -> DockLiveUndockRetainedVisualCleanupEvidence {
        match prepared {
            DockLiveUndockPreparedRetainedVisualCleanup::AlreadyReleased(ticket) => {
                DockLiveUndockRetainedVisualCleanupEvidence::AlreadyReleased(ticket.identity())
            }
            DockLiveUndockPreparedRetainedVisualCleanup::Exact {
                source_window,
                ticket,
                prepared,
            } => {
                source_window
                    .update(cx, |_, window, _| {
                        retained_visual::commit_prepared_release(window, prepared);
                    })
                    .expect("preflighted orphan cleanup must retain its exact source window");
                self.mark_orphan_retained_visual_released(identity, payload_lease);
                DockLiveUndockRetainedVisualCleanupEvidence::Released(ticket.identity())
            }
            DockLiveUndockPreparedRetainedVisualCleanup::AuthorityAbsent { ticket, .. } => {
                self.mark_orphan_retained_visual_released(identity, payload_lease);
                DockLiveUndockRetainedVisualCleanupEvidence::AuthorityAbsent(ticket.identity())
            }
            DockLiveUndockPreparedRetainedVisualCleanup::WindowUnavailable { ticket, .. } => {
                self.mark_orphan_retained_visual_released(identity, payload_lease);
                DockLiveUndockRetainedVisualCleanupEvidence::WindowUnavailable(ticket.identity())
            }
        }
    }

    fn prepare_orphan_cleanup(
        &self,
        identity: DockLiveUndockIdentity,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        provisional: Option<AnyWindowHandle>,
        cx: &mut App,
    ) -> Option<DockLiveUndockPreparedOrphanCleanup> {
        let (
            runtime,
            source_host,
            destination_host,
            projection,
            source_window,
            retained,
            retained_released,
            transport,
        ) = {
            let state = self.state.borrow();
            let execution = state.executions.get(&identity)?;
            let presentation = execution.presentation.as_ref()?;
            if presentation.lease != payload_lease
                || presentation.lease.identity() != identity
                || execution.request.key() != identity.opening()
                || presentation.retained.identity() != payload_lease.retained_visual()?
                || matches!(
                    execution.promotion.as_ref(),
                    Some(DockLiveUndockPromotionExecution::Durable(_))
                )
                || provisional.is_some_and(|window| {
                    execution
                        .destination_host
                        .is_none_or(|current| current.window_id() != window.window_id())
                })
            {
                return None;
            }
            (
                execution.seed.source.runtime.clone(),
                execution.seed.source.source_host.clone(),
                execution.destination_host,
                presentation.projection.clone(),
                execution.seed.source.source_window,
                presentation.retained,
                presentation.retained_released,
                execution.seed.source.source_transport.clone(),
            )
        };
        let source_window_id = payload_lease.source().window_id();
        let destination_window_id = payload_lease.destination_window();
        let presentation_generation = projection.generation();
        let source_transport_host = source_host.clone();
        let source_host = source_host.upgrade().map_or(
            Some(
                DockLiveUndockPreparedHostPresentationAbandonment::HostUnavailable {
                    window_id: source_window_id,
                },
            ),
            |host| {
                Self::prepare_host_presentation_abandonment(
                    host,
                    identity,
                    presentation_generation,
                    source_window_id,
                    cx,
                )
            },
        )?;
        let destination_host = destination_host.map_or(
            Some(
                DockLiveUndockPreparedHostPresentationAbandonment::HostUnavailable {
                    window_id: destination_window_id,
                },
            ),
            |window| {
                if window.window_id() != destination_window_id {
                    return None;
                }
                match window.entity(cx) {
                    Ok(host) => Self::prepare_host_presentation_abandonment(
                        host,
                        identity,
                        presentation_generation,
                        destination_window_id,
                        cx,
                    ),
                    Err(_) => Some(
                        DockLiveUndockPreparedHostPresentationAbandonment::HostUnavailable {
                            window_id: destination_window_id,
                        },
                    ),
                }
            },
        )?;
        let retained =
            Self::prepare_retained_visual_cleanup(source_window, retained, retained_released, cx)?;
        if !self.presentation_projection_matches(identity, payload_lease, &projection) {
            return None;
        }
        let presentation = match self.prepare_presentation_cleanup(identity, payload_lease, cx)? {
            Ok(prepared) => prepared,
            Err(_) => return None,
        };
        Some(DockLiveUndockPreparedOrphanCleanup {
            identity,
            payload_lease,
            runtime,
            presentation,
            presentation_generation,
            source_host,
            destination_host,
            retained,
            source_transport_host,
            transport,
        })
    }

    fn preflight_orphan_cleanup(
        &self,
        prepared: &DockLiveUndockPreparedOrphanCleanup,
        cx: &mut App,
    ) -> bool {
        let execution_is_exact = self
            .state
            .borrow()
            .executions
            .get(&prepared.identity)
            .is_some_and(|execution| {
                execution.request.key() == prepared.identity.opening()
                    && execution.presentation.as_ref().is_some_and(|presentation| {
                        presentation.lease == prepared.payload_lease
                            && presentation.projection.generation()
                                == prepared.presentation_generation
                            && presentation.session.is_active()
                            && Some(presentation.retained.identity())
                                == prepared.payload_lease.retained_visual()
                            && match &prepared.retained {
                                DockLiveUndockPreparedRetainedVisualCleanup::AlreadyReleased(_) => {
                                    presentation.retained_released
                                }
                                DockLiveUndockPreparedRetainedVisualCleanup::Exact { .. }
                                | DockLiveUndockPreparedRetainedVisualCleanup::AuthorityAbsent {
                                    ..
                                }
                                | DockLiveUndockPreparedRetainedVisualCleanup::WindowUnavailable {
                                    ..
                                } => !presentation.retained_released,
                            }
                    })
                    && execution.seed.source.source_transport.key() == prepared.transport.key()
                    && !matches!(
                        execution.promotion.as_ref(),
                        Some(DockLiveUndockPromotionExecution::Durable(_))
                    )
            });
        execution_is_exact
            && Self::can_commit_presentation_cleanup(&prepared.presentation, cx)
            && Self::can_commit_host_presentation_abandonment(&prepared.source_host, cx)
            && Self::can_commit_host_presentation_abandonment(&prepared.destination_host, cx)
            && Self::can_commit_retained_visual_cleanup(&prepared.retained, cx)
    }

    fn commit_orphan_transport_cleanup(
        source_host: WeakEntity<DockHost>,
        transport: crate::native_captured_drag::DockNativeCapturedDragTransportLease,
        cx: &mut App,
    ) -> crate::native_captured_drag::DockNativeCapturedDragTransportRetirementReceipt {
        let key = transport.key();
        let receipt = transport.retire();
        if let Some(source_host) = source_host.upgrade() {
            let _ = cx.update_entity(&source_host, |host, host_cx| {
                host.retire_native_drag_transport_proxy(key, host_cx)
            });
            assert!(!cx.read_entity(&source_host, |host, _| {
                host.has_native_drag_transport_proxy_key(key)
            }));
        }
        assert!(!transport.is_active());
        receipt
    }

    fn commit_orphan_cleanup(
        &self,
        prepared: DockLiveUndockPreparedOrphanCleanup,
        cx: &mut App,
    ) -> DockLiveUndockOrphanCleanupReceipt {
        let DockLiveUndockPreparedOrphanCleanup {
            identity,
            payload_lease,
            runtime: _,
            presentation,
            presentation_generation,
            source_host,
            destination_host,
            retained,
            source_transport_host,
            transport,
        } = prepared;
        let rehost = Self::commit_presentation_cleanup(presentation, cx);
        assert_eq!(
            rehost.authority().0,
            presentation_generation,
            "orphan cleanup must abandon the exact presentation generation"
        );
        let source_host = Self::commit_host_presentation_abandonment(source_host, cx);
        let destination_host = Self::commit_host_presentation_abandonment(destination_host, cx);
        let retained = self.commit_retained_visual_cleanup(identity, payload_lease, retained, cx);
        let transport = Self::commit_orphan_transport_cleanup(source_transport_host, transport, cx);
        DockLiveUndockOrphanCleanupReceipt::new(
            payload_lease,
            rehost,
            source_host,
            destination_host,
            retained,
            transport,
        )
        .expect("committed orphan cleanup must preserve exact aggregate authority")
    }

    fn prepare_orphan_recovery(
        &self,
        owner: &Entity<DockSurfaceOwner>,
        identity: DockLiveUndockIdentity,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        provisional: Option<AnyWindowHandle>,
        cx: &mut App,
    ) -> Option<DockLiveUndockPreparedOrphanRecoveryExecution> {
        let payload_identity = self
            .state
            .borrow()
            .executions
            .get(&identity)?
            .seed
            .move_plan
            .source_identity()
            .clone();
        #[cfg(test)]
        if self.state.borrow().reject_orphan_recovery_records {
            return None;
        }
        let recovery = cx.update_entity(owner, |owner, owner_cx| {
            if !owner.accepts_live_undock_identity(identity) {
                return None;
            }
            match owner.prepare_payload_recovery(
                DockPayloadRecoveryAuthority::presentation_lease(payload_lease),
                &payload_identity,
                DockPayloadRecoveryReason::PreCommitOrphan,
                owner_cx,
            ) {
                Ok(prepared) => Some(DockLiveUndockPreparedRecoveryRecord::Prepared(prepared)),
                Err(
                    unresolved @ (DockPayloadRecoveryPrepareError::PayloadMissing
                    | DockPayloadRecoveryPrepareError::PayloadAmbiguous),
                ) => owner
                    .prepare_unresolved_payload_recovery(
                        DockPayloadRecoveryAuthority::presentation_lease(payload_lease),
                        &payload_identity,
                        DockPayloadRecoveryReason::PreCommitOrphan,
                        unresolved,
                        owner_cx,
                    )
                    .ok()
                    .map(DockLiveUndockPreparedRecoveryRecord::Prepared),
                Err(DockPayloadRecoveryPrepareError::PayloadAlreadyCommitted) => owner
                    .committed_payload_recovery_receipt(
                        DockPayloadRecoveryAuthority::presentation_lease(payload_lease),
                        DockPayloadRecoveryReason::PreCommitOrphan,
                    )
                    .map(DockLiveUndockPreparedRecoveryRecord::AlreadyCommitted),
                Err(_) => None,
            }
        });
        let recovery = recovery?;
        let cleanup = self.prepare_orphan_cleanup(identity, payload_lease, provisional, cx);
        let cleanup = cleanup?;
        Some(DockLiveUndockPreparedOrphanRecoveryExecution { recovery, cleanup })
    }

    fn preflight_orphan_recovery(
        &self,
        owner: &Entity<DockSurfaceOwner>,
        prepared: &DockLiveUndockPreparedOrphanRecoveryExecution,
        cx: &mut App,
    ) -> bool {
        self.preflight_orphan_cleanup(&prepared.cleanup, cx)
            && cx.update_entity(owner, |owner, owner_cx| {
                owner.accepts_live_undock_identity(prepared.cleanup.identity)
                    && match &prepared.recovery {
                        DockLiveUndockPreparedRecoveryRecord::Prepared(recovery) => {
                            owner.can_commit_payload_recovery(recovery, owner_cx)
                        }
                        DockLiveUndockPreparedRecoveryRecord::AlreadyCommitted(receipt) => {
                            owner.committed_payload_recovery_receipt(
                                DockPayloadRecoveryAuthority::presentation_lease(
                                    prepared.cleanup.payload_lease,
                                ),
                                DockPayloadRecoveryReason::PreCommitOrphan,
                            ) == Some(*receipt)
                        }
                    }
            })
    }

    fn commit_orphan_recovery(
        &self,
        owner: &Entity<DockSurfaceOwner>,
        prepared: DockLiveUndockPreparedOrphanRecoveryExecution,
        cx: &mut App,
    ) -> Option<(
        super::payload_recovery::DockPayloadRecoveryCommitReceipt,
        DockLiveUndockOrphanCleanupReceipt,
    )> {
        let transaction_runtime = prepared.cleanup.runtime.clone();
        let mut prepared = Some(prepared);
        let committed = transaction_runtime.with_surface_transaction(cx, |transaction, cx| {
            let Some(transaction) = transaction else {
                return None;
            };
            let current = prepared
                .as_ref()
                .expect("orphan recovery preparation must remain owned until commit");
            if !self.preflight_orphan_recovery(owner, current, cx) {
                return None;
            }
            let DockLiveUndockPreparedOrphanRecoveryExecution { recovery, cleanup } = prepared
                .take()
                .expect("preflighted orphan recovery must commit exactly once");
            let recovery = match &recovery {
                DockLiveUndockPreparedRecoveryRecord::Prepared(recovery) => cx
                    .update_entity(owner, |owner, owner_cx| {
                        owner.commit_payload_recovery(transaction, recovery, owner_cx)
                    })
                    .expect("preflighted payload recovery must commit in the same transaction"),
                DockLiveUndockPreparedRecoveryRecord::AlreadyCommitted(receipt) => *receipt,
            };
            #[cfg(test)]
            if std::mem::take(
                &mut self
                    .state
                    .borrow_mut()
                    .interrupt_orphan_cleanup_after_recovery_commit_once,
            ) {
                return None;
            }
            let cleanup = self.commit_orphan_cleanup(cleanup, cx);
            Some((recovery, cleanup))
        });
        if committed.is_none()
            && let Some(prepared) = prepared.take()
        {
            self.restore_orphan_cleanup_presentation(prepared.cleanup, cx);
        }
        committed
    }

    fn execute_shutdown_orphan_cleanup(
        &self,
        identity: DockLiveUndockIdentity,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        provisional: Option<AnyWindowHandle>,
        cx: &mut App,
    ) -> Result<DockLiveUndockOrphanCleanupReceipt, DockLiveUndockOrphanCleanupFailure> {
        let prepared = self
            .prepare_orphan_cleanup(identity, payload_lease, provisional, cx)
            .ok_or(DockLiveUndockOrphanCleanupFailure::PreparationRejected)?;
        let transaction_runtime = prepared.runtime.clone();
        let mut prepared = Some(prepared);
        let committed = transaction_runtime.with_surface_transaction(cx, |transaction, cx| {
            transaction?;
            let current = prepared
                .as_ref()
                .expect("orphan cleanup preparation must remain owned until commit");
            if !self.preflight_orphan_cleanup(current, cx) {
                return None;
            }
            Some(
                self.commit_orphan_cleanup(
                    prepared
                        .take()
                        .expect("preflighted orphan cleanup must commit exactly once"),
                    cx,
                ),
            )
        });
        if committed.is_none()
            && let Some(prepared) = prepared.take()
        {
            self.restore_orphan_cleanup_presentation(prepared, cx);
        }
        committed.ok_or(DockLiveUndockOrphanCleanupFailure::PreflightRejected)
    }

    fn recover_orphaned_payload_topology(
        &self,
        owner: &Entity<DockSurfaceOwner>,
        identity: DockLiveUndockIdentity,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
        provisional: Option<AnyWindowHandle>,
        cx: &mut App,
    ) -> Option<DockLiveUndockFact> {
        let (recovery, cleanup) = self
            .prepare_orphan_recovery(owner, identity, payload_lease, provisional, cx)
            .and_then(|prepared| self.commit_orphan_recovery(owner, prepared, cx))?;
        let receipt = DockLiveUndockOrphanRecoveryReceipt::new(recovery, cleanup)
            .expect("orphan recovery must retain presentation and cleanup authority");
        Some(
            if recovery.disposition() == DockPayloadRecoveryDisposition::Unresolved {
                DockLiveUndockFact::OrphanRecoveryFailed { identity, receipt }
            } else {
                DockLiveUndockFact::OrphanRecoveryCommitted { identity, receipt }
            },
        )
    }

    fn prepare_committed_destination_recovery(
        &self,
        owner: &Entity<DockSurfaceOwner>,
        identity: DockLiveUndockIdentity,
        authority: DockPayloadRecoveryAuthority,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
        cx: &mut App,
    ) -> Option<DockLiveUndockPreparedCommittedDestinationRecoveryExecution> {
        #[cfg(test)]
        if self
            .state
            .borrow()
            .reject_committed_destination_recovery_records
        {
            return None;
        }
        let (
            runtime,
            payload_identity,
            recovery_focus,
            destination_window,
            presentation_lease,
            source_semantic_seed,
            presentation_origin,
        ) = {
            let state = self.state.borrow();
            let execution = state.executions.get(&identity)?;
            let DockLiveUndockPromotionExecution::Durable(durable) =
                execution.promotion.as_ref()?
            else {
                return None;
            };
            if execution.request.key() != identity.opening()
                || authority.promotion() != Some((identity, token, destination))
                || durable.identity() != identity
                || durable.token() != token
                || durable.destination() != destination
                || durable.destination_window().window_id() != destination.window_id()
            {
                return None;
            }
            let (presentation_lease, source_semantic_seed) = match execution.presentation.as_ref() {
                Some(presentation) => (
                    Some(presentation.lease),
                    presentation.source_key.map(|source_key| {
                        (
                            execution.seed.source.source_host.clone(),
                            source_key,
                            presentation.lease,
                        )
                    }),
                ),
                None => {
                    if matches!(
                        destination,
                        DockLiveUndockPromotionDestination::SameWindowDesktop { .. }
                    ) {
                        return None;
                    }
                    (None, None)
                }
            };
            let presentation_origin = DockPayloadRecoveryPresentationOrigin::new(
                durable.destination_window(),
                durable.destination_binding(),
                durable.registration().clone(),
            )?;
            (
                execution.seed.source.runtime.clone(),
                execution.seed.move_plan.source_identity().clone(),
                DockPayloadRecoveryFocus::new(
                    execution.seed.source.session.focus_item().cloned(),
                    execution.seed.source.source_focus.clone(),
                ),
                durable.destination_window(),
                presentation_lease,
                source_semantic_seed,
                presentation_origin,
            )
        };
        let source_semantic = match source_semantic_seed {
            Some((source_host, source_key, payload_lease)) => {
                Some(Self::prepare_source_semantic_retirement(
                    source_host,
                    source_key,
                    payload_lease,
                    cx,
                )?)
            }
            None => None,
        };

        let recovery = cx.update_entity(owner, |owner, owner_cx| {
            if !owner.accepts_live_undock_identity(identity) {
                return None;
            }
            match owner.prepare_payload_recovery_with_focus_and_origin(
                authority,
                &payload_identity,
                DockPayloadRecoveryReason::LostViewportRecovery,
                recovery_focus,
                presentation_origin.clone(),
                owner_cx,
            ) {
                Ok(prepared) => Some(DockLiveUndockPreparedRecoveryRecord::Prepared(prepared)),
                Err(
                    unresolved @ (DockPayloadRecoveryPrepareError::PayloadMissing
                    | DockPayloadRecoveryPrepareError::PayloadAmbiguous),
                ) => owner
                    .prepare_unresolved_payload_recovery_with_origin(
                        authority,
                        &payload_identity,
                        DockPayloadRecoveryReason::LostViewportRecovery,
                        unresolved,
                        presentation_origin,
                        owner_cx,
                    )
                    .ok()
                    .map(DockLiveUndockPreparedRecoveryRecord::Prepared),
                Err(DockPayloadRecoveryPrepareError::PayloadAlreadyCommitted) => owner
                    .committed_payload_recovery_receipt(
                        authority,
                        DockPayloadRecoveryReason::LostViewportRecovery,
                    )
                    .map(DockLiveUndockPreparedRecoveryRecord::AlreadyCommitted),
                Err(_) => None,
            }
        })?;

        Some(
            DockLiveUndockPreparedCommittedDestinationRecoveryExecution {
                identity,
                authority,
                presentation_lease,
                token,
                destination,
                runtime,
                recovery,
                source_semantic,
                destination_window,
            },
        )
    }

    fn preflight_committed_destination_recovery(
        &self,
        owner: &Entity<DockSurfaceOwner>,
        prepared: &DockLiveUndockPreparedCommittedDestinationRecoveryExecution,
        cx: &mut App,
    ) -> bool {
        let execution_is_exact = self
            .state
            .borrow()
            .executions
            .get(&prepared.identity)
            .is_some_and(|execution| {
                execution.request.key() == prepared.identity.opening()
                    && match (execution.presentation.as_ref(), prepared.presentation_lease) {
                        (Some(presentation), Some(payload_lease)) => {
                            presentation.lease == payload_lease
                        }
                        (None, None) => matches!(
                            prepared.destination,
                            DockLiveUndockPromotionDestination::Host(_)
                        ),
                        (Some(_), None) | (None, Some(_)) => false,
                    }
                    && matches!(
                        execution.promotion.as_ref(),
                        Some(DockLiveUndockPromotionExecution::Durable(durable))
                            if durable.identity() == prepared.identity
                                && durable.token() == prepared.token
                                && durable.destination() == prepared.destination
                                && durable.destination_window()
                                    == prepared.destination_window
                    )
            });
        execution_is_exact
            && cx.update_entity(owner, |owner, owner_cx| {
                owner.accepts_live_undock_identity(prepared.identity)
                    && match prepared.recovery {
                        DockLiveUndockPreparedRecoveryRecord::Prepared(ref recovery) => {
                            owner.can_commit_payload_recovery(recovery, owner_cx)
                        }
                        DockLiveUndockPreparedRecoveryRecord::AlreadyCommitted(receipt) => {
                            owner.committed_payload_recovery_receipt(
                                prepared.authority,
                                DockPayloadRecoveryReason::LostViewportRecovery,
                            ) == Some(receipt)
                        }
                    }
            })
            && prepared
                .source_semantic
                .as_ref()
                .is_none_or(|source_semantic| {
                    Self::can_commit_source_semantic_retirement(source_semantic, cx)
                })
    }

    fn commit_committed_destination_recovery(
        &self,
        owner: &Entity<DockSurfaceOwner>,
        prepared: DockLiveUndockPreparedCommittedDestinationRecoveryExecution,
        cx: &mut App,
    ) -> Option<super::payload_recovery::DockPayloadRecoveryCommitReceipt> {
        let transaction_runtime = prepared.runtime.clone();
        transaction_runtime.with_surface_transaction(cx, |transaction, cx| {
            let transaction = transaction?;
            if !self.preflight_committed_destination_recovery(owner, &prepared, cx) {
                return None;
            }
            let receipt = match &prepared.recovery {
                DockLiveUndockPreparedRecoveryRecord::Prepared(recovery) => {
                    cx.update_entity(owner, |owner, owner_cx| {
                        owner
                            .commit_payload_recovery(transaction, recovery, owner_cx)
                            .ok()
                    })?
                }
                DockLiveUndockPreparedRecoveryRecord::AlreadyCommitted(receipt) => *receipt,
            };
            if let Some(source_semantic) = prepared.source_semantic {
                Self::commit_source_semantic_retirement(source_semantic, cx);
            }
            Some(receipt)
        })
    }

    fn attempt_committed_destination_recovery(
        &self,
        owner: &Entity<DockSurfaceOwner>,
        identity: DockLiveUndockIdentity,
        authority: DockPayloadRecoveryAuthority,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
        cx: &mut App,
    ) -> Result<DockPayloadRecoveryCommitReceipt, DockLiveUndockCommittedDestinationRecoveryFailure>
    {
        let prepared = self
            .prepare_committed_destination_recovery(
                owner,
                identity,
                authority,
                token,
                destination,
                cx,
            )
            .ok_or(DockLiveUndockCommittedDestinationRecoveryFailure::PreparationRejected)?;
        self.commit_committed_destination_recovery(owner, prepared, cx)
            .ok_or(DockLiveUndockCommittedDestinationRecoveryFailure::PreflightRejected)
    }

    fn restore_host_release_authority(
        &self,
        identity: DockLiveUndockIdentity,
        authority: DockLiveUndockHostReleaseAuthority,
    ) -> bool {
        let mut state = self.state.borrow_mut();
        let Some(execution) = state.executions.get_mut(&identity) else {
            return false;
        };
        if execution.host_release.is_some() {
            return false;
        }
        execution.host_release = Some(authority);
        true
    }

    fn prepare_host_promotion(
        &self,
        owner: &Entity<DockSurfaceOwner>,
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
        release: DockLiveUndockReleaseLock,
        cx: &mut App,
    ) -> Option<DockLiveUndockPreparedPromotionExecution> {
        let DockLiveUndockPromotionDestination::Host(target) = destination else {
            return None;
        };
        if release.hit() != super::live_undock::DockLiveUndockRouteFeedback::Host(target) {
            return None;
        }

        let (controller, current_revision, identity_is_current) =
            cx.read_entity(owner, |owner, _| {
                (
                    owner.controller(),
                    owner.revision(),
                    owner.accepts_live_undock_identity(identity),
                )
            });
        if !identity_is_current {
            return None;
        }

        let (
            runtime,
            work_context,
            target_window,
            target_host_weak,
            target_binding,
            target_space,
            target_frame,
            presentation_seed,
            surface_revision,
        ) = {
            let state = self.state.borrow();
            let execution = state.executions.get(&identity)?;
            let host_release = execution.host_release.as_ref()?;
            if execution.promotion.is_some()
                || execution.request.key() != identity.opening()
                || !host_release.matches_release(target, &execution.seed.source.session)
                || !host_release.locked_drop.is_workspace()
            {
                return None;
            }
            let presentation_seed = execution.presentation.as_ref().map(|presentation| {
                (
                    presentation.lease,
                    presentation.projection.clone(),
                    execution.seed.source.source_host.clone(),
                    execution.destination_host,
                    execution.seed.source.source_window,
                    presentation.retained,
                    presentation.retained_released,
                )
            });
            (
                execution.seed.source.runtime.clone(),
                execution.seed.source.work_context,
                host_release.target_window,
                host_release.target_host.clone(),
                host_release.target_binding,
                host_release.target_space.clone(),
                host_release.target_frame.clone(),
                presentation_seed,
                execution.surface_revision,
            )
        };
        let target_registration = target_frame.registration_key().clone();
        if surface_revision != current_revision
            || target_window.window_id() != target.window_id()
            || target_frame.generation() != target.host_scene_generation()
            || !runtime.admits_work_context(work_context)
        {
            return None;
        }

        let target_host = target_host_weak.upgrade()?;
        let target_is_exact = cx.read_entity(&target_host, |host, _| {
            host.controller_entity() == controller
                && host.surface_owner_entity().as_ref() == Some(owner)
                && host.viewport_runtime().identity() == runtime.identity()
                && host.space() == &target_space
                && host.current_window_binding() == Some(target_binding)
                && host.current_viewport_registration() == Some(target_registration.clone())
                && host
                    .interaction()
                    .viewport_host_scene_frame()
                    .is_some_and(|frame| frame == &target_frame)
        });
        if !target_is_exact {
            return None;
        }

        let presentation_cleanup_seed = if let Some((
            payload_lease,
            projection,
            source_host,
            provisional_window,
            source_window,
            retained,
            retained_released,
        )) = presentation_seed
        {
            let provisional_window = provisional_window?;
            if payload_lease.identity() != identity
                || payload_lease.destination_window() != provisional_window.window_id()
            {
                return None;
            }
            let presentation_generation = projection.generation();
            let source_window_id = source_window.window_id();
            let provisional_window_id = provisional_window.window_id();
            let source_host = source_host.upgrade().map_or(
                Some(
                    DockLiveUndockPreparedHostPresentationAbandonment::HostUnavailable {
                        window_id: source_window_id,
                    },
                ),
                |host| {
                    Self::prepare_host_presentation_abandonment(
                        host,
                        identity,
                        presentation_generation,
                        source_window_id,
                        cx,
                    )
                },
            )?;
            let provisional_host = match provisional_window.entity(cx) {
                Ok(host) => Self::prepare_host_presentation_abandonment(
                    host,
                    identity,
                    presentation_generation,
                    provisional_window_id,
                    cx,
                )?,
                Err(_) => DockLiveUndockPreparedHostPresentationAbandonment::HostUnavailable {
                    window_id: provisional_window_id,
                },
            };
            let retained_release = if retained_released {
                None
            } else {
                Some(
                    source_window
                        .update(cx, |_, window, _| {
                            retained_visual::prepare_release(window, &retained)
                        })
                        .ok()?
                        .ok()?,
                )
            };
            Some((
                payload_lease,
                projection,
                presentation_generation,
                source_host,
                provisional_host,
                source_window,
                retained_release,
            ))
        } else {
            None
        };

        let authority = self
            .state
            .borrow_mut()
            .executions
            .get_mut(&identity)?
            .host_release
            .take()?;
        let DockLiveUndockHostReleaseAuthority {
            locked_drop,
            target_window: locked_target_window,
            target_host: locked_target_host,
            target_binding: locked_target_binding,
            target_space: locked_target_space,
            target_frame: locked_target_frame,
        } = authority;
        if locked_target_window != target_window
            || locked_target_host != target_host_weak
            || locked_target_binding != target_binding
            || locked_target_space != target_space
            || locked_target_frame != target_frame
        {
            let _ = self.restore_host_release_authority(
                identity,
                DockLiveUndockHostReleaseAuthority {
                    locked_drop,
                    target_window: locked_target_window,
                    target_host: locked_target_host,
                    target_binding: locked_target_binding,
                    target_space: locked_target_space,
                    target_frame: locked_target_frame,
                },
            );
            return None;
        }

        let presentation_cleanup = if let Some((
            payload_lease,
            projection,
            presentation_generation,
            source_host,
            provisional_host,
            source_window,
            retained_release,
        )) = presentation_cleanup_seed
        {
            if !self.presentation_projection_matches(identity, payload_lease, &projection) {
                let _ = self.restore_host_release_authority(
                    identity,
                    DockLiveUndockHostReleaseAuthority {
                        locked_drop,
                        target_window: locked_target_window,
                        target_host: locked_target_host,
                        target_binding: locked_target_binding,
                        target_space: locked_target_space,
                        target_frame: locked_target_frame,
                    },
                );
                return None;
            }
            let Some(presentation_preparation) =
                self.prepare_presentation_cleanup(identity, payload_lease, cx)
            else {
                let _ = self.restore_host_release_authority(
                    identity,
                    DockLiveUndockHostReleaseAuthority {
                        locked_drop,
                        target_window: locked_target_window,
                        target_host: locked_target_host,
                        target_binding: locked_target_binding,
                        target_space: locked_target_space,
                        target_frame: locked_target_frame,
                    },
                );
                return None;
            };
            let presentation = match presentation_preparation {
                Ok(prepared) => prepared,
                Err(_) => {
                    let _ = self.restore_host_release_authority(
                        identity,
                        DockLiveUndockHostReleaseAuthority {
                            locked_drop,
                            target_window: locked_target_window,
                            target_host: locked_target_host,
                            target_binding: locked_target_binding,
                            target_space: locked_target_space,
                            target_frame: locked_target_frame,
                        },
                    );
                    return None;
                }
            };
            Some(DockLiveUndockPreparedHostPromotionPresentationCleanup {
                payload_lease,
                presentation_generation,
                presentation,
                source_host,
                provisional_host,
                source_window,
                retained_release,
            })
        } else {
            None
        };
        let drop = runtime
            .prepare_live_undock_host_drop(locked_drop, target_window.into())
            .expect("validated host release authority must contain a workspace drop");

        Some(DockLiveUndockPreparedPromotionExecution::Host(
            DockLiveUndockPreparedHostPromotionExecution {
                identity,
                token,
                destination,
                release,
                surface_revision,
                controller,
                runtime,
                work_context,
                drop,
                target_window,
                target_host,
                target_binding,
                target_registration,
                target_frame,
                presentation_cleanup,
            },
        ))
    }

    fn restore_host_promotion_presentation_cleanup(
        &self,
        identity: DockLiveUndockIdentity,
        cleanup: Option<DockLiveUndockPreparedHostPromotionPresentationCleanup>,
        cx: &mut App,
    ) -> bool {
        let _ = (identity, cx);
        cleanup
            .is_none_or(|cleanup| self.restore_prepared_presentation_cleanup(cleanup.presentation))
    }

    fn restore_prepared_promotion_session(
        &self,
        prepared: DockLiveUndockPreparedPromotionExecution,
        cx: &mut App,
    ) -> bool {
        match prepared {
            DockLiveUndockPreparedPromotionExecution::SameWindow(prepared) => {
                drop(prepared.presentation);
                true
            }
            DockLiveUndockPreparedPromotionExecution::Host(prepared) => self
                .restore_host_promotion_presentation_cleanup(
                    prepared.identity,
                    prepared.presentation_cleanup,
                    cx,
                ),
        }
    }

    fn prepare_same_window_promotion(
        &self,
        owner: &Entity<DockSurfaceOwner>,
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
        release: DockLiveUndockReleaseLock,
        cx: &mut App,
    ) -> Option<DockLiveUndockPreparedPromotionExecution> {
        let DockLiveUndockPromotionDestination::SameWindowDesktop { window_id } = destination
        else {
            return None;
        };
        if !matches!(
            release.hit(),
            super::live_undock::DockLiveUndockRouteFeedback::Desktop
                | super::live_undock::DockLiveUndockRouteFeedback::OpaqueBarrier
        ) {
            return None;
        }

        let (controller, current_revision, identity_is_current) =
            cx.read_entity(owner, |owner, _| {
                (
                    owner.controller(),
                    owner.revision(),
                    owner.accepts_live_undock_identity(identity),
                )
            });
        if !identity_is_current {
            return None;
        }

        let (
            runtime,
            work_context,
            destination_window_facts,
            source_space,
            target_space,
            move_plan,
            opening,
            provisional_session,
            destination_window,
            source_window,
            source_host,
            source_key,
            destination_key,
            presentation,
            payload_lease,
            retained,
            reveal,
            surface_revision,
        ) = {
            let state = self.state.borrow();
            let execution = state.executions.get(&identity)?;
            let presentation = execution.presentation.as_ref()?;
            let observed_placement = execution.observed_release_placement?;
            if execution.promotion.is_some()
                || execution.destination_host?.window_id() != window_id
                || execution.request.key() != identity.opening()
                || presentation.lease.identity() != identity
                || observed_placement.window_id != window_id
                || observed_placement.generation != release.placement_generation()
            {
                return None;
            }
            (
                execution.seed.source.runtime.clone(),
                execution.seed.source.work_context,
                observed_placement.facts,
                execution.seed.source.payload.source_space.clone(),
                execution.seed.target_space.clone(),
                execution.seed.move_plan.clone(),
                execution.request.key(),
                execution.request.provisional_session().clone(),
                execution.destination_host?,
                execution.seed.source.source_window,
                execution.seed.source.source_host.clone(),
                presentation.source_key?,
                presentation.destination_key?,
                presentation.projection.clone(),
                presentation.lease,
                presentation.retained,
                presentation.reveal?,
                execution.surface_revision,
            )
        };
        if surface_revision != current_revision
            || payload_lease.surface_revision() != current_revision
            || payload_lease.destination_window() != window_id
            || reveal.reveal_frame().window_id() != window_id
            || reveal.preflight().mount().proxy().lease() != payload_lease
            || !runtime.admits_work_context(work_context)
        {
            return None;
        }

        let (next_graph, changed) = cx
            .read_entity(&controller, |controller, _| {
                move_plan.project_graph(controller.workspace())
            })
            .ok()?;
        if !changed || next_graph.validate().is_err() {
            return None;
        }

        let source_host = source_host.upgrade()?;
        let destination_host = destination_window.entity(cx).ok()?;
        let source_host_matches = cx.read_entity(&source_host, |host, _| {
            host.controller_entity() == controller
                && host.surface_owner_entity().as_ref() == Some(owner)
                && host.viewport_runtime().identity() == runtime.identity()
                && host.space() == &source_space
        });
        let destination_host_matches = cx.read_entity(&destination_host, |host, _| {
            host.controller_entity() == controller
                && host.surface_owner_entity().as_ref() == Some(owner)
                && host.viewport_runtime().identity() == runtime.identity()
                && host.space() == &target_space
        });
        if !source_host_matches || !destination_host_matches {
            return None;
        }

        let viewport = runtime.prepare_live_undock_provisional_promotion(
            &target_space,
            destination_window.into(),
            opening,
            work_context,
            destination_window_facts,
        )?;
        let source = cx.read_entity(&source_host, |host, _| {
            host.prepare_live_source_retirement(source_key)
        })?;
        let committed_surface_revision = current_revision.checked_add(1)?;
        let destination_host_promotion = cx.read_entity(&destination_host, |host, _| {
            host.prepare_live_destination_promotion(
                destination_key,
                opening,
                token,
                committed_surface_revision,
                &target_space,
                viewport.registration().clone(),
                destination_window_facts,
            )
        })?;
        let viewport =
            viewport.with_host_geometry(destination_host_promotion.host_geometry().clone());
        let retained_release = source_window
            .update(cx, |_, window, _| {
                retained_visual::prepare_release(window, &retained)
            })
            .ok()?
            .ok()?;

        // Begin destination semantics before preparing the provider terminal. The unique session
        // remains in the runtime so any rejected aggregate preparation stays compensatable.
        let semantics = destination_window
            .update(cx, |_, window, cx| {
                window.begin_provisional_destination_semantics(
                    &provisional_session,
                    token.get(),
                    cx,
                )
            })
            .ok()?
            .ok()?;

        let destination_presentation = reveal.preflight().rehost_presentation()?;
        if !self.presentation_projection_matches(identity, payload_lease, &presentation) {
            return None;
        }
        let presentation = match self.prepare_presentation_terminal(
            identity,
            payload_lease,
            view_presentation_window::RehostTerminalIntent::CommitDestination(
                &destination_presentation,
            ),
            cx,
        )? {
            Ok(prepared) => prepared,
            Err(_) => return None,
        };

        Some(DockLiveUndockPreparedPromotionExecution::SameWindow(
            DockLiveUndockPreparedSameWindowPromotionExecution {
                identity,
                token,
                destination,
                release,
                surface_revision,
                controller,
                move_plan,
                runtime,
                viewport,
                source_window,
                retained_release,
                source_host,
                source,
                destination_host,
                destination_host_promotion,
                presentation,
                reveal,
                provisional_session,
                semantics,
            },
        ))
    }

    fn preflight_host_promotion_commit(
        &self,
        owner: &Entity<DockSurfaceOwner>,
        prepared: DockLiveUndockPreparedHostPromotionExecution,
        cx: &mut App,
    ) -> Option<DockLiveUndockPreflightedHostPromotionExecution> {
        let DockLiveUndockPreparedHostPromotionExecution {
            identity,
            token,
            destination,
            release,
            surface_revision,
            controller,
            runtime,
            work_context,
            drop,
            target_window,
            target_host,
            target_binding,
            target_registration,
            target_frame,
            presentation_cleanup,
        } = prepared;
        let DockLiveUndockPromotionDestination::Host(target) = destination else {
            self.restore_host_promotion_presentation_cleanup(identity, presentation_cleanup, cx);
            return None;
        };
        if release.hit() != super::live_undock::DockLiveUndockRouteFeedback::Host(target) {
            self.restore_host_promotion_presentation_cleanup(identity, presentation_cleanup, cx);
            return None;
        }

        let owner_is_exact = cx.read_entity(owner, |current, _| {
            current.controller() == controller
                && current.revision() == surface_revision
                && current.accepts_live_undock_identity(identity)
                && current.window_session().admits(identity.opening().lease())
        });
        let execution_is_exact =
            self.state
                .borrow()
                .executions
                .get(&identity)
                .is_some_and(|execution| {
                    execution.request.key() == identity.opening()
                        && execution.surface_revision == surface_revision
                        && execution.host_release.is_none()
                        && match (
                            execution.presentation.as_ref(),
                            presentation_cleanup.as_ref(),
                        ) {
                            (None, None) => true,
                            (Some(presentation), Some(cleanup)) => {
                                presentation.lease == cleanup.payload_lease
                                    && presentation.projection.generation()
                                        == cleanup.presentation_generation
                                    && presentation.session.is_active()
                            }
                            (None, Some(_)) | (Some(_), None) => false,
                        }
                });
        let target_is_exact = cx.read_entity(&target_host, |host, _| {
            host.controller_entity() == controller
                && host.surface_owner_entity().as_ref() == Some(owner)
                && host.viewport_runtime().identity() == runtime.identity()
                && host.current_window_binding() == Some(target_binding)
                && host.current_viewport_registration() == Some(target_registration.clone())
                && host
                    .interaction()
                    .viewport_host_scene_frame()
                    .is_some_and(|frame| frame == &target_frame)
        });
        let presentation_is_exact = presentation_cleanup.as_ref().is_none_or(|cleanup| {
            Self::can_commit_presentation_cleanup(&cleanup.presentation, cx)
                && Self::can_commit_host_presentation_abandonment(&cleanup.source_host, cx)
                && Self::can_commit_host_presentation_abandonment(&cleanup.provisional_host, cx)
                && cleanup
                    .retained_release
                    .as_ref()
                    .is_none_or(|retained_release| {
                        cleanup
                            .source_window
                            .update(cx, |_, window, _| {
                                retained_visual::can_commit_prepared_release(
                                    window,
                                    retained_release,
                                )
                            })
                            .unwrap_or(false)
                    })
        });
        if !(owner_is_exact
            && execution_is_exact
            && target_is_exact
            && presentation_is_exact
            && runtime.admits_work_context(work_context)
            && target_window.window_id() == target.window_id()
            && target_frame.generation() == target.host_scene_generation())
        {
            self.restore_host_promotion_presentation_cleanup(identity, presentation_cleanup, cx);
            return None;
        }
        let drop = match runtime.preflight_live_undock_host_drop_commit(drop, cx) {
            Ok(drop) => drop,
            Err(_) => {
                self.restore_host_promotion_presentation_cleanup(
                    identity,
                    presentation_cleanup,
                    cx,
                );
                return None;
            }
        };
        Some(DockLiveUndockPreflightedHostPromotionExecution {
            identity,
            token,
            destination,
            release,
            surface_revision,
            runtime,
            drop,
            target_window,
            target_host,
            target_binding,
            target_registration,
            presentation_cleanup,
        })
    }

    fn commit_host_promotion(
        &self,
        owner: &Entity<DockSurfaceOwner>,
        prepared: DockLiveUndockPreflightedHostPromotionExecution,
        cx: &mut App,
    ) -> DockLiveUndockDurablePromotionExecution {
        let DockLiveUndockPreflightedHostPromotionExecution {
            identity,
            token,
            destination,
            release,
            surface_revision,
            runtime,
            drop,
            target_window,
            target_host,
            target_binding,
            target_registration,
            presentation_cleanup,
        } = prepared;
        let DockLiveUndockPromotionDestination::Host(target) = destination else {
            unreachable!("host promotion commit received a same-window destination");
        };
        assert_eq!(
            release.hit(),
            super::live_undock::DockLiveUndockRouteFeedback::Host(target)
        );

        let released_retained = presentation_cleanup
            .as_ref()
            .is_some_and(|cleanup| cleanup.retained_release.is_some());
        let cleanup_payload_lease = presentation_cleanup
            .as_ref()
            .map(|cleanup| cleanup.payload_lease);
        let cleanup_runtime = self.clone();
        let committed = runtime.commit_preflighted_live_undock_host_drop(
            drop,
            move |cx| {
                let Some(cleanup) = presentation_cleanup else {
                    return;
                };
                let abandonment = Self::commit_presentation_cleanup(cleanup.presentation, cx);
                assert_eq!(
                    abandonment.authority().0,
                    cleanup.presentation_generation,
                    "host promotion must abandon the exact provisional presentation"
                );
                let _ = Self::commit_host_presentation_abandonment(cleanup.source_host, cx);
                let _ = Self::commit_host_presentation_abandonment(cleanup.provisional_host, cx);
                if let Some(retained_release) = cleanup.retained_release {
                    cleanup
                        .source_window
                        .update(cx, |_, window, _| {
                            retained_visual::commit_prepared_release(window, retained_release);
                        })
                        .expect("preflighted host promotion must retain its exact source window");
                }
                assert!(
                    cleanup_runtime.retire_presentation_session_after_terminal_commit(
                        identity,
                        cleanup.payload_lease,
                    ),
                    "host promotion must retire its exact committed rehost session"
                );
            },
            cx,
        );
        let crate::DockViewportDropRouteOutcome::Action(action) = committed.outcome() else {
            unreachable!("an existing-host promotion must commit a workspace drop");
        };
        assert!(
            action.action().changed(),
            "an existing-host promotion must change the dock graph"
        );
        if released_retained && let Some(payload_lease) = cleanup_payload_lease {
            let marked = self
                .state
                .borrow_mut()
                .executions
                .get_mut(&identity)
                .and_then(|execution| execution.presentation.as_mut())
                .filter(|presentation| presentation.lease == payload_lease)
                .is_some_and(|presentation| {
                    presentation.retained_released = true;
                    true
                });
            assert!(
                marked,
                "host promotion must checkpoint its retained-visual release"
            );
        }
        let target_is_exact = cx.read_entity(&target_host, |host, _| {
            host.current_window_binding() == Some(target_binding)
                && host.current_viewport_registration() == Some(target_registration.clone())
        });
        assert!(
            target_is_exact,
            "committed host promotion must retain its exact target registration"
        );
        let committed_revision = cx.read_entity(owner, |owner, _| owner.revision());
        assert_eq!(
            committed_revision,
            surface_revision
                .checked_add(1)
                .expect("live-undock surface revision space exhausted"),
            "one host promotion must publish exactly one surface revision"
        );
        let (outcome, pending_window_effects) = committed.into_parts();
        let activation = outcome.activation_transaction();
        DockLiveUndockDurablePromotionExecution::Host(DockLiveUndockDurableHostPromotionExecution {
            identity,
            token,
            destination,
            destination_window: target_window,
            destination_host: target_host.downgrade(),
            destination_binding: target_binding,
            registration: target_registration,
            activation,
            pending_window_effects: Some(pending_window_effects),
        })
    }

    fn preflight_same_window_promotion_commit(
        &self,
        owner: &Entity<DockSurfaceOwner>,
        prepared: &DockLiveUndockPreparedSameWindowPromotionExecution,
        cx: &mut App,
    ) -> Option<DockGraph> {
        let DockLiveUndockPromotionDestination::SameWindowDesktop { window_id } =
            prepared.destination
        else {
            return None;
        };
        if !matches!(
            prepared.release.hit(),
            super::live_undock::DockLiveUndockRouteFeedback::Desktop
                | super::live_undock::DockLiveUndockRouteFeedback::OpaqueBarrier
        ) {
            return None;
        }

        let owner_is_exact = cx.read_entity(owner, |current, _| {
            current.controller() == prepared.controller
                && current.revision() == prepared.surface_revision
                && current.accepts_live_undock_identity(prepared.identity)
                && current
                    .window_session()
                    .admits(prepared.identity.opening().lease())
        });
        let payload_lease = prepared.reveal.preflight().mount().proxy().lease();
        let execution_is_exact = self
            .state
            .borrow()
            .executions
            .get(&prepared.identity)
            .is_some_and(|execution| {
                execution.request.key() == prepared.identity.opening()
                    && execution.surface_revision == prepared.surface_revision
                    && execution.presentation.as_ref().is_some_and(|presentation| {
                        presentation.lease == payload_lease
                            && presentation.projection.generation()
                                == payload_lease.rehost_generation()
                            && presentation.session.is_active()
                            && presentation.reveal == Some(prepared.reveal)
                    })
            });
        if !owner_is_exact
            || !execution_is_exact
            || !prepared
                .runtime
                .can_commit_live_undock_provisional_promotion(&prepared.viewport)
            || !cx.read_entity(&prepared.source_host, |host, _| {
                host.can_commit_prepared_live_source_retirement(&prepared.source)
            })
            || !cx.read_entity(&prepared.destination_host, |host, _| {
                host.can_commit_prepared_live_destination_promotion(
                    &prepared.destination_host_promotion,
                )
            })
            || !prepared.presentation.can_commit(cx)
            || !prepared
                .source_window
                .update(cx, |_, window, _| {
                    retained_visual::can_commit_prepared_release(window, &prepared.retained_release)
                })
                .unwrap_or(false)
        {
            return None;
        }

        let session = prepared.provisional_session.snapshot();
        let semantics = prepared.semantics.snapshot();
        if session.window_id() != Some(window_id)
            || session.phase() != WindowProvisionalSessionPhase::ProjectingDestinationSemantics
            || semantics.window_id() != window_id
            || semantics.session_generation() != session.generation()
            || semantics.destination_generation() != prepared.token.get()
            || semantics.outcome() != WindowProvisionalSemanticsOutcome::Pending
        {
            return None;
        }

        let (next_graph, changed) = cx
            .read_entity(&prepared.controller, |controller, _| {
                prepared.move_plan.project_graph(controller.workspace())
            })
            .ok()?;
        (changed && next_graph.validate().is_ok()).then_some(next_graph)
    }

    fn commit_same_window_promotion(
        &self,
        owner: &Entity<DockSurfaceOwner>,
        prepared: DockLiveUndockPreparedSameWindowPromotionExecution,
        next_graph: DockGraph,
        cx: &mut App,
    ) -> DockLiveUndockDurablePromotionExecution {
        let DockLiveUndockPreparedSameWindowPromotionExecution {
            identity,
            token,
            destination,
            release,
            surface_revision,
            controller,
            move_plan: _,
            runtime,
            viewport,
            source_window,
            retained_release,
            source_host,
            source,
            destination_host,
            destination_host_promotion,
            presentation,
            reveal,
            provisional_session,
            semantics,
        } = prepared;
        let DockLiveUndockPromotionDestination::SameWindowDesktop { window_id } = destination
        else {
            unreachable!("same-window promotion commit received a host destination");
        };
        assert!(matches!(
            release.hit(),
            super::live_undock::DockLiveUndockRouteFeedback::Desktop
                | super::live_undock::DockLiveUndockRouteFeedback::OpaqueBarrier
        ));
        let payload_lease = reveal.preflight().mount().proxy().lease();

        let destination_window = self
            .state
            .borrow()
            .executions
            .get(&identity)
            .and_then(|execution| execution.destination_host)
            .filter(|window| window.window_id() == window_id)
            .expect("prepared same-window promotion must retain its destination window");
        let view_presentation_window::RehostTerminalOutcome::DestinationCommitted(batch) =
            presentation
                .try_commit(cx)
                .expect("preflighted destination terminal must remain exact")
        else {
            unreachable!("destination terminal preparation abandoned its rehost")
        };
        assert_eq!(batch.window_id(), window_id);

        let commit_runtime = runtime.clone();
        let (registration, destination_binding) = runtime.with_surface_transaction(cx, |_, cx| {
            cx.update_entity(&controller, |controller, controller_cx| {
                controller.workspace_mut().set_graph(next_graph);
                controller_cx.notify();
            });
            let committed_viewport =
                commit_runtime.commit_live_undock_provisional_promotion(viewport);
            cx.update_entity(&source_host, |host, host_cx| {
                host.commit_prepared_live_source_retirement(source, host_cx);
            });
            cx.update_entity(&destination_host, |host, host_cx| {
                host.commit_prepared_live_destination_promotion(
                    destination_host_promotion,
                    host_cx,
                );
            });
            let registration =
                commit_runtime.publish_live_undock_promotion_commit(committed_viewport, true, cx);
            let binding = cx.read_entity(&destination_host, |host, _| {
                assert_eq!(
                    host.current_viewport_registration().as_ref(),
                    Some(&registration)
                );
                host.current_window_binding()
                    .expect("promoted destination host must retain its exact window binding")
            });
            source_window
                .update(cx, |_, window, _| {
                    retained_visual::commit_prepared_release(window, retained_release);
                })
                .expect("prepared retained visual must keep its exact source window");
            let retained_marked = self
                .state
                .borrow_mut()
                .executions
                .get_mut(&identity)
                .and_then(|execution| execution.presentation.as_mut())
                .is_some_and(|presentation| {
                    presentation.retained_released = true;
                    true
                });
            assert!(
                retained_marked,
                "durable promotion must checkpoint its retained-visual release"
            );
            assert!(
                self.retire_presentation_session_after_terminal_commit(identity, payload_lease),
                "same-window promotion must retire its exact committed rehost session"
            );
            (registration, binding)
        });

        let committed_revision = cx.read_entity(owner, |owner, _| owner.revision());
        assert_eq!(
            committed_revision,
            surface_revision
                .checked_add(1)
                .expect("live-undock surface revision space exhausted"),
            "one prepared live-undock promotion must publish exactly one surface revision"
        );
        DockLiveUndockDurablePromotionExecution::SameWindow(
            DockLiveUndockDurableSameWindowPromotionExecution {
                identity,
                token,
                destination,
                surface_revision: committed_revision,
                destination_window,
                destination_binding,
                registration,
                reveal,
                provisional_session,
                semantics,
            },
        )
    }

    fn execute_effect(
        &self,
        owner: &open_gpui::Entity<DockSurfaceOwner>,
        effect: DockLiveUndockEffect,
        cx: &mut App,
    ) {
        match effect {
            DockLiveUndockEffect::RetireSourceTransportProxy { identity } => {
                self.retire_source_transport_proxy(identity, cx);
            }
            DockLiveUndockEffect::OpenProvisional { identity, request } => {
                let open = {
                    let state = self.state.borrow();
                    let execution = state
                        .executions
                        .get(&identity)
                        .expect("an opening effect must have one exact execution");
                    debug_assert_eq!(execution.request.identity(), request.identity());
                    (
                        execution.seed.source.runtime.clone(),
                        execution.seed.target_space.clone(),
                        execution.seed.source.suggested_window_bounds.clone(),
                    )
                };
                let options = WindowOptions {
                    window_bounds: open.2,
                    focus_on_appearing: false,
                    ..WindowOptions::default()
                };
                match open
                    .0
                    .open_triggered_live_undock_provisional_viewport(open.1, options, &request, cx)
                {
                    Ok(destination_host) => {
                        if let Some(execution) =
                            self.state.borrow_mut().executions.get_mut(&identity)
                        {
                            execution.destination_host = Some(destination_host);
                        }
                    }
                    Err(error) => {
                        log::debug!("live-undock provisional opening failed closed: {error:#}");
                    }
                }
            }
            DockLiveUndockEffect::OpeningFailed {
                identity,
                dependency,
            } => super::settle_live_undock_dependency(owner, identity, dependency, cx),
            DockLiveUndockEffect::ProvisionalRetirementRequired {
                identity,
                window,
                dependency,
                binding,
                runtime,
                ..
            } => super::retire_live_undock_provisional(
                owner, identity, window, dependency, binding, runtime, cx,
            ),
            DockLiveUndockEffect::ProvisionalAdmitted {
                identity, window, ..
            } => match self.prepare_presentation_handoff(identity, window, cx) {
                Ok(presentation) => {
                    let receipt = presentation.lease;
                    let mut presentation = Some(presentation);
                    let source_window = self
                        .state
                        .borrow()
                        .executions
                        .get(&identity)
                        .map(|execution| execution.seed.source.source_window);
                    let installed = self
                        .state
                        .borrow_mut()
                        .executions
                        .get_mut(&identity)
                        .is_some_and(|execution| {
                            if execution.presentation.is_some() {
                                return false;
                            }
                            execution.presentation = presentation.take();
                            true
                        });
                    if installed {
                        let _ = self.submit(
                            DockLiveUndockFact::PresentationLeaseActivated { identity, receipt },
                            cx,
                        );
                    } else {
                        let mut presentation = presentation
                            .expect("rejected presentation installation must retain its session");
                        if let Some(session) = presentation.checkout_session() {
                            if let Some(source_window) = source_window {
                                Self::cancel_prepared_presentation(
                                    source_window,
                                    presentation.retained,
                                    session,
                                    cx,
                                );
                            } else {
                                let mut session = session;
                                let _ = session.abandon_after_source_loss(cx);
                            }
                        }
                        self.submit_presentation_failure(
                            identity,
                            DockLiveUndockPresentationFailure::RehostPreparation,
                            cx,
                        );
                    }
                }
                Err(failure) => self.submit_presentation_failure(identity, failure, cx),
            },
            DockLiveUndockEffect::CommitSourceProxy { identity, lease } => {
                let source = self
                    .state
                    .borrow()
                    .executions
                    .get(&identity)
                    .and_then(|execution| {
                        let presentation = execution.presentation.as_ref()?;
                        (presentation.lease == lease).then(|| {
                            (
                                execution.seed.source.source_host.clone(),
                                execution.seed.source.source_binding,
                                execution.seed.source_session.clone(),
                                SharedString::from(
                                    execution.seed.source.payload.title().to_owned(),
                                ),
                                execution.seed.source.source_focus.clone(),
                                presentation.snapshot(),
                            )
                        })
                    });
                let Some((
                    source_host,
                    source_binding,
                    source_session,
                    accessible_name,
                    source_focus,
                    presentation,
                )) = source
                else {
                    self.submit_presentation_failure(
                        identity,
                        DockLiveUndockPresentationFailure::SourceProxyReplay { lease },
                        cx,
                    );
                    return;
                };
                let installed = source_host
                    .update(cx, |host, cx| {
                        host.install_live_source_projection(
                            source_binding,
                            identity,
                            lease,
                            source_session,
                            presentation.projection.clone(),
                            presentation.retained,
                            presentation.carrier.clone(),
                            accessible_name,
                            source_focus,
                            cx,
                        )
                    })
                    .ok()
                    .flatten();
                let Some(key) = installed else {
                    self.submit_presentation_failure(
                        identity,
                        DockLiveUndockPresentationFailure::SourceProxyReplay { lease },
                        cx,
                    );
                    return;
                };
                if let Some(presentation) = self
                    .state
                    .borrow_mut()
                    .executions
                    .get_mut(&identity)
                    .and_then(|execution| execution.presentation.as_mut())
                    .filter(|presentation| presentation.lease == lease)
                {
                    presentation.source_key = Some(key);
                }
            }
            DockLiveUndockEffect::MountAndExposePayload {
                identity,
                proxy,
                window,
            } => {
                let destination =
                    self.state
                        .borrow()
                        .executions
                        .get(&identity)
                        .and_then(|execution| {
                            let presentation = execution.presentation.as_ref()?;
                            let destination_host = execution.destination_host?;
                            (presentation.lease == proxy.lease()
                                && destination_host.window_id() == window.window_id())
                            .then(|| {
                                (
                                    destination_host,
                                    execution.seed.payload_session.clone(),
                                    presentation.snapshot(),
                                )
                            })
                        });
                let Some((destination_host, payload_session, presentation)) = destination else {
                    self.submit_presentation_failure(
                        identity,
                        DockLiveUndockPresentationFailure::DestinationExposureFinish { proxy },
                        cx,
                    );
                    return;
                };
                let installed = destination_host
                    .update(cx, |host, window, cx| {
                        host.ensure_window_binding(window, cx);
                        let binding = host.current_window_binding()?;
                        host.install_live_destination_projection(
                            binding,
                            identity,
                            proxy,
                            payload_session,
                            presentation.projection.clone(),
                            presentation.projection.destination().clone(),
                            cx,
                        )
                    })
                    .ok()
                    .flatten();
                let Some(key) = installed else {
                    self.submit_presentation_failure(
                        identity,
                        DockLiveUndockPresentationFailure::DestinationExposureFinish { proxy },
                        cx,
                    );
                    return;
                };
                if let Some(presentation) = self
                    .state
                    .borrow_mut()
                    .executions
                    .get_mut(&identity)
                    .and_then(|execution| execution.presentation.as_mut())
                    .filter(|presentation| presentation.lease == proxy.lease())
                {
                    presentation.destination_key = Some(key);
                }
            }
            DockLiveUndockEffect::ObservePayloadPresentation {
                identity,
                mount,
                window,
            } => {
                let destination =
                    self.state
                        .borrow()
                        .executions
                        .get(&identity)
                        .and_then(|execution| {
                            let presentation = execution.presentation.as_ref()?;
                            let key = presentation.destination_key?;
                            let host = execution.destination_host?;
                            (presentation.lease == mount.proxy().lease()
                                && host.window_id() == window.window_id())
                            .then_some((host, key))
                        });
                let current = destination.is_some_and(|(host, key)| {
                    host.update(cx, |host, window, _| {
                        let current = host.accepts_live_presentation_key(key);
                        if current {
                            window.refresh();
                        }
                        current
                    })
                    .unwrap_or(false)
                });
                if !current {
                    self.submit_presentation_failure(
                        identity,
                        DockLiveUndockPresentationFailure::PayloadPresentationObservation { mount },
                        cx,
                    );
                }
            }
            DockLiveUndockEffect::ArmExactReveal {
                identity,
                presentation,
                window,
            } => {
                let destination =
                    self.state
                        .borrow()
                        .executions
                        .get(&identity)
                        .and_then(|execution| {
                            let authority = execution.presentation.as_ref()?;
                            let key = authority.destination_key?;
                            let host = execution.destination_host?;
                            (authority.lease == presentation.mount().proxy().lease()
                                && host.window_id() == window.window_id())
                            .then(|| (host, key, execution.request.provisional_session().clone()))
                        });
                let reveal_runtime = self.clone();
                let reveal_outcome = destination
                    .map(|(host, key, provisional_session)| {
                        host.update(cx, |host, destination_window, cx| {
                        if !host.can_arm_live_destination_reveal(key, presentation) {
                            return DockLiveUndockRevealArmOutcome::Rejected;
                        }
                        match destination_window
                            .presentation_facts()
                            .initial_presentation
                        {
                            WindowInitialPresentationStatus::Pending => {
                                let reveal_runtime = reveal_runtime.clone();
                                destination_window
                                    .observe_window_initial_presentation(move |window, cx| {
                                        match window.presentation_facts().initial_presentation {
                                            WindowInitialPresentationStatus::Completed => {
                                                reveal_runtime.enqueue_effects(
                                                    DockLiveUndockEffects::single(
                                                        DockLiveUndockEffect::ArmExactReveal {
                                                            identity,
                                                            presentation,
                                                            window: window.window_handle(),
                                                        },
                                                    ),
                                                    cx,
                                                );
                                            }
                                            WindowInitialPresentationStatus::Rejected => {
                                                reveal_runtime.submit_presentation_failure(
                                                    identity,
                                                    DockLiveUndockPresentationFailure::ExactRevealTicket {
                                                        presentation,
                                                    },
                                                    cx,
                                                );
                                            }
                                            WindowInitialPresentationStatus::Pending => {}
                                        }
                                    })
                                    .detach();
                                return DockLiveUndockRevealArmOutcome::WaitingForInitialPresentation;
                            }
                            WindowInitialPresentationStatus::Rejected => {
                                return DockLiveUndockRevealArmOutcome::Rejected;
                            }
                            WindowInitialPresentationStatus::Completed => {}
                        }
                        let ticket = match destination_window
                            .arm_provisional_presentation(&provisional_session, cx)
                        {
                            Ok(ticket) => ticket,
                            Err(_) => return DockLiveUndockRevealArmOutcome::Rejected,
                        };
                        if host.arm_live_destination_reveal(key, presentation, ticket, cx) {
                            DockLiveUndockRevealArmOutcome::Armed
                        } else {
                            DockLiveUndockRevealArmOutcome::Rejected
                        }
                    })
                    .unwrap_or(DockLiveUndockRevealArmOutcome::Rejected)
                    })
                    .unwrap_or(DockLiveUndockRevealArmOutcome::Rejected);
                if reveal_outcome == DockLiveUndockRevealArmOutcome::Rejected {
                    self.submit_presentation_failure(
                        identity,
                        DockLiveUndockPresentationFailure::ExactRevealTicket { presentation },
                        cx,
                    );
                }
            }
            DockLiveUndockEffect::RetireFrozenSourceVisual { identity, reveal } => {
                let source = self
                    .state
                    .borrow()
                    .executions
                    .get(&identity)
                    .and_then(|execution| {
                        let presentation = execution.presentation.as_ref()?;
                        let key = presentation.source_key?;
                        (presentation.lease == reveal.preflight().mount().proxy().lease()).then(
                            || {
                                (
                                    execution.seed.source.source_window,
                                    execution.seed.source.source_host.clone(),
                                    key,
                                    presentation.retained,
                                )
                            },
                        )
                    });
                let Some((_source_window, source_host, key, _retained)) = source else {
                    return;
                };
                let retired = source_host
                    .update(cx, |host, cx| host.retire_live_source_visual(key, cx))
                    .unwrap_or(false);
                if !retired {
                    return;
                }
                if let Some(presentation) = self
                    .state
                    .borrow_mut()
                    .executions
                    .get_mut(&identity)
                    .and_then(|execution| execution.presentation.as_mut())
                    .filter(|presentation| {
                        presentation.lease == reveal.preflight().mount().proxy().lease()
                            && presentation.source_key == Some(key)
                    })
                {
                    presentation.reveal = Some(reveal);
                }
            }
            DockLiveUndockEffect::RequestReleasePlacement {
                identity,
                window,
                release,
            } => self.request_release_placement(identity, window, release, cx),
            DockLiveUndockEffect::PreparePromotion {
                identity,
                token,
                destination,
                release,
            } => {
                let prepared = match destination {
                    DockLiveUndockPromotionDestination::SameWindowDesktop { .. } => self
                        .prepare_same_window_promotion(
                            owner,
                            identity,
                            token,
                            destination,
                            release,
                            cx,
                        ),
                    DockLiveUndockPromotionDestination::Host(_) => self.prepare_host_promotion(
                        owner,
                        identity,
                        token,
                        destination,
                        release,
                        cx,
                    ),
                };
                let fact = if let Some(prepared) = prepared {
                    let mut state = self.state.borrow_mut();
                    let execution = state
                        .executions
                        .get_mut(&identity)
                        .expect("prepared live-undock promotion must retain its execution");
                    execution.release_deadline.clear();
                    assert!(execution.promotion.is_none());
                    execution.promotion =
                        Some(DockLiveUndockPromotionExecution::Prepared(prepared));
                    DockLiveUndockFact::PromotionPrepared { identity, token }
                } else {
                    if let Some(execution) = self.state.borrow_mut().executions.get_mut(&identity) {
                        execution.release_deadline.clear();
                    }
                    DockLiveUndockFact::PromotionPreparationFailed { identity, token }
                };
                self.enqueue_fact(DockLiveUndockQueuedFact::Reduce(fact), cx);
            }
            DockLiveUndockEffect::CommitPreparedPromotion {
                identity,
                token,
                destination,
            } => {
                let commit_plan = {
                    let state = self.state.borrow();
                    let Some(execution) = state.executions.get(&identity) else {
                        return;
                    };
                    let Some(DockLiveUndockPromotionExecution::Prepared(prepared)) =
                        execution.promotion.as_ref()
                    else {
                        return;
                    };
                    if prepared.identity() != identity
                        || prepared.token() != token
                        || prepared.destination() != destination
                        || execution.surface_revision != prepared.surface_revision()
                        || execution.request.key() != identity.opening()
                    {
                        return;
                    }
                    match prepared {
                        DockLiveUndockPreparedPromotionExecution::SameWindow(prepared) => {
                            if execution
                                .destination_host
                                .is_none_or(|window| window.window_id() != destination.window_id())
                                || execution.presentation.as_ref().is_none_or(|presentation| {
                                    presentation.lease.identity() != identity
                                        || presentation.reveal != Some(prepared.reveal)
                                })
                            {
                                None
                            } else {
                                self.preflight_same_window_promotion_commit(owner, prepared, cx)
                                    .map(Some)
                            }
                        }
                        DockLiveUndockPreparedPromotionExecution::Host(_) => Some(None),
                    }
                };
                let Some(next_graph) = commit_plan else {
                    let prepared = {
                        let mut state = self.state.borrow_mut();
                        state
                            .executions
                            .get_mut(&identity)
                            .and_then(|execution| {
                                let is_exact = matches!(
                                    execution.promotion.as_ref(),
                                    Some(DockLiveUndockPromotionExecution::Prepared(prepared))
                                        if prepared.identity() == identity
                                            && prepared.token() == token
                                            && prepared.destination() == destination
                                );
                                is_exact.then(|| execution.promotion.take()).flatten()
                            })
                            .and_then(|promotion| match promotion {
                                DockLiveUndockPromotionExecution::Prepared(prepared) => {
                                    Some(prepared)
                                }
                                DockLiveUndockPromotionExecution::Durable(_) => None,
                            })
                    };
                    if let Some(prepared) = prepared {
                        self.restore_prepared_promotion_session(prepared, cx);
                    }
                    self.enqueue_fact(
                        DockLiveUndockQueuedFact::Reduce(
                            DockLiveUndockFact::PromotionPreparationFailed { identity, token },
                        ),
                        cx,
                    );
                    return;
                };
                let prepared = {
                    let mut state = self.state.borrow_mut();
                    let Some(execution) = state.executions.get_mut(&identity) else {
                        return;
                    };
                    let matches = matches!(
                        execution.promotion.as_ref(),
                        Some(DockLiveUndockPromotionExecution::Prepared(prepared))
                            if prepared.identity() == identity
                                && prepared.token() == token
                                && prepared.destination() == destination
                    );
                    if !matches {
                        return;
                    }
                    let Some(DockLiveUndockPromotionExecution::Prepared(prepared)) =
                        execution.promotion.take()
                    else {
                        unreachable!("validated prepared promotion changed before consumption");
                    };
                    prepared
                };
                let durable = match (prepared, next_graph) {
                    (
                        DockLiveUndockPreparedPromotionExecution::SameWindow(prepared),
                        Some(graph),
                    ) => Some(self.commit_same_window_promotion(owner, prepared, graph, cx)),
                    (DockLiveUndockPreparedPromotionExecution::Host(prepared), None) => self
                        .preflight_host_promotion_commit(owner, prepared, cx)
                        .map(|prepared| self.commit_host_promotion(owner, prepared, cx)),
                    (prepared @ DockLiveUndockPreparedPromotionExecution::SameWindow(_), None)
                    | (prepared @ DockLiveUndockPreparedPromotionExecution::Host(_), Some(_)) => {
                        self.restore_prepared_promotion_session(prepared, cx);
                        None
                    }
                };
                let Some(durable) = durable else {
                    self.enqueue_fact(
                        DockLiveUndockQueuedFact::Reduce(
                            DockLiveUndockFact::PromotionPreparationFailed { identity, token },
                        ),
                        cx,
                    );
                    return;
                };
                let mut state = self.state.borrow_mut();
                let execution = state
                    .executions
                    .get_mut(&identity)
                    .expect("durable live-undock promotion must retain its execution");
                execution.promotion = Some(DockLiveUndockPromotionExecution::Durable(durable));
                drop(state);
                self.enqueue_fact(
                    DockLiveUndockQueuedFact::Reduce(DockLiveUndockFact::DurableSwapCommitted {
                        identity,
                        token,
                    }),
                    cx,
                );
            }
            DockLiveUndockEffect::RestoreSource {
                identity,
                source,
                payload_lease,
                restore_focus,
            } => self.restore_source(identity, source, payload_lease, restore_focus, cx),
            DockLiveUndockEffect::RestoreSourceFocus {
                identity,
                source,
                payload_lease,
            } => self.restore_source_focus(identity, source, payload_lease, cx),
            DockLiveUndockEffect::ShutdownSourceRestorationRequired {
                identity,
                source,
                payload_lease,
            } => {
                self.clear_source_restoration_retry(identity);
                self.restore_source(identity, source, payload_lease, false, cx);
            }
            DockLiveUndockEffect::ApplyCommittedHostWindowEffects {
                identity,
                token,
                destination,
            } => {
                let pending = {
                    let mut state = self.state.borrow_mut();
                    let Some(execution) = state.executions.get_mut(&identity) else {
                        return;
                    };
                    let runtime = execution.seed.source.runtime.clone();
                    let effects = match execution.promotion.as_mut() {
                        Some(DockLiveUndockPromotionExecution::Durable(
                            DockLiveUndockDurablePromotionExecution::Host(durable),
                        )) if durable.identity == identity
                            && durable.token == token
                            && durable.destination == destination =>
                        {
                            durable.pending_window_effects.take()
                        }
                        _ => None,
                    };
                    effects.map(|effects| (runtime, effects))
                };
                if let Some((runtime, effects)) = pending {
                    runtime.apply_committed_window_effects(effects, cx);
                }
            }
            DockLiveUndockEffect::DestinationSemanticsCommitRequired {
                identity,
                token,
                destination,
            } => {
                enum Authority {
                    SameWindow(WindowHandle<DockHost>),
                    Host {
                        host: WeakEntity<DockHost>,
                        binding: DockHostWindowBinding,
                        registration: crate::viewport_registry::DockViewportRegistrationKey,
                        target: super::live_undock::DockLiveUndockHostTarget,
                    },
                }
                let authority =
                    self.state
                        .borrow()
                        .executions
                        .get(&identity)
                        .and_then(|execution| match execution.promotion.as_ref()? {
                            DockLiveUndockPromotionExecution::Durable(
                                DockLiveUndockDurablePromotionExecution::SameWindow(durable),
                            ) if durable.token == token && durable.destination == destination => {
                                Some(Authority::SameWindow(durable.destination_window))
                            }
                            DockLiveUndockPromotionExecution::Durable(
                                DockLiveUndockDurablePromotionExecution::Host(durable),
                            ) if durable.token == token && durable.destination == destination => {
                                let DockLiveUndockPromotionDestination::Host(target) = destination
                                else {
                                    return None;
                                };
                                Some(Authority::Host {
                                    host: durable.destination_host.clone(),
                                    binding: durable.destination_binding,
                                    registration: durable.registration.clone(),
                                    target,
                                })
                            }
                            _ => None,
                        });
                match authority {
                    Some(Authority::SameWindow(window)) => {
                        #[cfg(test)]
                        let terminate = std::mem::take(
                            &mut self
                                .state
                                .borrow_mut()
                                .terminate_next_same_window_destination_before_semantics_ack,
                        );
                        #[cfg(not(test))]
                        let terminate = false;
                        if terminate {
                            let _ = window.update(cx, |_, window, cx| window.remove_window(cx));
                        } else {
                            let _ = window.update(cx, |_, window, _| window.refresh());
                        }
                    }
                    Some(Authority::Host {
                        host,
                        binding,
                        registration,
                        target,
                    }) => {
                        let committed = host.upgrade().is_some_and(|host| {
                            cx.read_entity(&host, |host, _| {
                                host.surface_owner_entity().as_ref() == Some(owner)
                                    && host.current_window_binding() == Some(binding)
                                    && host.current_viewport_registration()
                                        == Some(registration.clone())
                            })
                        });
                        let fact = committed
                            .then(|| {
                                DockLiveUndockDestinationSemanticsReceipt::new_host(
                                    identity, token, target,
                                )
                            })
                            .flatten()
                            .map_or_else(
                                || DockLiveUndockFact::DestinationSemanticsCommitFailed {
                                    identity,
                                    token,
                                    destination,
                                },
                                |receipt| DockLiveUndockFact::DestinationSemanticsCommitted {
                                    identity,
                                    receipt,
                                },
                            );
                        self.enqueue_fact(DockLiveUndockQueuedFact::Reduce(fact), cx);
                    }
                    None => {
                        self.enqueue_fact(
                            DockLiveUndockQueuedFact::Reduce(
                                DockLiveUndockFact::DestinationSemanticsCommitFailed {
                                    identity,
                                    token,
                                    destination,
                                },
                            ),
                            cx,
                        );
                    }
                }
            }
            DockLiveUndockEffect::DestinationInteractionAdmissionRequired {
                identity,
                semantics,
            } => {
                enum Authority {
                    SameWindow {
                        runtime: DockViewportRuntimeHandle,
                        window: WindowHandle<DockHost>,
                        registration: crate::viewport_registry::DockViewportRegistrationKey,
                        session: WindowProvisionalSession,
                        ticket: WindowProvisionalSemanticsTicket,
                        source_host: WeakEntity<DockHost>,
                        source_key: DockHostLivePresentationKey,
                        source_lease: DockLiveUndockPayloadLeaseReceipt,
                    },
                    Host {
                        host: WeakEntity<DockHost>,
                        binding: DockHostWindowBinding,
                        registration: crate::viewport_registry::DockViewportRegistrationKey,
                        source_host: WeakEntity<DockHost>,
                    },
                }
                let authority =
                    self.state
                        .borrow()
                        .executions
                        .get(&identity)
                        .and_then(|execution| match execution.promotion.as_ref()? {
                            DockLiveUndockPromotionExecution::Durable(
                                DockLiveUndockDurablePromotionExecution::SameWindow(durable),
                            ) if durable.identity == identity
                                && durable.token == semantics.token()
                                && durable.destination == semantics.destination() =>
                            {
                                let presentation = execution.presentation.as_ref()?;
                                let source_key = presentation.source_key?;
                                Some(Authority::SameWindow {
                                    runtime: execution.seed.source.runtime.clone(),
                                    window: durable.destination_window,
                                    registration: durable.registration.clone(),
                                    session: durable.provisional_session.clone(),
                                    ticket: durable.semantics.clone(),
                                    source_host: execution.seed.source.source_host.clone(),
                                    source_key,
                                    source_lease: presentation.lease,
                                })
                            }
                            DockLiveUndockPromotionExecution::Durable(
                                DockLiveUndockDurablePromotionExecution::Host(durable),
                            ) if durable.identity == identity
                                && durable.token == semantics.token()
                                && durable.destination == semantics.destination() =>
                            {
                                Some(Authority::Host {
                                    host: durable.destination_host.clone(),
                                    binding: durable.destination_binding,
                                    registration: durable.registration.clone(),
                                    source_host: execution.seed.source.source_host.clone(),
                                })
                            }
                            _ => None,
                        });
                #[cfg(test)]
                let reject_admission = std::mem::take(
                    &mut self
                        .state
                        .borrow_mut()
                        .reject_next_destination_interaction_admission,
                );
                #[cfg(not(test))]
                let reject_admission = false;
                let admitted =
                    (!reject_admission)
                        .then_some(authority)
                        .flatten()
                        .and_then(|authority| match authority {
                            Authority::SameWindow {
                                runtime,
                                window,
                                registration,
                                session,
                                ticket,
                                source_host,
                                source_key,
                                source_lease,
                            } => {
                                if !runtime.adopt_live_undock_committed_window_lifecycle(
                                    &registration,
                                    window.into(),
                                    cx,
                                ) {
                                    return None;
                                }
                                let source_proxies_retired = source_host
                                    .update(cx, |host, cx| {
                                        match host.live_source_semantic_proxy() {
                                            Some(proxy)
                                                if proxy.key() == source_key
                                                    && proxy.lease() == source_lease =>
                                            {
                                                host.retire_live_source_semantic_proxy(
                                                    source_key,
                                                    source_lease,
                                                    cx,
                                                )
                                            }
                                            None => true,
                                            Some(_) => false,
                                        }
                                    })
                                    .unwrap_or(true);
                                if !source_proxies_retired {
                                    return None;
                                }
                                window
                            .update(cx, |_, window, cx| {
                                window
                                    .admit_provisional_interaction(&session, &ticket, cx)
                                    .ok()?;
                                DockLiveUndockDestinationInteractionReceipt::new_same_window(
                                    semantics, &session,
                                )
                            })
                            .ok()
                            .flatten()
                            }
                            Authority::Host {
                                host,
                                binding,
                                registration,
                                source_host,
                            } => {
                                let source_proxy_absent = source_host
                                    .read_with(cx, |host, _| {
                                        host.live_source_semantic_proxy().is_none()
                                    })
                                    .unwrap_or(true);
                                let target_is_exact = host.upgrade().is_some_and(|host| {
                                    cx.read_entity(&host, |host, _| {
                                        host.surface_owner_entity().as_ref() == Some(owner)
                                            && host.current_window_binding() == Some(binding)
                                            && host.current_viewport_registration()
                                                == Some(registration.clone())
                                    })
                                });
                                (source_proxy_absent && target_is_exact)
                                    .then(|| {
                                        DockLiveUndockDestinationInteractionReceipt::new_host(
                                            semantics,
                                        )
                                    })
                                    .flatten()
                            }
                        });
                let fact = admitted.map_or_else(
                    || DockLiveUndockFact::DestinationInteractionAdmissionFailed {
                        identity,
                        semantics,
                    },
                    |receipt| DockLiveUndockFact::DestinationInteractionAdmitted {
                        identity,
                        receipt,
                    },
                );
                self.enqueue_fact(DockLiveUndockQueuedFact::Reduce(fact), cx);
            }
            DockLiveUndockEffect::DestinationInteractionReady {
                identity,
                interaction,
                destination,
            } => {
                let activation =
                    self.state
                        .borrow()
                        .executions
                        .get(&identity)
                        .and_then(|execution| match execution.promotion.as_ref()? {
                            DockLiveUndockPromotionExecution::Durable(
                                DockLiveUndockDurablePromotionExecution::SameWindow(durable),
                            ) if durable.destination == destination
                                && interaction.semantics().token() == durable.token =>
                            {
                                let focus = execution
                                    .seed
                                    .source
                                    .session
                                    .focus_item()
                                    .cloned()
                                    .map(crate::DockViewportFocusRequest::panel)
                                    .unwrap_or_else(
                                        crate::DockViewportFocusRequest::no_panel_focus,
                                    );
                                Some(crate::DockViewportActivationTransaction::registered(
                                    durable.registration.clone(),
                                    durable.destination_window,
                                    focus,
                                ))
                            }
                            DockLiveUndockPromotionExecution::Durable(
                                DockLiveUndockDurablePromotionExecution::Host(durable),
                            ) if durable.destination == destination
                                && interaction.semantics().token() == durable.token =>
                            {
                                durable.activation.clone()
                            }
                            _ => None,
                        });
                #[cfg(test)]
                let before_activation = self
                    .state
                    .borrow_mut()
                    .before_destination_interaction_activation_test_hook
                    .take();
                #[cfg(test)]
                if let Some(before_activation) = before_activation {
                    before_activation(cx);
                }
                if let Some(activation) = activation {
                    let _ = crate::viewport_activation::apply_viewport_activation_transaction(
                        Some(activation),
                        cx,
                    );
                }
            }
            DockLiveUndockEffect::RecoverOrphanedPayloadTopology {
                identity,
                payload_lease,
                provisional,
            } => {
                self.clear_source_restoration_retry(identity);
                if let Some(fact) = self.recover_orphaned_payload_topology(
                    owner,
                    identity,
                    payload_lease,
                    provisional,
                    cx,
                ) {
                    self.clear_orphan_recovery_retry(identity);
                    self.enqueue_fact(DockLiveUndockQueuedFact::Reduce(fact), cx);
                } else {
                    self.schedule_orphan_recovery_retry(identity, payload_lease, provisional, cx);
                }
            }
            DockLiveUndockEffect::ShutdownOrphanRecoveryRequired {
                identity,
                payload_lease,
                provisional,
            } => {
                self.clear_orphan_recovery_retry(identity);
                let fact = self
                    .recover_orphaned_payload_topology(
                        owner,
                        identity,
                        payload_lease,
                        provisional,
                        cx,
                    )
                    .unwrap_or_else(|| {
                        match self.execute_shutdown_orphan_cleanup(
                            identity,
                            payload_lease,
                            provisional,
                            cx,
                        ) {
                            Ok(receipt) => {
                                DockLiveUndockFact::ShutdownOrphanCleanupCompleted { receipt }
                            }
                            Err(failure) => DockLiveUndockFact::ShutdownOrphanCleanupFailed {
                                identity,
                                payload_lease,
                                failure,
                            },
                        }
                    });
                self.enqueue_fact(DockLiveUndockQueuedFact::Reduce(fact), cx);
            }
            DockLiveUndockEffect::RecoverCommittedDestinationTopology {
                identity,
                authority,
                token,
                destination,
            } => {
                match self.attempt_committed_destination_recovery(
                    owner,
                    identity,
                    authority,
                    token,
                    destination,
                    cx,
                ) {
                    Ok(recovery) => {
                        self.clear_committed_destination_recovery_retry(identity);
                        self.enqueue_fact(
                            DockLiveUndockQueuedFact::Reduce(
                                DockLiveUndockFact::CommittedDestinationRecoveryCommitted {
                                    identity,
                                    receipt:
                                        DockLiveUndockCommittedDestinationRecoveryReceipt::new(
                                            recovery,
                                        )
                                        .expect(
                                            "committed destination recovery must retain durable authority",
                                        ),
                                },
                            ),
                            cx,
                        );
                    }
                    Err(_) => self.schedule_committed_destination_recovery_retry(
                        identity,
                        authority,
                        token,
                        destination,
                        cx,
                    ),
                }
            }
            DockLiveUndockEffect::ShutdownCommittedDestinationRecoveryRequired {
                identity,
                authority,
                token,
                destination,
            } => {
                self.clear_committed_destination_recovery_retry(identity);
                let fact = match self.attempt_committed_destination_recovery(
                    owner,
                    identity,
                    authority,
                    token,
                    destination,
                    cx,
                ) {
                    Ok(recovery) => DockLiveUndockFact::CommittedDestinationRecoveryCommitted {
                        identity,
                        receipt: DockLiveUndockCommittedDestinationRecoveryReceipt::new(recovery)
                            .expect("committed destination recovery must retain durable authority"),
                    },
                    Err(failure) => {
                        DockLiveUndockFact::ShutdownCommittedDestinationRecoveryFailed {
                            identity,
                            authority,
                            token,
                            destination,
                            failure,
                        }
                    }
                };
                self.enqueue_fact(DockLiveUndockQueuedFact::Reduce(fact), cx);
            }
            DockLiveUndockEffect::RetireCommittedSameWindowDestination {
                identity,
                token,
                window_id,
            } => {
                let retirement =
                    self.state
                        .borrow()
                        .executions
                        .get(&identity)
                        .and_then(|execution| {
                            let DockLiveUndockPromotionExecution::Durable(
                                DockLiveUndockDurablePromotionExecution::SameWindow(durable),
                            ) = execution.promotion.as_ref()?
                            else {
                                return None;
                            };
                            let destination =
                                DockLiveUndockPromotionDestination::SameWindowDesktop { window_id };
                            (execution.request.key() == identity.opening()
                                && durable.identity == identity
                                && durable.token == token
                                && durable.destination == destination
                                && durable.destination_window.window_id() == window_id
                                && durable.destination_binding.window_id() == window_id
                                && durable.destination_binding.generation() != 0
                                && durable.registration.window_id() == window_id
                                && durable.registration.lineage()
                                    == crate::DockViewportRuntimeLineage::Surface(
                                        identity.opening().lease(),
                                    )
                                && execution.destination_host == Some(durable.destination_window)
                                && execution.presentation.as_ref().is_some_and(|presentation| {
                                    presentation.lease.identity() == identity
                                        && presentation.lease.destination_window() == window_id
                                }))
                            .then_some(durable.destination_window)
                        });
                let Some(destination_window) = retirement else {
                    return;
                };
                super::close_live_undock_window_quietly(destination_window.into(), cx);
            }
            DockLiveUndockEffect::SettleShutdownDependency {
                identity,
                dependency,
            } => super::settle_live_undock_dependency(owner, identity, Some(dependency), cx),
            DockLiveUndockEffect::PublishTerminal { identity, .. } => {
                self.finalize_live_payload_drag(identity, cx);
            }
            DockLiveUndockEffect::FailShutdownDependency {
                identity,
                dependency,
                failure,
            } => {
                log::error!(
                    "live-undock shutdown cleanup failed; closing with an explicit failure terminal: identity={identity:?}, dependency={dependency:?}, failure={failure:?}"
                );
                super::fail_live_undock_dependency(owner, identity, dependency, cx);
            }
            DockLiveUndockEffect::TriggerDeferred { .. }
            | DockLiveUndockEffect::RouteFeedbackChanged { .. }
            | DockLiveUndockEffect::ShutdownFrozen(_)
            | DockLiveUndockEffect::ShutdownDependencyTransferred { .. }
            | DockLiveUndockEffect::WindowTerminalSettled(_) => {
                let _ = owner;
            }
        }
    }

    fn retire_source_transport_proxy(&self, identity: DockLiveUndockIdentity, cx: &mut App) {
        let authority = self
            .state
            .borrow()
            .executions
            .get(&identity)
            .map(|execution| {
                (
                    execution.seed.source.source_host.clone(),
                    execution.seed.source.source_transport.clone(),
                )
            });
        let Some((source_host, transport)) = authority else {
            return;
        };
        let key = transport.key();
        transport.retire();
        if let Some(source_host) = source_host.upgrade() {
            let _ = cx.update_entity(&source_host, |host, host_cx| {
                host.retire_native_drag_transport_proxy(key, host_cx)
            });
        }
    }

    fn finalize_live_payload_drag(&self, identity: DockLiveUndockIdentity, cx: &mut App) {
        let session_checked_out = self
            .state
            .borrow()
            .executions
            .get(&identity)
            .and_then(|execution| execution.presentation.as_ref())
            .is_some_and(|presentation| presentation.session.is_checked_out());
        if session_checked_out {
            let runtime = self.clone();
            cx.defer(move |cx| runtime.finalize_live_payload_drag(identity, cx));
            return;
        }
        let execution = self.state.borrow_mut().executions.remove(&identity);
        let Some(execution) = execution else {
            return;
        };
        if execution.seed.source.identity_slot.get() == Some(identity) {
            execution.seed.source.identity_slot.set(None);
        }
        let finalizer = execution.seed.source.payload_finalizer;
        let runtime = execution.seed.source.runtime;
        let work_context = execution.seed.source.work_context;
        let session = execution.seed.source.session;
        let _ = settle_payload_drag_finalizer_claim(
            finalizer.claim_live_undock(identity),
            &runtime,
            work_context,
            &session,
            cx,
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockPayloadDragFinalization {
    NotClaimed,
    Finalized,
    TransferredToSurfaceShutdown,
}

pub(crate) fn settle_payload_drag_finalizer_claim(
    claim: Option<DockPayloadDragFinalizerClaim>,
    runtime: &DockViewportRuntimeHandle,
    work_context: DockViewportRuntimeWorkContext,
    session: &DockRuntimeDragSession,
    cx: &mut App,
) -> DockPayloadDragFinalization {
    let Some(claim) = claim else {
        return DockPayloadDragFinalization::NotClaimed;
    };
    let changed = runtime.finish_payload_drag_with_app(session, cx);
    if changed || runtime.admits_work_context(work_context) {
        claim.complete();
        return DockPayloadDragFinalization::Finalized;
    }
    match work_context.lineage() {
        crate::DockViewportRuntimeLineage::Surface(lease) => {
            let finalizer = claim.transfer_to_surface_shutdown(lease);
            if super::register_surface_shutdown_payload_finalizer(lease, finalizer, cx) {
                DockPayloadDragFinalization::TransferredToSurfaceShutdown
            } else {
                DockPayloadDragFinalization::Finalized
            }
        }
        crate::DockViewportRuntimeLineage::Unmanaged => {
            claim.complete();
            DockPayloadDragFinalization::Finalized
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::{
        live_undock::{
            DockLiveUndockDragGeneration, DockLiveUndockRouteFeedback, DockLiveUndockSession,
            DockLiveUndockSourceSnapshot,
        },
        window_session::DockSurfaceWindowSession,
    };
    use open_gpui::{EntityId, WindowId};

    fn identity(generation: u64) -> DockLiveUndockIdentity {
        let mut windows = DockSurfaceWindowSession::new(EntityId::from(7));
        let opening = windows
            .reserve_opening()
            .expect("test opening should reserve");
        let lease = windows
            .commit_opening(opening, WindowId::from(8))
            .expect("test opening should commit");
        let trigger = DockLiveUndockTrigger::new(
            DockLiveUndockDragGeneration::new(generation)
                .expect("test drag generation must be non-zero"),
            DockLiveUndockSourceSnapshot::new(WindowId::from(8), generation),
            DockLiveUndockRouteFeedback::Desktop,
        )
        .expect("desktop trigger should be eligible");
        DockLiveUndockSession::new()
            .apply(DockLiveUndockFact::Trigger { lease, trigger })
            .into_iter()
            .find_map(|effect| match effect {
                DockLiveUndockEffect::OpenProvisional { identity, .. } => Some(identity),
                _ => None,
            })
            .expect("test trigger should mint an identity")
    }

    #[test]
    fn finalizer_claim_restores_authority_when_cleanup_panics() {
        let finalizer = DockPayloadDragFinalizer::new();
        let claim = finalizer.claim_route().expect("route should own cleanup");
        assert_eq!(
            finalizer.authority.get(),
            DockPayloadDragFinalizerAuthority::Finalizing
        );
        drop(claim);
        assert_eq!(
            finalizer.authority.get(),
            DockPayloadDragFinalizerAuthority::Route
        );
        finalizer
            .claim_route()
            .expect("restored route authority should remain retryable")
            .complete();
        assert_eq!(
            finalizer.authority.get(),
            DockPayloadDragFinalizerAuthority::Finalized
        );
    }

    #[test]
    fn finalizer_handoff_is_exact_and_route_cannot_finalize_live_work() {
        let first = identity(1);
        let second = identity(2);
        let finalizer = DockPayloadDragFinalizer::new();

        assert!(finalizer.begin_live_undock(first));
        assert!(finalizer.claim_route().is_none());
        assert!(!finalizer.commit_live_undock(second));
        assert!(finalizer.commit_live_undock(first));
        assert!(finalizer.claim_route().is_none());
        assert!(finalizer.claim_live_undock(second).is_none());

        finalizer
            .claim_live_undock(first)
            .expect("the exact live identity should own finalization")
            .complete();
        assert_eq!(
            finalizer.authority.get(),
            DockPayloadDragFinalizerAuthority::Finalized
        );
    }

    #[test]
    fn release_deadline_only_expires_the_exact_armed_placement_generation() {
        let first = DockLiveUndockPlacementGeneration::new(7)
            .expect("test placement generation must be non-zero");
        let replacement = DockLiveUndockPlacementGeneration::new(8)
            .expect("test placement generation must be non-zero");
        let mut deadline = DockLiveUndockReleaseDeadline::default();

        deadline.arm(first);
        deadline.arm(replacement);

        assert!(!deadline.claim_expiration(first));
        assert!(deadline.claim_expiration(replacement));
        assert!(!deadline.claim_expiration(replacement));
    }

    #[test]
    fn orphan_recovery_retry_remains_wakeable_and_invalidates_late_timers() {
        let mut retry = DockLiveUndockRetryBackoff::default();
        let (first, first_delay) = retry.arm_if_idle().expect("an idle retry should arm once");
        assert_eq!(first, 1);
        assert_eq!(first_delay, Duration::from_millis(16));
        assert!(retry.arm_if_idle().is_none());
        assert!(retry.claim(first));
        assert!(!retry.claim(first));

        let (second, second_delay) = retry.arm_if_idle().expect("a claimed retry should rearm");
        assert_eq!(second, 2);
        assert_eq!(second_delay, Duration::from_millis(32));
        assert!(!retry.claim(first));
        assert!(retry.claim(second));

        let (third, _) = retry.arm_if_idle().expect("third retry should arm");
        assert!(retry.claim(third));
        let (fourth, _) = retry.arm_if_idle().expect("fourth retry should arm");
        assert!(retry.claim(fourth));
        let (capped, capped_delay) = retry.arm_if_idle().expect("capped retry should arm");
        assert_eq!(capped_delay, LIVE_UNDOCK_RETRY_CAP);
        retry.clear();
        assert!(!retry.claim(capped));

        let (rearmed, rearmed_delay) = retry
            .arm_if_idle()
            .expect("clearing should make the retry idle");
        assert!(rearmed > capped);
        assert_eq!(rearmed_delay, Duration::from_millis(16));
    }

    #[test]
    fn surface_shutdown_record_is_the_only_terminal_owner_after_transfer() {
        let identity = identity(3);
        let lease = identity.opening().lease();
        let finalizer = DockPayloadDragFinalizer::new();

        assert!(finalizer.begin_live_undock(identity));
        assert!(finalizer.commit_live_undock(identity));
        let record = finalizer
            .claim_live_undock(identity)
            .expect("the live generation should own finalization")
            .transfer_to_surface_shutdown(lease);

        assert!(finalizer.claim_route().is_none());
        assert!(finalizer.claim_live_undock(identity).is_none());
        assert!(record.complete());
        assert_eq!(
            finalizer.authority.get(),
            DockPayloadDragFinalizerAuthority::Finalized
        );
    }
}
