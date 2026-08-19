mod activation;
mod builder;
pub(crate) mod live_payload_carrier;
pub(crate) mod live_undock;
mod live_undock_pump;
pub(crate) mod live_undock_runtime;
mod owner;
mod panel;
pub(crate) mod payload_recovery;
pub(crate) mod payload_recovery_executor;
mod state;
mod viewport;
mod viewport_readiness;
pub(crate) mod window_session;

#[cfg(test)]
mod live_undock_tests;
#[cfg(test)]
mod payload_recovery_tests;
#[cfg(test)]
mod window_session_tests;

pub use activation::{DockSurfaceActivationOutcome, DockSurfaceActivationRequestId};
pub use builder::{DockSurfaceBuildError, DockSurfaceBuilder};
pub use owner::{DockSurfaceChangeCategory, DockSurfaceChangeEvent, DockSurfaceTransition};
pub use panel::{
    DockSurfaceChange, DockSurfaceFloatingPanelSnapshot, DockSurfacePanelError,
    DockSurfacePanelLocation, DockSurfacePanelLocationKind, DockSurfacePanelOutcome,
    DockSurfacePanelSnapshot,
};
pub use state::DockSurfaceSnapshot;
pub use viewport::{
    DockSurfaceViewportCloseOutcome, DockSurfaceViewportCloseStatus,
    DockSurfaceViewportOpenOutcome, DockSurfaceViewportOpenReport, DockSurfaceViewportOpenStatus,
    DockSurfaceViewportOpened, DockSurfaceViewportRestoreOutcome, DockSurfaceViewportRestoreReport,
    DockSurfaceViewportShouldCloseOutcome, DockSurfaceViewportShouldCloseStatus,
    DockSurfaceViewportSpec, DockSurfaceViewportSpecError, DockSurfaceViewportUnavailable,
    DockSurfaceViewports,
};
pub use viewport_readiness::{
    DockSurfaceViewportFlagWarning, DockSurfaceViewportInputStatus,
    DockSurfaceViewportLifecycleReadiness, DockSurfaceViewportPlatformCapabilities,
    DockSurfaceViewportPlatformReadiness, DockSurfaceViewportReadiness,
    DockSurfaceViewportReadinessReport, DockSurfaceViewportReadinessStatus,
    DockSurfaceViewportRouteStatus, DockSurfaceViewportStaleReason,
    DockSurfaceViewportUnsupportedFlag,
};
pub use window_session::{
    DockSurfacePrimaryWindowOpenConflict, DockSurfacePrimaryWindowOpenOutcome,
    DockSurfacePrimaryWindowOpened, DockSurfacePrimaryWindowUnavailable,
    DockSurfaceWindowSessionOpeningRollbackReason, DockSurfaceWindowSessionPhase,
    DockSurfaceWindowSessionReason, DockSurfaceWindowSessionShutdownReason,
    DockSurfaceWindowSessionStatus,
};

use crate::{
    DockController, DockHost, DockSpaceId, DockViewportClosePolicy,
    DockViewportRuntimeCommitAuthority, DockViewportRuntimeHandle, DockViewportRuntimeLineage,
    DockViewportSurfaceShutdownReservation, DockViewportWindowRole, DockVisualStyleResolver,
    native_captured_drag::{
        DockNativeCapturedSurfaceRelease, DockNativeCapturedSurfaceReleaseOutcome,
    },
    viewport_runtime_effects::unique_windows,
};
pub(crate) use activation::{
    DockSurfaceActivationBinding, DockSurfaceActivationHostRegistration,
    DockSurfaceActivationHostRegistrationStatus, DockSurfaceActivationSettlements,
    DockSurfaceActivationState,
};
#[cfg(test)]
pub(crate) use activation::{DockSurfaceActivationDispatch, DockSurfaceActivationHostLookup};
use open_gpui::{
    AnyView, AnyWindowHandle, App, AppContext, Bounds, Context, Entity, Global, Pixels,
    Subscription, WeakEntity, Window, WindowBounds, WindowId, WindowInitialPresentationStatus,
    WindowOpenFailureStage, WindowOptions,
};
#[cfg(any(test, feature = "test-support"))]
use open_gpui::{NativeCapturedDragReleaseBarrier, NativeCapturedDragReleaseTerminal};
pub(crate) use owner::{
    DockSurfaceDeferredPublication, DockSurfaceOwner, DockSurfaceTransactionId,
    DockSurfaceTransactionReceipt, with_detached_deferred_tracked_root_transaction,
    with_detached_root_transaction, with_root_transaction,
};
use std::{
    any::Any,
    cell::{Cell, RefCell},
    collections::HashMap,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

type DockSurfaceShutdownPanic = Box<dyn Any + Send + 'static>;

#[cfg(any(test, feature = "test-support"))]
static NEXT_DOCK_SURFACE_SHUTDOWN_TEST_ORDINAL: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

/// Exact native capture-release evidence retained by the DockSurface shutdown test seam.
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DockSurfaceShutdownCaptureReleaseEvidence {
    barrier: NativeCapturedDragReleaseBarrier,
    terminal: NativeCapturedDragReleaseTerminal,
}

#[cfg(any(test, feature = "test-support"))]
impl DockSurfaceShutdownCaptureReleaseEvidence {
    /// Returns the exact source, drag generation, and native release generation.
    pub fn barrier(self) -> NativeCapturedDragReleaseBarrier {
        self.barrier
    }

    /// Returns the terminal fact that authorized dependent shutdown effects.
    pub fn terminal(self) -> NativeCapturedDragReleaseTerminal {
        self.terminal
    }
}

/// Typed DockSurface shutdown boundary observed by native and deterministic tests.
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DockSurfaceShutdownTestEventKind {
    /// Every exact native capture candidate reached terminal and its Dock route cleanup ran.
    CaptureCleanupCompleted {
        /// Exact release barriers that required native capture settlement.
        releases: Vec<DockSurfaceShutdownCaptureReleaseEvidence>,
    },
    /// A test-only cleanup callback panicked after capture cleanup and before close dispatch.
    CleanupCallbackPanicked,
    /// One dependent close attempt could not enter the checked-out window update and was retried.
    DependentCloseDispatchDeferredBusy {
        /// Dependent window whose close dispatch remained pending.
        window: WindowId,
    },
    /// The primary anchor close attempt could not enter its checked-out window update.
    AnchorCloseDispatchDeferredBusy {
        /// Primary anchor whose close dispatch remained pending.
        window: WindowId,
    },
    /// One dependent close crossed the window-update dispatch boundary.
    DependentCloseDispatched {
        /// Dependent window whose logical close was dispatched.
        window: WindowId,
    },
    /// The primary anchor close crossed the window-update dispatch boundary.
    AnchorCloseDispatched {
        /// Primary anchor whose logical close was dispatched.
        window: WindowId,
    },
}

/// One process-ordered observation for an exact DockSurface session generation.
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DockSurfaceShutdownTestEvent {
    ordinal: u64,
    session_generation: u64,
    anchor: WindowId,
    kind: DockSurfaceShutdownTestEventKind,
}

#[cfg(any(test, feature = "test-support"))]
impl DockSurfaceShutdownTestEvent {
    /// Returns the process-wide order assigned at the production shutdown boundary.
    pub fn ordinal(&self) -> u64 {
        self.ordinal
    }

    /// Returns the exact DockSurface session generation being retired.
    pub fn session_generation(&self) -> u64 {
        self.session_generation
    }

    /// Returns the primary anchor identity captured by the exact session lease.
    pub fn anchor(&self) -> WindowId {
        self.anchor
    }

    /// Returns the typed shutdown boundary.
    pub fn kind(&self) -> &DockSurfaceShutdownTestEventKind {
        &self.kind
    }
}

/// Read-only observation handle for one DockSurface authority.
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
#[derive(Clone, Debug, Default)]
pub struct DockSurfaceShutdownTestObservation {
    events: Rc<RefCell<Vec<DockSurfaceShutdownTestEvent>>>,
}

#[cfg(any(test, feature = "test-support"))]
impl DockSurfaceShutdownTestObservation {
    /// Returns all events ordered by their production-boundary ordinal.
    pub fn events(&self) -> Vec<DockSurfaceShutdownTestEvent> {
        let mut events = self.events.borrow().clone();
        events.sort_unstable_by_key(DockSurfaceShutdownTestEvent::ordinal);
        events
    }

    /// Removes events already consumed by the current test scenario.
    pub fn clear(&self) {
        self.events.borrow_mut().clear();
    }

    fn record(
        &self,
        lease: window_session::DockSurfaceWindowSessionLease,
        kind: DockSurfaceShutdownTestEventKind,
    ) {
        let ordinal = NEXT_DOCK_SURFACE_SHUTDOWN_TEST_ORDINAL
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        assert_ne!(
            ordinal, 0,
            "DockSurface shutdown test observation ordinal space exhausted"
        );
        self.events.borrow_mut().push(DockSurfaceShutdownTestEvent {
            ordinal,
            session_generation: lease.generation(),
            anchor: lease.anchor(),
            kind,
        });
    }
}

/// Accepted live-undock authority boundary exposed only to deterministic and native tests.
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DockSurfaceLiveUndockTestEventKind {
    /// The source committed its stable semantic/focus proxy.
    SourceProxyCommitted,
    /// The destination mounted the exact retained payload lease.
    PayloadMounted,
    /// The mounted payload produced an accepted visible presentation candidate.
    PayloadPresented,
    /// Durable destination semantics received renderer-submitted evidence.
    DestinationSemanticsSubmitted,
    /// The destination interaction gate opened for the submitted semantics.
    DestinationInteractionAdmitted,
}

/// Immutable evidence for one accepted live-undock authority transition.
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DockSurfaceLiveUndockTestEvent {
    kind: DockSurfaceLiveUndockTestEventKind,
    opening_generation: u64,
    drag_generation: u64,
    source_window: Option<WindowId>,
    destination_window: WindowId,
    payload_lease_generation: Option<u64>,
    provisional_session_generation: Option<u64>,
    frame_generation: Option<u64>,
    root_count: Option<usize>,
}

#[cfg(any(test, feature = "test-support"))]
impl DockSurfaceLiveUndockTestEvent {
    fn from_payload_lease(
        kind: DockSurfaceLiveUndockTestEventKind,
        lease: live_undock::DockLiveUndockPayloadLeaseReceipt,
        frame_generation: Option<u64>,
        root_count: Option<usize>,
    ) -> Self {
        let identity = lease.identity();
        Self {
            kind,
            opening_generation: identity.opening().generation(),
            drag_generation: identity.drag_generation().get(),
            source_window: Some(lease.source().window_id()),
            destination_window: lease.destination_window(),
            payload_lease_generation: Some(lease.lease_generation().get()),
            provisional_session_generation: Some(lease.provisional_session_generation()),
            frame_generation,
            root_count,
        }
    }

    pub(crate) fn source_proxy(receipt: live_undock::DockLiveUndockSourceProxyReceipt) -> Self {
        Self::from_payload_lease(
            DockSurfaceLiveUndockTestEventKind::SourceProxyCommitted,
            receipt.lease(),
            Some(receipt.proxy_frame_generation()),
            None,
        )
    }

    pub(crate) fn payload_mounted(receipt: live_undock::DockLiveUndockPayloadMountReceipt) -> Self {
        debug_assert_eq!(
            receipt.destination_lease_generation(),
            receipt.proxy().lease().lease_generation().get(),
        );
        Self::from_payload_lease(
            DockSurfaceLiveUndockTestEventKind::PayloadMounted,
            receipt.proxy().lease(),
            Some(receipt.mount_frame_generation()),
            Some(receipt.root_count()),
        )
    }

    pub(crate) fn payload_presented(
        receipt: live_undock::DockLiveUndockPayloadPresentationReceipt,
    ) -> Self {
        Self::from_payload_lease(
            DockSurfaceLiveUndockTestEventKind::PayloadPresented,
            receipt.mount().proxy().lease(),
            Some(receipt.frame_generation()),
            Some(receipt.mount().root_count()),
        )
    }

    pub(crate) fn destination_semantics_submitted(
        receipt: live_undock::DockLiveUndockDestinationSemanticsReceipt,
    ) -> Self {
        let identity = receipt.identity();
        let lease = receipt.payload_lease();
        Self {
            kind: DockSurfaceLiveUndockTestEventKind::DestinationSemanticsSubmitted,
            opening_generation: identity.opening().generation(),
            drag_generation: identity.drag_generation().get(),
            source_window: lease.map(|lease| lease.source().window_id()),
            destination_window: receipt.destination().window_id(),
            payload_lease_generation: lease.map(|lease| lease.lease_generation().get()),
            provisional_session_generation: lease
                .map(|lease| lease.provisional_session_generation()),
            frame_generation: receipt.submitted_frame_generation(),
            root_count: None,
        }
    }

    pub(crate) fn destination_interaction_admitted(
        receipt: live_undock::DockLiveUndockDestinationInteractionReceipt,
    ) -> Self {
        let semantics = receipt.semantics();
        let identity = semantics.identity();
        let lease = semantics.payload_lease();
        Self {
            kind: DockSurfaceLiveUndockTestEventKind::DestinationInteractionAdmitted,
            opening_generation: identity.opening().generation(),
            drag_generation: identity.drag_generation().get(),
            source_window: lease.map(|lease| lease.source().window_id()),
            destination_window: semantics.destination().window_id(),
            payload_lease_generation: lease.map(|lease| lease.lease_generation().get()),
            provisional_session_generation: receipt
                .admitted_session_generation()
                .or_else(|| lease.map(|lease| lease.provisional_session_generation())),
            frame_generation: semantics.submitted_frame_generation(),
            root_count: None,
        }
    }

    /// Returns the accepted transition kind.
    pub const fn kind(&self) -> DockSurfaceLiveUndockTestEventKind {
        self.kind
    }

    /// Returns the provisional-opening generation that owns the transition.
    pub const fn opening_generation(&self) -> u64 {
        self.opening_generation
    }

    /// Returns the captured-drag generation that owns the transition.
    pub const fn drag_generation(&self) -> u64 {
        self.drag_generation
    }

    /// Returns the exact source window when the transition carries a payload lease.
    pub const fn source_window(&self) -> Option<WindowId> {
        self.source_window
    }

    /// Returns the exact destination window.
    pub const fn destination_window(&self) -> WindowId {
        self.destination_window
    }

    /// Returns the retained payload lease generation when present.
    pub const fn payload_lease_generation(&self) -> Option<u64> {
        self.payload_lease_generation
    }

    /// Returns the provisional session generation when present.
    pub const fn provisional_session_generation(&self) -> Option<u64> {
        self.provisional_session_generation
    }

    /// Returns the accepted renderer/presentation frame generation when present.
    pub const fn frame_generation(&self) -> Option<u64> {
        self.frame_generation
    }

    /// Returns the mounted root count when present.
    pub const fn root_count(&self) -> Option<usize> {
        self.root_count
    }
}

/// Read-only observation of accepted live-undock authority transitions for one DockSurface.
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
#[derive(Clone, Debug, Default)]
pub struct DockSurfaceLiveUndockTestObservation {
    events: Rc<RefCell<Vec<DockSurfaceLiveUndockTestEvent>>>,
}

#[cfg(any(test, feature = "test-support"))]
impl DockSurfaceLiveUndockTestObservation {
    /// Returns accepted transitions in reducer order.
    pub fn events(&self) -> Vec<DockSurfaceLiveUndockTestEvent> {
        self.events.borrow().clone()
    }

    /// Removes events already consumed by the current test scenario.
    pub fn clear(&self) {
        self.events.borrow_mut().clear();
    }

    pub(crate) fn record(&self, event: DockSurfaceLiveUndockTestEvent) {
        self.events.borrow_mut().push(event);
    }
}

pub(crate) struct DockSurfaceCaptureReleaseFailure {
    lease: window_session::DockSurfaceWindowSessionLease,
    prior_panic: Option<DockSurfaceShutdownPanic>,
}

impl fmt::Debug for DockSurfaceCaptureReleaseFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DockSurfaceCaptureReleaseFailure")
            .field("lease", &self.lease)
            .field("has_prior_panic", &self.prior_panic.is_some())
            .finish()
    }
}

