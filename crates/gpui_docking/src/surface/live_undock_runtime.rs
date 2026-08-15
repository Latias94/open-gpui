use super::{
    DockSurfaceDeferredPublication, DockSurfaceOwner, DockSurfaceTransactionReceipt,
    live_undock::{
        DockLiveUndockCommittedDestinationRecoveryFailure,
        DockLiveUndockCommittedDestinationRecoveryReceipt,
        DockLiveUndockDestinationInteractionReceipt, DockLiveUndockDestinationSemanticsReceipt,
        DockLiveUndockEffect, DockLiveUndockEffects, DockLiveUndockFact,
        DockLiveUndockFinalPlacementReceipt, DockLiveUndockHostCleanupEvidence,
        DockLiveUndockIdentity, DockLiveUndockOpenRequest, DockLiveUndockOrphanCleanupFailure,
        DockLiveUndockOrphanCleanupReceipt, DockLiveUndockOrphanRecoveryReceipt,
        DockLiveUndockPayloadLeaseReceipt, DockLiveUndockPayloadPresentationReceipt,
        DockLiveUndockPhysicalBounds, DockLiveUndockPlacementGeneration,
        DockLiveUndockPresentationAuthorityLossReceipt, DockLiveUndockPresentationFailure,
        DockLiveUndockPromotionCommitDisposition, DockLiveUndockPromotionDestination,
        DockLiveUndockPromotionToken, DockLiveUndockRehostCleanupEvidence,
        DockLiveUndockReleaseLock, DockLiveUndockRetainedVisualCleanupEvidence,
        DockLiveUndockRevealObservation, DockLiveUndockRevealOutcome, DockLiveUndockRevealReceipt,
        DockLiveUndockRouteGeneration, DockLiveUndockRoutePlacementOutcome,
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
    DockViewportCommittedWindowEffectsAcceptanceOutcome, DockViewportCommittedWindowEffectsReceipt,
    DockViewportDropPayload, DockViewportLockedDropRoute,
    DockViewportPreflightedLiveUndockHostDrop, DockViewportPreparedLiveUndockHostDrop,
    DockViewportRuntimeHandle, DockViewportRuntimeWorkContext, DockViewportTearOffRequest,
    DockViewportWindowFacts,
    drag::DockDragPayload,
    host::{
        DockHostLiveDestinationPromotionReceipt, DockHostLiveDestinationSemantics,
        DockHostLivePresentationKey, DockHostLiveSourceRestorationInstallOutcome,
        DockHostLiveSourceRetirementReceipt, DockHostPreparedLiveDestinationPromotion,
        DockHostPreparedLivePresentationAbandonment, DockHostPreparedLiveSourceRetirement,
        DockHostPreparedLiveSourceSemanticRetirement,
    },
    host_render_session::DockHostPresentationSession,
    interaction::DockRuntimeDragSession,
    presentation_scene::DockPresentationScene,
    surface::live_payload_carrier::{DockLivePayloadCarrier, resolve_live_payload_carrier},
    viewport_drop_scene::DockViewportHostSceneFrame,
    viewport_runtime::DockViewportPreparedLiveUndockPromotion,
    viewport_runtime_handle::DockViewportCommittedLiveUndockHostDrop,
    viewport_tear_off_move::{DockViewportTearOffMovePlan, lock_tear_off_move},
    workspace::{
        DockWorkspaceGraphCommitId, DockWorkspaceGraphCommitObservation,
        DockWorkspaceGraphCommitPreparation, DockWorkspaceGraphCommitReceipt,
    },
};
use open_gpui::{
    AnyWindowHandle, App, AppContext, Bounds, DevicePixels, Entity, SharedString, Subscription,
    WeakEntity, Window, WindowBounds, WindowHandle, WindowId, WindowInitialPresentationStatus,
    WindowMutationDispatch, WindowMutationOutcome, WindowOptions, WindowPhysicalPlacementRequest,
    WindowProvisionalPlacementOutcome, WindowProvisionalPlacementPurpose,
    WindowProvisionalPlacementRequest, WindowProvisionalPlacementSnapshot,
    WindowProvisionalRevealZOrder, WindowProvisionalSemanticsOutcome,
    WindowProvisionalSemanticsTicket, WindowProvisionalSession, WindowProvisionalSessionPhase,
    point,
    retained_visual::{self, Ticket},
    size,
    view_presentation_window::{
        self, PreparedRehostTerminal, RehostProjection, RehostSession, RehostTerminalPreparation,
    },
};
use std::{
    any::Any,
    cell::{Cell, RefCell},
    collections::HashMap,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
    time::Duration,
};

const LIVE_UNDOCK_RELEASE_DEADLINE: Duration = Duration::from_millis(500);
const LIVE_UNDOCK_RETRY_CAP: Duration = Duration::from_millis(250);
pub(crate) const LIVE_UNDOCK_DESTINATION_SEMANTICS_WATCHDOG_INTERVAL: Duration =
    Duration::from_millis(250);

fn gpui_physical_bounds(bounds: DockLiveUndockPhysicalBounds) -> Option<Bounds<DevicePixels>> {
    let width = i32::try_from(bounds.width()).ok()?;
    let height = i32::try_from(bounds.height()).ok()?;
    bounds.origin().x().checked_add(width)?;
    bounds.origin().y().checked_add(height)?;
    Some(Bounds::new(
        point(
            DevicePixels(bounds.origin().x()),
            DevicePixels(bounds.origin().y()),
        ),
        size(DevicePixels(width), DevicePixels(height)),
    ))
}

fn workspace_graph_projection_is_exact(
    controller: &Entity<DockController>,
    receipt: Option<DockWorkspaceGraphCommitReceipt>,
    cx: &App,
) -> bool {
    receipt.is_some_and(|receipt| {
        cx.read_entity(controller, |controller, _| {
            controller.workspace().observe_graph_commit(receipt)
                == Some(DockWorkspaceGraphCommitObservation::Exact)
        })
    })
}

fn destination_host_semantics_are_exact(
    host: &Entity<DockHost>,
    marker: &DockHostLiveDestinationSemantics,
    cx: &App,
) -> bool {
    cx.read_entity(host, |host, _| {
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
    })
}

fn record_post_commit_panic<T>(
    first_panic: &mut Option<Box<dyn Any + Send>>,
    stage: impl FnOnce() -> T,
) -> Option<T> {
    match catch_unwind(AssertUnwindSafe(stage)) {
        Ok(value) => Some(value),
        Err(payload) => {
            if first_panic.is_none() {
                *first_panic = Some(payload);
            } else {
                log::error!("suppressed a secondary live-undock post-commit panic");
            }
            None
        }
    }
}

fn notify_post_commit_entity<T: 'static>(
    first_panic: &mut Option<Box<dyn Any + Send>>,
    entity: &Entity<T>,
    cx: &mut App,
) -> bool {
    let entered = Cell::new(false);
    let completed = record_post_commit_panic(first_panic, || {
        cx.update_entity(entity, |_, entity_cx| {
            entered.set(true);
            entity_cx.notify();
        });
    });
    completed.is_some() || entered.get()
}

fn accept_host_drop_window_effects(
    runtime: &DockViewportRuntimeHandle,
    committed: &DockViewportCommittedLiveUndockHostDrop,
    cx: &mut App,
) -> Option<DockViewportCommittedWindowEffectsReceipt> {
    match runtime.accept_live_undock_host_drop_window_effects(committed, cx) {
        DockViewportCommittedWindowEffectsAcceptanceOutcome::Accepted(receipt) => Some(receipt),
        DockViewportCommittedWindowEffectsAcceptanceOutcome::InProgress => None,
        DockViewportCommittedWindowEffectsAcceptanceOutcome::Stale => {
            panic!("committed Host window effects lost their canonical runtime receipt")
        }
    }
}

#[derive(Clone)]
struct DockLiveUndockRetainedVisualRelease {
    inner: Rc<DockLiveUndockRetainedVisualReleaseInner>,
}

struct DockLiveUndockRetainedVisualReleaseInner {
    source_window: AnyWindowHandle,
    ticket: Ticket,
    prepared: RefCell<Option<retained_visual::PreparedRelease>>,
    receipt: Cell<Option<retained_visual::ReleaseReceipt>>,
    source_window_terminal: Cell<bool>,
    call_in_flight: Cell<bool>,
}

enum DockLiveUndockRetainedVisualReleaseRecovery {
    Prepared(retained_visual::PreparedRelease),
    Released(retained_visual::ReleaseReceipt),
}

impl DockLiveUndockRetainedVisualRelease {
    fn new(source_window: AnyWindowHandle, prepared: retained_visual::PreparedRelease) -> Self {
        let ticket = prepared.ticket();
        assert_eq!(
            source_window.window_id(),
            ticket.source_window(),
            "a retained-visual release must remain bound to its source window"
        );
        Self {
            inner: Rc::new(DockLiveUndockRetainedVisualReleaseInner {
                source_window,
                ticket,
                prepared: RefCell::new(Some(prepared)),
                receipt: Cell::new(None),
                source_window_terminal: Cell::new(false),
                call_in_flight: Cell::new(false),
            }),
        }
    }

    fn is_settled(&self) -> bool {
        self.inner.receipt.get().is_some() || self.inner.source_window_terminal.get()
    }

    fn can_commit(&self, cx: &mut App) -> bool {
        if self.is_settled() {
            return true;
        }
        if self.inner.call_in_flight.get() {
            return false;
        }
        let prepared = self.inner.prepared.borrow();
        let Some(prepared) = prepared.as_ref() else {
            return false;
        };
        self.inner
            .source_window
            .update(cx, |_, window, _| {
                retained_visual::can_commit_prepared_release(window, prepared)
            })
            .unwrap_or(false)
    }

    fn recover_interrupted_call(&self, cx: &mut App) -> bool {
        let ticket = self.inner.ticket;
        let identity = ticket.identity();
        let recovered = self.inner.source_window.update(cx, |_, window, _| {
            if let Some(receipt) = retained_visual::observe_release(window, identity)
                .expect("a retained-visual release identity must remain bound to its source window")
            {
                DockLiveUndockRetainedVisualReleaseRecovery::Released(receipt)
            } else {
                DockLiveUndockRetainedVisualReleaseRecovery::Prepared(
                    retained_visual::prepare_release(window, &ticket).expect(
                        "an active retained-visual lease must remain preparable after an interrupted release",
                    ),
                )
            }
        });
        match recovered {
            Ok(DockLiveUndockRetainedVisualReleaseRecovery::Prepared(prepared)) => {
                *self.inner.prepared.borrow_mut() = Some(prepared);
                false
            }
            Ok(DockLiveUndockRetainedVisualReleaseRecovery::Released(receipt)) => {
                self.inner.receipt.set(Some(receipt));
                true
            }
            Err(_) => {
                self.inner.source_window_terminal.set(true);
                true
            }
        }
    }

    fn commit_prepared_infallible(&self, cx: &mut App) -> retained_visual::ReleaseReceipt {
        assert!(
            !self.is_settled(),
            "a retained visual release cannot commit twice"
        );
        let prepared = self
            .inner
            .prepared
            .borrow_mut()
            .take()
            .expect("a retained visual release must retain its prepared token until commit");
        let receipt = self
            .inner
            .source_window
            .update(cx, |_, window, _| {
                retained_visual::commit_prepared_release(window, prepared)
            })
            .expect("the prepared retained visual source window must remain live until commit");
        self.inner.receipt.set(Some(receipt));
        receipt
    }

    fn settle(&self, cx: &mut App) -> bool {
        if self.is_settled() {
            return true;
        }
        if self.inner.call_in_flight.replace(true) {
            return false;
        }
        let Some(prepared) = self.inner.prepared.borrow_mut().take() else {
            self.inner.call_in_flight.set(false);
            return self.recover_interrupted_call(cx);
        };
        let source_window = self.inner.source_window;
        let result = catch_unwind(AssertUnwindSafe(|| {
            source_window.update(cx, |_, window, _| {
                retained_visual::commit_prepared_release(window, prepared)
            })
        }));
        self.inner.call_in_flight.set(false);
        match result {
            Ok(Ok(receipt)) => {
                self.inner.receipt.set(Some(receipt));
                true
            }
            Ok(Err(_)) => {
                self.inner.source_window_terminal.set(true);
                true
            }
            Err(payload) => {
                self.recover_interrupted_call(cx);
                resume_unwind(payload)
            }
        }
    }
}

#[derive(Clone, Debug)]
struct DockLiveUndockPostCommitReceipt {
    settled: Rc<Cell<bool>>,
    retry_delay: Rc<Cell<Duration>>,
}

impl DockLiveUndockPostCommitReceipt {
    fn pending() -> Self {
        Self {
            settled: Rc::new(Cell::new(false)),
            retry_delay: Rc::new(Cell::new(Duration::from_millis(16))),
        }
    }

    fn is_settled(&self) -> bool {
        self.settled.get()
    }

    fn settle(&self) -> bool {
        !self.settled.replace(true)
    }

    fn next_retry_delay(&self) -> Duration {
        let delay = self.retry_delay.get();
        self.retry_delay.set(
            delay
                .checked_mul(2)
                .unwrap_or(LIVE_UNDOCK_RETRY_CAP)
                .min(LIVE_UNDOCK_RETRY_CAP),
        );
        delay
    }

    fn matches(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.settled, &other.settled)
    }
}

struct DockLiveUndockCompletedPromotionCommit {
    durable: DockLiveUndockDurablePromotionExecution,
    retained_released: bool,
    post_commit: DockLiveUndockPostCommitPlan,
}

#[derive(Clone)]
enum DockLiveUndockPostCommitPlan {
    SameWindow {
        identity: DockLiveUndockIdentity,
        journal: Rc<DockLiveUndockPromotionCommitJournal>,
        receipt: DockLiveUndockPostCommitReceipt,
    },
    Host {
        identity: DockLiveUndockIdentity,
        journal: Rc<DockLiveUndockPromotionCommitJournal>,
        receipt: DockLiveUndockPostCommitReceipt,
    },
}

impl DockLiveUndockPostCommitPlan {
    fn start(self, live_runtime: DockLiveUndockRuntime, cx: &mut App) {
        self.schedule(live_runtime, Duration::ZERO, cx);
    }

    fn schedule(self, live_runtime: DockLiveUndockRuntime, delay: Duration, cx: &mut App) {
        cx.defer_after_or_shutdown_critical_before_window_registry_clear(delay, move |cx| {
            let mut first_panic = None;
            let (identity, settled, receipt) = match &self {
                Self::SameWindow {
                    identity,
                    journal,
                    receipt,
                } => (
                    *identity,
                    live_runtime.drive_same_window_post_commit_journal(
                        journal,
                        cx,
                        &mut first_panic,
                    ),
                    receipt,
                ),
                Self::Host {
                    identity,
                    journal,
                    receipt,
                } => (
                    *identity,
                    live_runtime.drive_host_post_commit_journal(
                        journal,
                        true,
                        cx,
                        &mut first_panic,
                    ),
                    receipt,
                ),
            };

            if settled {
                live_runtime.settle_post_commit_and_resume_terminal(identity, receipt, cx);
            } else {
                let retry = self.clone();
                retry.schedule(live_runtime.clone(), receipt.next_retry_delay(), cx);
            }
            if let Some(payload) = first_panic {
                resume_unwind(payload);
            }
        });
    }
}

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

    fn claim_terminal(
        &self,
        identity: DockLiveUndockIdentity,
    ) -> Option<DockPayloadDragFinalizerClaim> {
        let authority = self.authority.get();
        match authority {
            DockPayloadDragFinalizerAuthority::Route => self.claim(authority),
            DockPayloadDragFinalizerAuthority::PendingLiveUndock(current)
            | DockPayloadDragFinalizerAuthority::LiveUndock(current)
                if current == identity =>
            {
                self.claim(authority)
            }
            DockPayloadDragFinalizerAuthority::PendingLiveUndock(_)
            | DockPayloadDragFinalizerAuthority::LiveUndock(_)
            | DockPayloadDragFinalizerAuthority::Finalizing
            | DockPayloadDragFinalizerAuthority::SurfaceShutdown(_)
            | DockPayloadDragFinalizerAuthority::Finalized => None,
        }
    }

    fn is_terminally_settled(&self) -> bool {
        matches!(
            self.authority.get(),
            DockPayloadDragFinalizerAuthority::SurfaceShutdown(_)
                | DockPayloadDragFinalizerAuthority::Finalized
        )
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
            completed: false,
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
    completed: bool,
}

impl DockPayloadDragSurfaceShutdownFinalizer {
    pub(crate) fn same_token(&self, other: &Self) -> bool {
        self.lease == other.lease && self.finalizer.same_token(&other.finalizer)
    }

    pub(crate) fn complete(mut self) -> bool {
        let completed = self.complete_inner();
        self.completed = true;
        completed
    }

    fn complete_inner(&self) -> bool {
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

impl Drop for DockPayloadDragSurfaceShutdownFinalizer {
    fn drop(&mut self) {
        if !self.completed {
            let _ = self.complete_inner();
        }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DockLiveUndockDestinationSemanticsWatchdogKey {
    token: DockLiveUndockPromotionToken,
    destination: DockLiveUndockPromotionDestination,
    session_generation: u64,
    placement_mutation_generation: u64,
}

#[derive(Clone)]
struct DockLiveUndockSameWindowDestinationSemanticsAuthority {
    identity: DockLiveUndockIdentity,
    token: DockLiveUndockPromotionToken,
    destination: DockLiveUndockPromotionDestination,
    destination_window: WindowHandle<DockHost>,
    reveal: DockLiveUndockRevealReceipt,
    provisional_session: WindowProvisionalSession,
    semantics: WindowProvisionalSemanticsTicket,
    destination_host: WeakEntity<DockHost>,
    marker: DockHostLiveDestinationSemantics,
    controller: Entity<DockController>,
    graph_commit: Option<DockWorkspaceGraphCommitReceipt>,
}

impl DockLiveUndockSameWindowDestinationSemanticsAuthority {
    fn watchdog_key(&self) -> DockLiveUndockDestinationSemanticsWatchdogKey {
        DockLiveUndockRuntime::destination_semantics_watchdog_key(
            self.token,
            self.destination,
            &self.semantics,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DockLiveUndockDestinationSemanticsWatchdog {
    key: Option<DockLiveUndockDestinationSemanticsWatchdogKey>,
    generation: u64,
    armed: bool,
}

impl DockLiveUndockDestinationSemanticsWatchdog {
    fn arm(&mut self, key: DockLiveUndockDestinationSemanticsWatchdogKey) -> Option<u64> {
        if self.key != Some(key) {
            self.clear();
            self.key = Some(key);
        }
        if self.armed {
            return None;
        }
        self.generation = self
            .generation
            .checked_add(1)
            .expect("destination-semantics watchdog generation space exhausted");
        self.armed = true;
        Some(self.generation)
    }

    fn claim(
        &mut self,
        key: DockLiveUndockDestinationSemanticsWatchdogKey,
        generation: u64,
    ) -> bool {
        if self.key != Some(key) || self.generation != generation || !self.armed {
            return false;
        }
        self.armed = false;
        true
    }

    fn clear_if(&mut self, key: DockLiveUndockDestinationSemanticsWatchdogKey) {
        if self.key == Some(key) {
            self.clear();
        }
    }

    fn clear(&mut self) {
        self.key = None;
        self.armed = false;
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
    release_route_generation: Option<DockLiveUndockRouteGeneration>,
    route_placement: Option<DockLiveUndockRoutePlacementExecution>,
    release_placement: Option<DockLiveUndockReleasePlacementExecution>,
    observed_release_placement: Option<DockLiveUndockObservedReleasePlacement>,
    destination_host: Option<WindowHandle<DockHost>>,
    host_release: Option<DockLiveUndockHostReleaseAuthority>,
    presentation: Option<DockLiveUndockPresentationExecution>,
    promotion: Option<DockLiveUndockPromotionExecution>,
    destination_semantics_watchdog: DockLiveUndockDestinationSemanticsWatchdog,
    source_restoration_retry: DockLiveUndockRetryBackoff,
    orphan_recovery_retry: DockLiveUndockRetryBackoff,
    committed_destination_recovery_retry: DockLiveUndockRetryBackoff,
    committed_window_effects_retry: DockLiveUndockRetryBackoff,
    terminal_requested: bool,
    terminal_settlement_retry: DockLiveUndockRetryBackoff,
}

struct DockLiveUndockRoutePlacementExecution {
    window_id: WindowId,
    generation: DockLiveUndockRouteGeneration,
    mutation_generation: Option<u64>,
    subscription: Option<Subscription>,
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
    final_placement: DockLiveUndockFinalPlacementReceipt,
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
        match self {
            Self::Active(session) if session.is_terminal() => {
                *self = Self::Retired;
                true
            }
            Self::Retired => true,
            Self::Active(_) | Self::CheckedOut => false,
        }
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

#[derive(Clone)]
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
    promotion: DockLiveUndockCommittedDestinationPromotionAuthority,
    presentation_lease: Option<DockLiveUndockPayloadLeaseReceipt>,
    same_window_terminal_required: bool,
    token: DockLiveUndockPromotionToken,
    destination: DockLiveUndockPromotionDestination,
    runtime: DockViewportRuntimeHandle,
    recovery: DockLiveUndockPreparedRecoveryRecord,
    source_semantic: Option<DockLiveUndockPreparedSourceSemanticRetirement>,
    destination_window: WindowHandle<DockHost>,
}

enum DockLiveUndockCommittedDestinationPromotionAuthority {
    Durable,
    Journal(Rc<DockLiveUndockPromotionCommitJournal>),
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
    Committing(Rc<DockLiveUndockPromotionCommitJournal>),
    Durable(DockLiveUndockDurablePromotionExecution),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockLogicalCloseAuthority {
    ForwardCommitted(crate::viewport_registry::DockViewportRegistrationKey),
    Durable(crate::viewport_registry::DockViewportRegistrationKey),
}

impl DockLiveUndockLogicalCloseAuthority {
    pub(crate) fn into_registration(self) -> crate::viewport_registry::DockViewportRegistrationKey {
        match self {
            Self::ForwardCommitted(registration) | Self::Durable(registration) => registration,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockLiveUndockPromotionCommitBoundary {
    Reversible,
    AbortClaimed,
    InFlight,
    Irreversible,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockLiveUndockPromotionCommitDriveState {
    Idle,
    Driving,
    Terminal,
}

struct DockLiveUndockPromotionCommitJournal {
    identity: DockLiveUndockIdentity,
    token: DockLiveUndockPromotionToken,
    destination: DockLiveUndockPromotionDestination,
    surface_revision: u64,
    boundary: Cell<DockLiveUndockPromotionCommitBoundary>,
    drive: Cell<DockLiveUndockPromotionCommitDriveState>,
    recovery_receipt: Cell<Option<DockPayloadRecoveryCommitReceipt>>,
    recovery_requires_window_terminal: Cell<bool>,
    execution: RefCell<DockLiveUndockPromotionCommitExecution>,
}

enum DockLiveUndockPromotionCommitExecution {
    Pending(Option<DockLiveUndockPreparedPromotionExecution>),
    SameWindow(DockLiveUndockSameWindowPromotionCommit),
    Host(DockLiveUndockHostPromotionCommit),
    Aborted,
}

#[derive(Clone)]
enum DockLiveUndockSourceRetirementStage {
    Pending,
    Committed(DockHostLiveSourceRetirementReceipt),
    AuthorityAbsent,
    Retired,
}

struct DockLiveUndockSameWindowPromotionCommit {
    identity: DockLiveUndockIdentity,
    token: DockLiveUndockPromotionToken,
    destination: DockLiveUndockPromotionDestination,
    surface_revision: u64,
    controller: Entity<DockController>,
    prepared_graph: DockLiveUndockPreparedGraphCommit,
    graph_commit: Option<DockWorkspaceGraphCommitReceipt>,
    topology_recovery_required: bool,
    runtime: DockViewportRuntimeHandle,
    viewport: DockViewportPreparedLiveUndockPromotion,
    committed_viewport: Option<crate::viewport_runtime::DockViewportCommittedLiveUndockPromotion>,
    retained_release: DockLiveUndockRetainedVisualRelease,
    source_host: Entity<DockHost>,
    source: DockHostPreparedLiveSourceRetirement,
    source_retirement: DockLiveUndockSourceRetirementStage,
    destination_host: Entity<DockHost>,
    destination_host_promotion: DockHostPreparedLiveDestinationPromotion,
    destination_promotion: Option<DockHostLiveDestinationPromotionReceipt>,
    presentation: Option<RehostTerminalPreparation>,
    presentation_batch: Option<view_presentation_window::LeaseBatch>,
    provider_post_commit: Option<view_presentation_window::RehostTerminalPostCommit>,
    provider_refreshed: bool,
    reveal: DockLiveUndockRevealReceipt,
    provisional_session: WindowProvisionalSession,
    semantics: WindowProvisionalSemanticsTicket,
    surface: Option<DockSurfaceTransactionReceipt>,
    publication: Option<DockSurfaceDeferredPublication>,
    presentation_session_retired: bool,
    controller_notified: bool,
    source_host_notified: bool,
    destination_host_notified: bool,
    viewport_refreshed: bool,
}

#[derive(Clone)]
struct DockLiveUndockPreparedGraphCommit {
    commit_id: DockWorkspaceGraphCommitId,
    expected: DockGraph,
    projected: DockGraph,
}

struct DockLiveUndockHostPromotionCommit {
    identity: DockLiveUndockIdentity,
    token: DockLiveUndockPromotionToken,
    destination: DockLiveUndockPromotionDestination,
    surface_revision: u64,
    runtime: DockViewportRuntimeHandle,
    drop: DockViewportPreflightedLiveUndockHostDrop,
    committed_drop: Option<DockViewportCommittedLiveUndockHostDrop>,
    target_window: WindowHandle<DockHost>,
    target_host: Entity<DockHost>,
    target_binding: DockHostWindowBinding,
    target_registration: crate::viewport_registry::DockViewportRegistrationKey,
    presentation_cleanup: Option<DockLiveUndockHostPromotionCleanupCommit>,
    surface: Option<DockSurfaceTransactionReceipt>,
    publication: Option<DockSurfaceDeferredPublication>,
    host_drop_notified: bool,
    committed_destination_recovery_required: bool,
    lower_receipt_retired: bool,
}

struct DockLiveUndockHostPromotionCleanupCommit {
    payload_lease: DockLiveUndockPayloadLeaseReceipt,
    presentation_generation: u64,
    presentation: Option<DockLiveUndockPreparedPresentationCleanup>,
    presentation_committed: bool,
    provider_post_commit: Option<view_presentation_window::RehostTerminalPostCommit>,
    provider_refreshed: bool,
    source_host: DockLiveUndockPreparedHostPresentationAbandonment,
    source_host_committed: bool,
    provisional_host: DockLiveUndockPreparedHostPresentationAbandonment,
    provisional_host_committed: bool,
    retained_release: Option<DockLiveUndockRetainedVisualRelease>,
    session_retired: bool,
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
    retained_release: DockLiveUndockRetainedVisualRelease,
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
    retained_release: Option<DockLiveUndockRetainedVisualRelease>,
}

enum DockLiveUndockDurablePromotionExecution {
    SameWindow(DockLiveUndockDurableSameWindowPromotionExecution),
    Host(DockLiveUndockDurableHostPromotionExecution),
}

struct DockLiveUndockDurableSameWindowPromotionExecution {
    identity: DockLiveUndockIdentity,
    token: DockLiveUndockPromotionToken,
    destination: DockLiveUndockPromotionDestination,
    destination_window: WindowHandle<DockHost>,
    destination_binding: DockHostWindowBinding,
    registration: crate::viewport_registry::DockViewportRegistrationKey,
    reveal: DockLiveUndockRevealReceipt,
    provisional_session: WindowProvisionalSession,
    semantics: WindowProvisionalSemanticsTicket,
    viewport_commit: crate::viewport_runtime::DockViewportCommittedLiveUndockPromotion,
    controller: Entity<DockController>,
    graph_commit: Option<DockWorkspaceGraphCommitReceipt>,
    topology_recovery_required: bool,
    source_host: WeakEntity<DockHost>,
    source_retirement: DockHostLiveSourceRetirementReceipt,
    destination_host: WeakEntity<DockHost>,
    destination_promotion: DockHostLiveDestinationPromotionReceipt,
    post_commit: DockLiveUndockPostCommitReceipt,
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
    committed_destination_recovery_required: bool,
    host_drop_commit: DockViewportCommittedLiveUndockHostDrop,
    post_commit: DockLiveUndockPostCommitReceipt,
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

    fn provider_destination_already_committed(&self) -> bool {
        matches!(
            self,
            Self::SameWindow(DockLiveUndockPreparedSameWindowPromotionExecution {
                presentation: RehostTerminalPreparation::AlreadyCommitted(
                    view_presentation_window::RehostTerminalOutcome::DestinationCommitted(_),
                ),
                ..
            })
        )
    }
}

impl DockLiveUndockPromotionCommitJournal {
    fn pending(prepared: DockLiveUndockPreparedPromotionExecution) -> Self {
        let boundary = if prepared.provider_destination_already_committed() {
            DockLiveUndockPromotionCommitBoundary::Irreversible
        } else {
            DockLiveUndockPromotionCommitBoundary::Reversible
        };
        Self {
            identity: prepared.identity(),
            token: prepared.token(),
            destination: prepared.destination(),
            surface_revision: prepared.surface_revision(),
            boundary: Cell::new(boundary),
            drive: Cell::new(DockLiveUndockPromotionCommitDriveState::Idle),
            recovery_receipt: Cell::new(None),
            recovery_requires_window_terminal: Cell::new(false),
            execution: RefCell::new(DockLiveUndockPromotionCommitExecution::Pending(Some(
                prepared,
            ))),
        }
    }

    fn same_window(
        prepared: DockLiveUndockPreparedSameWindowPromotionExecution,
        prepared_graph: DockLiveUndockPreparedGraphCommit,
    ) -> Self {
        let DockLiveUndockPreparedSameWindowPromotionExecution {
            identity,
            token,
            destination,
            release: _,
            surface_revision,
            controller,
            move_plan: _,
            runtime,
            viewport,
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
        let boundary = if matches!(
            &presentation,
            RehostTerminalPreparation::AlreadyCommitted(
                view_presentation_window::RehostTerminalOutcome::DestinationCommitted(_)
            )
        ) {
            DockLiveUndockPromotionCommitBoundary::Irreversible
        } else {
            DockLiveUndockPromotionCommitBoundary::Reversible
        };
        Self {
            identity,
            token,
            destination,
            surface_revision,
            boundary: Cell::new(boundary),
            drive: Cell::new(DockLiveUndockPromotionCommitDriveState::Idle),
            recovery_receipt: Cell::new(None),
            recovery_requires_window_terminal: Cell::new(false),
            execution: RefCell::new(DockLiveUndockPromotionCommitExecution::SameWindow(
                DockLiveUndockSameWindowPromotionCommit {
                    identity,
                    token,
                    destination,
                    surface_revision,
                    controller,
                    prepared_graph,
                    graph_commit: None,
                    topology_recovery_required: false,
                    runtime,
                    viewport,
                    committed_viewport: None,
                    retained_release,
                    source_host,
                    source,
                    source_retirement: DockLiveUndockSourceRetirementStage::Pending,
                    destination_host,
                    destination_host_promotion,
                    destination_promotion: None,
                    presentation: Some(presentation),
                    presentation_batch: None,
                    provider_post_commit: None,
                    provider_refreshed: false,
                    reveal,
                    provisional_session,
                    semantics,
                    surface: None,
                    publication: None,
                    presentation_session_retired: false,
                    controller_notified: false,
                    source_host_notified: false,
                    destination_host_notified: false,
                    viewport_refreshed: false,
                },
            )),
        }
    }

    fn host(prepared: DockLiveUndockPreflightedHostPromotionExecution) -> Self {
        let DockLiveUndockPreflightedHostPromotionExecution {
            identity,
            token,
            destination,
            surface_revision,
            runtime,
            drop,
            target_window,
            target_host,
            target_binding,
            target_registration,
            presentation_cleanup,
        } = prepared;
        let presentation_cleanup = presentation_cleanup.map(|cleanup| {
            let DockLiveUndockPreparedHostPromotionPresentationCleanup {
                payload_lease,
                presentation_generation,
                presentation,
                source_host,
                provisional_host,
                retained_release,
            } = cleanup;
            DockLiveUndockHostPromotionCleanupCommit {
                payload_lease,
                presentation_generation,
                presentation: Some(presentation),
                presentation_committed: false,
                provider_post_commit: None,
                provider_refreshed: false,
                source_host,
                source_host_committed: false,
                provisional_host,
                provisional_host_committed: false,
                retained_release,
                session_retired: false,
            }
        });
        Self {
            identity,
            token,
            destination,
            surface_revision,
            boundary: Cell::new(DockLiveUndockPromotionCommitBoundary::Reversible),
            drive: Cell::new(DockLiveUndockPromotionCommitDriveState::Idle),
            recovery_receipt: Cell::new(None),
            recovery_requires_window_terminal: Cell::new(false),
            execution: RefCell::new(DockLiveUndockPromotionCommitExecution::Host(
                DockLiveUndockHostPromotionCommit {
                    identity,
                    token,
                    destination,
                    surface_revision,
                    runtime,
                    drop,
                    committed_drop: None,
                    target_window,
                    target_host,
                    target_binding,
                    target_registration,
                    presentation_cleanup,
                    surface: None,
                    publication: None,
                    host_drop_notified: false,
                    committed_destination_recovery_required: false,
                    lower_receipt_retired: false,
                },
            )),
        }
    }

    const fn identity(&self) -> DockLiveUndockIdentity {
        self.identity
    }

    const fn token(&self) -> DockLiveUndockPromotionToken {
        self.token
    }

    const fn destination(&self) -> DockLiveUndockPromotionDestination {
        self.destination
    }

    const fn surface_revision(&self) -> u64 {
        self.surface_revision
    }

    fn begin_commit_call(&self) -> bool {
        match self.boundary.get() {
            DockLiveUndockPromotionCommitBoundary::Reversible => {
                self.boundary
                    .set(DockLiveUndockPromotionCommitBoundary::InFlight);
                true
            }
            DockLiveUndockPromotionCommitBoundary::InFlight
            | DockLiveUndockPromotionCommitBoundary::Irreversible => true,
            DockLiveUndockPromotionCommitBoundary::AbortClaimed => false,
        }
    }

    fn confirm_irreversible(&self) {
        assert!(matches!(
            self.boundary.get(),
            DockLiveUndockPromotionCommitBoundary::InFlight
                | DockLiveUndockPromotionCommitBoundary::Irreversible
        ));
        self.boundary
            .set(DockLiveUndockPromotionCommitBoundary::Irreversible);
    }

    fn resolve_in_flight_as_reversible(&self) {
        if self.boundary.get() == DockLiveUndockPromotionCommitBoundary::InFlight {
            self.boundary
                .set(DockLiveUndockPromotionCommitBoundary::Reversible);
        }
    }

    fn claim_abort(&self) -> bool {
        if self.boundary.get() != DockLiveUndockPromotionCommitBoundary::Reversible {
            return false;
        }
        self.boundary
            .set(DockLiveUndockPromotionCommitBoundary::AbortClaimed);
        true
    }

    fn crossed_commit_boundary(&self) -> bool {
        matches!(
            self.boundary.get(),
            DockLiveUndockPromotionCommitBoundary::InFlight
                | DockLiveUndockPromotionCommitBoundary::Irreversible
        )
    }

    fn has_irreversible_authority(&self) -> bool {
        matches!(
            self.boundary.get(),
            DockLiveUndockPromotionCommitBoundary::Irreversible
        )
    }

    fn abort_was_claimed(&self) -> bool {
        self.boundary.get() == DockLiveUndockPromotionCommitBoundary::AbortClaimed
    }

    fn take_pending_preparation(&self) -> Option<DockLiveUndockPreparedPromotionExecution> {
        let mut execution = self.execution.borrow_mut();
        let DockLiveUndockPromotionCommitExecution::Pending(prepared) = &mut *execution else {
            return None;
        };
        prepared.take()
    }

    fn install_preflighted(&self, preflighted: Self) -> bool {
        assert_eq!(preflighted.identity, self.identity);
        assert_eq!(preflighted.token, self.token);
        assert_eq!(preflighted.destination, self.destination);
        assert_eq!(preflighted.surface_revision, self.surface_revision);
        if self.abort_was_claimed() {
            self.abort_execution();
            return false;
        }
        let mut execution = self.execution.borrow_mut();
        assert!(matches!(
            &*execution,
            DockLiveUndockPromotionCommitExecution::Pending(None)
        ));
        *execution = preflighted.execution.into_inner();
        true
    }

    fn abort_execution(&self) -> bool {
        if !matches!(
            self.boundary.get(),
            DockLiveUndockPromotionCommitBoundary::Reversible
                | DockLiveUndockPromotionCommitBoundary::AbortClaimed
        ) {
            return false;
        }
        self.boundary
            .set(DockLiveUndockPromotionCommitBoundary::AbortClaimed);
        self.drive
            .set(DockLiveUndockPromotionCommitDriveState::Terminal);
        *self.execution.borrow_mut() = DockLiveUndockPromotionCommitExecution::Aborted;
        true
    }

    fn restore_pending_preparation(&self, prepared: DockLiveUndockPreparedPromotionExecution) {
        let mut execution = self.execution.borrow_mut();
        assert!(matches!(
            &*execution,
            DockLiveUndockPromotionCommitExecution::Pending(None)
        ));
        *execution = DockLiveUndockPromotionCommitExecution::Pending(Some(prepared));
    }

    fn begin_drive(&self) -> bool {
        match self.drive.get() {
            DockLiveUndockPromotionCommitDriveState::Idle => {}
            DockLiveUndockPromotionCommitDriveState::Driving
            | DockLiveUndockPromotionCommitDriveState::Terminal => return false,
        }
        self.drive
            .set(DockLiveUndockPromotionCommitDriveState::Driving);
        true
    }

    fn finish_drive(&self) {
        self.drive
            .set(DockLiveUndockPromotionCommitDriveState::Terminal);
    }

    fn finish_drive_for_recovery(&self) {
        assert!(matches!(
            self.boundary.get(),
            DockLiveUndockPromotionCommitBoundary::InFlight
                | DockLiveUndockPromotionCommitBoundary::Irreversible
        ));
        self.boundary
            .set(DockLiveUndockPromotionCommitBoundary::Irreversible);
        self.finish_drive();
    }

    fn record_recovery(
        &self,
        receipt: DockPayloadRecoveryCommitReceipt,
        requires_window_terminal: bool,
    ) -> bool {
        if let Some(current) = self.recovery_receipt.get() {
            return current == receipt
                && self.recovery_requires_window_terminal.get() == requires_window_terminal;
        }
        self.recovery_receipt.set(Some(receipt));
        self.recovery_requires_window_terminal
            .set(requires_window_terminal);
        true
    }

    fn recovery_receipt(&self) -> Option<DockPayloadRecoveryCommitReceipt> {
        self.recovery_receipt.get()
    }

    fn recovery_requires_window_terminal(&self) -> bool {
        self.recovery_requires_window_terminal.get()
    }

    fn claim_abort_or_observe(&self) -> DockLiveUndockPromotionCommitDisposition {
        match self.boundary.get() {
            DockLiveUndockPromotionCommitBoundary::Reversible => {
                assert!(self.claim_abort());
                DockLiveUndockPromotionCommitDisposition::AbortClaimed
            }
            DockLiveUndockPromotionCommitBoundary::AbortClaimed => {
                DockLiveUndockPromotionCommitDisposition::AbortClaimed
            }
            DockLiveUndockPromotionCommitBoundary::InFlight => {
                DockLiveUndockPromotionCommitDisposition::ForwardOnly {
                    identity: self.identity,
                    token: self.token,
                    destination: self.destination,
                }
            }
            DockLiveUndockPromotionCommitBoundary::Irreversible => {
                // Crossing the lower commit boundary is not the same as publishing the aggregate
                // durable promotion. Shutdown must wait for the exact durable or failure fact
                // while the journal still owns unfinished forward-settlement authority.
                DockLiveUndockPromotionCommitDisposition::ForwardOnly {
                    identity: self.identity,
                    token: self.token,
                    destination: self.destination,
                }
            }
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

    const fn committed_destination_recovery_required(&self) -> bool {
        match self {
            Self::SameWindow(durable) => durable.topology_recovery_required,
            Self::Host(durable) => durable.committed_destination_recovery_required,
        }
    }

    fn post_commit(&self) -> &DockLiveUndockPostCommitReceipt {
        match self {
            Self::SameWindow(durable) => &durable.post_commit,
            Self::Host(durable) => &durable.post_commit,
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
    retire_next_same_window_graph_commit_before_semantics_ack: bool,
    #[cfg(test)]
    terminate_next_same_window_destination_before_semantics_ack: bool,
    #[cfg(test)]
    suppress_same_window_destination_semantics_frames: u32,
    #[cfg(test)]
    before_destination_interaction_admission_test_hook: Option<Box<dyn FnOnce(&mut App)>>,
    #[cfg(test)]
    before_destination_interaction_activation_test_hook: Option<Box<dyn FnOnce(&mut App)>>,
    #[cfg(test)]
    after_same_window_provider_commit_test_hook: Option<Box<dyn FnOnce(&mut App)>>,
    #[cfg(test)]
    after_same_window_viewport_commit_test_hook: Option<Box<dyn FnOnce(&mut App)>>,
    #[cfg(test)]
    after_host_drop_commit_test_hook: Option<Box<dyn FnOnce(&mut App)>>,
    #[cfg(test)]
    panic_next_committed_destination_recovery_attempt: bool,
    #[cfg(test)]
    panic_next_same_window_post_commit_refresh: bool,
    #[cfg(test)]
    same_window_post_commit_refresh_attempts: u32,
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

    /// Linearizes shutdown against the exact in-flight promotion commit.
    ///
    /// A reversible journal is claimed for abort before the reducer can schedule source
    /// restoration. An irreversible journal or an already durable runtime publication is returned
    /// as exact committed-loss authority.
    pub(crate) fn claim_promotion_commit_for_shutdown(
        &self,
        identity: DockLiveUndockIdentity,
    ) -> DockLiveUndockPromotionCommitDisposition {
        let state = self.state.borrow();
        let Some(execution) = state.executions.get(&identity) else {
            return DockLiveUndockPromotionCommitDisposition::RollbackAllowed;
        };
        match execution.promotion.as_ref() {
            Some(DockLiveUndockPromotionExecution::Prepared(prepared))
                if prepared.identity() == identity
                    && prepared.provider_destination_already_committed() =>
            {
                DockLiveUndockPromotionCommitDisposition::ForwardOnly {
                    identity,
                    token: prepared.token(),
                    destination: prepared.destination(),
                }
            }
            Some(DockLiveUndockPromotionExecution::Committing(journal))
                if journal.identity() == identity =>
            {
                journal.claim_abort_or_observe()
            }
            Some(DockLiveUndockPromotionExecution::Durable(durable))
                if durable.identity() == identity =>
            {
                DockLiveUndockPromotionCommitDisposition::Durable {
                    identity,
                    token: durable.token(),
                    destination: durable.destination(),
                }
            }
            Some(
                DockLiveUndockPromotionExecution::Prepared(_)
                | DockLiveUndockPromotionExecution::Committing(_)
                | DockLiveUndockPromotionExecution::Durable(_),
            )
            | None => DockLiveUndockPromotionCommitDisposition::RollbackAllowed,
        }
    }

    fn promotion_commit_forbids_rollback(&self, identity: DockLiveUndockIdentity) -> bool {
        self.state
            .borrow()
            .executions
            .get(&identity)
            .and_then(|execution| execution.promotion.as_ref())
            .is_some_and(|promotion| match promotion {
                DockLiveUndockPromotionExecution::Committing(journal) => {
                    journal.identity() == identity && journal.crossed_commit_boundary()
                }
                DockLiveUndockPromotionExecution::Durable(durable) => {
                    durable.identity() == identity
                }
                DockLiveUndockPromotionExecution::Prepared(prepared) => {
                    prepared.identity() == identity
                        && prepared.provider_destination_already_committed()
                }
            })
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
    pub(crate) fn retire_next_same_window_graph_commit_before_semantics_ack_for_test(&self) {
        self.state
            .borrow_mut()
            .retire_next_same_window_graph_commit_before_semantics_ack = true;
    }

    #[cfg(test)]
    pub(crate) fn after_same_window_provider_commit_for_test(
        &self,
        hook: impl FnOnce(&mut App) + 'static,
    ) {
        self.state
            .borrow_mut()
            .after_same_window_provider_commit_test_hook = Some(Box::new(hook));
    }

    #[cfg(test)]
    pub(crate) fn after_same_window_viewport_commit_for_test(
        &self,
        hook: impl FnOnce(&mut App) + 'static,
    ) {
        self.state
            .borrow_mut()
            .after_same_window_viewport_commit_test_hook = Some(Box::new(hook));
    }

    #[cfg(test)]
    pub(crate) fn after_host_drop_commit_for_test(&self, hook: impl FnOnce(&mut App) + 'static) {
        self.state.borrow_mut().after_host_drop_commit_test_hook = Some(Box::new(hook));
    }

    #[cfg(test)]
    fn run_after_promotion_final_swap_hooks(&self, cx: &mut App) {
        let hooks = {
            let mut state = self.state.borrow_mut();
            [
                state.after_same_window_provider_commit_test_hook.take(),
                state.after_same_window_viewport_commit_test_hook.take(),
                state.after_host_drop_commit_test_hook.take(),
            ]
        };
        if hooks.iter().all(Option::is_none) {
            return;
        }
        for hook in hooks.into_iter().flatten() {
            hook(cx);
        }
    }

    #[cfg(test)]
    pub(crate) fn panic_next_committed_destination_recovery_attempt_for_test(&self) {
        self.state
            .borrow_mut()
            .panic_next_committed_destination_recovery_attempt = true;
    }

    #[cfg(test)]
    pub(crate) fn panic_next_same_window_post_commit_refresh_for_test(&self) {
        self.state
            .borrow_mut()
            .panic_next_same_window_post_commit_refresh = true;
    }

    #[cfg(test)]
    pub(crate) fn same_window_post_commit_refresh_attempts_for_test(&self) -> u32 {
        self.state.borrow().same_window_post_commit_refresh_attempts
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

    pub(crate) fn committed_destination_logical_close_authority(
        &self,
        identity: DockLiveUndockIdentity,
        window_id: open_gpui::WindowId,
    ) -> Option<DockLiveUndockLogicalCloseAuthority> {
        let mut state = self.state.borrow_mut();
        let execution = state.executions.get_mut(&identity)?;
        match execution.promotion.as_mut()? {
            DockLiveUndockPromotionExecution::Durable(
                DockLiveUndockDurablePromotionExecution::SameWindow(durable),
            ) if durable.identity == identity
                && durable.destination_window.window_id() == window_id =>
            {
                Some(DockLiveUndockLogicalCloseAuthority::Durable(
                    durable.registration.clone(),
                ))
            }
            DockLiveUndockPromotionExecution::Durable(
                DockLiveUndockDurablePromotionExecution::Host(durable),
            ) if durable.identity == identity
                && durable.destination_window.window_id() == window_id =>
            {
                Some(DockLiveUndockLogicalCloseAuthority::Durable(
                    durable.registration.clone(),
                ))
            }
            DockLiveUndockPromotionExecution::Committing(journal)
                if journal.identity() == identity
                    && journal.destination().window_id() == window_id =>
            {
                let mut journal_execution = journal.execution.borrow_mut();
                match &mut *journal_execution {
                    DockLiveUndockPromotionCommitExecution::SameWindow(commit) => {
                        let registration = commit.committed_viewport.as_ref()?.registration.clone();
                        commit.topology_recovery_required = true;
                        Some(DockLiveUndockLogicalCloseAuthority::ForwardCommitted(
                            registration,
                        ))
                    }
                    DockLiveUndockPromotionCommitExecution::Host(commit) => {
                        commit.committed_drop.as_ref()?;
                        commit.committed_destination_recovery_required = true;
                        Some(DockLiveUndockLogicalCloseAuthority::ForwardCommitted(
                            commit.target_registration.clone(),
                        ))
                    }
                    DockLiveUndockPromotionCommitExecution::Pending(_)
                    | DockLiveUndockPromotionCommitExecution::Aborted => None,
                }
            }
            DockLiveUndockPromotionExecution::Prepared(_)
            | DockLiveUndockPromotionExecution::Committing(_)
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
    pub(crate) fn suppress_same_window_destination_semantics_frames_for_test(
        &self,
        frame_count: u32,
    ) {
        self.state
            .borrow_mut()
            .suppress_same_window_destination_semantics_frames = frame_count;
    }

    #[cfg(test)]
    pub(crate) fn install_before_destination_interaction_admission_hook_for_test(
        &self,
        hook: impl FnOnce(&mut App) + 'static,
    ) {
        let mut state = self.state.borrow_mut();
        assert!(
            state
                .before_destination_interaction_admission_test_hook
                .is_none(),
            "dock live-undock destination-interaction admission test hook is already installed"
        );
        state.before_destination_interaction_admission_test_hook = Some(Box::new(hook));
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
        let finalizer_adopted =
            release_authority_matches && execution_matches && finalizer.begin_live_undock(identity);
        if !finalizer_adopted {
            return DockLiveUndockReleaseAdoption::Rejected(host_release);
        }
        {
            let mut state = self.state.borrow_mut();
            let execution = state
                .executions
                .get_mut(&identity)
                .expect("validated release adoption must retain its execution");
            execution.host_release = host_release.take();
            execution.release_route_generation = Some(release.route_generation());
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
                    .and_then(|execution| {
                        execution.release_route_generation = None;
                        execution.host_release.take()
                    });
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
        #[cfg(test)]
        let promotion_final_swap_accepted = matches!(
            &fact,
            DockLiveUndockFact::DurableSwapCommitted { .. }
                | DockLiveUndockFact::CommittedDestinationRecoveryRequired { .. }
        );
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
                        release_route_generation: None,
                        route_placement: None,
                        release_placement: None,
                        observed_release_placement: None,
                        destination_host: None,
                        host_release: None,
                        presentation: None,
                        promotion: None,
                        destination_semantics_watchdog:
                            DockLiveUndockDestinationSemanticsWatchdog::default(),
                        source_restoration_retry: DockLiveUndockRetryBackoff::default(),
                        orphan_recovery_retry: DockLiveUndockRetryBackoff::default(),
                        committed_destination_recovery_retry: DockLiveUndockRetryBackoff::default(),
                        committed_window_effects_retry: DockLiveUndockRetryBackoff::default(),
                        terminal_requested: false,
                        terminal_settlement_retry: DockLiveUndockRetryBackoff::default(),
                    },
                );
                assert!(
                    previous.is_none(),
                    "one live-undock identity must install one execution"
                );
            }
        }

        #[cfg(test)]
        if promotion_final_swap_accepted {
            self.run_after_promotion_final_swap_hooks(cx);
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
        if let Some(execution) = self.state.borrow_mut().executions.get_mut(&identity) {
            execution.route_placement.take();
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
        let preparation = {
            let mut state = self.state.borrow_mut();
            let Some(execution) = state.executions.get_mut(&identity) else {
                return;
            };
            if execution.request.key() != identity.opening()
                || execution.destination_host.map(Into::into) != Some(window)
                || execution.release_placement.is_some()
            {
                None
            } else {
                execution.route_placement.take();
                execution.release_placement = Some(DockLiveUndockReleasePlacementExecution {
                    window_id: window.window_id(),
                    generation,
                    subscription: None,
                });
                Some(execution.request.provisional_session().clone())
            }
        };
        let Some(provisional_session) = preparation else {
            return;
        };

        let desired = release.desired_bounds();
        let request = {
            let (Ok(width), Ok(height)) = (
                i32::try_from(desired.width()),
                i32::try_from(desired.height()),
            ) else {
                self.observe_release_placement(
                    identity,
                    window.window_id(),
                    generation,
                    super::live_undock::DockLiveUndockPlacementOutcome::Rejected,
                    None,
                    None,
                    cx,
                );
                return;
            };
            WindowProvisionalPlacementRequest::try_new(
                generation.get(),
                WindowProvisionalPlacementPurpose::FinalRelease,
                Bounds::new(
                    point(
                        DevicePixels(desired.origin().x()),
                        DevicePixels(desired.origin().y()),
                    ),
                    size(DevicePixels(width), DevicePixels(height)),
                ),
                point(
                    DevicePixels(release.point().x()),
                    DevicePixels(release.point().y()),
                ),
                desired.target_display(),
            )
        };
        let Some(request) = request else {
            self.observe_release_placement(
                identity,
                window.window_id(),
                generation,
                super::live_undock::DockLiveUndockPlacementOutcome::Rejected,
                None,
                None,
                cx,
            );
            return;
        };

        let requested = match window.update(cx, |_, window, _| {
            window.request_provisional_placement(&provisional_session, request)
        }) {
            Ok(Ok(requested)) => requested,
            Ok(Err(_)) => {
                self.observe_release_placement(
                    identity,
                    window.window_id(),
                    generation,
                    super::live_undock::DockLiveUndockPlacementOutcome::Rejected,
                    None,
                    None,
                    cx,
                );
                return;
            }
            Err(_) => {
                self.observe_release_placement(
                    identity,
                    window.window_id(),
                    generation,
                    super::live_undock::DockLiveUndockPlacementOutcome::WindowClosed,
                    None,
                    None,
                    cx,
                );
                return;
            }
        };
        let (dispatch, placement_ticket) = requested;

        match dispatch {
            WindowMutationDispatch::Queued(ticket) => {
                let runtime = self.clone();
                let async_cx = cx.to_async();
                let subscription = ticket.subscribe(move |observation| {
                    let mut outcome = Self::dock_placement_outcome(observation.outcome);
                    let placement_snapshot = placement_ticket.snapshot();
                    let final_placement =
                        DockLiveUndockFinalPlacementReceipt::new(placement_snapshot);
                    if matches!(
                        outcome,
                        super::live_undock::DockLiveUndockPlacementOutcome::Exact
                            | super::live_undock::DockLiveUndockPlacementOutcome::Adjusted
                    ) {
                        outcome = match (
                            final_placement,
                            placement_snapshot
                                .native_facts()
                                .map(|facts| facts.z_order()),
                        ) {
                            (Some(_), Some(open_gpui::WindowProvisionalRevealZOrder::Exact)) => {
                                outcome
                            }
                            (Some(_), Some(open_gpui::WindowProvisionalRevealZOrder::Adjusted)) => {
                                super::live_undock::DockLiveUndockPlacementOutcome::Adjusted
                            }
                            _ => super::live_undock::DockLiveUndockPlacementOutcome::Rejected,
                        };
                    }
                    let facts = final_placement
                        .is_some()
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
                                    final_placement,
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
            WindowMutationDispatch::Unchanged => {
                let snapshot = placement_ticket.snapshot();
                let final_placement = DockLiveUndockFinalPlacementReceipt::new(snapshot);
                let outcome = if final_placement.is_some() {
                    super::live_undock::DockLiveUndockPlacementOutcome::Exact
                } else {
                    super::live_undock::DockLiveUndockPlacementOutcome::Rejected
                };
                let facts = final_placement.and_then(|_| {
                    window
                        .update(cx, |_, window, _| {
                            DockViewportWindowFacts::from_platform_facts(window.platform_facts())
                        })
                        .ok()
                });
                self.observe_release_placement(
                    identity,
                    window.window_id(),
                    generation,
                    outcome,
                    facts,
                    final_placement,
                    cx,
                );
            }
            WindowMutationDispatch::Unsupported | WindowMutationDispatch::Rejected => self
                .observe_release_placement(
                    identity,
                    window.window_id(),
                    generation,
                    super::live_undock::DockLiveUndockPlacementOutcome::Rejected,
                    None,
                    None,
                    cx,
                ),
            WindowMutationDispatch::WindowClosed => self.observe_release_placement(
                identity,
                window.window_id(),
                generation,
                super::live_undock::DockLiveUndockPlacementOutcome::WindowClosed,
                None,
                None,
                cx,
            ),
        }
    }

    fn request_route_placement(
        &self,
        identity: DockLiveUndockIdentity,
        window: AnyWindowHandle,
        generation: DockLiveUndockRouteGeneration,
        route_point: super::live_undock::DockLiveUndockPhysicalPoint,
        bounds: DockLiveUndockPhysicalBounds,
        cx: &mut App,
    ) {
        let (Ok(width), Ok(height)) = (
            i32::try_from(bounds.width()),
            i32::try_from(bounds.height()),
        ) else {
            return;
        };
        let Some(request) = WindowProvisionalPlacementRequest::try_new(
            generation.get(),
            WindowProvisionalPlacementPurpose::LiveRoute,
            Bounds::new(
                point(
                    DevicePixels(bounds.origin().x()),
                    DevicePixels(bounds.origin().y()),
                ),
                size(DevicePixels(width), DevicePixels(height)),
            ),
            point(DevicePixels(route_point.x()), DevicePixels(route_point.y())),
            bounds.target_display(),
        ) else {
            return;
        };

        let provisional_session = {
            let mut state = self.state.borrow_mut();
            let Some(execution) = state.executions.get_mut(&identity) else {
                return;
            };
            if execution.request.key() != identity.opening()
                || execution.destination_host.map(Into::into) != Some(window)
                || execution.release_route_generation.is_some()
                || execution.release_placement.is_some()
                || execution.promotion.is_some()
                || execution
                    .route_placement
                    .as_ref()
                    .is_some_and(|current| current.generation >= generation)
            {
                return;
            }
            execution.route_placement = Some(DockLiveUndockRoutePlacementExecution {
                window_id: window.window_id(),
                generation,
                mutation_generation: None,
                subscription: None,
            });
            execution.request.provisional_session().clone()
        };

        let (dispatch, placement_ticket) = match window.update(cx, |_, window, _| {
            window.request_provisional_live_placement(&provisional_session, request)
        }) {
            Ok(Ok(requested)) => requested,
            Ok(Err(_)) => {
                self.observe_route_placement(
                    identity,
                    window.window_id(),
                    generation,
                    None,
                    DockLiveUndockRoutePlacementOutcome::Rejected,
                    cx,
                );
                return;
            }
            Err(_) => {
                self.observe_route_placement(
                    identity,
                    window.window_id(),
                    generation,
                    None,
                    DockLiveUndockRoutePlacementOutcome::WindowClosed,
                    cx,
                );
                return;
            }
        };

        match dispatch {
            WindowMutationDispatch::Queued(ticket) => {
                let mutation_generation = ticket.generation();
                let installed = self
                    .state
                    .borrow_mut()
                    .executions
                    .get_mut(&identity)
                    .and_then(|execution| execution.route_placement.as_mut())
                    .filter(|current| {
                        current.window_id == window.window_id()
                            && current.generation == generation
                            && current.mutation_generation.is_none()
                    })
                    .is_some_and(|current| {
                        current.mutation_generation = Some(mutation_generation);
                        true
                    });
                if !installed {
                    return;
                }
                let runtime = self.clone();
                let async_cx = cx.to_async();
                let placement_ticket_for_observer = placement_ticket.clone();
                let subscription = ticket.subscribe(move |observation| {
                    let outcome = Self::dock_route_placement_outcome(
                        observation.outcome,
                        Some(observation.generation),
                        placement_ticket_for_observer.snapshot(),
                    );
                    async_cx
                        .spawn(async move |cx| {
                            cx.update(|cx| {
                                runtime.observe_route_placement(
                                    identity,
                                    window.window_id(),
                                    generation,
                                    Some(observation.generation),
                                    outcome,
                                    cx,
                                );
                            });
                        })
                        .detach();
                });
                let mut state = self.state.borrow_mut();
                if let Some(current) = state
                    .executions
                    .get_mut(&identity)
                    .and_then(|execution| execution.route_placement.as_mut())
                    .filter(|current| {
                        current.window_id == window.window_id()
                            && current.generation == generation
                            && current.mutation_generation == Some(mutation_generation)
                    })
                {
                    current.subscription = Some(subscription);
                }
            }
            WindowMutationDispatch::Unchanged => {
                let snapshot = placement_ticket.snapshot();
                self.observe_route_placement(
                    identity,
                    window.window_id(),
                    generation,
                    snapshot.mutation_generation(),
                    Self::dock_route_placement_outcome(
                        WindowMutationOutcome::Exact,
                        snapshot.mutation_generation(),
                        snapshot,
                    ),
                    cx,
                );
            }
            WindowMutationDispatch::Unsupported => self.observe_route_placement(
                identity,
                window.window_id(),
                generation,
                placement_ticket.snapshot().mutation_generation(),
                DockLiveUndockRoutePlacementOutcome::Unsupported,
                cx,
            ),
            WindowMutationDispatch::Rejected => self.observe_route_placement(
                identity,
                window.window_id(),
                generation,
                placement_ticket.snapshot().mutation_generation(),
                DockLiveUndockRoutePlacementOutcome::Rejected,
                cx,
            ),
            WindowMutationDispatch::WindowClosed => self.observe_route_placement(
                identity,
                window.window_id(),
                generation,
                placement_ticket.snapshot().mutation_generation(),
                DockLiveUndockRoutePlacementOutcome::WindowClosed,
                cx,
            ),
        }
    }

    fn dock_route_placement_outcome(
        outcome: WindowMutationOutcome,
        mutation_generation: Option<u64>,
        snapshot: WindowProvisionalPlacementSnapshot,
    ) -> DockLiveUndockRoutePlacementOutcome {
        match outcome {
            WindowMutationOutcome::Exact | WindowMutationOutcome::Adjusted => {
                let Some(native) = snapshot.native_facts() else {
                    return DockLiveUndockRoutePlacementOutcome::Rejected;
                };
                if snapshot.purpose() != WindowProvisionalPlacementPurpose::LiveRoute
                    || snapshot.mutation_generation() != mutation_generation
                    || snapshot.outcome() != WindowProvisionalPlacementOutcome::Settled
                    || !native.accepts_placement()
                {
                    return DockLiveUndockRoutePlacementOutcome::Rejected;
                }
                if outcome == WindowMutationOutcome::Adjusted
                    || native.z_order() == WindowProvisionalRevealZOrder::Adjusted
                {
                    DockLiveUndockRoutePlacementOutcome::Adjusted
                } else {
                    DockLiveUndockRoutePlacementOutcome::Exact
                }
            }
            WindowMutationOutcome::Superseded => DockLiveUndockRoutePlacementOutcome::Superseded,
            WindowMutationOutcome::Rejected => DockLiveUndockRoutePlacementOutcome::Rejected,
            WindowMutationOutcome::Unsupported => DockLiveUndockRoutePlacementOutcome::Unsupported,
            WindowMutationOutcome::WindowClosed => {
                DockLiveUndockRoutePlacementOutcome::WindowClosed
            }
        }
    }

    fn observe_route_placement(
        &self,
        identity: DockLiveUndockIdentity,
        window_id: WindowId,
        generation: DockLiveUndockRouteGeneration,
        mutation_generation: Option<u64>,
        outcome: DockLiveUndockRoutePlacementOutcome,
        cx: &mut App,
    ) {
        let current = self
            .state
            .borrow_mut()
            .executions
            .get_mut(&identity)
            .is_some_and(|execution| {
                let current = execution.route_placement.as_ref().is_some_and(|placement| {
                    placement.window_id == window_id
                        && placement.generation == generation
                        && placement.mutation_generation == mutation_generation
                });
                if current {
                    execution.route_placement.take();
                }
                current
            });
        if !current {
            return;
        }
        let fact = if outcome == DockLiveUndockRoutePlacementOutcome::WindowClosed {
            DockLiveUndockFact::WindowTerminal {
                identity,
                window_id,
            }
        } else {
            DockLiveUndockFact::RoutePlacementObserved {
                identity,
                window_id,
                generation,
                outcome,
            }
        };
        let _ = self.submit(fact, cx);
    }

    fn observe_release_placement(
        &self,
        identity: DockLiveUndockIdentity,
        window_id: WindowId,
        generation: DockLiveUndockPlacementGeneration,
        outcome: super::live_undock::DockLiveUndockPlacementOutcome,
        facts: Option<DockViewportWindowFacts>,
        final_placement: Option<DockLiveUndockFinalPlacementReceipt>,
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
                        facts.zip(final_placement).map(|(facts, final_placement)| {
                            DockLiveUndockObservedReleasePlacement {
                                window_id,
                                generation,
                                facts,
                                final_placement,
                            }
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
                    final_placement,
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
        cx.defer_after_or_shutdown_critical_before_window_registry_clear(
            LIVE_UNDOCK_RELEASE_DEADLINE,
            move |cx| {
                runtime.expire_release_deadline(identity, placement_generation, cx);
            },
        );
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

    fn same_window_destination_semantics_authority(
        &self,
        identity: DockLiveUndockIdentity,
    ) -> Option<DockLiveUndockSameWindowDestinationSemanticsAuthority> {
        let state = self.state.borrow();
        let execution = state.executions.get(&identity)?;
        let DockLiveUndockPromotionExecution::Durable(
            DockLiveUndockDurablePromotionExecution::SameWindow(durable),
        ) = execution.promotion.as_ref()?
        else {
            return None;
        };
        Some(DockLiveUndockSameWindowDestinationSemanticsAuthority {
            identity: durable.identity,
            token: durable.token,
            destination: durable.destination,
            destination_window: durable.destination_window,
            reveal: durable.reveal,
            provisional_session: durable.provisional_session.clone(),
            semantics: durable.semantics.clone(),
            destination_host: durable.destination_host.clone(),
            marker: durable.destination_promotion.semantics().clone(),
            controller: durable.controller.clone(),
            graph_commit: durable.graph_commit,
        })
    }

    fn destination_semantics_watchdog_key(
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
        semantics: &WindowProvisionalSemanticsTicket,
    ) -> DockLiveUndockDestinationSemanticsWatchdogKey {
        let semantics = semantics.snapshot();
        DockLiveUndockDestinationSemanticsWatchdogKey {
            token,
            destination,
            session_generation: semantics.session_generation(),
            placement_mutation_generation: semantics.placement_mutation_generation(),
        }
    }

    fn complete_submitted_destination_semantics(
        &self,
        authority: &DockLiveUndockSameWindowDestinationSemanticsAuthority,
        cx: &mut App,
    ) -> Option<DockLiveUndockDestinationSemanticsReceipt> {
        let semantics = authority.semantics.snapshot();
        let session = authority.provisional_session.snapshot();
        if semantics.outcome() != WindowProvisionalSemanticsOutcome::Submitted
            || session.window_id() != Some(authority.destination_window.window_id())
            || session.generation() != semantics.session_generation()
            || session.phase() != WindowProvisionalSessionPhase::DestinationSemanticsSubmitted
        {
            return None;
        }
        if !workspace_graph_projection_is_exact(&authority.controller, authority.graph_commit, cx) {
            return None;
        }
        let Some(host) = authority.destination_host.upgrade() else {
            return None;
        };
        if !destination_host_semantics_are_exact(&host, &authority.marker, cx) {
            return None;
        }
        let Some(receipt) = DockLiveUndockDestinationSemanticsReceipt::new_same_window_submitted(
            authority.identity,
            authority.token,
            authority.reveal,
            semantics,
        ) else {
            return None;
        };
        if receipt.destination() != authority.destination {
            return None;
        }
        let completed = cx.update_entity(&host, |host, host_cx| {
            host.complete_live_destination_semantics(&authority.marker, host_cx)
        });
        completed.then_some(receipt)
    }

    fn fail_destination_semantics_submission(
        &self,
        identity: DockLiveUndockIdentity,
        key: DockLiveUndockDestinationSemanticsWatchdogKey,
        cx: &mut App,
    ) {
        self.clear_destination_semantics_watchdog(identity, key);
        self.enqueue_fact(
            DockLiveUndockQueuedFact::Reduce(
                DockLiveUndockFact::DestinationSemanticsSubmissionFailed {
                    identity,
                    token: key.token,
                    destination: key.destination,
                },
            ),
            cx,
        );
    }

    fn publish_submitted_destination_semantics(
        &self,
        authority: &DockLiveUndockSameWindowDestinationSemanticsAuthority,
        key: DockLiveUndockDestinationSemanticsWatchdogKey,
        cx: &mut App,
    ) -> bool {
        let Some(receipt) = self.complete_submitted_destination_semantics(authority, cx) else {
            self.fail_destination_semantics_submission(authority.identity, key, cx);
            return false;
        };
        self.clear_destination_semantics_watchdog(authority.identity, key);
        self.enqueue_fact(
            DockLiveUndockQueuedFact::Reduce(DockLiveUndockFact::DestinationSemanticsSubmitted {
                identity: authority.identity,
                receipt,
            }),
            cx,
        );
        true
    }

    fn arm_destination_semantics_watchdog(
        &self,
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
        cx: &mut App,
    ) {
        let armed = {
            let mut state = self.state.borrow_mut();
            let Some(execution) = state.executions.get_mut(&identity) else {
                return;
            };
            let Some(DockLiveUndockPromotionExecution::Durable(
                DockLiveUndockDurablePromotionExecution::SameWindow(durable),
            )) = execution.promotion.as_ref()
            else {
                return;
            };
            if durable.token != token || durable.destination != destination {
                return;
            }
            let key = Self::destination_semantics_watchdog_key(
                durable.token,
                durable.destination,
                &durable.semantics,
            );
            execution
                .destination_semantics_watchdog
                .arm(key)
                .map(|generation| (key, generation))
        };
        let Some((key, generation)) = armed else {
            return;
        };

        let runtime = self.clone();
        cx.defer_after_or_shutdown_critical_before_window_registry_clear(
            LIVE_UNDOCK_DESTINATION_SEMANTICS_WATCHDOG_INTERVAL,
            move |cx| {
                runtime.wake_destination_semantics_watchdog(identity, key, generation, cx);
            },
        );
    }

    fn wake_destination_semantics_watchdog(
        &self,
        identity: DockLiveUndockIdentity,
        key: DockLiveUndockDestinationSemanticsWatchdogKey,
        generation: u64,
        cx: &mut App,
    ) {
        {
            let mut state = self.state.borrow_mut();
            let Some(execution) = state.executions.get_mut(&identity) else {
                return;
            };
            if !execution
                .destination_semantics_watchdog
                .claim(key, generation)
            {
                return;
            }
            let Some(DockLiveUndockPromotionExecution::Durable(
                DockLiveUndockDurablePromotionExecution::SameWindow(durable),
            )) = execution.promotion.as_ref()
            else {
                execution.destination_semantics_watchdog.clear();
                return;
            };
            if Self::destination_semantics_watchdog_key(
                durable.token,
                durable.destination,
                &durable.semantics,
            ) != key
            {
                execution.destination_semantics_watchdog.clear();
                return;
            }
        }
        let Some(authority) = self.same_window_destination_semantics_authority(identity) else {
            return;
        };

        let semantics = authority.semantics.snapshot();
        if semantics.outcome() == WindowProvisionalSemanticsOutcome::Submitted {
            self.publish_submitted_destination_semantics(&authority, key, cx);
            return;
        }
        if matches!(
            semantics.outcome(),
            WindowProvisionalSemanticsOutcome::Rejected
                | WindowProvisionalSemanticsOutcome::WindowTerminal
        ) {
            self.fail_destination_semantics_submission(identity, key, cx);
            return;
        }
        let session = authority.provisional_session.snapshot();
        let waiting_phase_is_exact = match (semantics.outcome(), session.phase()) {
            (
                WindowProvisionalSemanticsOutcome::Pending,
                WindowProvisionalSessionPhase::ProjectingDestinationSemantics,
            ) => {
                semantics.accepted_frame_generation().is_none()
                    && semantics.submitted_frame_generation().is_none()
            }
            (
                WindowProvisionalSemanticsOutcome::Accepted,
                WindowProvisionalSessionPhase::DestinationSemanticsAccepted,
            ) => {
                semantics.accepted_frame_generation().is_some()
                    && semantics.submitted_frame_generation().is_none()
            }
            _ => false,
        };
        let authority_is_live = waiting_phase_is_exact
            && session.window_id() == Some(authority.destination_window.window_id())
            && session.generation() == key.session_generation
            && semantics.session_generation() == key.session_generation
            && semantics.destination_generation() == key.token.get()
            && semantics.placement_mutation_generation() == key.placement_mutation_generation
            && workspace_graph_projection_is_exact(
                &authority.controller,
                authority.graph_commit,
                cx,
            )
            && authority.destination_host.upgrade().is_some_and(|host| {
                cx.read_entity(&host, |host, _| {
                    host.accepts_live_destination_semantics(&authority.marker)
                })
            });
        let destination_is_live = authority_is_live
            && match semantics.outcome() {
                WindowProvisionalSemanticsOutcome::Pending => authority
                    .destination_window
                    .update(cx, |_, window, _| window.refresh())
                    .is_ok(),
                WindowProvisionalSemanticsOutcome::Accepted => authority
                    .destination_window
                    .update(cx, |_, window, _| {
                        window.request_provisional_destination_semantics_presentation(
                            &authority.provisional_session,
                            &authority.semantics,
                        )
                    })
                    .is_ok_and(|result| result.is_ok()),
                WindowProvisionalSemanticsOutcome::Submitted
                | WindowProvisionalSemanticsOutcome::Rejected
                | WindowProvisionalSemanticsOutcome::WindowTerminal => false,
            };
        if destination_is_live {
            self.arm_destination_semantics_watchdog(identity, key.token, key.destination, cx);
        } else {
            self.fail_destination_semantics_submission(identity, key, cx);
        }
    }

    fn clear_destination_semantics_watchdog(
        &self,
        identity: DockLiveUndockIdentity,
        key: DockLiveUndockDestinationSemanticsWatchdogKey,
    ) {
        if let Some(execution) = self.state.borrow_mut().executions.get_mut(&identity) {
            execution.destination_semantics_watchdog.clear_if(key);
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
        cx.defer_after_or_shutdown_critical_before_window_registry_clear(delay, move |cx| {
            runtime.retry_source_restoration(identity, source, payload_lease, retry_generation, cx);
        });
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
        cx.defer_after_or_shutdown_critical_before_window_registry_clear(delay, move |cx| {
            runtime.retry_orphan_recovery(
                identity,
                payload_lease,
                provisional,
                retry_generation,
                cx,
            );
        });
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
        cx.defer_after_or_shutdown_critical_before_window_registry_clear(delay, move |cx| {
            runtime.retry_committed_destination_recovery(
                identity,
                authority,
                token,
                destination,
                retry_generation,
                cx,
            );
        });
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

    fn schedule_committed_window_effects_retry(
        &self,
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
        cx: &mut App,
    ) {
        let Some((retry_generation, delay)) = self
            .state
            .borrow_mut()
            .executions
            .get_mut(&identity)
            .and_then(|execution| execution.committed_window_effects_retry.arm_if_idle())
        else {
            return;
        };
        let runtime = self.clone();
        cx.defer_after_or_shutdown_critical_before_window_registry_clear(delay, move |cx| {
            runtime.retry_committed_window_effects(
                identity,
                token,
                destination,
                retry_generation,
                cx,
            );
        });
    }

    fn retry_committed_window_effects(
        &self,
        identity: DockLiveUndockIdentity,
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
                    .committed_window_effects_retry
                    .claim(retry_generation)
                    && matches!(
                        execution.promotion.as_ref(),
                        Some(DockLiveUndockPromotionExecution::Durable(
                            DockLiveUndockDurablePromotionExecution::Host(durable),
                        )) if durable.token == token
                            && durable.destination == destination
                            && durable.host_drop_commit.window_effects_receipt().is_none()
                    )
            });
        if current {
            self.enqueue_effects(
                DockLiveUndockEffects::single(
                    DockLiveUndockEffect::ApplyCommittedHostWindowEffects {
                        identity,
                        token,
                        destination,
                    },
                ),
                cx,
            );
        }
    }

    fn schedule_terminal_settlement_retry(&self, identity: DockLiveUndockIdentity, cx: &mut App) {
        let Some((retry_generation, delay)) = self
            .state
            .borrow_mut()
            .executions
            .get_mut(&identity)
            .filter(|execution| execution.terminal_requested)
            .and_then(|execution| execution.terminal_settlement_retry.arm_if_idle())
        else {
            return;
        };
        let runtime = self.clone();
        cx.defer_after_or_shutdown_critical_before_window_registry_clear(delay, move |cx| {
            runtime.retry_terminal_settlement(identity, retry_generation, cx);
        });
    }

    fn drive_same_window_post_commit_journal(
        &self,
        journal: &Rc<DockLiveUndockPromotionCommitJournal>,
        cx: &mut App,
        first_panic: &mut Option<Box<dyn Any + Send>>,
    ) -> bool {
        let provider_post_commit = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::SameWindow(commit) = &*execution else {
                return false;
            };
            (!commit.provider_refreshed)
                .then(|| commit.provider_post_commit.as_ref())
                .flatten()
                .cloned()
        };
        if let Some(provider_post_commit) = provider_post_commit {
            let completed =
                record_post_commit_panic(first_panic, || provider_post_commit.publish(cx));
            if completed.is_some()
                && let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
                    &mut *journal.execution.borrow_mut()
            {
                commit.provider_refreshed = true;
            }
        }

        let controller = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::SameWindow(commit) = &*execution else {
                return false;
            };
            (!commit.controller_notified).then(|| commit.controller.clone())
        };
        if let Some(controller) = controller
            && notify_post_commit_entity(first_panic, &controller, cx)
            && let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
                &mut *journal.execution.borrow_mut()
        {
            commit.controller_notified = true;
        }

        let source_host = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::SameWindow(commit) = &*execution else {
                return false;
            };
            (!commit.source_host_notified).then(|| commit.source_host.clone())
        };
        if let Some(source_host) = source_host
            && notify_post_commit_entity(first_panic, &source_host, cx)
            && let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
                &mut *journal.execution.borrow_mut()
        {
            commit.source_host_notified = true;
        }

        let destination_host = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::SameWindow(commit) = &*execution else {
                return false;
            };
            (!commit.destination_host_notified).then(|| commit.destination_host.clone())
        };
        if let Some(destination_host) = destination_host
            && notify_post_commit_entity(first_panic, &destination_host, cx)
            && let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
                &mut *journal.execution.borrow_mut()
        {
            commit.destination_host_notified = true;
        }

        let viewport = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::SameWindow(commit) = &*execution else {
                return false;
            };
            (!commit.viewport_refreshed)
                .then(|| {
                    commit
                        .committed_viewport
                        .clone()
                        .map(|committed| (commit.runtime.clone(), committed))
                })
                .flatten()
        };
        if let Some((runtime, committed_viewport)) = viewport {
            let completed = record_post_commit_panic(first_panic, || {
                #[cfg(test)]
                {
                    let mut state = self.state.borrow_mut();
                    state.same_window_post_commit_refresh_attempts += 1;
                    if std::mem::take(&mut state.panic_next_same_window_post_commit_refresh) {
                        panic!("injected same-window post-commit refresh panic");
                    }
                }
                runtime.refresh_live_undock_promotion_commit(&committed_viewport, cx)
            });
            if completed.is_some()
                && let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
                    &mut *journal.execution.borrow_mut()
            {
                commit.viewport_refreshed = true;
            }
        }

        let publication = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::SameWindow(commit) = &*execution else {
                return false;
            };
            commit.publication.clone()
        };
        if let Some(publication) = publication
            && !publication.is_settled()
        {
            record_post_commit_panic(first_panic, || publication.publish(cx));
        }

        let execution = journal.execution.borrow();
        matches!(
            &*execution,
            DockLiveUndockPromotionCommitExecution::SameWindow(commit)
                if commit.presentation_session_retired
                    && commit.provider_refreshed
                    && commit.controller_notified
                    && commit.source_host_notified
                    && commit.destination_host_notified
                    && commit.viewport_refreshed
                    && commit
                        .surface
                        .as_ref()
                        .and_then(DockSurfaceTransactionReceipt::committed_revision)
                        .is_some()
                    && commit
                        .publication
                        .as_ref()
                        .is_none_or(DockSurfaceDeferredPublication::is_settled)
        )
    }

    fn drive_host_post_commit_journal(
        &self,
        journal: &Rc<DockLiveUndockPromotionCommitJournal>,
        require_surface_publication: bool,
        cx: &mut App,
        first_panic: &mut Option<Box<dyn Any + Send>>,
    ) -> bool {
        let provider_post_commit = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::Host(commit) = &*execution else {
                return false;
            };
            commit.presentation_cleanup.as_ref().and_then(|cleanup| {
                (!cleanup.provider_refreshed)
                    .then(|| cleanup.provider_post_commit.as_ref())
                    .flatten()
                    .cloned()
            })
        };
        if let Some(provider_post_commit) = provider_post_commit {
            let completed =
                record_post_commit_panic(first_panic, || provider_post_commit.publish(cx));
            if completed.is_some()
                && let DockLiveUndockPromotionCommitExecution::Host(commit) =
                    &mut *journal.execution.borrow_mut()
                && let Some(cleanup) = commit.presentation_cleanup.as_mut()
            {
                cleanup.provider_refreshed = true;
            }
        }

        let host_drop = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::Host(commit) = &*execution else {
                return false;
            };
            (!commit.host_drop_notified)
                .then(|| {
                    commit
                        .committed_drop
                        .clone()
                        .map(|committed| (commit.runtime.clone(), committed))
                })
                .flatten()
        };
        if let Some((runtime, committed_drop)) = host_drop {
            let completed = record_post_commit_panic(first_panic, || {
                runtime.notify_live_undock_host_drop_commit(&committed_drop, cx)
            });
            if completed.is_some()
                && let DockLiveUndockPromotionCommitExecution::Host(commit) =
                    &mut *journal.execution.borrow_mut()
            {
                commit.host_drop_notified = true;
            }
        }

        let publication = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::Host(commit) = &*execution else {
                return false;
            };
            commit.publication.clone()
        };
        if let Some(publication) = publication
            && !publication.is_settled()
        {
            record_post_commit_panic(first_panic, || publication.publish(cx));
        }

        let execution = journal.execution.borrow();
        matches!(
            &*execution,
            DockLiveUndockPromotionCommitExecution::Host(commit)
                if commit
                    .presentation_cleanup
                    .as_ref()
                    .is_none_or(|cleanup| cleanup.provider_refreshed)
                    && commit.host_drop_notified
                    && (!require_surface_publication
                        || commit
                            .surface
                            .as_ref()
                            .and_then(DockSurfaceTransactionReceipt::committed_revision)
                            .is_some())
                    && commit
                        .publication
                        .as_ref()
                        .is_none_or(DockSurfaceDeferredPublication::is_settled)
        )
    }

    fn settle_post_commit_and_resume_terminal(
        &self,
        identity: DockLiveUndockIdentity,
        receipt: &DockLiveUndockPostCommitReceipt,
        cx: &mut App,
    ) {
        let receipt_is_current = self
            .state
            .borrow()
            .executions
            .get(&identity)
            .and_then(|execution| execution.promotion.as_ref())
            .is_some_and(|promotion| {
                matches!(promotion, DockLiveUndockPromotionExecution::Durable(durable)
                    if durable.post_commit().matches(receipt))
            });
        if !receipt_is_current {
            return;
        }
        if !receipt.settle() {
            return;
        }
        let terminal_requested = self
            .state
            .borrow()
            .executions
            .get(&identity)
            .is_some_and(|execution| execution.terminal_requested);
        if terminal_requested {
            self.finalize_live_payload_drag(identity, cx);
        }
    }

    fn retry_terminal_settlement(
        &self,
        identity: DockLiveUndockIdentity,
        retry_generation: u64,
        cx: &mut App,
    ) {
        let current = self
            .state
            .borrow_mut()
            .executions
            .get_mut(&identity)
            .is_some_and(|execution| {
                execution.terminal_requested
                    && execution.terminal_settlement_retry.claim(retry_generation)
            });
        if current {
            self.finalize_live_payload_drag(identity, cx);
        }
    }

    fn promotion_commit_journal_is_current(
        &self,
        journal: &Rc<DockLiveUndockPromotionCommitJournal>,
    ) -> bool {
        self.state
            .borrow()
            .executions
            .get(&journal.identity())
            .is_some_and(|execution| {
                execution.request.key() == journal.identity().opening()
                    && matches!(
                        execution.promotion.as_ref(),
                        Some(DockLiveUndockPromotionExecution::Committing(current))
                            if Rc::ptr_eq(current, journal)
                    )
            })
    }

    fn resolve_in_flight_promotion_commit(
        &self,
        journal: &Rc<DockLiveUndockPromotionCommitJournal>,
        cx: &App,
    ) {
        if journal.boundary.get() != DockLiveUndockPromotionCommitBoundary::InFlight {
            return;
        }
        let same_window_identity = {
            let execution = journal.execution.borrow();
            match &*execution {
                DockLiveUndockPromotionCommitExecution::SameWindow(commit)
                    if commit.presentation_batch.is_some() =>
                {
                    journal.confirm_irreversible();
                    return;
                }
                DockLiveUndockPromotionCommitExecution::SameWindow(commit) => Some((
                    commit.identity,
                    commit.reveal.preflight().mount().proxy().lease(),
                )),
                DockLiveUndockPromotionCommitExecution::Host(commit)
                    if commit.committed_drop.is_some() =>
                {
                    journal.confirm_irreversible();
                    return;
                }
                DockLiveUndockPromotionCommitExecution::Host(commit)
                    if commit.drop.committed_workspace(cx).is_some() =>
                {
                    journal.confirm_irreversible();
                    return;
                }
                DockLiveUndockPromotionCommitExecution::Host(_)
                | DockLiveUndockPromotionCommitExecution::Pending(_)
                | DockLiveUndockPromotionCommitExecution::Aborted => None,
            }
        };
        let Some((identity, lease)) = same_window_identity else {
            journal.resolve_in_flight_as_reversible();
            return;
        };
        let terminal = self
            .state
            .borrow()
            .executions
            .get(&identity)
            .and_then(|execution| execution.presentation.as_ref())
            .filter(|presentation| presentation.lease == lease)
            .and_then(|presentation| presentation.session.active())
            .and_then(RehostSession::terminal_disposition);
        if terminal
            == Some(view_presentation_window::RehostTerminalDisposition::DestinationCommitted)
        {
            journal.confirm_irreversible();
        } else {
            journal.resolve_in_flight_as_reversible();
        }
    }

    fn settle_promotion_commit_attempt_failure(
        &self,
        journal: &Rc<DockLiveUndockPromotionCommitJournal>,
        cx: &mut App,
    ) {
        if !self.promotion_commit_journal_is_current(journal) {
            return;
        }
        self.resolve_in_flight_promotion_commit(journal, cx);
        if journal.abort_was_claimed() {
            let _ = journal.abort_execution();
            self.clear_promotion_commit_journal(journal);
            return;
        }
        match journal.boundary.get() {
            DockLiveUndockPromotionCommitBoundary::Reversible
            | DockLiveUndockPromotionCommitBoundary::AbortClaimed => {
                if journal.abort_execution() {
                    self.clear_promotion_commit_journal(journal);
                    self.enqueue_fact(
                        DockLiveUndockQueuedFact::Reduce(
                            DockLiveUndockFact::PromotionPreparationFailed {
                                identity: journal.identity(),
                                token: journal.token(),
                            },
                        ),
                        cx,
                    );
                }
            }
            DockLiveUndockPromotionCommitBoundary::InFlight
            | DockLiveUndockPromotionCommitBoundary::Irreversible => {
                journal.finish_drive_for_recovery();
                self.enqueue_fact(
                    DockLiveUndockQueuedFact::Reduce(
                        DockLiveUndockFact::CommittedDestinationRecoveryRequired {
                            identity: journal.identity(),
                            token: journal.token(),
                            destination: journal.destination(),
                        },
                    ),
                    cx,
                );
            }
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
        log::debug!(
            "live-undock presentation handoff failed before the durable boundary: identity={identity:?} failure={failure:?}"
        );
        #[cfg(feature = "test-support")]
        eprintln!("OPEN_GPUI_DOCK_PRESENTATION_FAILURE identity={identity:?} failure={failure:?}");
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
        #[cfg(test)]
        {
            let mut state = self.state.borrow_mut();
            if state.suppress_same_window_destination_semantics_frames > 0 {
                state.suppress_same_window_destination_semantics_frames = state
                    .suppress_same_window_destination_semantics_frames
                    .saturating_sub(1);
                return;
            }
        }
        let Some(authority) = self.same_window_destination_semantics_authority(marker.identity())
        else {
            return;
        };
        if authority.token != marker.token()
            || authority.destination.window_id() != window.window_handle().window_id()
            || authority.marker.surface_revision() != marker.surface_revision()
            || authority.marker.binding() != marker.binding()
            || authority.marker.registration() != marker.registration()
            || authority.marker.destination().window_id() != marker.destination().window_id()
            || authority.marker.destination().leases() != marker.destination().leases()
            || authority.destination_window.window_id() != window.window_handle().window_id()
        {
            return;
        }
        let identity = authority.identity;
        let token = authority.token;
        let destination = authority.destination;
        let watchdog_key = authority.watchdog_key();
        #[cfg(test)]
        let retire_graph_commit = std::mem::take(
            &mut self
                .state
                .borrow_mut()
                .retire_next_same_window_graph_commit_before_semantics_ack,
        );
        #[cfg(test)]
        if retire_graph_commit && let Some(receipt) = authority.graph_commit {
            cx.update_entity(&authority.controller, |controller, _| {
                controller.workspace_mut().retire_graph_commit(receipt);
            });
        }
        if !workspace_graph_projection_is_exact(&authority.controller, authority.graph_commit, cx) {
            self.fail_destination_semantics_submission(identity, watchdog_key, cx);
            return;
        }
        if !destination_host_semantics_are_exact(host, marker, cx) {
            window.refresh();
            return;
        }

        let current = authority.semantics.snapshot();
        match current.outcome() {
            WindowProvisionalSemanticsOutcome::Submitted => {
                self.publish_submitted_destination_semantics(&authority, watchdog_key, cx);
                return;
            }
            WindowProvisionalSemanticsOutcome::Accepted => {
                let session = authority.provisional_session.snapshot();
                let acceptance_is_live = session.window_id()
                    == Some(window.window_handle().window_id())
                    && session.generation() == current.session_generation()
                    && session.phase()
                        == WindowProvisionalSessionPhase::DestinationSemanticsAccepted
                    && current.accepted_frame_generation().is_some()
                    && current.submitted_frame_generation().is_none();
                if !acceptance_is_live {
                    self.fail_destination_semantics_submission(identity, watchdog_key, cx);
                    return;
                }
                if current
                    .accepted_frame_generation()
                    .is_some_and(|accepted| frame_generation <= accepted)
                {
                    self.arm_destination_semantics_watchdog(identity, token, destination, cx);
                    return;
                }
            }
            WindowProvisionalSemanticsOutcome::Rejected
            | WindowProvisionalSemanticsOutcome::WindowTerminal => {
                self.fail_destination_semantics_submission(identity, watchdog_key, cx);
                return;
            }
            WindowProvisionalSemanticsOutcome::Pending => {}
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

        let before_acceptance = authority.semantics.snapshot();
        let Some(prepared_receipt) = DockLiveUndockDestinationSemanticsReceipt::prepare_same_window(
            identity,
            token,
            authority.reveal,
            before_acceptance,
            frame_generation,
        ) else {
            let retryable = before_acceptance.outcome()
                == WindowProvisionalSemanticsOutcome::Pending
                && before_acceptance.accepted_frame_generation().is_none()
                && before_acceptance.submitted_frame_generation().is_none()
                && before_acceptance.window_id() == window.window_handle().window_id()
                && before_acceptance.session_generation()
                    == authority.provisional_session.snapshot().generation()
                && before_acceptance.destination_generation() == token.get()
                && (frame_generation < before_acceptance.minimum_frame_generation()
                    || frame_generation <= authority.reveal.reveal_frame().frame_generation());
            if retryable {
                window.refresh();
            } else {
                self.fail_destination_semantics_submission(identity, watchdog_key, cx);
            }
            return;
        };

        let accepted = match window.accept_provisional_destination_semantics_frame(
            &authority.provisional_session,
            &authority.semantics,
            frame_generation,
            cx,
        ) {
            Ok(accepted) => accepted,
            Err(_) => {
                let semantics = authority.semantics.snapshot();
                let session = authority.provisional_session.snapshot();
                match (semantics.outcome(), session.phase()) {
                    (
                        WindowProvisionalSemanticsOutcome::Pending,
                        WindowProvisionalSessionPhase::ProjectingDestinationSemantics,
                    ) => window.refresh(),
                    (
                        WindowProvisionalSemanticsOutcome::Accepted,
                        WindowProvisionalSessionPhase::DestinationSemanticsAccepted,
                    ) => self.arm_destination_semantics_watchdog(identity, token, destination, cx),
                    (
                        WindowProvisionalSemanticsOutcome::Submitted,
                        WindowProvisionalSessionPhase::DestinationSemanticsSubmitted,
                    ) => {
                        self.publish_submitted_destination_semantics(&authority, watchdog_key, cx);
                    }
                    _ => {
                        self.fail_destination_semantics_submission(identity, watchdog_key, cx);
                    }
                }
                return;
            }
        };
        if prepared_receipt.accepts(accepted) {
            self.arm_destination_semantics_watchdog(identity, token, destination, cx);
        } else {
            self.fail_destination_semantics_submission(identity, watchdog_key, cx);
        }
    }

    fn source_restoration_execution_for_identity(
        &self,
        identity: DockLiveUndockIdentity,
    ) -> Option<DockLiveUndockSourceRestorationExecution> {
        if self.promotion_commit_forbids_rollback(identity) {
            return None;
        }
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
        if self.promotion_commit_forbids_rollback(identity) {
            return;
        }
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
        if self.promotion_commit_forbids_rollback(identity) {
            self.clear_source_restoration_retry(identity);
            return;
        }
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
        let (evidence, post_commit) = Self::commit_presentation_cleanup_prepared(prepared, cx);
        post_commit.publish(cx);
        evidence
    }

    fn commit_presentation_cleanup_prepared(
        prepared: DockLiveUndockPreparedPresentationCleanup,
        cx: &mut App,
    ) -> (
        DockLiveUndockRehostCleanupEvidence,
        view_presentation_window::RehostTerminalPostCommit,
    ) {
        match prepared {
            DockLiveUndockPreparedPresentationCleanup::Exact(prepared) => {
                let (
                    view_presentation_window::RehostTerminalOutcome::Abandoned(receipt),
                    post_commit,
                ) = prepared.commit_prepared(cx)
                else {
                    unreachable!("source-loss terminal preparation committed a destination")
                };
                (
                    DockLiveUndockRehostCleanupEvidence::abandoned(
                        receipt.generation(),
                        receipt.source_window(),
                        receipt.destination_window(),
                    ),
                    post_commit,
                )
            }
            DockLiveUndockPreparedPresentationCleanup::AlreadyTerminal(evidence) => (
                evidence,
                view_presentation_window::RehostTerminalPostCommit::default(),
            ),
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

    fn commit_host_presentation_abandonment_without_notify(
        prepared: &DockLiveUndockPreparedHostPresentationAbandonment,
        cx: &mut App,
    ) -> DockLiveUndockHostCleanupEvidence {
        match prepared {
            DockLiveUndockPreparedHostPresentationAbandonment::Exact { host, prepared } => {
                let receipt = cx.update_entity(host, |host, _| {
                    host.commit_prepared_live_presentation_abandonment_without_notify(
                        prepared.clone(),
                    )
                });
                DockLiveUndockHostCleanupEvidence::abandoned(receipt)
            }
            DockLiveUndockPreparedHostPresentationAbandonment::AlreadyAbsent {
                window_id, ..
            } => DockLiveUndockHostCleanupEvidence::already_absent(*window_id),
            DockLiveUndockPreparedHostPresentationAbandonment::HostUnavailable { window_id } => {
                DockLiveUndockHostCleanupEvidence::host_unavailable(*window_id)
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
        if self.promotion_commit_forbids_rollback(identity) {
            return None;
        }
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
        if self.promotion_commit_forbids_rollback(prepared.identity) {
            return false;
        }
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
        if self.promotion_commit_forbids_rollback(identity) {
            return None;
        }
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
        !self.promotion_commit_forbids_rollback(prepared.cleanup.identity)
            && self.preflight_orphan_cleanup(&prepared.cleanup, cx)
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
            promotion,
            runtime,
            payload_identity,
            recovery_focus,
            destination_window,
            presentation_lease,
            source_semantic_seed,
            presentation_origin,
            same_window_terminal_required,
            provisional_terminal_candidate,
        ) = {
            let state = self.state.borrow();
            let execution = state.executions.get(&identity)?;
            if execution.request.key() != identity.opening()
                || authority.promotion() != Some((identity, token, destination))
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
            let (
                promotion,
                destination_window,
                presentation_origin,
                same_window_terminal_required,
                provisional_terminal_candidate,
            ) = match execution.promotion.as_ref()? {
                DockLiveUndockPromotionExecution::Durable(durable)
                    if durable.identity() == identity
                        && durable.token() == token
                        && durable.destination() == destination
                        && durable.destination_window().window_id() == destination.window_id() =>
                {
                    (
                        DockLiveUndockCommittedDestinationPromotionAuthority::Durable,
                        durable.destination_window(),
                        DockPayloadRecoveryPresentationOrigin::new(
                            durable.destination_window(),
                            durable.destination_binding(),
                            durable.registration().clone(),
                        )?,
                        matches!(
                            destination,
                            DockLiveUndockPromotionDestination::SameWindowDesktop { .. }
                        ),
                        None,
                    )
                }
                DockLiveUndockPromotionExecution::Committing(journal)
                    if journal.identity() == identity
                        && journal.token() == token
                        && journal.destination() == destination
                        && journal.crossed_commit_boundary() =>
                {
                    let destination_window = execution
                        .destination_host
                        .filter(|window| window.window_id() == destination.window_id())?;
                    let journal_execution = journal.execution.borrow();
                    match &*journal_execution {
                        DockLiveUndockPromotionCommitExecution::SameWindow(commit) => {
                            let presentation_origin = match (
                                commit.committed_viewport.as_ref(),
                                commit.destination_promotion.as_ref(),
                            ) {
                                (Some(viewport), Some(promotion)) => {
                                    DockPayloadRecoveryPresentationOrigin::new(
                                        destination_window,
                                        promotion.semantics().binding(),
                                        viewport.registration.clone(),
                                    )?
                                }
                                _ => DockPayloadRecoveryPresentationOrigin::provider_terminal(
                                    destination_window,
                                    commit.presentation_batch.clone()?,
                                )?,
                            };
                            let registered = presentation_origin.registered_host().is_some();
                            (
                                DockLiveUndockCommittedDestinationPromotionAuthority::Journal(
                                    journal.clone(),
                                ),
                                destination_window,
                                presentation_origin,
                                registered,
                                (!registered).then(|| commit.destination_host.clone()),
                            )
                        }
                        DockLiveUndockPromotionCommitExecution::Pending(Some(
                            DockLiveUndockPreparedPromotionExecution::SameWindow(prepared),
                        )) if matches!(
                            prepared.presentation,
                            RehostTerminalPreparation::AlreadyCommitted(
                                view_presentation_window::RehostTerminalOutcome::DestinationCommitted(_)
                            )
                        ) => {
                            let RehostTerminalPreparation::AlreadyCommitted(
                                view_presentation_window::RehostTerminalOutcome::DestinationCommitted(
                                    batch,
                                ),
                            ) = &prepared.presentation
                            else {
                                return None;
                            };
                            (
                                DockLiveUndockCommittedDestinationPromotionAuthority::Journal(
                                    journal.clone(),
                                ),
                                destination_window,
                                DockPayloadRecoveryPresentationOrigin::provider_terminal(
                                    destination_window,
                                    batch.clone(),
                                )?,
                                false,
                                Some(prepared.destination_host.clone()),
                            )
                        }
                        DockLiveUndockPromotionCommitExecution::Host(commit)
                            if commit.committed_drop.is_some() =>
                        {
                            (
                                DockLiveUndockCommittedDestinationPromotionAuthority::Journal(
                                    journal.clone(),
                                ),
                                commit.target_window,
                                DockPayloadRecoveryPresentationOrigin::new(
                                    commit.target_window,
                                    commit.target_binding,
                                    commit.target_registration.clone(),
                                )?,
                                false,
                                None,
                            )
                        }
                        DockLiveUndockPromotionCommitExecution::Pending(_)
                        | DockLiveUndockPromotionCommitExecution::Host(_)
                        | DockLiveUndockPromotionCommitExecution::Aborted => return None,
                    }
                }
                DockLiveUndockPromotionExecution::Prepared(_)
                | DockLiveUndockPromotionExecution::Committing(_)
                | DockLiveUndockPromotionExecution::Durable(_) => return None,
            };
            (
                promotion,
                execution.seed.source.runtime.clone(),
                execution.seed.move_plan.source_identity().clone(),
                DockPayloadRecoveryFocus::new(
                    execution.seed.source.session.focus_item().cloned(),
                    execution.seed.source.source_focus.clone(),
                ),
                destination_window,
                presentation_lease,
                source_semantic_seed,
                presentation_origin,
                same_window_terminal_required,
                provisional_terminal_candidate,
            )
        };
        let same_window_terminal_required = same_window_terminal_required
            || provisional_terminal_candidate.is_some_and(|host| {
                cx.read_entity(&host, |host, _| {
                    host.surface_owner_entity().as_ref() == Some(owner)
                        && host.is_provisional_viewport_for(identity.opening())
                        && host.current_viewport_registration().is_none()
                })
            });
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
                        presentation_origin.clone(),
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
                promotion,
                presentation_lease,
                same_window_terminal_required,
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
                    && match (&prepared.promotion, execution.promotion.as_ref()) {
                        (
                            DockLiveUndockCommittedDestinationPromotionAuthority::Durable,
                            Some(DockLiveUndockPromotionExecution::Durable(durable)),
                        ) => {
                            durable.identity() == prepared.identity
                                && durable.token() == prepared.token
                                && durable.destination() == prepared.destination
                                && durable.destination_window() == prepared.destination_window
                        }
                        (
                            DockLiveUndockCommittedDestinationPromotionAuthority::Journal(
                                prepared_journal,
                            ),
                            Some(DockLiveUndockPromotionExecution::Committing(current_journal)),
                        ) => {
                            Rc::ptr_eq(prepared_journal, current_journal)
                                && current_journal.identity() == prepared.identity
                                && current_journal.token() == prepared.token
                                && current_journal.destination() == prepared.destination
                                && current_journal.crossed_commit_boundary()
                        }
                        (DockLiveUndockCommittedDestinationPromotionAuthority::Durable, _)
                        | (DockLiveUndockCommittedDestinationPromotionAuthority::Journal(_), _) => {
                            false
                        }
                    }
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
    ) -> Option<DockLiveUndockCommittedDestinationRecoveryReceipt> {
        let journal = match &prepared.promotion {
            DockLiveUndockCommittedDestinationPromotionAuthority::Durable => None,
            DockLiveUndockCommittedDestinationPromotionAuthority::Journal(journal) => {
                Some(journal.clone())
            }
        };
        let same_window_terminal_required = prepared.same_window_terminal_required;
        let transaction_runtime = prepared.runtime.clone();
        let recovery = transaction_runtime.with_surface_transaction(cx, |transaction, cx| {
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
        })?;
        if let Some(journal) = journal
            && !journal.record_recovery(recovery, same_window_terminal_required)
        {
            return None;
        }
        DockLiveUndockCommittedDestinationRecoveryReceipt::new(
            recovery,
            same_window_terminal_required,
        )
    }

    fn attempt_committed_destination_recovery(
        &self,
        owner: &Entity<DockSurfaceOwner>,
        identity: DockLiveUndockIdentity,
        authority: DockPayloadRecoveryAuthority,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
        cx: &mut App,
    ) -> Result<
        DockLiveUndockCommittedDestinationRecoveryReceipt,
        DockLiveUndockCommittedDestinationRecoveryFailure,
    > {
        #[cfg(test)]
        if std::mem::take(
            &mut self
                .state
                .borrow_mut()
                .panic_next_committed_destination_recovery_attempt,
        ) {
            panic!("injected committed-destination recovery panic");
        }
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
                let prepared = source_window
                    .update(cx, |_, window, _| {
                        retained_visual::prepare_release(window, &retained)
                    })
                    .ok()?
                    .ok()?;
                Some(DockLiveUndockRetainedVisualRelease::new(
                    source_window,
                    prepared,
                ))
            };
            Some((
                payload_lease,
                projection,
                presentation_generation,
                source_host,
                provisional_host,
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
                || !observed_placement
                    .final_placement
                    .matches(identity, window_id, release)
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
        let retained_release =
            DockLiveUndockRetainedVisualRelease::new(source_window, retained_release);

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
        let destination_presentation = self
            .current_destination_presentation(destination_key, reveal.preflight().mount(), cx)?
            .ok()?
            .rehost_presentation()?;
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
                    .is_none_or(|retained_release| retained_release.can_commit(cx))
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

    fn clear_promotion_commit_journal(&self, journal: &Rc<DockLiveUndockPromotionCommitJournal>) {
        let mut state = self.state.borrow_mut();
        let Some(execution) = state.executions.get_mut(&journal.identity()) else {
            return;
        };
        if matches!(
            execution.promotion.as_ref(),
            Some(DockLiveUndockPromotionExecution::Committing(current))
                if Rc::ptr_eq(current, journal)
        ) {
            execution.promotion = None;
        }
    }

    fn preflight_promotion_commit_journal(
        &self,
        owner: &Entity<DockSurfaceOwner>,
        journal: &Rc<DockLiveUndockPromotionCommitJournal>,
        cx: &mut App,
    ) -> bool {
        match &*journal.execution.borrow() {
            DockLiveUndockPromotionCommitExecution::SameWindow(_)
            | DockLiveUndockPromotionCommitExecution::Host(_) => return true,
            DockLiveUndockPromotionCommitExecution::Pending(_)
            | DockLiveUndockPromotionCommitExecution::Aborted => {}
        }
        let Some(prepared) = journal.take_pending_preparation() else {
            return false;
        };
        let preflight = match prepared {
            DockLiveUndockPreparedPromotionExecution::SameWindow(prepared) => {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    self.preflight_same_window_promotion_commit(owner, &prepared, cx)
                }));
                match result {
                    Ok(Some(prepared_graph)) => Ok(Some(
                        DockLiveUndockPromotionCommitJournal::same_window(prepared, prepared_graph),
                    )),
                    Ok(None) if journal.has_irreversible_authority() => {
                        journal.restore_pending_preparation(
                            DockLiveUndockPreparedPromotionExecution::SameWindow(prepared),
                        );
                        Ok(None)
                    }
                    Ok(None) => {
                        self.restore_prepared_promotion_session(
                            DockLiveUndockPreparedPromotionExecution::SameWindow(prepared),
                            cx,
                        );
                        Ok(None)
                    }
                    Err(payload) if journal.has_irreversible_authority() => {
                        journal.restore_pending_preparation(
                            DockLiveUndockPreparedPromotionExecution::SameWindow(prepared),
                        );
                        Err(payload)
                    }
                    Err(payload) => Err(payload),
                }
            }
            DockLiveUndockPreparedPromotionExecution::Host(prepared) => {
                catch_unwind(AssertUnwindSafe(|| {
                    self.preflight_host_promotion_commit(owner, prepared, cx)
                        .map(DockLiveUndockPromotionCommitJournal::host)
                }))
            }
        };
        let preflighted = match preflight {
            Ok(Some(preflighted)) => preflighted,
            Ok(None) if journal.has_irreversible_authority() => return false,
            Ok(None) => {
                if journal.abort_execution() {
                    self.clear_promotion_commit_journal(journal);
                    self.enqueue_fact(
                        DockLiveUndockQueuedFact::Reduce(
                            DockLiveUndockFact::PromotionPreparationFailed {
                                identity: journal.identity(),
                                token: journal.token(),
                            },
                        ),
                        cx,
                    );
                }
                return false;
            }
            Err(payload) if journal.has_irreversible_authority() => resume_unwind(payload),
            Err(payload) => {
                if journal.abort_execution() {
                    self.clear_promotion_commit_journal(journal);
                    self.enqueue_fact(
                        DockLiveUndockQueuedFact::Reduce(
                            DockLiveUndockFact::PromotionPreparationFailed {
                                identity: journal.identity(),
                                token: journal.token(),
                            },
                        ),
                        cx,
                    );
                }
                resume_unwind(payload);
            }
        };
        if journal.install_preflighted(preflighted) {
            true
        } else {
            self.clear_promotion_commit_journal(journal);
            false
        }
    }

    fn mark_same_window_topology_recovery_if_superseded(
        journal: &Rc<DockLiveUndockPromotionCommitJournal>,
        cx: &App,
    ) {
        let observation = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::SameWindow(commit) = &*execution else {
                return;
            };
            let Some(receipt) = commit.graph_commit else {
                return;
            };
            cx.read_entity(&commit.controller, |controller, _| {
                controller.workspace().observe_graph_commit(receipt)
            })
        };
        if observation == Some(DockWorkspaceGraphCommitObservation::Exact) {
            return;
        }
        if let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
            &mut *journal.execution.borrow_mut()
        {
            commit.topology_recovery_required = true;
        }
    }

    fn commit_preflighted_same_window_promotion(
        &self,
        owner: &Entity<DockSurfaceOwner>,
        journal: &Rc<DockLiveUndockPromotionCommitJournal>,
        cx: &mut App,
    ) -> DockLiveUndockCompletedPromotionCommit {
        let (
            controller,
            prepared_graph,
            runtime,
            viewport,
            source_host,
            source,
            destination_host,
            destination_host_promotion,
            destination,
            identity,
            reveal,
        ) = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::SameWindow(commit) = &*execution else {
                panic!("same-window final swap requires one preflighted same-window commit");
            };
            assert!(commit.graph_commit.is_none());
            assert!(commit.committed_viewport.is_none());
            assert!(matches!(
                commit.source_retirement,
                DockLiveUndockSourceRetirementStage::Pending
            ));
            assert!(commit.destination_promotion.is_none());
            assert!(commit.presentation_batch.is_none());
            assert!(commit.surface.is_none());
            (
                commit.controller.clone(),
                commit.prepared_graph.clone(),
                commit.runtime.clone(),
                commit.viewport.clone(),
                commit.source_host.clone(),
                commit.source.clone(),
                commit.destination_host.clone(),
                commit.destination_host_promotion.clone(),
                commit.destination,
                commit.identity,
                commit.reveal,
            )
        };

        let graph = cx.read_entity(&controller, |controller, _| {
            assert!(
                controller
                    .workspace()
                    .graph()
                    .matches_exactly(&prepared_graph.expected),
                "preflighted promotion graph must remain exact before the final swap"
            );
            match controller
                .workspace()
                .prepare_graph_commit(prepared_graph.commit_id, prepared_graph.projected.clone())
            {
                DockWorkspaceGraphCommitPreparation::Prepared(prepared) => prepared,
                DockWorkspaceGraphCommitPreparation::AlreadyCommitted(_) => {
                    panic!("a fresh promotion graph commit cannot already be committed")
                }
            }
        });
        let destination_window = self
            .state
            .borrow()
            .executions
            .get(&identity)
            .and_then(|execution| execution.destination_host)
            .filter(|window| window.window_id() == destination.window_id())
            .expect("preflighted promotion must retain its exact destination window");

        assert!(
            journal.begin_commit_call(),
            "shutdown cannot claim an already-sealed promotion final swap"
        );
        let (surface, publication) =
            runtime.with_deferred_tracked_surface_transaction(cx, |transaction, surface, cx| {
                let presentation = {
                    let mut execution = journal.execution.borrow_mut();
                    let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
                        &mut *execution
                    else {
                        panic!("same-window final swap lost its journal route");
                    };
                    commit
                        .presentation
                        .take()
                        .expect("preflighted promotion must retain its provider token")
                };
                let (
                    view_presentation_window::RehostTerminalOutcome::DestinationCommitted(batch),
                    provider_post_commit,
                ) = presentation.commit_prepared(cx)
                else {
                    unreachable!("same-window promotion must commit the destination provider")
                };
                assert_eq!(batch.window_id(), destination.window_id());
                journal.confirm_irreversible();
                if let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
                    &mut *journal.execution.borrow_mut()
                {
                    commit.presentation_batch = Some(batch);
                    commit.provider_post_commit = Some(provider_post_commit);
                }

                let graph_commit = cx.update_entity(&controller, |controller, _| {
                    controller.workspace_mut().commit_prepared_graph(graph)
                });
                if let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
                    &mut *journal.execution.borrow_mut()
                {
                    commit.graph_commit = Some(graph_commit);
                }

                let committed_viewport =
                    runtime.commit_prepared_live_undock_provisional_promotion(viewport);
                if let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
                    &mut *journal.execution.borrow_mut()
                {
                    commit.committed_viewport = Some(committed_viewport.clone());
                }

                let source_retirement = cx.update_entity(&source_host, |host, _| {
                    host.commit_prepared_live_source_retirement_without_notify(source)
                });
                if let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
                    &mut *journal.execution.borrow_mut()
                {
                    commit.source_retirement =
                        DockLiveUndockSourceRetirementStage::Committed(source_retirement);
                }

                let destination_promotion = cx.update_entity(&destination_host, |host, _| {
                    host.commit_prepared_live_destination_promotion_without_notify(
                        destination_host_promotion,
                    )
                });
                if let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
                    &mut *journal.execution.borrow_mut()
                {
                    commit.destination_promotion = Some(destination_promotion);
                }

                let retained_release = {
                    let execution = journal.execution.borrow();
                    let DockLiveUndockPromotionCommitExecution::SameWindow(commit) = &*execution
                    else {
                        panic!("same-window final swap lost its retained visual authority");
                    };
                    commit.retained_release.clone()
                };
                let _ = retained_release.commit_prepared_infallible(cx);

                assert!(
                    self.retire_presentation_session_after_terminal_commit(
                        identity,
                        reveal.preflight().mount().proxy().lease(),
                    ),
                    "provider terminal commit must retire its exact presentation session"
                );
                if let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
                    &mut *journal.execution.borrow_mut()
                {
                    commit.presentation_session_retired = true;
                }

                let mut surface_update = committed_viewport.runtime_update.clone();
                let work_context = surface_update
                    .work_context()
                    .expect("prepared promotion must retain its exact runtime work context");
                surface_update.mark_graph_commit(true, work_context);
                cx.update_entity(owner, |owner, _| {
                    owner.record_changes(
                        transaction,
                        surface_update.change_categories().iter().copied(),
                    );
                });
                surface
            });
        if let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
            &mut *journal.execution.borrow_mut()
        {
            commit.surface = Some(surface);
            commit.publication = Some(publication);
        }

        let execution = journal.execution.borrow();
        let DockLiveUndockPromotionCommitExecution::SameWindow(commit) = &*execution else {
            unreachable!("same-window final swap changed its journal route");
        };
        let committed_revision = commit
            .surface
            .as_ref()
            .and_then(DockSurfaceTransactionReceipt::committed_revision)
            .expect("same-window final swap must commit its surface revision");
        assert!(committed_revision > commit.surface_revision);
        let viewport = commit
            .committed_viewport
            .as_ref()
            .expect("same-window final swap must commit its viewport");
        let destination_promotion = commit
            .destination_promotion
            .as_ref()
            .expect("same-window final swap must commit destination semantics");
        let DockLiveUndockSourceRetirementStage::Committed(source_retirement) =
            &commit.source_retirement
        else {
            unreachable!("same-window final swap must retire source semantics")
        };
        let post_commit_receipt = DockLiveUndockPostCommitReceipt::pending();
        let post_commit = DockLiveUndockPostCommitPlan::SameWindow {
            identity: commit.identity,
            journal: journal.clone(),
            receipt: post_commit_receipt.clone(),
        };
        let durable = DockLiveUndockDurablePromotionExecution::SameWindow(
            DockLiveUndockDurableSameWindowPromotionExecution {
                identity: commit.identity,
                token: commit.token,
                destination: commit.destination,
                destination_window,
                destination_binding: destination_promotion.semantics().binding(),
                registration: viewport.registration.clone(),
                reveal: commit.reveal,
                provisional_session: commit.provisional_session.clone(),
                semantics: commit.semantics.clone(),
                viewport_commit: viewport.clone(),
                controller: commit.controller.clone(),
                graph_commit: commit.graph_commit,
                topology_recovery_required: false,
                source_host: commit.source_host.downgrade(),
                source_retirement: source_retirement.clone(),
                destination_host: commit.destination_host.downgrade(),
                destination_promotion: destination_promotion.clone(),
                post_commit: post_commit_receipt,
            },
        );
        drop(execution);
        DockLiveUndockCompletedPromotionCommit {
            durable,
            retained_released: true,
            post_commit,
        }
    }

    fn commit_preflighted_host_promotion(
        &self,
        owner: &Entity<DockSurfaceOwner>,
        journal: &Rc<DockLiveUndockPromotionCommitJournal>,
        cx: &mut App,
    ) -> DockLiveUndockCompletedPromotionCommit {
        let (runtime, drop, identity) = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::Host(commit) = &*execution else {
                panic!("host final swap requires one preflighted host commit");
            };
            assert!(commit.committed_drop.is_none());
            assert!(commit.surface.is_none());
            (commit.runtime.clone(), commit.drop.clone(), commit.identity)
        };

        assert!(
            journal.begin_commit_call(),
            "shutdown cannot claim an already-sealed host final swap"
        );
        let (surface, publication) =
            runtime.with_deferred_tracked_surface_transaction(cx, |transaction, surface, cx| {
                let committed_drop = runtime.commit_preflighted_live_undock_host_drop(&drop, cx);
                journal.confirm_irreversible();
                if let DockLiveUndockPromotionCommitExecution::Host(commit) =
                    &mut *journal.execution.borrow_mut()
                {
                    commit.committed_drop = Some(committed_drop.clone());
                }

                let cleanup = {
                    let mut execution = journal.execution.borrow_mut();
                    let DockLiveUndockPromotionCommitExecution::Host(commit) = &mut *execution
                    else {
                        panic!("host final swap lost its journal route");
                    };
                    commit.presentation_cleanup.take()
                };
                if let Some(mut cleanup) = cleanup {
                    let presentation = cleanup
                        .presentation
                        .take()
                        .expect("host final swap must retain its presentation cleanup token");
                    let (evidence, provider_post_commit) =
                        Self::commit_presentation_cleanup_prepared(presentation, cx);
                    assert_eq!(
                        evidence.authority().0,
                        cleanup.presentation_generation,
                        "host presentation cleanup must retain its exact generation"
                    );
                    cleanup.presentation_committed = true;
                    cleanup.provider_post_commit = Some(provider_post_commit);

                    let source_host = cleanup.source_host.clone();
                    let _ =
                        Self::commit_host_presentation_abandonment_without_notify(&source_host, cx);
                    cleanup.source_host_committed = true;

                    let provisional_host = cleanup.provisional_host.clone();
                    let _ = Self::commit_host_presentation_abandonment_without_notify(
                        &provisional_host,
                        cx,
                    );
                    cleanup.provisional_host_committed = true;

                    if let Some(retained_release) = cleanup.retained_release.as_ref() {
                        let _ = retained_release.commit_prepared_infallible(cx);
                    }
                    assert!(
                        self.retire_presentation_session_after_terminal_commit(
                            identity,
                            cleanup.payload_lease,
                        ),
                        "host provider cleanup must retire its exact presentation session"
                    );
                    cleanup.session_retired = true;

                    if let DockLiveUndockPromotionCommitExecution::Host(commit) =
                        &mut *journal.execution.borrow_mut()
                    {
                        commit.presentation_cleanup = Some(cleanup);
                    }
                }

                cx.update_entity(owner, |owner, _| {
                    owner.record_changes(
                        transaction,
                        committed_drop
                            .runtime_update()
                            .change_categories()
                            .iter()
                            .copied(),
                    );
                });
                surface
            });
        if let DockLiveUndockPromotionCommitExecution::Host(commit) =
            &mut *journal.execution.borrow_mut()
        {
            commit.surface = Some(surface);
            commit.publication = Some(publication);
        }

        let execution = journal.execution.borrow();
        let DockLiveUndockPromotionCommitExecution::Host(commit) = &*execution else {
            unreachable!("host final swap changed its journal route");
        };
        let committed_revision = commit
            .surface
            .as_ref()
            .and_then(DockSurfaceTransactionReceipt::committed_revision)
            .expect("host final swap must commit its surface revision");
        let committed = commit
            .committed_drop
            .as_ref()
            .expect("host final swap must commit its drop");
        assert!(
            committed_revision > commit.surface_revision,
            "host final swap must advance its surface revision: baseline={}, committed={}, commit_id={:?}, categories={:?}, outcome={:?}",
            commit.surface_revision,
            committed_revision,
            committed.workspace_commit().commit_id(),
            committed.runtime_update().change_categories(),
            committed.outcome(),
        );
        let crate::DockViewportDropRouteOutcome::Action(action) = committed.outcome() else {
            unreachable!("host final swap must produce one changed drop action")
        };
        assert!(action.action().changed());
        let post_commit_receipt = DockLiveUndockPostCommitReceipt::pending();
        let post_commit = DockLiveUndockPostCommitPlan::Host {
            identity: commit.identity,
            journal: journal.clone(),
            receipt: post_commit_receipt.clone(),
        };
        let retained_released = commit.presentation_cleanup.as_ref().is_none_or(|cleanup| {
            cleanup
                .retained_release
                .as_ref()
                .is_none_or(DockLiveUndockRetainedVisualRelease::is_settled)
        });
        let durable = DockLiveUndockDurablePromotionExecution::Host(
            DockLiveUndockDurableHostPromotionExecution {
                identity: commit.identity,
                token: commit.token,
                destination: commit.destination,
                destination_window: commit.target_window,
                destination_host: commit.target_host.downgrade(),
                destination_binding: commit.target_binding,
                registration: commit.target_registration.clone(),
                activation: committed.outcome().activation_transaction(),
                committed_destination_recovery_required: false,
                host_drop_commit: committed.clone(),
                post_commit: post_commit_receipt,
            },
        );
        std::mem::drop(execution);
        DockLiveUndockCompletedPromotionCommit {
            durable,
            retained_released,
            post_commit,
        }
    }

    fn resume_same_window_promotion_commit(
        &self,
        journal: &Rc<DockLiveUndockPromotionCommitJournal>,
        cx: &mut App,
    ) -> Option<DockLiveUndockCompletedPromotionCommit> {
        let needs_presentation = matches!(
            &*journal.execution.borrow(),
            DockLiveUndockPromotionCommitExecution::SameWindow(commit)
                if commit.presentation_batch.is_none()
        );
        if needs_presentation {
            let (identity, reveal, presentation) = {
                let mut execution = journal.execution.borrow_mut();
                let DockLiveUndockPromotionCommitExecution::SameWindow(commit) = &mut *execution
                else {
                    return None;
                };
                (commit.identity, commit.reveal, commit.presentation.take())
            };
            let presentation = match presentation {
                Some(presentation) => presentation,
                None => {
                    let payload_lease = reveal.preflight().mount().proxy().lease();
                    let destination_key = self
                        .state
                        .borrow()
                        .executions
                        .get(&identity)?
                        .presentation
                        .as_ref()
                        .filter(|presentation| presentation.lease == payload_lease)?
                        .destination_key?;
                    let destination = self
                        .current_destination_presentation(
                            destination_key,
                            reveal.preflight().mount(),
                            cx,
                        )?
                        .ok()?
                        .rehost_presentation()?;
                    self.prepare_presentation_terminal(
                        identity,
                        payload_lease,
                        view_presentation_window::RehostTerminalIntent::CommitDestination(
                            &destination,
                        ),
                        cx,
                    )?
                    .ok()?
                }
            };
            if !journal.begin_commit_call() {
                return None;
            }
            let view_presentation_window::RehostTerminalOutcome::DestinationCommitted(batch) =
                presentation.try_commit(cx).ok()?
            else {
                return None;
            };
            journal.confirm_irreversible();
            let mut execution = journal.execution.borrow_mut();
            let DockLiveUndockPromotionCommitExecution::SameWindow(commit) = &mut *execution else {
                return None;
            };
            if batch.window_id() != commit.destination.window_id() {
                return None;
            }
            commit.presentation_batch = Some(batch);
            commit.provider_refreshed = true;
        }

        let graph_stage = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::SameWindow(commit) = &*execution else {
                return None;
            };
            commit
                .graph_commit
                .is_none()
                .then(|| (commit.controller.clone(), commit.prepared_graph.clone()))
        };
        if let Some((controller, prepared_graph)) = graph_stage {
            let committed = cx.update_entity(&controller, |controller, _| {
                if let Some(receipt) = controller
                    .workspace()
                    .graph_commit(prepared_graph.commit_id)
                {
                    return Some(receipt);
                }
                controller.workspace_mut().commit_or_replay_graph(
                    prepared_graph.commit_id,
                    &prepared_graph.expected,
                    prepared_graph.projected,
                )
            });
            if let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
                &mut *journal.execution.borrow_mut()
            {
                commit.graph_commit = committed;
                commit.topology_recovery_required = committed.is_none();
            }
        }

        Self::mark_same_window_topology_recovery_if_superseded(journal, cx);

        let viewport_stage = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::SameWindow(commit) = &*execution else {
                return None;
            };
            commit
                .committed_viewport
                .is_none()
                .then(|| (commit.runtime.clone(), commit.viewport.clone()))
        };
        if let Some((runtime, viewport)) = viewport_stage {
            let committed =
                runtime.commit_or_replay_live_undock_provisional_promotion(&viewport)?;
            if let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
                &mut *journal.execution.borrow_mut()
            {
                commit.committed_viewport = Some(committed);
            }
        }

        let source_stage = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::SameWindow(commit) = &*execution else {
                return None;
            };
            matches!(
                commit.source_retirement,
                DockLiveUndockSourceRetirementStage::Pending
            )
            .then(|| (commit.source_host.clone(), commit.source.clone()))
        };
        if let Some((source_host, source)) = source_stage {
            let source_retirement = cx.update_entity(&source_host, |host, _| {
                host.commit_or_replay_prepared_live_source_retirement_without_notify(source)
            })?;
            if let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
                &mut *journal.execution.borrow_mut()
            {
                commit.source_retirement =
                    DockLiveUndockSourceRetirementStage::Committed(source_retirement);
            }
        }

        let destination_stage = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::SameWindow(commit) = &*execution else {
                return None;
            };
            commit.destination_promotion.is_none().then(|| {
                (
                    commit.destination_host.clone(),
                    commit.destination_host_promotion.clone(),
                )
            })
        };
        if let Some((destination_host, promotion)) = destination_stage {
            let promotion = cx.update_entity(&destination_host, |host, _| {
                host.commit_or_replay_prepared_live_destination_promotion_without_notify(promotion)
            })?;
            if let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
                &mut *journal.execution.borrow_mut()
            {
                commit.destination_promotion = Some(promotion);
            }
        }

        let retained_release = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::SameWindow(commit) = &*execution else {
                return None;
            };
            commit.retained_release.clone()
        };
        if !retained_release.settle(cx) {
            return None;
        }

        Self::mark_same_window_topology_recovery_if_superseded(journal, cx);

        let surface_stage = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::SameWindow(commit) = &*execution else {
                return None;
            };
            commit.surface.is_none().then(|| {
                (
                    commit.runtime.clone(),
                    commit
                        .committed_viewport
                        .clone()
                        .expect("promotion viewport must commit before surface publication"),
                    !commit.topology_recovery_required,
                )
            })
        };
        if let Some((runtime, committed_viewport, graph_changed)) = surface_stage {
            let (receipt, publication) =
                runtime.with_deferred_tracked_surface_transaction(cx, |_, receipt, cx| {
                    runtime.publish_live_undock_promotion_commit(
                        &committed_viewport,
                        graph_changed,
                        cx,
                    );
                    receipt
                });
            if let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
                &mut *journal.execution.borrow_mut()
            {
                commit.surface = Some(receipt);
                commit.publication = Some(publication);
            }
        }

        let presentation_session_stage = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::SameWindow(commit) = &*execution else {
                return None;
            };
            (!commit.presentation_session_retired).then_some((
                commit.identity,
                commit.reveal.preflight().mount().proxy().lease(),
            ))
        };
        if let Some((identity, payload_lease)) = presentation_session_stage {
            if !self.retire_presentation_session_after_terminal_commit(identity, payload_lease) {
                return None;
            }
            if let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
                &mut *journal.execution.borrow_mut()
            {
                commit.presentation_session_retired = true;
            }
        }

        Self::mark_same_window_topology_recovery_if_superseded(journal, cx);

        let execution = journal.execution.borrow();
        let DockLiveUndockPromotionCommitExecution::SameWindow(commit) = &*execution else {
            return None;
        };
        let committed_revision = commit.surface.as_ref()?.committed_revision()?;
        if committed_revision <= commit.surface_revision {
            return None;
        }
        let viewport = commit.committed_viewport.as_ref()?;
        let graph_commit = commit.graph_commit;
        if graph_commit.is_none() && !commit.topology_recovery_required {
            return None;
        }
        let destination_promotion = commit.destination_promotion.as_ref()?;
        let destination_semantics = destination_promotion.semantics();
        let DockLiveUndockSourceRetirementStage::Committed(source_retirement) =
            &commit.source_retirement
        else {
            return None;
        };
        let destination_window = self
            .state
            .borrow()
            .executions
            .get(&commit.identity)?
            .destination_host
            .filter(|window| window.window_id() == commit.destination.window_id())?;
        let post_commit_receipt = DockLiveUndockPostCommitReceipt::pending();
        let post_commit = DockLiveUndockPostCommitPlan::SameWindow {
            identity: commit.identity,
            journal: journal.clone(),
            receipt: post_commit_receipt.clone(),
        };
        let retained_released = commit.retained_release.is_settled();
        let durable = DockLiveUndockDurablePromotionExecution::SameWindow(
            DockLiveUndockDurableSameWindowPromotionExecution {
                identity: commit.identity,
                token: commit.token,
                destination: commit.destination,
                destination_window,
                destination_binding: destination_semantics.binding(),
                registration: viewport.registration.clone(),
                reveal: commit.reveal,
                provisional_session: commit.provisional_session.clone(),
                semantics: commit.semantics.clone(),
                viewport_commit: viewport.clone(),
                controller: commit.controller.clone(),
                graph_commit,
                topology_recovery_required: commit.topology_recovery_required,
                source_host: commit.source_host.downgrade(),
                source_retirement: source_retirement.clone(),
                destination_host: commit.destination_host.downgrade(),
                destination_promotion: destination_promotion.clone(),
                post_commit: post_commit_receipt,
            },
        );
        drop(execution);
        Some(DockLiveUndockCompletedPromotionCommit {
            durable,
            retained_released,
            post_commit,
        })
    }

    fn resume_host_promotion_cleanup(
        &self,
        journal: &Rc<DockLiveUndockPromotionCommitJournal>,
        cx: &mut App,
    ) -> Option<()> {
        let presentation_stage = {
            let mut execution = journal.execution.borrow_mut();
            let DockLiveUndockPromotionCommitExecution::Host(commit) = &mut *execution else {
                return None;
            };
            commit.presentation_cleanup.as_mut().and_then(|cleanup| {
                (!cleanup.presentation_committed).then(|| {
                    (
                        commit.identity,
                        cleanup.payload_lease,
                        cleanup.presentation.take(),
                    )
                })
            })
        };
        if let Some((identity, payload_lease, prepared)) = presentation_stage {
            let prepared = match prepared {
                Some(prepared) => prepared,
                None => match self
                    .prepare_presentation_terminal(
                        identity,
                        payload_lease,
                        view_presentation_window::RehostTerminalIntent::AbandonAfterSourceLoss,
                        cx,
                    )?
                    .ok()?
                {
                    RehostTerminalPreparation::Prepared(prepared) => {
                        DockLiveUndockPreparedPresentationCleanup::Exact(prepared)
                    }
                    RehostTerminalPreparation::AlreadyCommitted(
                        view_presentation_window::RehostTerminalOutcome::Abandoned(receipt),
                    ) => DockLiveUndockPreparedPresentationCleanup::AlreadyTerminal(
                        DockLiveUndockRehostCleanupEvidence::already_absent(
                            receipt.generation(),
                            receipt.source_window(),
                            receipt.destination_window(),
                        ),
                    ),
                    RehostTerminalPreparation::AlreadyCommitted(
                        view_presentation_window::RehostTerminalOutcome::DestinationCommitted(_),
                    ) => return None,
                },
            };
            let abandonment = Self::commit_presentation_cleanup(prepared, cx);
            let mut execution = journal.execution.borrow_mut();
            let DockLiveUndockPromotionCommitExecution::Host(commit) = &mut *execution else {
                return None;
            };
            let cleanup = commit.presentation_cleanup.as_mut()?;
            if abandonment.authority().0 != cleanup.presentation_generation {
                return None;
            }
            cleanup.presentation_committed = true;
            cleanup.provider_refreshed = true;
        }

        let source_host_stage = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::Host(commit) = &*execution else {
                return None;
            };
            commit.presentation_cleanup.as_ref().and_then(|cleanup| {
                (!cleanup.source_host_committed).then(|| cleanup.source_host.clone())
            })
        };
        if let Some(source_host) = source_host_stage {
            let _ = Self::commit_host_presentation_abandonment_without_notify(&source_host, cx);
            if let DockLiveUndockPromotionCommitExecution::Host(commit) =
                &mut *journal.execution.borrow_mut()
                && let Some(cleanup) = commit.presentation_cleanup.as_mut()
            {
                cleanup.source_host_committed = true;
            }
        }

        let provisional_host_stage = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::Host(commit) = &*execution else {
                return None;
            };
            commit.presentation_cleanup.as_ref().and_then(|cleanup| {
                (!cleanup.provisional_host_committed).then(|| cleanup.provisional_host.clone())
            })
        };
        if let Some(provisional_host) = provisional_host_stage {
            let _ =
                Self::commit_host_presentation_abandonment_without_notify(&provisional_host, cx);
            if let DockLiveUndockPromotionCommitExecution::Host(commit) =
                &mut *journal.execution.borrow_mut()
                && let Some(cleanup) = commit.presentation_cleanup.as_mut()
            {
                cleanup.provisional_host_committed = true;
            }
        }

        let retained_release = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::Host(commit) = &*execution else {
                return None;
            };
            commit
                .presentation_cleanup
                .as_ref()
                .and_then(|cleanup| cleanup.retained_release.clone())
        };
        if let Some(retained_release) = retained_release
            && !retained_release.settle(cx)
        {
            return None;
        }

        let retirement_stage = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::Host(commit) = &*execution else {
                return None;
            };
            commit.presentation_cleanup.as_ref().and_then(|cleanup| {
                (!cleanup.session_retired).then_some((commit.identity, cleanup.payload_lease))
            })
        };
        if let Some((identity, payload_lease)) = retirement_stage {
            if !self.retire_presentation_session_after_terminal_commit(identity, payload_lease) {
                return None;
            }
            if let DockLiveUndockPromotionCommitExecution::Host(commit) =
                &mut *journal.execution.borrow_mut()
                && let Some(cleanup) = commit.presentation_cleanup.as_mut()
            {
                cleanup.session_retired = true;
            }
        }

        Some(())
    }

    fn resume_host_promotion_commit(
        &self,
        journal: &Rc<DockLiveUndockPromotionCommitJournal>,
        cx: &mut App,
    ) -> Option<DockLiveUndockCompletedPromotionCommit> {
        let drop_stage = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::Host(commit) = &*execution else {
                return None;
            };
            commit
                .committed_drop
                .is_none()
                .then(|| (commit.runtime.clone(), commit.drop.clone()))
        };
        if let Some((runtime, drop)) = drop_stage {
            if !journal.begin_commit_call() {
                return None;
            }
            let committed = runtime.commit_preflighted_live_undock_host_drop(&drop, cx);
            journal.confirm_irreversible();
            if let DockLiveUndockPromotionCommitExecution::Host(commit) =
                &mut *journal.execution.borrow_mut()
            {
                commit.committed_drop = Some(committed);
            }
        }

        self.resume_host_promotion_cleanup(journal, cx)?;

        let surface_stage = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::Host(commit) = &*execution else {
                return None;
            };
            commit.surface.is_none().then(|| {
                (
                    commit.runtime.clone(),
                    commit
                        .committed_drop
                        .clone()
                        .expect("host drop must commit before surface publication"),
                )
            })
        };
        if let Some((runtime, committed_drop)) = surface_stage {
            let (receipt, publication) =
                runtime.with_deferred_tracked_surface_transaction(cx, |_, receipt, cx| {
                    runtime.publish_live_undock_host_drop_commit(&committed_drop, cx);
                    receipt
                });
            if let DockLiveUndockPromotionCommitExecution::Host(commit) =
                &mut *journal.execution.borrow_mut()
            {
                commit.surface = Some(receipt);
                commit.publication = Some(publication);
            }
        }

        let execution = journal.execution.borrow();
        let DockLiveUndockPromotionCommitExecution::Host(commit) = &*execution else {
            return None;
        };
        let committed_revision = commit.surface.as_ref()?.committed_revision()?;
        if committed_revision <= commit.surface_revision {
            return None;
        }
        let target_is_exact = cx.read_entity(&commit.target_host, |host, _| {
            host.current_window_binding() == Some(commit.target_binding)
                && host.current_viewport_registration() == Some(commit.target_registration.clone())
        });
        let committed = commit.committed_drop.clone()?;
        let outcome = committed.outcome();
        let crate::DockViewportDropRouteOutcome::Action(action) = &outcome else {
            return None;
        };
        if !action.action().changed() {
            return None;
        }
        let post_commit_receipt = DockLiveUndockPostCommitReceipt::pending();
        let post_commit = DockLiveUndockPostCommitPlan::Host {
            identity: commit.identity,
            journal: journal.clone(),
            receipt: post_commit_receipt.clone(),
        };
        let retained_released = commit.presentation_cleanup.as_ref().is_some_and(|cleanup| {
            cleanup
                .retained_release
                .as_ref()
                .is_none_or(DockLiveUndockRetainedVisualRelease::is_settled)
        });
        let durable = DockLiveUndockDurablePromotionExecution::Host(
            DockLiveUndockDurableHostPromotionExecution {
                identity: commit.identity,
                token: commit.token,
                destination: commit.destination,
                destination_window: commit.target_window,
                destination_host: commit.target_host.downgrade(),
                destination_binding: commit.target_binding,
                registration: commit.target_registration.clone(),
                activation: outcome.activation_transaction(),
                committed_destination_recovery_required: commit
                    .committed_destination_recovery_required
                    || !target_is_exact,
                host_drop_commit: committed,
                post_commit: post_commit_receipt,
            },
        );
        drop(execution);
        Some(DockLiveUndockCompletedPromotionCommit {
            durable,
            retained_released,
            post_commit,
        })
    }

    fn prepare_same_window_graph_commit(
        controller: &Entity<DockController>,
        move_plan: &DockViewportTearOffMovePlan,
        allow_forward_rebase: bool,
        cx: &App,
    ) -> Option<DockLiveUndockPreparedGraphCommit> {
        cx.read_entity(controller, |controller, _| {
            let commit_id = controller.workspace().allocate_graph_commit_id();
            let expected = controller.graph().clone();
            let projected = move_plan
                .project_graph(controller.workspace())
                .or_else(|error| {
                    if allow_forward_rebase {
                        move_plan
                            .project_graph_forward_rebased(controller.workspace())
                            .map_err(|_| error)
                    } else {
                        Err(error)
                    }
                });
            let (projected, changed) = projected.ok()?;
            (changed && projected.validate().is_ok()).then_some(DockLiveUndockPreparedGraphCommit {
                commit_id,
                expected,
                projected,
            })
        })
    }

    fn preflight_same_window_promotion_commit(
        &self,
        owner: &Entity<DockSurfaceOwner>,
        prepared: &DockLiveUndockPreparedSameWindowPromotionExecution,
        cx: &mut App,
    ) -> Option<DockLiveUndockPreparedGraphCommit> {
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

        let provider_already_committed = matches!(
            &prepared.presentation,
            RehostTerminalPreparation::AlreadyCommitted(
                view_presentation_window::RehostTerminalOutcome::DestinationCommitted(_)
            )
        );
        if provider_already_committed {
            let payload_lease = prepared.reveal.preflight().mount().proxy().lease();
            let owner_is_current = cx.read_entity(owner, |current, _| {
                current.controller() == prepared.controller
                    && current.accepts_live_undock_identity(prepared.identity)
                    && current
                        .window_session()
                        .admits(prepared.identity.opening().lease())
            });
            let execution_is_current = self
                .state
                .borrow()
                .executions
                .get(&prepared.identity)
                .is_some_and(|execution| {
                    execution.request.key() == prepared.identity.opening()
                        && execution.surface_revision == prepared.surface_revision
                        && execution.destination_host.is_some_and(|destination| {
                            destination.window_id() == prepared.destination.window_id()
                        })
                        && execution.presentation.as_ref().is_some_and(|presentation| {
                            presentation.lease == payload_lease
                                && presentation.reveal == Some(prepared.reveal)
                        })
                });
            let session = prepared.provisional_session.snapshot();
            let semantics = prepared.semantics.snapshot();
            let semantics_is_current = session.window_id() == Some(window_id)
                && session.projects_destination_semantics()
                && semantics.window_id() == window_id
                && semantics.session_generation() == session.generation()
                && semantics.destination_generation() == prepared.token.get()
                && semantics.outcome() != WindowProvisionalSemanticsOutcome::WindowTerminal;
            if !(owner_is_current && execution_is_current && semantics_is_current) {
                return None;
            }
            return Self::prepare_same_window_graph_commit(
                &prepared.controller,
                &prepared.move_plan,
                true,
                cx,
            );
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
        let viewport_is_exact = prepared
            .runtime
            .can_commit_live_undock_provisional_promotion(&prepared.viewport);
        let source_host_is_exact = cx.read_entity(&prepared.source_host, |host, _| {
            host.can_commit_prepared_live_source_retirement(&prepared.source)
        });
        let destination_host_is_exact = cx.read_entity(&prepared.destination_host, |host, _| {
            host.can_commit_prepared_live_destination_promotion(
                &prepared.destination_host_promotion,
            )
        });
        let presentation_is_exact = prepared.presentation.can_commit(cx);
        let retained_is_exact = prepared.retained_release.can_commit(cx);
        if !(owner_is_exact
            && execution_is_exact
            && viewport_is_exact
            && source_host_is_exact
            && destination_host_is_exact
            && presentation_is_exact
            && retained_is_exact)
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

        Self::prepare_same_window_graph_commit(&prepared.controller, &prepared.move_plan, false, cx)
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
                reason: _retirement_reason,
            } => {
                #[cfg(feature = "test-support")]
                eprintln!(
                    "OPEN_GPUI_DOCK_PROVISIONAL_RETIRE identity={identity:?} window={:?} reason={_retirement_reason:?}",
                    window.map(|window| window.window_id()),
                );
                super::retire_live_undock_provisional(
                    owner, identity, window, dependency, binding, runtime, cx,
                );
            }
            DockLiveUndockEffect::ProvisionalAdmitted {
                identity, window, ..
            } => {
                #[cfg(feature = "test-support")]
                eprintln!(
                    "OPEN_GPUI_DOCK_PROVISIONAL_ADMITTED identity={identity:?} window={:?}",
                    window.window_id(),
                );
                match self.prepare_presentation_handoff(identity, window, cx) {
                    Ok(presentation) => {
                        #[cfg(feature = "test-support")]
                        eprintln!(
                            "OPEN_GPUI_DOCK_PRESENTATION_PREPARED identity={identity:?} window={:?}",
                            window.window_id(),
                        );
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
                                DockLiveUndockFact::PresentationLeaseActivated {
                                    identity,
                                    receipt,
                                },
                                cx,
                            );
                        } else {
                            let mut presentation = presentation.expect(
                                "rejected presentation installation must retain its session",
                            );
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
                }
            }
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
                point: reveal_point,
                bounds,
            } => {
                let Some(initial_client_bounds) = gpui_physical_bounds(bounds) else {
                    self.submit_presentation_failure(
                        identity,
                        DockLiveUndockPresentationFailure::ExactRevealTicket { presentation },
                        cx,
                    );
                    return;
                };
                let Some(initial_placement) = WindowPhysicalPlacementRequest::try_new_for_display(
                    initial_client_bounds,
                    point(
                        DevicePixels(reveal_point.x()),
                        DevicePixels(reveal_point.y()),
                    ),
                    bounds.target_display(),
                ) else {
                    self.submit_presentation_failure(
                        identity,
                        DockLiveUndockPresentationFailure::ExactRevealTicket { presentation },
                        cx,
                    );
                    return;
                };
                let destination = self
                    .state
                    .borrow()
                    .executions
                    .get(&identity)
                    .and_then(|execution| {
                        let authority = execution.presentation.as_ref()?;
                        let key = authority.destination_key?;
                        let host = execution.destination_host?;
                        (authority.lease == presentation.mount().proxy().lease()
                            && host.window_id() == window.window_id())
                        .then(|| {
                            (
                                host,
                                key,
                                execution.request.provisional_session().clone(),
                                execution.seed.source.runtime.clone(),
                                execution.seed.source.work_context,
                            )
                        })
                    })
                    .and_then(|(host, key, provisional_session, runtime, work_context)| {
                        runtime
                            .provisional_peer_window_ids(
                                work_context,
                                identity.opening(),
                                window.window_id(),
                            )
                            .map(|peer_windows| (host, key, provisional_session, peer_windows))
                    });
                let reveal_runtime = self.clone();
                let reveal_outcome = destination
                    .map(|(host, key, provisional_session, peer_windows)| {
                        host.update(cx, |host, destination_window, cx| {
                            if !host.can_arm_live_destination_reveal(key, presentation) {
                                return DockLiveUndockRevealArmOutcome::Rejected;
                            }
                            match destination_window.presentation_facts().initial_presentation {
                                WindowInitialPresentationStatus::Pending => {
                                    let reveal_runtime = reveal_runtime.clone();
                                    destination_window
                                        .observe_window_initial_presentation(move |window, cx| {
                                            match window.presentation_facts().initial_presentation {
                                                WindowInitialPresentationStatus::Completed => {
                                                    let _ = reveal_runtime.submit(
                                                        DockLiveUndockFact::InitialPresentationReady {
                                                            identity,
                                                            presentation,
                                                        },
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
                                .arm_provisional_presentation_with_initial_physical_placement(
                                    &provisional_session,
                                    initial_placement,
                                    peer_windows,
                                    cx,
                                )
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
            DockLiveUndockEffect::RequestRoutePlacement {
                identity,
                window,
                generation,
                point,
                bounds,
            } => self.request_route_placement(identity, window, generation, point, bounds, cx),
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
                        Some(if prepared.provider_destination_already_committed() {
                            DockLiveUndockPromotionExecution::Committing(Rc::new(
                                DockLiveUndockPromotionCommitJournal::pending(prepared),
                            ))
                        } else {
                            DockLiveUndockPromotionExecution::Prepared(prepared)
                        });
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
                let (journal, committed_recovery_required) = {
                    let mut state = self.state.borrow_mut();
                    let Some(execution) = state.executions.get_mut(&identity) else {
                        return;
                    };
                    if execution.request.key() != identity.opening() {
                        return;
                    }
                    match execution.promotion.as_ref() {
                        Some(DockLiveUndockPromotionExecution::Prepared(prepared))
                            if prepared.identity() == identity
                                && prepared.token() == token
                                && prepared.destination() == destination
                                && execution.surface_revision == prepared.surface_revision() =>
                        {
                            let Some(DockLiveUndockPromotionExecution::Prepared(prepared)) =
                                execution.promotion.take()
                            else {
                                unreachable!("validated prepared promotion changed before commit")
                            };
                            let journal =
                                Rc::new(DockLiveUndockPromotionCommitJournal::pending(prepared));
                            execution.promotion = Some(
                                DockLiveUndockPromotionExecution::Committing(journal.clone()),
                            );
                            (Some(journal), None)
                        }
                        Some(DockLiveUndockPromotionExecution::Committing(current))
                            if {
                                current.identity() == identity
                                    && current.token() == token
                                    && current.destination() == destination
                                    && current.surface_revision() == execution.surface_revision
                            } =>
                        {
                            (Some(current.clone()), None)
                        }
                        Some(DockLiveUndockPromotionExecution::Durable(durable))
                            if durable.identity() == identity
                                && durable.token() == token
                                && durable.destination() == destination =>
                        {
                            (
                                None,
                                Some(durable.committed_destination_recovery_required()),
                            )
                        }
                        Some(
                            DockLiveUndockPromotionExecution::Prepared(_)
                            | DockLiveUndockPromotionExecution::Committing(_)
                            | DockLiveUndockPromotionExecution::Durable(_),
                        )
                        | None => return,
                    }
                };

                if let Some(recovery_required) = committed_recovery_required {
                    let fact = if recovery_required {
                        DockLiveUndockFact::CommittedDestinationRecoveryRequired {
                            identity,
                            token,
                            destination,
                        }
                    } else {
                        DockLiveUndockFact::DurableSwapCommitted { identity, token }
                    };
                    self.enqueue_fact(DockLiveUndockQueuedFact::Reduce(fact), cx);
                    return;
                }

                let journal = journal.expect("promotion commit must retain its exact journal");
                if !journal.begin_drive() {
                    return;
                }
                let durable = catch_unwind(AssertUnwindSafe(|| {
                    if !self.preflight_promotion_commit_journal(owner, &journal, cx) {
                        return None;
                    }
                    let same_window = match &*journal.execution.borrow() {
                        DockLiveUndockPromotionCommitExecution::SameWindow(_) => true,
                        DockLiveUndockPromotionCommitExecution::Host(_) => false,
                        DockLiveUndockPromotionCommitExecution::Pending(_)
                        | DockLiveUndockPromotionCommitExecution::Aborted => return None,
                    };
                    if same_window {
                        if journal.has_irreversible_authority() {
                            self.resume_same_window_promotion_commit(&journal, cx)
                        } else {
                            Some(self.commit_preflighted_same_window_promotion(owner, &journal, cx))
                        }
                    } else {
                        if journal.has_irreversible_authority() {
                            self.resume_host_promotion_commit(&journal, cx)
                        } else {
                            Some(self.commit_preflighted_host_promotion(owner, &journal, cx))
                        }
                    }
                }));
                let completed = match durable {
                    Ok(Some(completed)) => completed,
                    Ok(None) => {
                        self.settle_promotion_commit_attempt_failure(&journal, cx);
                        return;
                    }
                    Err(payload) => {
                        self.settle_promotion_commit_attempt_failure(&journal, cx);
                        resume_unwind(payload);
                    }
                };

                let DockLiveUndockCompletedPromotionCommit {
                    durable,
                    retained_released,
                    post_commit,
                } = completed;
                let committed_destination_recovery_required =
                    durable.committed_destination_recovery_required();
                {
                    let mut state = self.state.borrow_mut();
                    let execution = state
                        .executions
                        .get_mut(&identity)
                        .expect("durable live-undock promotion must retain its execution");
                    let current_is_exact = matches!(
                        execution.promotion.as_ref(),
                        Some(DockLiveUndockPromotionExecution::Committing(current))
                            if Rc::ptr_eq(current, &journal)
                    );
                    if !current_is_exact {
                        return;
                    }
                    if retained_released && let Some(presentation) = execution.presentation.as_mut()
                    {
                        presentation.retained_released = true;
                    }
                    execution.promotion = Some(DockLiveUndockPromotionExecution::Durable(durable));
                }
                journal.finish_drive();
                let fact = if committed_destination_recovery_required {
                    DockLiveUndockFact::CommittedDestinationRecoveryRequired {
                        identity,
                        token,
                        destination,
                    }
                } else {
                    DockLiveUndockFact::DurableSwapCommitted { identity, token }
                };
                self.enqueue_fact(DockLiveUndockQueuedFact::Reduce(fact), cx);
                post_commit.start(self.clone(), cx);
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
                    let state = self.state.borrow();
                    let Some(execution) = state.executions.get(&identity) else {
                        return;
                    };
                    let runtime = execution.seed.source.runtime.clone();
                    let committed = match execution.promotion.as_ref() {
                        Some(DockLiveUndockPromotionExecution::Durable(
                            DockLiveUndockDurablePromotionExecution::Host(durable),
                        )) if durable.identity == identity
                            && durable.token == token
                            && durable.destination == destination
                            && durable.host_drop_commit.window_effects_receipt().is_none() =>
                        {
                            Some(durable.host_drop_commit.clone())
                        }
                        _ => None,
                    };
                    committed.map(|committed| (runtime, committed))
                };
                if let Some((runtime, committed)) = pending {
                    let applied = catch_unwind(AssertUnwindSafe(|| {
                        accept_host_drop_window_effects(&runtime, &committed, cx)
                    }));
                    match applied {
                        Ok(Some(_acceptance)) => {
                            if let Some(execution) =
                                self.state.borrow_mut().executions.get_mut(&identity)
                            {
                                execution.committed_window_effects_retry.clear();
                            }
                        }
                        Ok(None) => self.schedule_committed_window_effects_retry(
                            identity,
                            token,
                            destination,
                            cx,
                        ),
                        Err(payload) => {
                            self.schedule_committed_window_effects_retry(
                                identity,
                                token,
                                destination,
                                cx,
                            );
                            resume_unwind(payload);
                        }
                    }
                }
            }
            DockLiveUndockEffect::DestinationSemanticsSubmissionRequired {
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
                        self.arm_destination_semantics_watchdog(identity, token, destination, cx);
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
                                || DockLiveUndockFact::DestinationSemanticsSubmissionFailed {
                                    identity,
                                    token,
                                    destination,
                                },
                                |receipt| DockLiveUndockFact::DestinationSemanticsSubmitted {
                                    identity,
                                    receipt,
                                },
                            );
                        self.enqueue_fact(DockLiveUndockQueuedFact::Reduce(fact), cx);
                    }
                    None => {
                        self.enqueue_fact(
                            DockLiveUndockQueuedFact::Reduce(
                                DockLiveUndockFact::DestinationSemanticsSubmissionFailed {
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
                        controller: Entity<DockController>,
                        graph_commit: Option<DockWorkspaceGraphCommitReceipt>,
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
                #[cfg(test)]
                let before_admission = self
                    .state
                    .borrow_mut()
                    .before_destination_interaction_admission_test_hook
                    .take();
                #[cfg(test)]
                if let Some(before_admission) = before_admission {
                    before_admission(cx);
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
                                    controller: durable.controller.clone(),
                                    graph_commit: durable.graph_commit,
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
                                controller,
                                graph_commit,
                                source_host,
                                source_key,
                                source_lease,
                            } => {
                                if !workspace_graph_projection_is_exact(
                                    &controller,
                                    graph_commit,
                                    cx,
                                ) {
                                    return None;
                                }
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
                                let admission = window.update(cx, |_, window, cx| {
                                    if !workspace_graph_projection_is_exact(
                                        &controller,
                                        graph_commit,
                                        cx,
                                    ) {
                                        return None;
                                    }
                                    window
                                        .admit_provisional_interaction(&session, &ticket, cx)
                                        .ok()?;
                                    DockLiveUndockDestinationInteractionReceipt::new_same_window(
                                        semantics, &session,
                                    )
                                });
                                admission.ok().flatten()
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
                if self.promotion_commit_forbids_rollback(identity) {
                    self.clear_orphan_recovery_retry(identity);
                    return;
                }
                if let Some(fact) = self.recover_orphaned_payload_topology(
                    owner,
                    identity,
                    payload_lease,
                    provisional,
                    cx,
                ) {
                    self.clear_orphan_recovery_retry(identity);
                    self.enqueue_fact(DockLiveUndockQueuedFact::Reduce(fact), cx);
                } else if !self.promotion_commit_forbids_rollback(identity) {
                    self.schedule_orphan_recovery_retry(identity, payload_lease, provisional, cx);
                }
            }
            DockLiveUndockEffect::ShutdownOrphanRecoveryRequired {
                identity,
                payload_lease,
                provisional,
            } => {
                self.clear_orphan_recovery_retry(identity);
                if self.promotion_commit_forbids_rollback(identity) {
                    return;
                }
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
                let attempt = catch_unwind(AssertUnwindSafe(|| {
                    self.attempt_committed_destination_recovery(
                        owner,
                        identity,
                        authority,
                        token,
                        destination,
                        cx,
                    )
                }));
                match attempt {
                    Ok(Ok(recovery)) => {
                        self.clear_committed_destination_recovery_retry(identity);
                        self.enqueue_fact(
                            DockLiveUndockQueuedFact::Reduce(
                                DockLiveUndockFact::CommittedDestinationRecoveryCommitted {
                                    identity,
                                    receipt: recovery,
                                },
                            ),
                            cx,
                        );
                    }
                    Ok(Err(_)) => self.schedule_committed_destination_recovery_retry(
                        identity,
                        authority,
                        token,
                        destination,
                        cx,
                    ),
                    Err(payload) => {
                        self.schedule_committed_destination_recovery_retry(
                            identity,
                            authority,
                            token,
                            destination,
                            cx,
                        );
                        resume_unwind(payload);
                    }
                }
            }
            DockLiveUndockEffect::ShutdownCommittedDestinationRecoveryRequired {
                identity,
                authority,
                token,
                destination,
            } => {
                self.clear_committed_destination_recovery_retry(identity);
                let attempt = catch_unwind(AssertUnwindSafe(|| {
                    self.attempt_committed_destination_recovery(
                        owner,
                        identity,
                        authority,
                        token,
                        destination,
                        cx,
                    )
                }));
                let (fact, panic) = match attempt {
                    Ok(Ok(recovery)) => (
                        DockLiveUndockFact::CommittedDestinationRecoveryCommitted {
                            identity,
                            receipt: recovery,
                        },
                        None,
                    ),
                    Ok(Err(failure)) => (
                        DockLiveUndockFact::ShutdownCommittedDestinationRecoveryFailed {
                            identity,
                            authority,
                            token,
                            destination,
                            failure,
                        },
                        None,
                    ),
                    Err(payload) => (
                        DockLiveUndockFact::ShutdownCommittedDestinationRecoveryFailed {
                            identity,
                            authority,
                            token,
                            destination,
                            failure: DockLiveUndockCommittedDestinationRecoveryFailure::Panicked,
                        },
                        Some(payload),
                    ),
                };
                self.enqueue_fact(DockLiveUndockQueuedFact::Reduce(fact), cx);
                if let Some(payload) = panic {
                    resume_unwind(payload);
                }
            }
            DockLiveUndockEffect::RetireCommittedSameWindowDestination {
                identity,
                token,
                window_id,
            } => {
                enum RetirementAuthority {
                    Durable(WindowHandle<DockHost>),
                    ProviderTerminal {
                        window: WindowHandle<DockHost>,
                        host: Entity<DockHost>,
                    },
                    RegisteredHost {
                        window: WindowHandle<DockHost>,
                        host: Entity<DockHost>,
                        semantics: DockHostLiveDestinationSemantics,
                    },
                }

                let destination =
                    DockLiveUndockPromotionDestination::SameWindowDesktop { window_id };
                let retirement =
                    self.state
                        .borrow()
                        .executions
                        .get(&identity)
                        .and_then(|execution| {
                            if execution.request.key() != identity.opening()
                                || execution.destination_host?.window_id() != window_id
                                || !execution.presentation.as_ref().is_some_and(|presentation| {
                                    presentation.lease.identity() == identity
                                        && presentation.lease.destination_window() == window_id
                                })
                            {
                                return None;
                            }
                            match execution.promotion.as_ref()? {
                                DockLiveUndockPromotionExecution::Durable(
                                    DockLiveUndockDurablePromotionExecution::SameWindow(durable),
                                ) if durable.identity == identity
                                    && durable.token == token
                                    && durable.destination == destination
                                    && durable.destination_window.window_id() == window_id
                                    && durable.destination_binding.window_id() == window_id
                                    && durable.destination_binding.generation() != 0
                                    && durable.registration.window_id() == window_id
                                    && durable.registration.lineage()
                                        == crate::DockViewportRuntimeLineage::Surface(
                                            identity.opening().lease(),
                                        ) =>
                                {
                                    Some(RetirementAuthority::Durable(durable.destination_window))
                                }
                                DockLiveUndockPromotionExecution::Committing(journal)
                                    if journal.identity() == identity
                                        && journal.token() == token
                                        && journal.destination() == destination
                                        && journal.recovery_receipt().is_some()
                                        && journal.recovery_requires_window_terminal() =>
                                {
                                    let journal_execution = journal.execution.borrow();
                                    match &*journal_execution {
                                        DockLiveUndockPromotionCommitExecution::Pending(Some(
                                            DockLiveUndockPreparedPromotionExecution::SameWindow(
                                                prepared,
                                            ),
                                        )) => Some(RetirementAuthority::ProviderTerminal {
                                            window: execution.destination_host?,
                                            host: prepared.destination_host.clone(),
                                        }),
                                        DockLiveUndockPromotionCommitExecution::SameWindow(
                                            commit,
                                        ) => {
                                            if let Some(semantics) = commit
                                                .destination_promotion
                                                .as_ref()
                                                .map(|promotion| promotion.semantics().clone())
                                            {
                                                Some(RetirementAuthority::RegisteredHost {
                                                    window: execution.destination_host?,
                                                    host: commit.destination_host.clone(),
                                                    semantics,
                                                })
                                            } else {
                                                Some(RetirementAuthority::ProviderTerminal {
                                                    window: execution.destination_host?,
                                                    host: commit.destination_host.clone(),
                                                })
                                            }
                                        }
                                        DockLiveUndockPromotionCommitExecution::Pending(_)
                                        | DockLiveUndockPromotionCommitExecution::Host(_)
                                        | DockLiveUndockPromotionCommitExecution::Aborted => None,
                                    }
                                }
                                DockLiveUndockPromotionExecution::Prepared(_)
                                | DockLiveUndockPromotionExecution::Committing(_)
                                | DockLiveUndockPromotionExecution::Durable(_) => None,
                            }
                        });
                let destination_window = match retirement {
                    Some(RetirementAuthority::Durable(window)) => Some(window),
                    Some(RetirementAuthority::ProviderTerminal { window, host }) => window
                        .entity(cx)
                        .ok()
                        .filter(|current| current == &host)
                        .filter(|current| {
                            cx.read_entity(current, |host, _| {
                                host.is_provisional_viewport_for(identity.opening())
                                    && host.current_viewport_registration().is_none()
                            })
                        })
                        .map(|_| window),
                    Some(RetirementAuthority::RegisteredHost {
                        window,
                        host,
                        semantics,
                    }) => window
                        .entity(cx)
                        .ok()
                        .filter(|current| current == &host)
                        .filter(|current| {
                            cx.read_entity(current, |host, _| {
                                host.accepts_live_destination_semantics(&semantics)
                            })
                        })
                        .map(|_| window),
                    None => None,
                };
                let Some(destination_window) = destination_window else {
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
        let terminal_is_current = self
            .state
            .borrow_mut()
            .executions
            .get_mut(&identity)
            .is_some_and(|execution| {
                execution.terminal_requested = true;
                true
            });
        if !terminal_is_current {
            return;
        }
        let session_checked_out = self
            .state
            .borrow()
            .executions
            .get(&identity)
            .and_then(|execution| execution.presentation.as_ref())
            .is_some_and(|presentation| presentation.session.is_checked_out());
        if session_checked_out {
            self.schedule_terminal_settlement_retry(identity, cx);
            return;
        }

        let settlement = catch_unwind(AssertUnwindSafe(|| {
            self.settle_terminal_promotion_authority(identity, cx)
        }));
        match settlement {
            Ok(true) => {}
            Ok(false) => {
                self.schedule_terminal_settlement_retry(identity, cx);
                return;
            }
            Err(payload) => {
                self.schedule_terminal_settlement_retry(identity, cx);
                resume_unwind(payload);
            }
        }

        let finalization = self
            .state
            .borrow()
            .executions
            .get(&identity)
            .map(|execution| {
                (
                    execution.seed.source.payload_finalizer.clone(),
                    execution.seed.source.runtime.clone(),
                    execution.seed.source.work_context,
                    execution.seed.source.session.clone(),
                )
            });
        let Some((finalizer, runtime, work_context, session)) = finalization else {
            return;
        };
        let finalization = catch_unwind(AssertUnwindSafe(|| {
            finalize_payload_drag_after_terminal(
                &finalizer,
                identity,
                cx,
                |_| {},
                |claim, cx| {
                    settle_payload_drag_finalizer_claim(claim, &runtime, work_context, &session, cx)
                },
            )
        }));
        let settled = match finalization {
            Ok(
                DockPayloadDragFinalization::Finalized
                | DockPayloadDragFinalization::TransferredToSurfaceShutdown,
            ) => true,
            Ok(DockPayloadDragFinalization::NotClaimed) => finalizer.is_terminally_settled(),
            Err(payload) => {
                self.schedule_terminal_settlement_retry(identity, cx);
                resume_unwind(payload);
            }
        };
        if !settled {
            self.schedule_terminal_settlement_retry(identity, cx);
            return;
        }

        let execution = self.state.borrow_mut().executions.remove(&identity);
        if let Some(execution) = execution
            && execution.seed.source.identity_slot.get() == Some(identity)
        {
            execution.seed.source.identity_slot.set(None);
        }
    }

    fn settle_terminal_promotion_authority(
        &self,
        identity: DockLiveUndockIdentity,
        cx: &mut App,
    ) -> bool {
        enum Authority {
            None,
            Prepared,
            Committing(Rc<DockLiveUndockPromotionCommitJournal>),
            DurableSameWindow {
                runtime: DockViewportRuntimeHandle,
                committed: crate::viewport_runtime::DockViewportCommittedLiveUndockPromotion,
                controller: Entity<DockController>,
                graph_commit: Option<DockWorkspaceGraphCommitReceipt>,
                source_host: WeakEntity<DockHost>,
                source_retirement: DockHostLiveSourceRetirementReceipt,
                destination_host: WeakEntity<DockHost>,
                destination_promotion: DockHostLiveDestinationPromotionReceipt,
                post_commit: DockLiveUndockPostCommitReceipt,
            },
            DurableHost {
                runtime: DockViewportRuntimeHandle,
                committed: DockViewportCommittedLiveUndockHostDrop,
                post_commit: DockLiveUndockPostCommitReceipt,
            },
        }

        let authority = {
            let state = self.state.borrow();
            let Some(execution) = state.executions.get(&identity) else {
                return true;
            };
            match execution.promotion.as_ref() {
                None => Authority::None,
                Some(DockLiveUndockPromotionExecution::Prepared(_)) => Authority::Prepared,
                Some(DockLiveUndockPromotionExecution::Committing(journal)) => {
                    Authority::Committing(journal.clone())
                }
                Some(DockLiveUndockPromotionExecution::Durable(
                    DockLiveUndockDurablePromotionExecution::SameWindow(durable),
                )) => Authority::DurableSameWindow {
                    runtime: execution.seed.source.runtime.clone(),
                    committed: durable.viewport_commit.clone(),
                    controller: durable.controller.clone(),
                    graph_commit: durable.graph_commit,
                    source_host: durable.source_host.clone(),
                    source_retirement: durable.source_retirement.clone(),
                    destination_host: durable.destination_host.clone(),
                    destination_promotion: durable.destination_promotion.clone(),
                    post_commit: durable.post_commit.clone(),
                },
                Some(DockLiveUndockPromotionExecution::Durable(
                    DockLiveUndockDurablePromotionExecution::Host(durable),
                )) => Authority::DurableHost {
                    runtime: execution.seed.source.runtime.clone(),
                    committed: durable.host_drop_commit.clone(),
                    post_commit: durable.post_commit.clone(),
                },
            }
        };

        match authority {
            Authority::None | Authority::Prepared => true,
            Authority::Committing(journal) => {
                let route = {
                    let execution = journal.execution.borrow();
                    match &*execution {
                        DockLiveUndockPromotionCommitExecution::SameWindow(_) => 0,
                        DockLiveUndockPromotionCommitExecution::Host(_) => 1,
                        DockLiveUndockPromotionCommitExecution::Pending(_) => 2,
                        DockLiveUndockPromotionCommitExecution::Aborted => 3,
                    }
                };
                if journal.recovery_receipt().is_some() {
                    match route {
                        0 | 2 => {
                            return self.settle_recovered_same_window_promotion(&journal, cx);
                        }
                        1 => return self.settle_recovered_host_promotion(&journal, cx),
                        _ => {}
                    }
                }
                let completed = match route {
                    0 => self.resume_same_window_promotion_commit(&journal, cx),
                    1 => self.resume_host_promotion_commit(&journal, cx),
                    2 => return self.settle_failed_promotion_commit_journal(&journal, cx),
                    3 => return true,
                    _ => unreachable!(),
                };
                if let Some(DockLiveUndockCompletedPromotionCommit {
                    durable,
                    retained_released,
                    post_commit,
                }) = completed
                {
                    let mut state = self.state.borrow_mut();
                    let Some(execution) = state.executions.get_mut(&identity) else {
                        return true;
                    };
                    if matches!(
                        execution.promotion.as_ref(),
                        Some(DockLiveUndockPromotionExecution::Committing(current))
                            if Rc::ptr_eq(current, &journal)
                    ) {
                        if retained_released
                            && let Some(presentation) = execution.presentation.as_mut()
                        {
                            presentation.retained_released = true;
                        }
                        execution.promotion =
                            Some(DockLiveUndockPromotionExecution::Durable(durable));
                    }
                    drop(state);
                    journal.finish_drive();
                    post_commit.start(self.clone(), cx);
                    return self.settle_terminal_promotion_authority(identity, cx);
                }
                self.settle_failed_promotion_commit_journal(&journal, cx)
            }
            Authority::DurableSameWindow {
                runtime,
                committed,
                controller,
                graph_commit,
                source_host,
                source_retirement,
                destination_host,
                destination_promotion,
                post_commit,
            } => {
                if !post_commit.is_settled() {
                    return false;
                }
                runtime.retire_live_undock_provisional_promotion_commit(&committed);
                if let Some(graph_commit) = graph_commit {
                    cx.update_entity(&controller, |controller, _| {
                        controller.workspace_mut().retire_graph_commit(graph_commit);
                    });
                }
                if let Some(source_host) = source_host.upgrade()
                    && !cx.update_entity(&source_host, |host, _| {
                        host.retire_live_source_retirement(&source_retirement)
                    })
                {
                    return false;
                }
                if let Some(destination_host) = destination_host.upgrade()
                    && !cx.update_entity(&destination_host, |host, _| {
                        host.retire_live_destination_promotion(&destination_promotion)
                    })
                {
                    return false;
                }
                true
            }
            Authority::DurableHost {
                runtime,
                committed,
                post_commit,
            } => {
                if !post_commit.is_settled() {
                    return false;
                }
                let Some(acceptance) = accept_host_drop_window_effects(&runtime, &committed, cx)
                else {
                    return false;
                };
                runtime.retire_live_undock_host_drop_commit(&committed, acceptance, cx)
            }
        }
    }

    fn settle_failed_promotion_commit_journal(
        &self,
        journal: &Rc<DockLiveUndockPromotionCommitJournal>,
        cx: &mut App,
    ) -> bool {
        if let Some(settled) = self.settle_provider_terminal_pending_promotion(journal, cx) {
            return settled;
        }

        let route = {
            let execution = journal.execution.borrow();
            match &*execution {
                DockLiveUndockPromotionCommitExecution::SameWindow(_) => 0,
                DockLiveUndockPromotionCommitExecution::Host(_) => 1,
                DockLiveUndockPromotionCommitExecution::Pending(_) => 2,
                DockLiveUndockPromotionCommitExecution::Aborted => 3,
            }
        };
        let mut first_panic = None;
        let post_commit_settled = match route {
            0 => self.drive_same_window_post_commit_journal(journal, cx, &mut first_panic),
            1 => self.drive_host_post_commit_journal(journal, false, cx, &mut first_panic),
            2 | 3 => true,
            _ => unreachable!(),
        };
        if let Some(payload) = first_panic {
            resume_unwind(payload);
        }
        if !post_commit_settled {
            return false;
        }

        enum ForwardSettlement {
            SameWindow {
                runtime: DockViewportRuntimeHandle,
                committed: crate::viewport_runtime::DockViewportCommittedLiveUndockPromotion,
                controller: Entity<DockController>,
                graph_commit: DockWorkspaceGraphCommitReceipt,
                source_host: Entity<DockHost>,
                source_retirement: DockHostLiveSourceRetirementReceipt,
                destination_host: Entity<DockHost>,
                destination_promotion: DockHostLiveDestinationPromotionReceipt,
            },
            Host {
                runtime: DockViewportRuntimeHandle,
                committed: DockViewportCommittedLiveUndockHostDrop,
                retire_lower_receipt: bool,
            },
            Pending,
            Settled,
        }

        let settlement = {
            let execution = journal.execution.borrow();
            match &*execution {
                DockLiveUndockPromotionCommitExecution::SameWindow(commit) => {
                    let post_commit_settled = commit.presentation_session_retired
                        && commit.controller_notified
                        && commit.source_host_notified
                        && commit.destination_host_notified
                        && commit.viewport_refreshed
                        && commit
                            .publication
                            .as_ref()
                            .is_some_and(DockSurfaceDeferredPublication::is_settled)
                        && commit
                            .surface
                            .as_ref()
                            .and_then(DockSurfaceTransactionReceipt::committed_revision)
                            .is_some();
                    match (
                        post_commit_settled,
                        commit.committed_viewport.clone(),
                        commit.graph_commit,
                        &commit.source_retirement,
                        commit.destination_promotion.clone(),
                    ) {
                        (
                            true,
                            Some(committed),
                            Some(graph_commit),
                            DockLiveUndockSourceRetirementStage::Committed(source_retirement),
                            Some(destination_promotion),
                        ) => ForwardSettlement::SameWindow {
                            runtime: commit.runtime.clone(),
                            committed,
                            controller: commit.controller.clone(),
                            graph_commit,
                            source_host: commit.source_host.clone(),
                            source_retirement: source_retirement.clone(),
                            destination_host: commit.destination_host.clone(),
                            destination_promotion,
                        },
                        _ => ForwardSettlement::Pending,
                    }
                }
                DockLiveUndockPromotionCommitExecution::Host(commit) => {
                    let cleanup_settled =
                        commit.presentation_cleanup.as_ref().is_none_or(|cleanup| {
                            cleanup.presentation_committed
                                && cleanup.source_host_committed
                                && cleanup.provisional_host_committed
                                && cleanup
                                    .retained_release
                                    .as_ref()
                                    .is_none_or(DockLiveUndockRetainedVisualRelease::is_settled)
                                && cleanup.session_retired
                        });
                    let post_commit_settled = cleanup_settled
                        && commit.host_drop_notified
                        && commit
                            .publication
                            .as_ref()
                            .is_some_and(DockSurfaceDeferredPublication::is_settled)
                        && commit
                            .surface
                            .as_ref()
                            .and_then(DockSurfaceTransactionReceipt::committed_revision)
                            .is_some();
                    match (post_commit_settled, commit.committed_drop.clone()) {
                        (true, Some(committed)) => ForwardSettlement::Host {
                            runtime: commit.runtime.clone(),
                            committed,
                            retire_lower_receipt: !commit.lower_receipt_retired,
                        },
                        _ => ForwardSettlement::Pending,
                    }
                }
                DockLiveUndockPromotionCommitExecution::Pending(_) => ForwardSettlement::Pending,
                DockLiveUndockPromotionCommitExecution::Aborted => ForwardSettlement::Settled,
            }
        };

        match settlement {
            ForwardSettlement::SameWindow {
                runtime,
                committed,
                controller,
                graph_commit,
                source_host,
                source_retirement,
                destination_host,
                destination_promotion,
            } => {
                runtime.retire_live_undock_provisional_promotion_commit(&committed);
                cx.update_entity(&controller, |controller, _| {
                    controller.workspace_mut().retire_graph_commit(graph_commit);
                });
                if !cx.update_entity(&source_host, |host, _| {
                    host.retire_live_source_retirement(&source_retirement)
                }) {
                    return false;
                }
                if !cx.update_entity(&destination_host, |host, _| {
                    host.retire_live_destination_promotion(&destination_promotion)
                }) {
                    return false;
                }
                true
            }
            ForwardSettlement::Host {
                runtime,
                committed,
                retire_lower_receipt,
            } => {
                let Some(acceptance) = accept_host_drop_window_effects(&runtime, &committed, cx)
                else {
                    return false;
                };
                if retire_lower_receipt {
                    if !runtime.retire_live_undock_host_drop_commit(&committed, acceptance, cx) {
                        return false;
                    }
                    if let DockLiveUndockPromotionCommitExecution::Host(commit) =
                        &mut *journal.execution.borrow_mut()
                    {
                        commit.lower_receipt_retired = true;
                    }
                }
                true
            }
            ForwardSettlement::Pending => false,
            ForwardSettlement::Settled => true,
        }
    }

    fn settle_recovered_host_promotion(
        &self,
        journal: &Rc<DockLiveUndockPromotionCommitJournal>,
        cx: &mut App,
    ) -> bool {
        if journal.recovery_receipt().is_none()
            || self.resume_host_promotion_cleanup(journal, cx).is_none()
        {
            return false;
        }

        let mut first_panic = None;
        let post_commit_settled =
            self.drive_host_post_commit_journal(journal, false, cx, &mut first_panic);
        if let Some(payload) = first_panic {
            resume_unwind(payload);
        }
        if !post_commit_settled {
            return false;
        }

        enum Settlement {
            ApplyWindowEffects {
                runtime: DockViewportRuntimeHandle,
                committed: DockViewportCommittedLiveUndockHostDrop,
            },
            RetireLowerReceipt {
                runtime: DockViewportRuntimeHandle,
                committed: DockViewportCommittedLiveUndockHostDrop,
            },
            Settled,
            Pending,
        }

        loop {
            let settlement = {
                let execution = journal.execution.borrow();
                let DockLiveUndockPromotionCommitExecution::Host(commit) = &*execution else {
                    return false;
                };
                let cleanup_settled = commit.presentation_cleanup.as_ref().is_none_or(|cleanup| {
                    cleanup.presentation_committed
                        && cleanup.source_host_committed
                        && cleanup.provisional_host_committed
                        && cleanup
                            .retained_release
                            .as_ref()
                            .is_none_or(DockLiveUndockRetainedVisualRelease::is_settled)
                        && cleanup.session_retired
                });
                let surface_settled = commit
                    .surface
                    .as_ref()
                    .is_none_or(|receipt| receipt.committed_revision().is_some());
                let publication_settled = commit
                    .publication
                    .as_ref()
                    .is_none_or(DockSurfaceDeferredPublication::is_settled);
                let Some(committed) = commit.committed_drop.clone() else {
                    return false;
                };
                if !(cleanup_settled && surface_settled && publication_settled) {
                    Settlement::Pending
                } else if committed.window_effects_receipt().is_none() {
                    Settlement::ApplyWindowEffects {
                        runtime: commit.runtime.clone(),
                        committed,
                    }
                } else if !commit.lower_receipt_retired {
                    Settlement::RetireLowerReceipt {
                        runtime: commit.runtime.clone(),
                        committed,
                    }
                } else {
                    Settlement::Settled
                }
            };

            match settlement {
                Settlement::ApplyWindowEffects { runtime, committed } => {
                    if accept_host_drop_window_effects(&runtime, &committed, cx).is_none() {
                        return false;
                    }
                }
                Settlement::RetireLowerReceipt { runtime, committed } => {
                    let Some(acceptance) = committed.window_effects_receipt() else {
                        return false;
                    };
                    if !runtime.retire_live_undock_host_drop_commit(&committed, acceptance, cx) {
                        return false;
                    }
                    if let DockLiveUndockPromotionCommitExecution::Host(commit) =
                        &mut *journal.execution.borrow_mut()
                    {
                        commit.lower_receipt_retired = true;
                    }
                }
                Settlement::Settled => return true,
                Settlement::Pending => return false,
            }
        }
    }

    fn settle_provider_terminal_pending_promotion(
        &self,
        journal: &Rc<DockLiveUndockPromotionCommitJournal>,
        cx: &mut App,
    ) -> Option<bool> {
        let (batch, source_host, source, retained_release, payload_lease) = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::Pending(Some(
                DockLiveUndockPreparedPromotionExecution::SameWindow(prepared),
            )) = &*execution
            else {
                return None;
            };
            let RehostTerminalPreparation::AlreadyCommitted(
                view_presentation_window::RehostTerminalOutcome::DestinationCommitted(batch),
            ) = &prepared.presentation
            else {
                return Some(false);
            };
            (
                batch.clone(),
                prepared.source_host.clone(),
                prepared.source.clone(),
                prepared.retained_release.clone(),
                prepared.reveal.preflight().mount().proxy().lease(),
            )
        };
        if journal.recovery_receipt().is_none() {
            return Some(false);
        }

        view_presentation_window::release_stable_leases_after_endpoint_loss(cx, batch.leases());
        let source_key = source.key();
        let source_retired = cx.update_entity(&source_host, |host, host_cx| {
            if let Some(receipt) =
                host.commit_or_replay_prepared_live_source_retirement_without_notify(source)
            {
                host_cx.notify();
                return host.retire_live_source_retirement(&receipt);
            }
            !host.accepts_live_presentation_key(source_key)
        });
        if !source_retired {
            return Some(false);
        }

        if !retained_release.settle(cx) {
            return Some(false);
        }
        if !self
            .retire_presentation_session_after_terminal_commit(journal.identity(), payload_lease)
        {
            return Some(false);
        }

        let mut execution = journal.execution.borrow_mut();
        if matches!(
            &*execution,
            DockLiveUndockPromotionCommitExecution::Pending(Some(
                DockLiveUndockPreparedPromotionExecution::SameWindow(_)
            ))
        ) {
            *execution = DockLiveUndockPromotionCommitExecution::Aborted;
            Some(true)
        } else {
            Some(false)
        }
    }

    fn settle_recovered_same_window_promotion(
        &self,
        journal: &Rc<DockLiveUndockPromotionCommitJournal>,
        cx: &mut App,
    ) -> bool {
        if let Some(settled) = self.settle_provider_terminal_pending_promotion(journal, cx) {
            return settled;
        }

        let (
            batch,
            source_host,
            source,
            source_retirement,
            destination_host,
            destination_promotion,
            retained_release,
            payload_lease,
            publication,
            committed_viewport,
            runtime,
            controller,
            graph_commit,
        ) = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::SameWindow(commit) = &*execution else {
                return false;
            };
            (
                commit.presentation_batch.clone(),
                commit.source_host.clone(),
                commit.source.clone(),
                commit.source_retirement.clone(),
                commit.destination_host.clone(),
                commit.destination_promotion.clone(),
                commit.retained_release.clone(),
                commit.reveal.preflight().mount().proxy().lease(),
                commit.publication.clone(),
                commit.committed_viewport.clone(),
                commit.runtime.clone(),
                commit.controller.clone(),
                commit.graph_commit,
            )
        };
        let Some(batch) = batch else {
            return false;
        };

        view_presentation_window::release_stable_leases_after_endpoint_loss(cx, batch.leases());

        if matches!(
            source_retirement,
            DockLiveUndockSourceRetirementStage::Pending
        ) {
            let source_key = source.key();
            let retirement = cx.update_entity(&source_host, |host, host_cx| {
                if let Some(receipt) =
                    host.commit_or_replay_prepared_live_source_retirement_without_notify(source)
                {
                    host_cx.notify();
                    return Some(DockLiveUndockSourceRetirementStage::Committed(receipt));
                }
                (!host.accepts_live_presentation_key(source_key))
                    .then_some(DockLiveUndockSourceRetirementStage::AuthorityAbsent)
            });
            let Some(retirement) = retirement else {
                return false;
            };
            if let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
                &mut *journal.execution.borrow_mut()
            {
                commit.source_retirement = retirement;
            }
        }

        if !retained_release.settle(cx) {
            return false;
        }

        let presentation_session_retired = {
            let execution = journal.execution.borrow();
            matches!(
                &*execution,
                DockLiveUndockPromotionCommitExecution::SameWindow(commit)
                    if commit.presentation_session_retired
            )
        };
        if !presentation_session_retired {
            if !self.retire_presentation_session_after_terminal_commit(
                journal.identity(),
                payload_lease,
            ) {
                return false;
            }
            if let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
                &mut *journal.execution.borrow_mut()
            {
                commit.presentation_session_retired = true;
            }
        }

        if let Some(publication) = publication
            && !publication.is_settled()
        {
            publication.publish(cx);
            if !publication.is_settled() {
                return false;
            }
        }

        if let Some(committed_viewport) = committed_viewport {
            runtime.retire_live_undock_provisional_promotion_commit(&committed_viewport);
        }
        if let Some(graph_commit) = graph_commit {
            cx.update_entity(&controller, |controller, _| {
                controller.workspace_mut().retire_graph_commit(graph_commit);
            });
        }

        let source_retirement = {
            let execution = journal.execution.borrow();
            let DockLiveUndockPromotionCommitExecution::SameWindow(commit) = &*execution else {
                return false;
            };
            commit.source_retirement.clone()
        };
        if let DockLiveUndockSourceRetirementStage::Committed(receipt) = source_retirement {
            if !cx.update_entity(&source_host, |host, _| {
                host.retire_live_source_retirement(&receipt)
            }) {
                return false;
            }
            if let DockLiveUndockPromotionCommitExecution::SameWindow(commit) =
                &mut *journal.execution.borrow_mut()
            {
                commit.source_retirement = DockLiveUndockSourceRetirementStage::Retired;
            }
        }
        if let Some(destination_promotion) = destination_promotion
            && !cx.update_entity(&destination_host, |host, _| {
                host.retire_live_destination_promotion(&destination_promotion)
            })
        {
            return false;
        }

        let mut execution = journal.execution.borrow_mut();
        if matches!(
            &*execution,
            DockLiveUndockPromotionCommitExecution::SameWindow(_)
        ) {
            *execution = DockLiveUndockPromotionCommitExecution::Aborted;
            true
        } else {
            false
        }
    }
}

fn finalize_payload_drag_after_terminal<C, T>(
    finalizer: &DockPayloadDragFinalizer,
    identity: DockLiveUndockIdentity,
    cx: &mut C,
    cleanup: impl FnOnce(&mut C),
    settle: impl FnOnce(Option<DockPayloadDragFinalizerClaim>, &mut C) -> T,
) -> T {
    cleanup(cx);
    settle(finalizer.claim_terminal(identity), cx)
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
            DockLiveUndockDragGeneration, DockLiveUndockPhysicalPoint, DockLiveUndockRouteFeedback,
            DockLiveUndockSession, DockLiveUndockSourceSnapshot,
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
            DockLiveUndockRouteGeneration::new(generation)
                .expect("test route generation must be non-zero"),
            DockLiveUndockRouteFeedback::Desktop,
            DockLiveUndockPhysicalPoint::new(50, 50),
            DockLiveUndockPhysicalBounds::new(DockLiveUndockPhysicalPoint::new(0, 0), 640, 480)
                .expect("test trigger bounds must be non-empty"),
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
    fn terminal_finalizer_can_claim_unadopted_route_authority() {
        let identity = identity(11);
        let finalizer = DockPayloadDragFinalizer::new();

        finalizer
            .claim_terminal(identity)
            .expect("the exact terminal execution must be able to settle route-owned payload work")
            .complete();

        assert_eq!(
            finalizer.authority.get(),
            DockPayloadDragFinalizerAuthority::Finalized
        );
        assert!(finalizer.claim_route().is_none());
    }

    #[test]
    fn terminal_cleanup_panic_preserves_retryable_payload_authority() {
        let identity = identity(12);
        let finalizer = DockPayloadDragFinalizer::new();
        assert!(finalizer.begin_live_undock(identity));
        assert!(finalizer.commit_live_undock(identity));
        let settlement_ran = Cell::new(false);
        let panic = catch_unwind(AssertUnwindSafe(|| {
            finalize_payload_drag_after_terminal(
                &finalizer,
                identity,
                &mut (),
                |_| panic!("injected lower-receipt retirement panic"),
                |claim, _| {
                    settlement_ran.set(true);
                    claim
                        .expect("the terminal must retain its finalizer claim")
                        .complete();
                },
            );
        }));

        assert!(panic.is_err());
        assert!(!settlement_ran.get());
        assert_eq!(
            finalizer.authority.get(),
            DockPayloadDragFinalizerAuthority::LiveUndock(identity)
        );
    }

    #[test]
    fn terminal_settlement_panic_restores_retryable_payload_authority() {
        let identity = identity(13);
        let finalizer = DockPayloadDragFinalizer::new();
        assert!(finalizer.begin_live_undock(identity));
        assert!(finalizer.commit_live_undock(identity));
        let panic = catch_unwind(AssertUnwindSafe(|| {
            finalize_payload_drag_after_terminal(
                &finalizer,
                identity,
                &mut (),
                |_| {},
                |_claim, _| panic!("injected payload settlement panic"),
            );
        }));

        assert!(panic.is_err());
        assert_eq!(
            finalizer.authority.get(),
            DockPayloadDragFinalizerAuthority::LiveUndock(identity)
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
    fn destination_semantics_watchdog_is_generation_bound_without_time_based_failure() {
        let token = DockLiveUndockPromotionToken::new(7)
            .expect("the test promotion token must be non-zero");
        let first = DockLiveUndockDestinationSemanticsWatchdogKey {
            token,
            destination: DockLiveUndockPromotionDestination::SameWindowDesktop {
                window_id: WindowId::from(10),
            },
            session_generation: 11,
            placement_mutation_generation: 12,
        };
        let replacement = DockLiveUndockDestinationSemanticsWatchdogKey {
            placement_mutation_generation: 13,
            ..first
        };
        let mut watchdog = DockLiveUndockDestinationSemanticsWatchdog::default();

        let first_generation = watchdog.arm(first).expect("the first wait must arm");
        assert!(watchdog.arm(first).is_none());
        assert_eq!(watchdog.claim(first, first_generation), true);

        let stale_generation = watchdog.arm(first).expect("the retry must rearm");
        let replacement_generation = watchdog
            .arm(replacement)
            .expect("a new placement generation must replace the old wait");
        assert!(!watchdog.claim(first, stale_generation));
        assert!(watchdog.claim(replacement, replacement_generation));

        for _ in 0..8 {
            let generation = watchdog
                .arm(replacement)
                .expect("each exact retry acknowledgement must rearm once");
            assert!(watchdog.claim(replacement, generation));
        }
    }

    #[test]
    fn shutdown_abort_claim_wins_before_the_promotion_commit_boundary() {
        let identity = identity(9);
        let token = DockLiveUndockPromotionToken::new(1)
            .expect("the test promotion token must be non-zero");
        let destination = DockLiveUndockPromotionDestination::SameWindowDesktop {
            window_id: WindowId::from(10),
        };
        let journal = DockLiveUndockPromotionCommitJournal {
            identity,
            token,
            destination,
            surface_revision: 3,
            boundary: Cell::new(DockLiveUndockPromotionCommitBoundary::Reversible),
            drive: Cell::new(DockLiveUndockPromotionCommitDriveState::Idle),
            recovery_receipt: Cell::new(None),
            recovery_requires_window_terminal: Cell::new(false),
            execution: RefCell::new(DockLiveUndockPromotionCommitExecution::Aborted),
        };

        assert_eq!(
            journal.claim_abort_or_observe(),
            DockLiveUndockPromotionCommitDisposition::AbortClaimed
        );
        assert!(journal.abort_was_claimed());
        assert!(!journal.begin_commit_call());
    }

    #[test]
    fn shutdown_waits_after_the_promotion_commit_boundary_until_durable_publication() {
        let identity = identity(10);
        let token = DockLiveUndockPromotionToken::new(2)
            .expect("the test promotion token must be non-zero");
        let destination = DockLiveUndockPromotionDestination::SameWindowDesktop {
            window_id: WindowId::from(11),
        };
        let journal = DockLiveUndockPromotionCommitJournal {
            identity,
            token,
            destination,
            surface_revision: 4,
            boundary: Cell::new(DockLiveUndockPromotionCommitBoundary::Irreversible),
            drive: Cell::new(DockLiveUndockPromotionCommitDriveState::Idle),
            recovery_receipt: Cell::new(None),
            recovery_requires_window_terminal: Cell::new(false),
            execution: RefCell::new(DockLiveUndockPromotionCommitExecution::Aborted),
        };

        assert_eq!(
            journal.claim_abort_or_observe(),
            DockLiveUndockPromotionCommitDisposition::ForwardOnly {
                identity,
                token,
                destination,
            }
        );
        assert!(journal.begin_commit_call());
    }

    #[test]
    fn recovery_required_authority_remains_forward_replayable() {
        let identity = identity(11);
        let token = DockLiveUndockPromotionToken::new(3)
            .expect("the test promotion token must be non-zero");
        let destination = DockLiveUndockPromotionDestination::SameWindowDesktop {
            window_id: WindowId::from(12),
        };
        let journal = DockLiveUndockPromotionCommitJournal {
            identity,
            token,
            destination,
            surface_revision: 5,
            boundary: Cell::new(DockLiveUndockPromotionCommitBoundary::Irreversible),
            drive: Cell::new(DockLiveUndockPromotionCommitDriveState::Terminal),
            recovery_receipt: Cell::new(None),
            recovery_requires_window_terminal: Cell::new(false),
            execution: RefCell::new(DockLiveUndockPromotionCommitExecution::Aborted),
        };

        assert_eq!(
            journal.claim_abort_or_observe(),
            DockLiveUndockPromotionCommitDisposition::ForwardOnly {
                identity,
                token,
                destination,
            }
        );
        assert!(journal.begin_commit_call());
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

    #[test]
    fn dropped_surface_shutdown_record_completes_payload_authority() {
        let identity = identity(14);
        let lease = identity.opening().lease();
        let finalizer = DockPayloadDragFinalizer::new();
        assert!(finalizer.begin_live_undock(identity));
        assert!(finalizer.commit_live_undock(identity));
        let record = finalizer
            .claim_live_undock(identity)
            .expect("the live generation should own finalization")
            .transfer_to_surface_shutdown(lease);

        drop(record);

        assert_eq!(
            finalizer.authority.get(),
            DockPayloadDragFinalizerAuthority::Finalized
        );
    }
}