impl fmt::Display for DockSurfaceCaptureReleaseFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "native pointer capture could not be released while retiring DockSurface {:?}",
            self.lease
        )
    }
}

impl std::error::Error for DockSurfaceCaptureReleaseFailure {}

#[derive(Default)]
struct DockSurfaceAppShutdownFanoutState {
    participants: Vec<WeakEntity<DockSurfaceOwner>>,
    observer_armed: bool,
}

/// Holds every live surface at the post-registry-clear barrier before the first panic resumes.
#[derive(Clone, Default)]
struct DockSurfaceAppShutdownFanout {
    state: Rc<RefCell<DockSurfaceAppShutdownFanoutState>>,
}

impl Global for DockSurfaceAppShutdownFanout {}

impl DockSurfaceAppShutdownFanout {
    fn register(&self, owner: &Entity<DockSurfaceOwner>, cx: &mut App) {
        let install_observer = {
            let mut state = self.state.borrow_mut();
            state
                .participants
                .retain(|participant| participant.upgrade().is_some());
            if !state
                .participants
                .iter()
                .any(|participant| participant == owner)
            {
                state.participants.push(owner.downgrade());
            }
            if state.observer_armed {
                false
            } else {
                state.observer_armed = true;
                true
            }
        };
        if install_observer {
            install_surface_app_shutdown_observer(self.clone(), cx);
        }
    }

    fn snapshot_for_shutdown(&self) -> Vec<Entity<DockSurfaceOwner>> {
        let mut state = self.state.borrow_mut();
        state.observer_armed = false;
        state
            .participants
            .retain(|participant| participant.upgrade().is_some());
        state.participants.sort_unstable();
        state.participants.dedup();
        state
            .participants
            .iter()
            .filter_map(WeakEntity::upgrade)
            .collect()
    }

    fn rearm_for_next_lifecycle(&self, cx: &mut App) {
        let install_observer = {
            let mut state = self.state.borrow_mut();
            if state.observer_armed {
                false
            } else {
                state.observer_armed = true;
                true
            }
        };
        if install_observer {
            install_surface_app_shutdown_observer(self.clone(), cx);
        }
    }
}

struct DockSurfaceAppShutdownSettlement {
    close_dispatch: std::thread::Result<()>,
}

#[derive(Default)]
struct DockSurfaceAppShutdownRoundState {
    participants: Vec<Entity<DockSurfaceOwner>>,
    pending: usize,
    sealed: bool,
    completion_queued: bool,
    settlements: Vec<DockSurfaceAppShutdownSettlement>,
}

#[derive(Clone, Default)]
struct DockSurfaceAppShutdownRound {
    state: Rc<RefCell<DockSurfaceAppShutdownRoundState>>,
}

impl DockSurfaceAppShutdownRound {
    fn register(&self, owner: &Entity<DockSurfaceOwner>) {
        let mut state = self.state.borrow_mut();
        assert!(
            !state.sealed,
            "cannot register a surface after sealing App shutdown"
        );
        state.participants.push(owner.clone());
        state.pending += 1;
    }

    fn settle(&self, settlement: DockSurfaceAppShutdownSettlement, cx: &mut App) {
        {
            let mut state = self.state.borrow_mut();
            assert!(
                state.pending > 0,
                "surface App shutdown settled more than once"
            );
            state.pending -= 1;
            state.settlements.push(settlement);
        }
        self.queue_completion_if_ready(cx);
    }

    fn seal(&self, cx: &mut App) {
        {
            let mut state = self.state.borrow_mut();
            state.sealed = true;
        }
        self.queue_completion_if_ready(cx);
    }

    fn queue_completion_if_ready(&self, cx: &mut App) {
        let settlements = {
            let mut state = self.state.borrow_mut();
            if !state.sealed || state.pending != 0 || state.completion_queued {
                return;
            }
            state.completion_queued = true;
            state.participants.clear();
            std::mem::take(&mut state.settlements)
        };
        cx.defer_shutdown_critical_after_window_registry_clear(move |cx| {
            complete_surface_app_shutdown_round(settlements, cx)
        });
    }
}

struct DockSurfacePendingShutdown {
    effects: DockSurfaceShutdownCloseEffects,
    app_shutdown_round: Option<DockSurfaceAppShutdownRound>,
    payload_finalizers: Vec<live_undock_runtime::DockPayloadDragSurfaceShutdownFinalizer>,
}

#[cfg(test)]
enum DockSurfaceShutdownStartFault {
    WindowAuthorityRevoke {
        attempts: Rc<Cell<usize>>,
        panic_delivered: bool,
    },
    CaptureSetup {
        late_callback_ran: Rc<Cell<bool>>,
    },
}

#[derive(Default)]
struct DockSurfaceShutdownCoordinatorState {
    pending: HashMap<window_session::DockSurfaceWindowSessionLease, DockSurfacePendingShutdown>,
    #[cfg(test)]
    start_faults:
        HashMap<window_session::DockSurfaceWindowSessionLease, DockSurfaceShutdownStartFault>,
}

#[derive(Clone, Default)]
struct DockSurfaceShutdownCoordinator {
    state: Rc<RefCell<DockSurfaceShutdownCoordinatorState>>,
}

impl Global for DockSurfaceShutdownCoordinator {}

impl DockSurfaceShutdownCoordinator {
    fn begin(
        &self,
        effects: DockSurfaceShutdownCloseEffects,
        app_shutdown_round: Option<DockSurfaceAppShutdownRound>,
    ) {
        let lease = effects.lease;
        let pending = DockSurfacePendingShutdown {
            effects,
            app_shutdown_round,
            payload_finalizers: Vec::new(),
        };
        let replaced = self.state.borrow_mut().pending.insert(lease, pending);
        assert!(
            replaced.is_none(),
            "one surface lease cannot own multiple pending shutdown continuations"
        );
    }

    fn attach_app_shutdown_round(
        &self,
        lease: window_session::DockSurfaceWindowSessionLease,
        round: DockSurfaceAppShutdownRound,
    ) -> bool {
        let mut state = self.state.borrow_mut();
        let Some(pending) = state.pending.get_mut(&lease) else {
            return false;
        };
        if pending.app_shutdown_round.is_some() {
            return false;
        }
        pending.app_shutdown_round = Some(round);
        true
    }

    fn register_payload_finalizer(
        &self,
        lease: window_session::DockSurfaceWindowSessionLease,
        finalizer: live_undock_runtime::DockPayloadDragSurfaceShutdownFinalizer,
    ) -> Result<(), live_undock_runtime::DockPayloadDragSurfaceShutdownFinalizer> {
        let mut state = self.state.borrow_mut();
        let Some(pending) = state.pending.get_mut(&lease) else {
            return Err(finalizer);
        };
        assert!(
            pending
                .payload_finalizers
                .iter()
                .all(|current| !current.same_token(&finalizer)),
            "one payload finalizer cannot be registered twice for surface shutdown"
        );
        pending.payload_finalizers.push(finalizer);
        Ok(())
    }

    fn take_after_capture_terminal(
        &self,
        lease: window_session::DockSurfaceWindowSessionLease,
    ) -> Option<DockSurfacePendingShutdown> {
        let mut state = self.state.borrow_mut();
        let pending = state.pending.remove(&lease);
        #[cfg(test)]
        state.start_faults.remove(&lease);
        pending
    }

    #[cfg(test)]
    fn arm_start_fault_for_test(
        &self,
        lease: window_session::DockSurfaceWindowSessionLease,
        fault: DockSurfaceShutdownStartFault,
    ) {
        let replaced = self.state.borrow_mut().start_faults.insert(lease, fault);
        assert!(
            replaced.is_none(),
            "one surface shutdown generation cannot own multiple startup fault injections"
        );
    }

    #[cfg(test)]
    fn observe_window_authority_revoke_for_test(
        &self,
        lease: window_session::DockSurfaceWindowSessionLease,
    ) -> bool {
        let mut state = self.state.borrow_mut();
        let Some(DockSurfaceShutdownStartFault::WindowAuthorityRevoke {
            attempts,
            panic_delivered,
        }) = state.start_faults.get_mut(&lease)
        else {
            return false;
        };
        attempts.set(attempts.get() + 1);
        if *panic_delivered {
            false
        } else {
            *panic_delivered = true;
            true
        }
    }

    #[cfg(test)]
    fn take_capture_setup_fault_for_test(
        &self,
        lease: window_session::DockSurfaceWindowSessionLease,
    ) -> Option<Rc<Cell<bool>>> {
        let mut state = self.state.borrow_mut();
        if !matches!(
            state.start_faults.get(&lease),
            Some(DockSurfaceShutdownStartFault::CaptureSetup { .. })
        ) {
            return None;
        }
        match state.start_faults.remove(&lease) {
            Some(DockSurfaceShutdownStartFault::CaptureSetup { late_callback_ran }) => {
                Some(late_callback_ran)
            }
            Some(DockSurfaceShutdownStartFault::WindowAuthorityRevoke { .. }) | None => {
                unreachable!()
            }
        }
    }

    #[cfg(test)]
    fn has_pending_shutdown_for_test(
        &self,
        lease: window_session::DockSurfaceWindowSessionLease,
    ) -> bool {
        self.state.borrow().pending.contains_key(&lease)
    }
}

fn complete_surface_app_shutdown_round(
    settlements: Vec<DockSurfaceAppShutdownSettlement>,
    _cx: &mut App,
) {
    let mut first_panic = None;
    for settlement in settlements {
        retain_first_surface_shutdown_panic(
            &mut first_panic,
            settlement.close_dispatch,
            "App shutdown forced-close dispatch",
        );
    }
    if let Some(payload) = first_panic {
        resume_unwind(payload);
    }
}

fn app_shutdown_fanout(cx: &mut App) -> DockSurfaceAppShutdownFanout {
    if let Some(fanout) = cx.try_global::<DockSurfaceAppShutdownFanout>() {
        return fanout.clone();
    }
    let fanout = DockSurfaceAppShutdownFanout::default();
    cx.set_global(fanout.clone());
    fanout
}

fn surface_shutdown_coordinator(cx: &mut App) -> DockSurfaceShutdownCoordinator {
    if let Some(coordinator) = cx.try_global::<DockSurfaceShutdownCoordinator>() {
        return coordinator.clone();
    }
    let coordinator = DockSurfaceShutdownCoordinator::default();
    cx.set_global(coordinator.clone());
    coordinator
}

#[cfg(any(test, feature = "test-support"))]
fn record_surface_shutdown_test_event(
    owner: &Entity<DockSurfaceOwner>,
    lease: window_session::DockSurfaceWindowSessionLease,
    kind: DockSurfaceShutdownTestEventKind,
    cx: &App,
) {
    cx.read_entity(owner, |owner, _| {
        if let Some(observation) = owner.shutdown_test_observation() {
            observation.record(lease, kind);
        }
    });
}

pub(crate) fn register_surface_shutdown_payload_finalizer(
    lease: window_session::DockSurfaceWindowSessionLease,
    finalizer: live_undock_runtime::DockPayloadDragSurfaceShutdownFinalizer,
    cx: &mut App,
) -> bool {
    match surface_shutdown_coordinator(cx).register_payload_finalizer(lease, finalizer) {
        Ok(()) => true,
        Err(finalizer) => {
            let completed = finalizer.complete();
            debug_assert!(
                completed,
                "an unregistered surface-shutdown payload finalizer must remain completable"
            );
            false
        }
    }
}

fn retain_first_surface_shutdown_panic(
    first_panic: &mut Option<DockSurfaceShutdownPanic>,
    result: std::thread::Result<()>,
    stage: &'static str,
) {
    let Err(payload) = result else {
        return;
    };
    if first_panic.is_none() {
        *first_panic = Some(payload);
    } else {
        log::error!(
            "suppressed secondary panic while settling DockSurface shutdown stage `{stage}`"
        );
    }
}

/// Application-level owner for one docked workspace and its viewport runtime.
///
/// `DockSurface` is the common app seam for docking. It keeps controller state, host creation, and
/// viewport runtime wiring together so ordinary applications do not need to assemble
/// [`runtime::DockHost`](crate::runtime::DockHost) and
/// [`runtime::DockViewportRuntimeHandle`](crate::runtime::DockViewportRuntimeHandle) directly.
#[derive(Clone, Debug)]
pub struct DockSurface {
    owner: Entity<DockSurfaceOwner>,
    primary_space: DockSpaceId,
}

pub(crate) fn reduce_live_undock_fact(
    owner: &Entity<DockSurfaceOwner>,
    fact: live_undock::DockLiveUndockFact,
    cx: &mut App,
) -> Option<live_undock::DockLiveUndockEffects> {
    cx.update_entity(owner, |owner, owner_cx| {
        let effects = owner.reduce_live_undock_fact(fact)?;
        if !effects.is_empty() {
            owner_cx.notify();
        }
        Some(effects)
    })
}

pub(crate) fn close_live_undock_window_quietly(window: open_gpui::AnyWindowHandle, cx: &mut App) {
    let _ = window.update(cx, |_, window, cx| window.remove_window(cx));
}

fn adopt_live_undock_shutdown_window(
    owner: &Entity<DockSurfaceOwner>,
    lease: window_session::DockSurfaceWindowSessionLease,
    window: open_gpui::AnyWindowHandle,
    cx: &mut App,
) -> bool {
    let outcome = cx.update_entity(owner, |owner, owner_cx| {
        let outcome = owner
            .window_session_mut()
            .adopt_shutdown_window(lease, window.window_id());
        if matches!(
            outcome,
            window_session::DockSurfaceWindowSessionAdoptWindowOutcome::Added
        ) {
            owner_cx.notify();
        }
        outcome
    });
    matches!(
        outcome,
        window_session::DockSurfaceWindowSessionAdoptWindowOutcome::Added
            | window_session::DockSurfaceWindowSessionAdoptWindowOutcome::AlreadyTracked
    )
}

fn retire_stale_live_undock_open_return(
    owner: &Entity<DockSurfaceOwner>,
    opening: live_undock::DockLiveUndockOpeningKey,
    window: open_gpui::AnyWindowHandle,
    completion: crate::DockViewportProvisionalOpenAttemptCompletion,
    cx: &mut App,
) {
    let lease = opening.lease();
    let shutdown_terminal_owned = adopt_live_undock_shutdown_window(owner, lease, window, cx);
    let runtime = cx.read_entity(owner, |owner, _| owner.runtime());
    match runtime.retire_live_undock_provisional(completion, window, shutdown_terminal_owned, cx) {
        crate::DockViewportLiveUndockProvisionalRetirementOutcome::CloseDispatched => {}
        crate::DockViewportLiveUndockProvisionalRetirementOutcome::ShutdownCloseRequired => {
            debug_assert!(
                shutdown_terminal_owned,
                "shutdown-owned provisional close requires one WindowSession terminal ticket"
            );
            close_surface_window(owner, lease, window, cx);
        }
        crate::DockViewportLiveUndockProvisionalRetirementOutcome::Stale => {
            if shutdown_terminal_owned {
                close_surface_window(owner, lease, window, cx);
            } else {
                close_live_undock_window_quietly(window, cx);
            }
        }
    }
}

pub(crate) fn finish_live_undock_open_return(
    owner: &Entity<DockSurfaceOwner>,
    opening: live_undock::DockLiveUndockOpeningKey,
    window: open_gpui::AnyWindowHandle,
    runtime: crate::DockViewportProvisionalOpenAttemptCompletion,
    cx: &mut App,
) -> live_undock::DockLiveUndockOpenReturnOutcome {
    let (outcome, effects) = cx.update_entity(owner, |owner, owner_cx| {
        let (outcome, effects) = owner
            .complete_live_undock_opening(opening, window, runtime)
            .into_parts();
        if !matches!(outcome, live_undock::DockLiveUndockOpenReturnOutcome::Stale) {
            owner_cx.notify();
        }
        (outcome, effects)
    });
    let live_runtime = cx.read_entity(owner, |owner, _| owner.live_undock_runtime());
    live_runtime.enqueue_effects(effects, cx);
    if matches!(outcome, live_undock::DockLiveUndockOpenReturnOutcome::Stale) {
        retire_stale_live_undock_open_return(owner, opening, window, runtime, cx);
    }
    outcome
}

pub(crate) fn finish_live_undock_open_failure(
    owner: &Entity<DockSurfaceOwner>,
    opening: live_undock::DockLiveUndockOpeningKey,
    cx: &mut App,
) -> live_undock::DockLiveUndockOpenFailureOutcome {
    let (outcome, effects) = cx.update_entity(owner, |owner, owner_cx| {
        let (outcome, effects) = owner.fail_live_undock_opening(opening).into_parts();
        if !matches!(
            outcome,
            live_undock::DockLiveUndockOpenFailureOutcome::Stale
        ) {
            owner_cx.notify();
        }
        (outcome, effects)
    });
    let live_runtime = cx.read_entity(owner, |owner, _| owner.live_undock_runtime());
    live_runtime.enqueue_effects(effects, cx);
    outcome
}

pub(crate) fn retire_live_undock_provisional(
    owner: &Entity<DockSurfaceOwner>,
    identity: live_undock::DockLiveUndockIdentity,
    window: Option<open_gpui::AnyWindowHandle>,
    dependency: Option<window_session::DockSurfaceWindowSessionDependencyId>,
    binding: Option<live_undock::DockLiveUndockOpeningBinding>,
    completion: Option<crate::DockViewportProvisionalOpenAttemptCompletion>,
    cx: &mut App,
) {
    let Some(window) = window else {
        return;
    };
    let opening = identity.opening();
    let lease = opening.lease();
    let runtime = cx.read_entity(owner, |owner, _| owner.runtime());
    let shutdown_terminal_owned = adopt_live_undock_shutdown_window(owner, lease, window, cx);
    let dependency_transferred = dependency.is_some_and(|dependency| {
        if !shutdown_terminal_owned
            || binding != Some(live_undock::DockLiveUndockOpeningBinding::ExactGated)
        {
            return false;
        }
        cx.update_entity(owner, |owner, owner_cx| {
            let outcome = owner.transfer_live_undock_dependency_to_window(opening, dependency);
            if matches!(
                outcome,
                window_session::DockSurfaceWindowSessionDependencyTerminalOutcome::Settled
            ) {
                owner_cx.notify();
            }
            matches!(
                outcome,
                window_session::DockSurfaceWindowSessionDependencyTerminalOutcome::Settled
                    | window_session::DockSurfaceWindowSessionDependencyTerminalOutcome::AlreadyTerminal
            )
        })
    });

    if !dependency_transferred {
        settle_live_undock_dependency(owner, identity, dependency, cx);
    }

    let retirement = completion
        .map(|completion| {
            runtime.retire_live_undock_provisional(completion, window, shutdown_terminal_owned, cx)
        })
        .unwrap_or(crate::DockViewportLiveUndockProvisionalRetirementOutcome::Stale);
    match retirement {
        crate::DockViewportLiveUndockProvisionalRetirementOutcome::CloseDispatched => {}
        crate::DockViewportLiveUndockProvisionalRetirementOutcome::ShutdownCloseRequired => {
            debug_assert!(
                shutdown_terminal_owned,
                "shutdown-owned provisional close requires one WindowSession terminal ticket"
            );
            close_surface_window(owner, lease, window, cx);
        }
        crate::DockViewportLiveUndockProvisionalRetirementOutcome::Stale => {
            if shutdown_terminal_owned {
                close_surface_window(owner, lease, window, cx);
            } else {
                close_live_undock_window_quietly(window, cx);
            }
        }
    }
}

pub(crate) fn settle_live_undock_dependency(
    owner: &Entity<DockSurfaceOwner>,
    identity: live_undock::DockLiveUndockIdentity,
    dependency: Option<window_session::DockSurfaceWindowSessionDependencyId>,
    cx: &mut App,
) {
    let Some(dependency) = dependency else {
        return;
    };
    let lease = identity.opening().lease();
    cx.update_entity(owner, |owner, owner_cx| {
        let outcome = owner
            .window_session_mut()
            .settle_dependency(lease, dependency);
        if matches!(
            outcome,
            window_session::DockSurfaceWindowSessionDependencyTerminalOutcome::Settled
        ) {
            owner_cx.notify();
        }
    });
    let runtime = cx.read_entity(owner, |owner, _| owner.runtime());
    drive_surface_shutdown_convergence(owner, &runtime, lease, cx);
}

pub(crate) fn fail_live_undock_dependency(
    owner: &Entity<DockSurfaceOwner>,
    identity: live_undock::DockLiveUndockIdentity,
    dependency: window_session::DockSurfaceWindowSessionDependencyId,
    cx: &mut App,
) {
    let lease = identity.opening().lease();
    cx.update_entity(owner, |owner, owner_cx| {
        let outcome = owner
            .window_session_mut()
            .fail_dependency(lease, dependency);
        if matches!(
            outcome,
            window_session::DockSurfaceWindowSessionDependencyTerminalOutcome::Failed
        ) {
            owner_cx.notify();
        }
    });
    let runtime = cx.read_entity(owner, |owner, _| owner.runtime());
    drive_surface_shutdown_convergence(owner, &runtime, lease, cx);
}

fn settle_surface_window_terminal(
    owner: &Entity<DockSurfaceOwner>,
    runtime: &DockViewportRuntimeHandle,
    lease: window_session::DockSurfaceWindowSessionLease,
    window_id: WindowId,
    disposition: window_session::DockSurfaceWindowSessionTerminalDisposition,
    cx: &mut App,
) -> Option<open_gpui::AnyWindowHandle> {
    let runtime_settled = runtime.settle_surface_window_terminal(lease, window_id, cx);
    let live_effects = cx.update_entity(owner, |owner, owner_cx| {
        let (live_terminal, live_effects) = owner
            .settle_live_undock_window_terminal(window_id)
            .into_parts();
        debug_assert!(live_terminal.is_none_or(|terminal| terminal.lease() == lease));
        let terminal = owner
            .window_session_mut()
            .settle_terminal(lease, window_id, disposition);
        if runtime_settled
            || live_terminal.is_some()
            || matches!(
                terminal,
                window_session::DockSurfaceWindowSessionTerminalOutcome::Settled
            )
        {
            owner_cx.notify();
        }
        live_effects
    });
    let anchor = advance_surface_shutdown_convergence(owner, runtime, lease, cx);
    let live_runtime = cx.read_entity(owner, |owner, _| owner.live_undock_runtime());
    live_runtime.enqueue_effects(live_effects, cx);
    anchor
}

fn claim_surface_window_close(
    owner: &Entity<DockSurfaceOwner>,
    lease: window_session::DockSurfaceWindowSessionLease,
    window_id: WindowId,
    cx: &mut App,
) -> bool {
    matches!(
        cx.update_entity(owner, |owner, owner_cx| {
            let outcome = owner
                .window_session_mut()
                .claim_close_dispatch(lease, window_id);
            if matches!(
                outcome,
                window_session::DockSurfaceWindowSessionCloseDispatchOutcome::Claimed
            ) {
                owner_cx.notify();
            }
            outcome
        }),
        window_session::DockSurfaceWindowSessionCloseDispatchOutcome::Claimed
    )
}

fn mark_surface_window_close_dispatched(
    owner: &Entity<DockSurfaceOwner>,
    lease: window_session::DockSurfaceWindowSessionLease,
    window_id: WindowId,
    cx: &mut App,
) {
    cx.update_entity(owner, |owner, owner_cx| {
        let outcome = owner
            .window_session_mut()
            .mark_close_dispatched(lease, window_id);
        if matches!(
            outcome,
            window_session::DockSurfaceWindowSessionCloseDispatchCommitOutcome::Dispatched
        ) {
            owner_cx.notify();
        }
    });
}

fn retry_surface_window_close_dispatch(
    owner: &Entity<DockSurfaceOwner>,
    lease: window_session::DockSurfaceWindowSessionLease,
    window_id: WindowId,
    cx: &mut App,
) -> bool {
    matches!(
        cx.update_entity(owner, |owner, owner_cx| {
            let outcome = owner
                .window_session_mut()
                .retry_close_dispatch(lease, window_id);
            if matches!(
                outcome,
                window_session::DockSurfaceWindowSessionCloseDispatchRetryOutcome::Pending
            ) {
                owner_cx.notify();
            }
            outcome
        }),
        window_session::DockSurfaceWindowSessionCloseDispatchRetryOutcome::Pending
    )
}

fn defer_surface_window_close_retry(
    owner: &Entity<DockSurfaceOwner>,
    lease: window_session::DockSurfaceWindowSessionLease,
    window: open_gpui::AnyWindowHandle,
    cx: &mut App,
) {
    let owner = owner.clone();
    cx.defer_shutdown_critical_before_window_registry_clear(move |cx| {
        close_surface_window(&owner, lease, window, cx)
    });
}

fn close_surface_window(
    owner: &Entity<DockSurfaceOwner>,
    lease: window_session::DockSurfaceWindowSessionLease,
    window: open_gpui::AnyWindowHandle,
    cx: &mut App,
) {
    let window_id = window.window_id();
    if !claim_surface_window_close(owner, lease, window_id, cx) {
        return;
    }

    #[cfg(any(test, feature = "test-support"))]
    let force_busy = cx.update_entity(owner, |owner, _| {
        owner.take_shutdown_test_busy_close(lease, window_id)
    });
    #[cfg(not(any(test, feature = "test-support")))]
    let force_busy = false;

    let mut entered_window_update = false;
    let mut dispatch_panic = None;
    if !force_busy {
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
            window.update(cx, |_, window, cx| {
                entered_window_update = true;
                window.remove_window(cx);
            })
        })) {
            dispatch_panic = Some(payload);
        }
    }

    if entered_window_update {
        mark_surface_window_close_dispatched(owner, lease, window_id, cx);
        #[cfg(any(test, feature = "test-support"))]
        record_surface_shutdown_test_event(
            owner,
            lease,
            if window_id == lease.anchor() {
                DockSurfaceShutdownTestEventKind::AnchorCloseDispatched { window: window_id }
            } else {
                DockSurfaceShutdownTestEventKind::DependentCloseDispatched { window: window_id }
            },
            cx,
        );
    } else if retry_surface_window_close_dispatch(owner, lease, window_id, cx) {
        #[cfg(any(test, feature = "test-support"))]
        record_surface_shutdown_test_event(
            owner,
            lease,
            if window_id == lease.anchor() {
                DockSurfaceShutdownTestEventKind::AnchorCloseDispatchDeferredBusy {
                    window: window_id,
                }
            } else {
                DockSurfaceShutdownTestEventKind::DependentCloseDispatchDeferredBusy {
                    window: window_id,
                }
            },
            cx,
        );
        if cx.windows().contains(&window) {
            defer_surface_window_close_retry(owner, lease, window, cx);
        }
    }

    if let Some(payload) = dispatch_panic {
        resume_unwind(payload);
    }
}

fn advance_surface_shutdown_convergence(
    owner: &Entity<DockSurfaceOwner>,
    runtime: &DockViewportRuntimeHandle,
    lease: window_session::DockSurfaceWindowSessionLease,
    cx: &mut App,
) -> Option<open_gpui::AnyWindowHandle> {
    if !cx.read_entity(owner, |owner, _| {
        owner.window_session().is_shutting_down(lease)
    }) {
        return None;
    }
    let runtime_empty = runtime.surface_generation_empty(lease);
    let (pending, has_pending_dependencies, closed) = cx.update_entity(owner, |owner, owner_cx| {
        if !owner.window_session().is_shutting_down(lease) {
            return (None, false, false);
        }
        let runtime = runtime_empty.then(|| owner.window_session_mut().mark_runtime_empty(lease));
        let pending = owner.window_session().pending_terminal_window_ids(lease);
        let has_pending_dependencies = owner.window_session().has_pending_dependencies(lease);
        let convergence = owner.window_session_mut().complete_shutdown(lease);
        let closed = matches!(
            convergence,
            window_session::DockSurfaceWindowSessionShutdownConvergenceOutcome::Closed
        );
        if matches!(
            runtime,
            Some(window_session::DockSurfaceWindowSessionRuntimeEmptyOutcome::Marked)
        ) || closed
        {
            owner_cx.notify();
        }
        (pending, has_pending_dependencies, closed)
    });
    if closed || has_pending_dependencies {
        return None;
    }
    let Some(pending) = pending else {
        return None;
    };
    if pending.as_slice() != [lease.anchor()] {
        return None;
    }

    runtime
        .windows_for_surface(lease)
        .into_iter()
        .find_map(|(role, window)| {
            (role == DockViewportWindowRole::PrimaryAnchor).then_some(window)
        })
        .filter(|anchor| cx.windows().contains(anchor))
}

fn drive_surface_shutdown_convergence(
    owner: &Entity<DockSurfaceOwner>,
    runtime: &DockViewportRuntimeHandle,
    lease: window_session::DockSurfaceWindowSessionLease,
    cx: &mut App,
) {
    if let Some(anchor) = advance_surface_shutdown_convergence(owner, runtime, lease, cx) {
        close_surface_window(owner, lease, anchor, cx);
    }
}

struct DockSurfaceShutdownCloseEffects {
    runtime: DockViewportRuntimeHandle,
    lease: window_session::DockSurfaceWindowSessionLease,
    reservation: DockViewportSurfaceShutdownReservation,
    activation_settlements: DockSurfaceActivationSettlements,
    first_panic: Option<DockSurfaceShutdownPanic>,
}

struct DockSurfaceShutdownStart {
    close_effects: DockSurfaceShutdownCloseEffects,
    live_runtime: live_undock_runtime::DockLiveUndockRuntime,
    live_effects: Option<live_undock::DockLiveUndockEffects>,
}

fn prepare_surface_shutdown_start(
    owner: &Entity<DockSurfaceOwner>,
    lease: window_session::DockSurfaceWindowSessionLease,
    reason: DockSurfaceWindowSessionShutdownReason,
    cx: &mut App,
) -> Option<DockSurfaceShutdownStart> {
    let (runtime, live_runtime) = cx.read_entity(owner, |owner, _| {
        (owner.runtime(), owner.live_undock_runtime())
    });
    let snapshot = runtime.windows_for_surface(lease);
    let (begin, activation_settlements, live_effects) =
        cx.update_entity(owner, |owner, owner_cx| {
            let live_shutdown = owner
                .window_session()
                .admits(lease)
                .then(|| owner.live_undock_shutdown_snapshot(lease))
                .flatten();
            let begin = owner.window_session_mut().begin_shutdown_with_dependencies(
                lease,
                reason,
                snapshot.iter().map(|(_, window)| window.window_id()),
                live_shutdown.map(|snapshot| snapshot.dependency()),
            );
            let (activation_settlements, live_effects) = if matches!(
                begin,
                window_session::DockSurfaceWindowSessionBeginShutdownOutcome::Started { .. }
            ) {
                let promotion_commit = live_shutdown.map_or(
                    live_undock::DockLiveUndockPromotionCommitDisposition::RollbackAllowed,
                    |snapshot| {
                        live_runtime.claim_promotion_commit_for_shutdown(snapshot.identity())
                    },
                );
                let (frozen, effects) = owner
                    .freeze_live_undock_for_shutdown(lease, promotion_commit)
                    .into_parts();
                assert_eq!(
                    frozen, live_shutdown,
                    "live-undock shutdown snapshot must remain exact inside one owner update"
                );
                owner_cx.notify();
                (owner.activation_mut().freeze_lease(lease), Some(effects))
            } else {
                (DockSurfaceActivationSettlements::default(), None)
            };
            (begin, activation_settlements, live_effects)
        });
    match begin {
        window_session::DockSurfaceWindowSessionBeginShutdownOutcome::Started { .. } => {}
        window_session::DockSurfaceWindowSessionBeginShutdownOutcome::AlreadyShuttingDown => {
            return None;
        }
        window_session::DockSurfaceWindowSessionBeginShutdownOutcome::StaleLease
        | window_session::DockSurfaceWindowSessionBeginShutdownOutcome::NotActive => return None,
    }

    let reservation = runtime
        .freeze_surface_shutdown(lease)
        .expect("an active DockSurface lease must own the matching runtime generation");
    Some(DockSurfaceShutdownStart {
        close_effects: DockSurfaceShutdownCloseEffects {
            runtime,
            lease,
            reservation,
            activation_settlements,
            first_panic: None,
        },
        live_runtime,
        live_effects,
    })
}

#[cfg(test)]
fn prepare_surface_shutdown(
    owner: &Entity<DockSurfaceOwner>,
    lease: window_session::DockSurfaceWindowSessionLease,
    reason: DockSurfaceWindowSessionShutdownReason,
    cx: &mut App,
) -> Option<DockSurfaceShutdownCloseEffects> {
    let start = prepare_surface_shutdown_start(owner, lease, reason, cx)?;
    if let Some(live_effects) = start.live_effects {
        start.live_runtime.enqueue_effects(live_effects, cx);
    }
    Some(start.close_effects)
}

fn apply_surface_shutdown_close_effects(
    owner: &Entity<DockSurfaceOwner>,
    effects: DockSurfaceShutdownCloseEffects,
    cx: &mut App,
) {
    let DockSurfaceShutdownCloseEffects {
        runtime,
        lease,
        reservation,
        activation_settlements,
        mut first_panic,
    } = effects;
    if !cx.read_entity(owner, |owner, _| {
        owner.window_session().is_shutting_down(lease)
    }) {
        return;
    }

    let recovery_windows = reservation.windows().to_vec();
    let windows = match catch_unwind(AssertUnwindSafe(|| {
        runtime.commit_surface_shutdown(reservation, cx)
    })) {
        Ok(windows) => windows,
        Err(payload) => {
            retain_first_surface_shutdown_panic(
                &mut first_panic,
                Err(payload),
                "viewport runtime shutdown commit",
            );
            recovery_windows
        }
    };
    dispatch_surface_shutdown_retirement_effects(
        owner,
        &runtime,
        lease,
        windows,
        activation_settlements,
        &mut first_panic,
        cx,
    );
    if let Some(payload) = first_panic {
        resume_unwind(payload);
    }
}

fn apply_surface_capture_failure_retirement(
    owner: &Entity<DockSurfaceOwner>,
    effects: DockSurfaceShutdownCloseEffects,
    cx: &mut App,
) -> Option<DockSurfaceShutdownPanic> {
    let DockSurfaceShutdownCloseEffects {
        runtime,
        lease,
        reservation,
        activation_settlements,
        mut first_panic,
    } = effects;
    if !cx.read_entity(owner, |owner, _| {
        owner.window_session().is_shutting_down(lease)
    }) {
        return first_panic;
    }

    let recovery_windows = reservation.windows().to_vec();
    let windows = match catch_unwind(AssertUnwindSafe(|| {
        runtime.retire_frozen_surface_after_capture_failure(reservation, cx)
    })) {
        Ok(windows) => windows,
        Err(payload) => {
            retain_first_surface_shutdown_panic(
                &mut first_panic,
                Err(payload),
                "viewport runtime capture-failure retirement",
            );
            recovery_windows
        }
    };
    dispatch_surface_shutdown_retirement_effects(
        owner,
        &runtime,
        lease,
        windows,
        activation_settlements,
        &mut first_panic,
        cx,
    );
    first_panic
}

fn dispatch_surface_shutdown_retirement_effects(
    owner: &Entity<DockSurfaceOwner>,
    runtime: &DockViewportRuntimeHandle,
    lease: window_session::DockSurfaceWindowSessionLease,
    mut windows: Vec<(DockViewportWindowRole, open_gpui::AnyWindowHandle)>,
    activation_settlements: DockSurfaceActivationSettlements,
    first_panic: &mut Option<DockSurfaceShutdownPanic>,
    cx: &mut App,
) {
    #[cfg(any(test, feature = "test-support"))]
    if let Some(message) = cx.update_entity(owner, |owner, _| {
        owner.take_shutdown_test_cleanup_panic(lease)
    }) {
        let panic = catch_unwind(AssertUnwindSafe(|| panic!("{}", message)));
        if panic.is_err() {
            record_surface_shutdown_test_event(
                owner,
                lease,
                DockSurfaceShutdownTestEventKind::CleanupCallbackPanicked,
                cx,
            );
        }
        retain_first_surface_shutdown_panic(first_panic, panic, "test cleanup callback delivery");
    }

    retain_first_surface_shutdown_panic(
        first_panic,
        catch_unwind(AssertUnwindSafe(|| activation_settlements.deliver(cx))),
        "activation settlement delivery",
    );

    let mut seen = Vec::new();
    windows.retain(|(_, window)| {
        let window_id = window.window_id();
        if seen.contains(&window_id) {
            false
        } else {
            seen.push(window_id);
            true
        }
    });
    for (_, window) in windows.iter().filter(|(role, _)| {
        matches!(
            role,
            DockViewportWindowRole::ManagedViewport
                | DockViewportWindowRole::ProvisionalViewport(_)
        )
    }) {
        retain_first_surface_shutdown_panic(
            first_panic,
            catch_unwind(AssertUnwindSafe(|| {
                close_surface_window(owner, lease, *window, cx);
            })),
            "managed viewport close dispatch",
        );
    }
    retain_first_surface_shutdown_panic(
        first_panic,
        catch_unwind(AssertUnwindSafe(|| {
            drive_surface_shutdown_convergence(owner, runtime, lease, cx);
        })),
        "primary anchor close dispatch",
    );
}

fn finish_scheduled_surface_shutdown(
    owner: &Entity<DockSurfaceOwner>,
    lease: window_session::DockSurfaceWindowSessionLease,
    coordinator: &DockSurfaceShutdownCoordinator,
    release: DockNativeCapturedSurfaceRelease,
    first_panic: &mut Option<DockSurfaceShutdownPanic>,
    cx: &mut App,
) {
    let release_outcome = release.outcome();
    let Some(mut pending) = coordinator.take_after_capture_terminal(lease) else {
        return;
    };
    match release_outcome {
        DockNativeCapturedSurfaceReleaseOutcome::Released => {
            #[cfg(any(test, feature = "test-support"))]
            record_surface_shutdown_test_event(
                owner,
                lease,
                DockSurfaceShutdownTestEventKind::CaptureCleanupCompleted {
                    releases: release
                        .evidence()
                        .iter()
                        .copied()
                        .map(
                            |(barrier, terminal)| DockSurfaceShutdownCaptureReleaseEvidence {
                                barrier,
                                terminal,
                            },
                        )
                        .collect(),
                },
                cx,
            );
            retain_first_surface_shutdown_panic(
                first_panic,
                catch_unwind(AssertUnwindSafe(|| {
                    apply_surface_shutdown_close_effects(owner, pending.effects, cx);
                })),
                "capture-terminal surface close effects",
            );
        }
        DockNativeCapturedSurfaceReleaseOutcome::Failed => {
            let mut prior_panic = first_panic.take();
            match catch_unwind(AssertUnwindSafe(|| {
                apply_surface_capture_failure_retirement(owner, pending.effects, cx)
            })) {
                Ok(Some(payload)) | Err(payload) => retain_first_surface_shutdown_panic(
                    &mut prior_panic,
                    Err(payload),
                    "capture-failure surface retirement",
                ),
                Ok(None) => {}
            }
            *first_panic = Some(Box::new(DockSurfaceCaptureReleaseFailure {
                lease,
                prior_panic,
            }));
        }
    }
    for finalizer in pending.payload_finalizers.drain(..) {
        let completed = finalizer.complete();
        debug_assert!(
            completed,
            "surface shutdown must complete each transferred payload finalizer exactly once"
        );
    }
    if let Some(round) = pending.app_shutdown_round.take() {
        let close_dispatch = match first_panic.take() {
            Some(payload) => Err(payload),
            None => Ok(()),
        };
        round.settle(DockSurfaceAppShutdownSettlement { close_dispatch }, cx);
    }
}

#[derive(Clone)]
struct DockSurfaceShutdownStartTransaction {
    owner: Entity<DockSurfaceOwner>,
    lease: window_session::DockSurfaceWindowSessionLease,
    coordinator: DockSurfaceShutdownCoordinator,
}

impl DockSurfaceShutdownStartTransaction {
    fn new(
        owner: Entity<DockSurfaceOwner>,
        lease: window_session::DockSurfaceWindowSessionLease,
        coordinator: DockSurfaceShutdownCoordinator,
    ) -> Self {
        Self {
            owner,
            lease,
            coordinator,
        }
    }

    fn settle_capture_terminal(
        &self,
        release: DockNativeCapturedSurfaceRelease,
        first_panic: &mut Option<DockSurfaceShutdownPanic>,
        cx: &mut App,
    ) {
        finish_scheduled_surface_shutdown(
            &self.owner,
            self.lease,
            &self.coordinator,
            release,
            first_panic,
            cx,
        );
    }

    fn run_startup(&self, startup: impl FnOnce(&mut App), cx: &mut App) {
        let startup = catch_unwind(AssertUnwindSafe(|| startup(cx)));
        if let Err(payload) = startup {
            self.settle_failed_start(payload, cx);
        }
    }

    fn settle_failed_start(&self, payload: DockSurfaceShutdownPanic, cx: &mut App) {
        let mut first_panic = Some(payload);
        let convergence = catch_unwind(AssertUnwindSafe(|| {
            self.settle_capture_terminal(
                DockNativeCapturedSurfaceRelease::without_evidence(
                    DockNativeCapturedSurfaceReleaseOutcome::Failed,
                ),
                &mut first_panic,
                cx,
            );
        }));
        retain_first_surface_shutdown_panic(
            &mut first_panic,
            convergence,
            "surface shutdown startup failure convergence",
        );
        if let Some(payload) = first_panic {
            resume_unwind(payload);
        }
    }
}

fn quiesce_surface_shutdown_windows(
    _coordinator: &DockSurfaceShutdownCoordinator,
    _lease: window_session::DockSurfaceWindowSessionLease,
    windows: Vec<(DockViewportWindowRole, AnyWindowHandle)>,
    mut current_window: Option<&mut Window>,
    cx: &mut App,
) -> std::thread::Result<Option<WindowId>> {
    let mut targets = unique_windows(windows.into_iter().map(|(_, window)| window).collect());
    targets.sort_by_key(|window| window.window_id().as_u64());

    let captured_source = cx
        .active_native_captured_drag_source_window()
        .filter(|source| targets.iter().any(|target| target.window_id() == *source));
    let mut first_panic = None;

    for target in &targets {
        let revoke = catch_unwind(AssertUnwindSafe(|| {
            #[cfg(test)]
            if _coordinator.observe_window_authority_revoke_for_test(_lease) {
                panic!("injected surface shutdown window-authority revoke panic");
            }
            let revoked = if captured_source == Some(target.window_id()) {
                cx.quiesce_window_interaction_authority_preserving_native_pointer_capture(
                    target.window_id(),
                )
            } else {
                cx.quiesce_window_interaction_authority(target.window_id())
            };
            assert!(
                revoked || cx.window_profile(*target).is_none(),
                "surface shutdown lost interaction authority for live window {:?}",
                target.window_id()
            );
        }));
        retain_first_surface_shutdown_panic(
            &mut first_panic,
            revoke,
            "surface-window interaction authority revocation",
        );
    }
    retain_first_surface_shutdown_panic(
        &mut first_panic,
        catch_unwind(AssertUnwindSafe(|| cx.stop_propagation())),
        "surface-window event propagation stop",
    );

    if let Some(window) = current_window.as_deref_mut() {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let window_id = window.window_handle().window_id();
            if targets.iter().any(|target| target.window_id() == window_id) {
                targets.retain(|target| target.window_id() != window_id);
                if captured_source == Some(window_id) {
                    window.quiesce_interaction_preserving_native_pointer_capture(cx);
                } else {
                    window.quiesce_interaction(cx);
                }
            }
        }));
        retain_first_surface_shutdown_panic(
            &mut first_panic,
            result,
            "current surface-window interaction quiescence",
        );
    }

    for target in targets {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let preserve_native_capture = captured_source == Some(target.window_id());
            let _ = target.update(cx, |_, window, cx| {
                if preserve_native_capture {
                    window.quiesce_interaction_preserving_native_pointer_capture(cx);
                } else {
                    window.quiesce_interaction(cx);
                }
            });
        }));
        retain_first_surface_shutdown_panic(
            &mut first_panic,
            result,
            "dependent surface-window interaction quiescence",
        );
    }
    match first_panic {
        Some(payload) => Err(payload),
        None => Ok(captured_source),
    }
}

fn start_surface_shutdown(
    owner: &Entity<DockSurfaceOwner>,
    lease: window_session::DockSurfaceWindowSessionLease,
    reason: DockSurfaceWindowSessionShutdownReason,
    current_window: Option<&mut Window>,
    app_shutdown_round: Option<DockSurfaceAppShutdownRound>,
    cx: &mut App,
) -> bool {
    let Some(start) = prepare_surface_shutdown_start(owner, lease, reason, cx) else {
        return false;
    };
    let DockSurfaceShutdownStart {
        close_effects,
        live_runtime,
        live_effects,
    } = start;
    let runtime_identity = close_effects.runtime.identity();
    let windows = close_effects.reservation.windows().to_vec();
    let coordinator = surface_shutdown_coordinator(cx);
    let start_transaction =
        DockSurfaceShutdownStartTransaction::new(owner.clone(), lease, coordinator.clone());
    coordinator.begin(close_effects, app_shutdown_round);
    start_transaction.run_startup(
        |cx| {
            let mut first_panic = None;
            let captured_source = match catch_unwind(AssertUnwindSafe(|| {
                quiesce_surface_shutdown_windows(&coordinator, lease, windows, current_window, cx)
            })) {
                Ok(Ok(captured_source)) => captured_source,
                Ok(Err(payload)) | Err(payload) => {
                    retain_first_surface_shutdown_panic(
                        &mut first_panic,
                        Err(payload),
                        "surface-window interaction quiescence",
                    );
                    None
                }
            };
            if let Some(live_effects) = live_effects {
                retain_first_surface_shutdown_panic(
                    &mut first_panic,
                    catch_unwind(AssertUnwindSafe(|| {
                        live_runtime.enqueue_effects(live_effects, cx);
                    })),
                    "live-undock shutdown effect enqueue",
                );
            }
            if let Some(payload) = first_panic {
                resume_unwind(payload);
            }

            #[cfg(test)]
            if let Some(late_callback_ran) = coordinator.take_capture_setup_fault_for_test(lease) {
                let late_transaction = start_transaction.clone();
                cx.defer_shutdown_critical_before_window_registry_clear(move |cx| {
                    late_callback_ran.set(true);
                    let mut first_panic = None;
                    late_transaction.settle_capture_terminal(
                        DockNativeCapturedSurfaceRelease::without_evidence(
                            DockNativeCapturedSurfaceReleaseOutcome::Released,
                        ),
                        &mut first_panic,
                        cx,
                    );
                    if let Some(payload) = first_panic {
                        resume_unwind(payload);
                    }
                });
                panic!("injected surface shutdown capture setup panic");
            }

            let capture_terminal = start_transaction.clone();
            crate::native_captured_drag::cancel_native_captured_drag_route_for_surface(
                runtime_identity,
                lease,
                captured_source.is_some(),
                move |release, first_panic, cx| {
                    capture_terminal.settle_capture_terminal(release, first_panic, cx);
                },
                cx,
            );
        },
        cx,
    );
    true
}

#[cfg(test)]
fn schedule_surface_shutdown_close_effects(
    owner: &Entity<DockSurfaceOwner>,
    effects: DockSurfaceShutdownCloseEffects,
    app_shutdown_round: Option<DockSurfaceAppShutdownRound>,
    cx: &mut App,
) {
    let lease = effects.lease;
    let runtime_identity = effects.runtime.identity();
    let coordinator = surface_shutdown_coordinator(cx);
    coordinator.begin(effects, app_shutdown_round);
    let owner = owner.clone();
    let completion_coordinator = coordinator.clone();
    crate::native_captured_drag::cancel_native_captured_drag_route_for_surface(
        runtime_identity,
        lease,
        false,
        move |release_terminal, first_panic, cx| {
            finish_scheduled_surface_shutdown(
                &owner,
                lease,
                &completion_coordinator,
                release_terminal,
                first_panic,
                cx,
            );
        },
        cx,
    );
}

pub(crate) fn handle_surface_window_closed(
    owner: &Entity<DockSurfaceOwner>,
    window_id: WindowId,
    cx: &mut App,
) -> Option<live_undock_runtime::DockLiveUndockLogicalCloseAuthority> {
    payload_recovery_executor::payload_recovery_source_window_closed(owner, window_id, cx);
    let lease = cx.read_entity(owner, |owner, _| {
        owner.window_session().active_lease_for_anchor(window_id)
    });
    if let Some(lease) = lease {
        // A directly removed anchor can no longer uphold the dependent-first native retirement
        // contract installed for guarded surface shutdown. Releasing that obsolete edge also lets
        // its native terminal settle an outstanding pointer-capture barrier before dependents are
        // retired.
        cx.cancel_native_window_retirement_dependencies(window_id);
        let _ = start_surface_shutdown(
            owner,
            lease,
            DockSurfaceWindowSessionShutdownReason::AnchorDestroyed,
            None,
            None,
            cx,
        );
        return None;
    }

    let committed_authority = cx.read_entity(owner, |owner, _| {
        owner.live_undock_committed_destination_logical_close_authority(window_id)
    });
    if committed_authority.is_some() {
        return committed_authority;
    }

    // Logical removal is the terminal boundary for an uncommitted provisional destination. The
    // native window may remain retained by a retirement dependency, so waiting for native-terminal
    // observation or DockHost release would strand reveal and payload finalization authority.
    let live_effects = cx.update_entity(owner, |owner, owner_cx| {
        let (terminal, effects) = owner
            .settle_live_undock_window_terminal(window_id)
            .into_parts();
        if terminal.is_some() || !effects.is_empty() {
            owner_cx.notify();
        }
        effects
    });
    let live_runtime = cx.read_entity(owner, |owner, _| owner.live_undock_runtime());
    live_runtime.enqueue_effects(live_effects, cx);
    None
}

fn handle_surface_window_native_terminal(
    owner: &Entity<DockSurfaceOwner>,
    window_id: WindowId,
    cx: &mut App,
) {
    let (runtime, live_undock_runtime) = cx.read_entity(owner, |owner, _| {
        (owner.runtime(), owner.live_undock_runtime())
    });
    live_undock_runtime.source_window_native_terminal(window_id, cx);
    payload_recovery_executor::payload_recovery_source_window_native_terminal(owner, window_id, cx);
    let lease = cx.read_entity(owner, |owner, _| {
        owner
            .window_session()
            .shutting_down_lease_for_window(window_id)
            .or_else(|| owner.live_undock_lease_for_window(window_id))
    });
    let Some(lease) = lease else {
        return;
    };
    let anchor = settle_surface_window_terminal(
        owner,
        &runtime,
        lease,
        window_id,
        window_session::DockSurfaceWindowSessionTerminalDisposition::ObservedClosed,
        cx,
    );
    if let Some(anchor) = anchor {
        let owner = owner.clone();
        cx.defer_shutdown_critical_before_window_registry_clear_or_run_now(move |cx| {
            close_surface_window(&owner, lease, anchor, cx);
        });
    }
}

fn handle_surface_app_shutdown(
    owner: &Entity<DockSurfaceOwner>,
    round: &DockSurfaceAppShutdownRound,
    cx: &mut App,
) {
    round.register(owner);
    let mut lease = None;
    let mut settlement_deferred = false;
    let close_dispatch = catch_unwind(AssertUnwindSafe(|| {
        let runtime = cx.read_entity(owner, |owner, _| owner.runtime());
        let (opening, active, shutting_down) = cx.read_entity(owner, |owner, _| {
            (
                owner.window_session().opening_token(),
                owner.window_session().active_lease(),
                owner.window_session().shutting_down_lease(),
            )
        });
        if let Some(opening) = opening {
            let windows = runtime.abort_surface_opening(opening);
            cx.update_entity(owner, |owner, owner_cx| {
                let _ = owner.window_session_mut().rollback_opening(
                    opening,
                    DockSurfaceWindowSessionOpeningRollbackReason::AppShutdown,
                );
                owner_cx.notify();
            });
            for window in windows {
                let _ = window.update(cx, |_, window, cx| window.remove_window(cx));
            }
            return;
        }
        lease = active.or(shutting_down);
        let Some(current_lease) = lease else {
            return;
        };
        if let Some(active_lease) = active
            && start_surface_shutdown(
                owner,
                active_lease,
                DockSurfaceWindowSessionShutdownReason::AppShutdown,
                None,
                Some(round.clone()),
                cx,
            )
        {
            settlement_deferred = true;
            return;
        }

        let coordinator = surface_shutdown_coordinator(cx);
        if coordinator.attach_app_shutdown_round(current_lease, round.clone()) {
            settlement_deferred = true;
        }
    }));
    if !settlement_deferred {
        round.settle(DockSurfaceAppShutdownSettlement { close_dispatch }, cx);
    }
}

fn install_surface_app_shutdown_observer(fanout: DockSurfaceAppShutdownFanout, cx: &mut App) {
    cx.on_app_quit(move |cx| {
        let owners = fanout.snapshot_for_shutdown();
        // App consumes quit observers before invoking them. The next lifecycle must be armed
        // before current cleanup can reenter or register another surface.
        fanout.rearm_for_next_lifecycle(cx);
        let round = DockSurfaceAppShutdownRound::default();
        for owner in owners {
            handle_surface_app_shutdown(&owner, &round, cx);
        }
        round.seal(cx);
        std::future::ready(())
    })
    .detach();
}

fn install_primary_window_lifecycle_hooks(
    owner: Entity<DockSurfaceOwner>,
    window: &mut Window,
    cx: &mut App,
) {
    let anchor = window.window_handle().window_id();
    let close_owner = owner.clone();
    window.on_window_should_close(cx, move |window, cx| {
        let lease = cx.read_entity(&close_owner, |owner, _| {
            owner.window_session().active_lease_for_anchor(anchor)
        });
        if let Some(lease) = lease {
            let _ = start_surface_shutdown(
                &close_owner,
                lease,
                DockSurfaceWindowSessionShutdownReason::AnchorCloseRequested,
                Some(window),
                None,
                cx,
            );
            return false;
        }
        !cx.read_entity(&close_owner, |owner, _| {
            owner
                .window_session()
                .protects_anchor_from_native_close(anchor)
        })
    });

    let presentation_owner = owner;
    window
        .observe_window_initial_presentation(move |window, cx| {
            if window.presentation_facts().initial_presentation
                != WindowInitialPresentationStatus::Rejected
            {
                return;
            }
            let Some(lease) = cx.read_entity(&presentation_owner, |owner, _| {
                owner.window_session().active_lease_for_anchor(anchor)
            }) else {
                return;
            };
            let _ = start_surface_shutdown(
                &presentation_owner,
                lease,
                DockSurfaceWindowSessionShutdownReason::PresentationFailed,
                Some(window),
                None,
                cx,
            );
        })
        .detach();
}

impl DockSurface {
    /// Starts a facade-first docking surface builder for a logical dock space.
    pub fn builder(space: impl Into<DockSpaceId>) -> DockSurfaceBuilder {
        DockSurfaceBuilder::new(space)
    }

    #[cfg(test)]
    pub(crate) fn from_controller(controller: Entity<DockController>, cx: &mut App) -> Self {
        Self::from_controller_with_close_policy_and_visual_style_resolver(
            controller,
            DockViewportClosePolicy::default(),
            None,
            cx,
        )
    }

    pub(crate) fn from_controller_with_close_policy_and_visual_style_resolver(
        controller: Entity<DockController>,
        close_policy: DockViewportClosePolicy,
        visual_style_resolver: Option<DockVisualStyleResolver>,
        cx: &mut App,
    ) -> Self {
        let primary_space = cx.read_entity(&controller, |controller, _| controller.space().clone());
        let owner = cx.new(|cx| {
            let viewport_runtime = DockViewportRuntimeHandle::for_surface(
                controller.clone(),
                cx.entity_id(),
                close_policy,
                visual_style_resolver,
            );
            DockSurfaceOwner::new(
                controller,
                viewport_runtime,
                primary_space.clone(),
                cx.entity_id(),
            )
        });
        let live_undock_runtime = cx.read_entity(&owner, |owner, _| owner.live_undock_runtime());
        live_undock_runtime.bind_owner(owner.downgrade());
        let weak_owner = owner.downgrade();
        let runtime = cx.read_entity(&owner, |owner, _| owner.runtime());
        runtime.install_surface_owner(owner.downgrade());
        runtime.install_surface_commit_sink(move |authority, transaction, categories, cx| {
            let Some(owner) = weak_owner.upgrade() else {
                return;
            };
            cx.update_entity(&owner, |owner, owner_cx| {
                let admitted = match authority {
                    DockViewportRuntimeCommitAuthority::Active(work_context) => {
                        match work_context.lineage() {
                            DockViewportRuntimeLineage::Surface(lease) => {
                                owner.window_session().admits(lease)
                            }
                            DockViewportRuntimeLineage::Unmanaged => false,
                        }
                    }
                    DockViewportRuntimeCommitAuthority::FrozenSurfaceShutdown(work_context) => {
                        match work_context.lineage() {
                            DockViewportRuntimeLineage::Surface(lease) => {
                                owner.window_session().shutting_down_lease() == Some(lease)
                            }
                            DockViewportRuntimeLineage::Unmanaged => false,
                        }
                    }
                };
                if !admitted {
                    return;
                }
                if let Some(transaction) = transaction {
                    owner.record_changes(transaction, categories.iter().copied());
                } else {
                    let transaction = owner.begin_root_transaction();
                    owner.record_changes(transaction, categories.iter().copied());
                    owner.finish_root_transaction(transaction, owner_cx);
                }
            });
        });
        let primary_space = cx.read_entity(&owner, |owner, _| owner.primary_space().clone());
        let activation_owner = owner.downgrade();
        cx.on_window_closed(move |cx, window_id| {
            let Some(owner) = activation_owner.upgrade() else {
                return;
            };
            let _ = handle_surface_window_closed(&owner, window_id, cx);
            let settlements = cx.update_entity(&owner, |owner, owner_cx| {
                let settlements = owner.activation_mut().window_closed(window_id);
                owner_cx.notify();
                settlements
            });
            settlements.deliver(cx);
        })
        .detach();
        let terminal_owner = owner.downgrade();
        cx.on_window_native_terminal(move |cx, window_id| {
            if let Some(owner) = terminal_owner.upgrade() {
                handle_surface_window_native_terminal(&owner, window_id, cx);
            }
        })
        .detach();
        let shutdown_fanout = app_shutdown_fanout(cx);
        shutdown_fanout.register(&owner, cx);
        Self {
            owner,
            primary_space,
        }
    }

    pub(crate) fn controller<C: AppContext>(&self, cx: &C) -> Entity<DockController> {
        cx.read_entity(&self.owner, |owner, _| owner.controller())
    }

    pub(crate) fn viewport_runtime<C: AppContext>(&self, cx: &C) -> DockViewportRuntimeHandle {
        cx.read_entity(&self.owner, |owner, _| owner.runtime())
    }

    /// Begins a typed shutdown observation scoped to this exact DockSurface authority.
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn observe_shutdown_for_test(&self, cx: &mut App) -> DockSurfaceShutdownTestObservation {
        let observation = DockSurfaceShutdownTestObservation::default();
        cx.update_entity(&self.owner, |owner, _| {
            owner.install_shutdown_test_observation(observation.clone());
        });
        observation
    }

    /// Observes reducer-accepted live-undock authority transitions for this exact surface.
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn observe_live_undock_for_test(
        &self,
        cx: &mut App,
    ) -> DockSurfaceLiveUndockTestObservation {
        let observation = DockSurfaceLiveUndockTestObservation::default();
        cx.update_entity(&self.owner, |owner, _| {
            owner.install_live_undock_test_observation(observation.clone());
        });
        observation
    }

    /// Arms one exact shutdown generation to exercise retry and panic-continuation behavior.
    ///
    /// The first close attempt for `busy_window` is routed through the production Busy retry path.
    /// The first cleanup callback then panics after native capture cleanup; shutdown must retain the
    /// panic, continue retiring every surface window, and only propagate it after dispatching the
    /// remaining cleanup work.
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn arm_shutdown_retry_and_cleanup_panic_for_test(
        &self,
        busy_window: WindowId,
        panic_message: impl Into<String>,
        cx: &mut App,
    ) -> bool {
        let Some(lease) = cx.read_entity(&self.owner, |owner, _| {
            owner.window_session().active_lease()
        }) else {
            return false;
        };
        let runtime = self.viewport_runtime(cx);
        if busy_window == lease.anchor()
            || !runtime
                .windows_for_surface(lease)
                .iter()
                .any(|(_, window)| window.window_id() == busy_window)
        {
            return false;
        }
        cx.update_entity(&self.owner, |owner, _| {
            owner.install_shutdown_test_faults(lease, busy_window, panic_message.into())
        })
    }

    /// Terminates the next same-window live-undock destination after durable promotion and before
    /// destination semantics acknowledgement.
    #[doc(hidden)]
    #[cfg(feature = "test-support")]
    pub fn terminate_next_live_undock_destination_before_semantics_ack_for_test(&self, cx: &App) {
        let runtime = cx.read_entity(&self.owner, |owner, _| owner.live_undock_runtime());
        runtime.terminate_next_same_window_destination_before_semantics_ack_for_test();
    }

    /// Opens one provisional live-undock viewport through the production opening protocol.
    ///
    /// The optional builder callback runs after the runtime accepts the exact provisional open
    /// attempt but before the window enters the application registry. Native shutdown tests use
    /// this narrow seam to inspect the real native window and prove that App shutdown compensates
    /// the late return without exposing reducer/runtime implementation types.
    #[doc(hidden)]
    #[cfg(feature = "test-support")]
    pub fn open_live_undock_provisional_for_test(
        &self,
        source_window: WindowId,
        drag_generation: u64,
        options: WindowOptions,
        on_builder_entered: Option<Box<dyn FnOnce(&mut Window, &mut App)>>,
        cx: &mut App,
    ) -> std::io::Result<()> {
        let lease = cx
            .read_entity(&self.owner, |owner, _| {
                owner.window_session().active_lease()
            })
            .ok_or_else(|| std::io::Error::other("the DockSurface has no active window lease"))?;
        let drag_generation = live_undock::DockLiveUndockDragGeneration::new(drag_generation)
            .ok_or_else(|| std::io::Error::other("live-undock drag generation must be non-zero"))?;
        let trigger = live_undock::DockLiveUndockTrigger::new(
            drag_generation,
            live_undock::DockLiveUndockSourceSnapshot::new(source_window, 1),
            live_undock::DockLiveUndockRouteGeneration::new(drag_generation.get())
                .expect("the test route generation should be non-zero"),
            live_undock::DockLiveUndockRouteFeedback::Desktop,
            live_undock::DockLiveUndockPhysicalPoint::new(50, 50),
            live_undock::DockLiveUndockPhysicalBounds::new(
                live_undock::DockLiveUndockPhysicalPoint::new(0, 0),
                640,
                480,
            )
            .expect("test provisional bounds must be non-empty"),
        )
        .expect("desktop must remain an eligible live-undock route");
        let request = reduce_live_undock_fact(
            &self.owner,
            live_undock::DockLiveUndockFact::Trigger { lease, trigger },
            cx,
        )
        .and_then(|effects| {
            effects.into_iter().find_map(|effect| match effect {
                live_undock::DockLiveUndockEffect::OpenProvisional { request, .. } => Some(request),
                _ => None,
            })
        })
        .ok_or_else(|| std::io::Error::other("the active DockSurface rejected live undock"))?;
        let runtime = self.viewport_runtime(cx);
        if let Some(on_builder_entered) = on_builder_entered {
            runtime.install_live_undock_provisional_builder_hook_for_test(on_builder_entered);
        }
        runtime
            .open_triggered_live_undock_provisional_viewport(
                self.primary_space.clone(),
                options,
                &request,
                cx,
            )
            .map(|_| ())
            .map_err(|error| std::io::Error::other(error.to_string()))
    }

    pub(crate) fn owner(&self) -> &Entity<DockSurfaceOwner> {
        &self.owner
    }

    /// Returns a read-only snapshot of this surface's primary-window session.
    pub fn window_session_status(&self, cx: &App) -> DockSurfaceWindowSessionStatus {
        cx.read_entity(&self.owner, |owner, _| owner.window_session().status())
    }

    /// Returns the default logical dock space for primary host windows.
    pub fn primary_space(&self) -> &DockSpaceId {
        &self.primary_space
    }

    #[cfg(test)]
    pub(crate) fn primary_host(&self, cx: &mut Context<DockHost>) -> DockHost {
        self.host(self.primary_space.clone(), cx)
    }

    pub(crate) fn host(
        &self,
        space: impl Into<DockSpaceId>,
        cx: &mut Context<DockHost>,
    ) -> DockHost {
        let controller = cx.read_entity(&self.owner, |owner, _| owner.controller());
        let viewport_runtime = cx.read_entity(&self.owner, |owner, _| owner.runtime());
        DockHost::from_embedded_surface_owner(controller, space, viewport_runtime, &self.owner, cx)
    }

    fn opening_primary_host(
        &self,
        opening: window_session::DockSurfaceWindowSessionOpeningToken,
        cx: &mut Context<DockHost>,
    ) -> DockHost {
        let controller = cx.read_entity(&self.owner, |owner, _| owner.controller());
        let viewport_runtime = cx.read_entity(&self.owner, |owner, _| owner.runtime());
        DockHost::from_opening_primary_surface_owner(
            controller,
            self.primary_space.clone(),
            viewport_runtime,
            &self.owner,
            opening,
            cx,
        )
    }

    /// Returns the latest committed persistence revision shared by all surface clones.
    pub fn revision(&self, cx: &App) -> u64 {
        cx.read_entity(&self.owner, |owner, _| owner.revision())
    }

    /// Subscribes to lightweight metadata for committed surface changes.
    ///
    /// Applications own debounce, snapshot export, storage, and file-I/O policy. Dropping the
    /// returned subscription only stops observation.
    pub fn subscribe_changes(
        &self,
        cx: &mut App,
        on_event: impl FnMut(&DockSurfaceChangeEvent, &mut App) + 'static,
    ) -> Subscription {
        owner::subscribe(&self.owner, cx, on_event)
    }

    /// Creates an erased GPUI view that renders the primary dock space inside an existing window.
    pub fn host_view(&self, cx: &mut App) -> AnyView {
        self.host_view_for_space(self.primary_space.clone(), cx)
    }

    /// Creates an erased GPUI view that renders one logical dock space inside an existing window.
    pub fn host_view_for_space(&self, space: impl Into<DockSpaceId>, cx: &mut App) -> AnyView {
        let surface = self.clone();
        let space = space.into();
        cx.new(move |cx| surface.host(space, cx)).into()
    }

    /// Opens a normal GPUI window that renders the primary dock host.
    ///
    /// This is for the main application window and does not require platform viewport-window
    /// capability. Detached platform viewports are opened through the viewport-runtime path.
    pub fn open_primary_window(
        &self,
        options: WindowOptions,
        cx: &mut App,
    ) -> DockSurfacePrimaryWindowOpenOutcome {
        let opening = match cx.update_entity(&self.owner, |owner, owner_cx| {
            let result = owner.window_session_mut().reserve_opening();
            if result.is_ok() {
                owner_cx.notify();
            }
            result
        }) {
            Ok(opening) => opening,
            Err(conflict) => {
                return DockSurfacePrimaryWindowOpenOutcome::Unavailable(
                    DockSurfacePrimaryWindowUnavailable::Conflict(conflict),
                );
            }
        };

        let surface = self.clone();
        let runtime = self.viewport_runtime(cx);
        let opening_attempt = Rc::new(Cell::new(None));
        let opening_attempt_for_builder = opening_attempt.clone();
        let opening_runtime = runtime.clone();
        let open_result = catch_unwind(AssertUnwindSafe(|| {
            cx.open_window_detailed(options, move |window, cx| {
                opening_attempt_for_builder.set(
                    opening_runtime
                        .begin_primary_anchor_open_attempt(window.window_handle(), opening),
                );
                let lifecycle_owner = surface.owner().clone();
                let host = cx.new(move |cx| surface.opening_primary_host(opening, cx));
                install_primary_window_lifecycle_hooks(lifecycle_owner, window, cx);
                host
            })
        }));
        let window = match open_result {
            Err(payload) => {
                if let Some(attempt) = opening_attempt.take() {
                    let _ = runtime.abort_window_open_attempt(attempt);
                }
                cx.update_entity(&self.owner, |owner, owner_cx| {
                    let _ = owner.window_session_mut().rollback_opening(
                        opening,
                        DockSurfaceWindowSessionOpeningRollbackReason::Panicked,
                    );
                    owner_cx.notify();
                });
                resume_unwind(payload);
            }
            Ok(Ok(window)) => window,
            Ok(Err(error)) => {
                if let Some(attempt) = opening_attempt.take() {
                    let _ = runtime.abort_window_open_attempt(attempt);
                }
                let reason = match error.stage() {
                    WindowOpenFailureStage::AppShutdown => {
                        DockSurfaceWindowSessionOpeningRollbackReason::AppShutdown
                    }
                    WindowOpenFailureStage::ClosedDuringNativeCreateOrMap
                    | WindowOpenFailureStage::ClosedDuringBuild
                    | WindowOpenFailureStage::ClosedDuringInitialDraw
                    | WindowOpenFailureStage::ClosedDuringInitialPresentation => {
                        DockSurfaceWindowSessionOpeningRollbackReason::ClosedDuringOpening
                    }
                    WindowOpenFailureStage::BeforeVisibilityPresentation => {
                        DockSurfaceWindowSessionOpeningRollbackReason::PresentationFailedBeforeVisibility
                    }
                    WindowOpenFailureStage::NativeCreateOrMap
                    | WindowOpenFailureStage::CommitRejected => {
                        DockSurfaceWindowSessionOpeningRollbackReason::WindowOpenFailed
                    }
                    _ => DockSurfaceWindowSessionOpeningRollbackReason::WindowOpenFailed,
                };
                cx.update_entity(&self.owner, |owner, owner_cx| {
                    let _ = owner.window_session_mut().rollback_opening(opening, reason);
                    owner_cx.notify();
                });
                return DockSurfacePrimaryWindowOpenOutcome::Unavailable(
                    DockSurfacePrimaryWindowUnavailable::OpeningRolledBack {
                        reason,
                        message: error.to_string(),
                    },
                );
            }
        };
        let Some(opening_attempt) = opening_attempt.take() else {
            crate::close_window_quietly(window.into(), cx);
            let reason = DockSurfaceWindowSessionOpeningRollbackReason::WindowOpenFailed;
            cx.update_entity(&self.owner, |owner, owner_cx| {
                let _ = owner.window_session_mut().rollback_opening(opening, reason);
                owner_cx.notify();
            });
            return DockSurfacePrimaryWindowOpenOutcome::Unavailable(
                DockSurfacePrimaryWindowUnavailable::OpeningRolledBack {
                    reason,
                    message: "Dock primary opening handle could not be reserved".to_string(),
                },
            );
        };
        let anchor = window.window_id();
        let host = window
            .entity(cx)
            .expect("a committed Dock primary window must retain its opening host");
        let host_can_promote = cx.read_entity(&host, |host, _| {
            host.can_promote_primary_anchor(opening, anchor)
        });
        assert!(
            host_can_promote,
            "committed Dock primary window lost its exact opening host authority"
        );

        let lease = cx.update_entity(&self.owner, |owner, owner_cx| {
            let lease = owner
                .window_session_mut()
                .commit_opening(opening, anchor)
                .expect("validated Dock primary opening changed before activation");
            assert!(
                owner.activation_mut().activate_lease(lease),
                "Dock primary activation must arm the matching surface activation lease"
            );
            owner_cx.notify();
            lease
        });
        let lineage_activation = runtime.activate_surface_lineage(lease);
        assert_eq!(
            lineage_activation,
            crate::DockViewportRuntimeLineageActivationOutcome::Activated,
            "Dock primary activation must arm the matching surface runtime exactly once"
        );
        assert!(
            runtime.promote_primary_anchor_open_attempt(opening_attempt, lease),
            "Dock primary opening handle must promote into the exact active session lease"
        );
        let promoted = cx.update_entity(&host, |host, host_cx| {
            host.promote_primary_anchor(opening, lease, anchor, host_cx)
        });
        assert!(
            promoted,
            "validated Dock primary host rejected its exact active session lease"
        );
        let _ = window.update(cx, |_, window, _| window.refresh());

        DockSurfacePrimaryWindowOpenOutcome::Opened(DockSurfacePrimaryWindowOpened::new(
            window.into(),
            lease.generation(),
        ))
    }

    /// Returns default window options for a centered primary dock host.
    pub fn primary_window_options(bounds: Bounds<Pixels>) -> WindowOptions {
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        }
    }
}
