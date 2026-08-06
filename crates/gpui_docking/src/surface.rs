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
    native_captured_drag::DockNativeCapturedSurfaceReleaseOutcome,
    viewport_registry::DockViewportRegistrationKey,
};
pub(crate) use activation::{
    DockSurfaceActivationBinding, DockSurfaceActivationHostRegistration,
    DockSurfaceActivationHostRegistrationStatus, DockSurfaceActivationSettlements,
    DockSurfaceActivationState,
};
#[cfg(test)]
pub(crate) use activation::{DockSurfaceActivationDispatch, DockSurfaceActivationHostLookup};
use open_gpui::{
    AnyView, App, AppContext, Bounds, Context, Entity, Global, Pixels, Subscription, WeakEntity,
    Window, WindowBounds, WindowId, WindowInitialPresentationStatus, WindowOpenFailureStage,
    WindowOptions,
};
pub(crate) use owner::{
    DockSurfaceOwner, DockSurfaceTransactionId, with_detached_root_transaction,
    with_root_transaction,
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
    effects: Option<DockSurfaceShutdownCloseEffects>,
    app_shutdown_round: Option<DockSurfaceAppShutdownRound>,
    capture_terminal: DockSurfaceShutdownCaptureTerminal,
    payload_finalizers: Vec<live_undock_runtime::DockPayloadDragSurfaceShutdownFinalizer>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockSurfaceShutdownCaptureTerminal {
    Awaiting,
    Released,
    Failed,
}

impl From<DockNativeCapturedSurfaceReleaseOutcome> for DockSurfaceShutdownCaptureTerminal {
    fn from(outcome: DockNativeCapturedSurfaceReleaseOutcome) -> Self {
        match outcome {
            DockNativeCapturedSurfaceReleaseOutcome::Released => Self::Released,
            DockNativeCapturedSurfaceReleaseOutcome::Failed => Self::Failed,
        }
    }
}

#[derive(Default)]
struct DockSurfaceShutdownCoordinatorState {
    pending: HashMap<window_session::DockSurfaceWindowSessionLease, DockSurfacePendingShutdown>,
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
            effects: Some(effects),
            app_shutdown_round,
            capture_terminal: DockSurfaceShutdownCaptureTerminal::Awaiting,
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
        terminal: DockSurfaceShutdownCaptureTerminal,
    ) -> Option<DockSurfacePendingShutdown> {
        debug_assert_ne!(terminal, DockSurfaceShutdownCaptureTerminal::Awaiting);
        let mut state = self.state.borrow_mut();
        let pending = state.pending.get_mut(&lease)?;
        assert_eq!(
            pending.capture_terminal,
            DockSurfaceShutdownCaptureTerminal::Awaiting,
            "one surface shutdown capture barrier must settle exactly once"
        );
        pending.capture_terminal = terminal;
        state.pending.remove(&lease)
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

pub(crate) fn finish_live_undock_open_return(
    owner: &Entity<DockSurfaceOwner>,
    opening: live_undock::DockLiveUndockOpeningKey,
    window: open_gpui::AnyWindowHandle,
    runtime_registered: bool,
    cx: &mut App,
) -> live_undock::DockLiveUndockOpenReturnOutcome {
    let (outcome, effects) = cx.update_entity(owner, |owner, owner_cx| {
        let (outcome, effects) = owner
            .complete_live_undock_opening(opening, window, runtime_registered)
            .into_parts();
        if !matches!(outcome, live_undock::DockLiveUndockOpenReturnOutcome::Stale) {
            owner_cx.notify();
        }
        (outcome, effects)
    });
    let live_runtime = cx.read_entity(owner, |owner, _| owner.live_undock_runtime());
    live_runtime.enqueue_effects(effects, cx);
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
    cx: &mut App,
) {
    let Some(window) = window else {
        return;
    };
    let opening = identity.opening();
    let lease = opening.lease();
    let Some(dependency) = dependency else {
        close_live_undock_window_quietly(window, cx);
        return;
    };
    if binding != Some(live_undock::DockLiveUndockOpeningBinding::ExactGated) {
        close_live_undock_window_quietly(window, cx);
        settle_live_undock_dependency(owner, identity, Some(dependency), cx);
        return;
    }

    let runtime = cx.read_entity(owner, |owner, _| owner.runtime());
    let adopt = cx.update_entity(owner, |owner, owner_cx| {
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
    let terminal_ticket_installed = matches!(
        adopt,
        window_session::DockSurfaceWindowSessionAdoptWindowOutcome::Added
            | window_session::DockSurfaceWindowSessionAdoptWindowOutcome::AlreadyTracked
    );
    let frozen_ownership_installed = terminal_ticket_installed
        && runtime.adopt_provisional_window_during_shutdown(window, opening);
    let dependency_transferred = frozen_ownership_installed
        && cx.update_entity(owner, |owner, owner_cx| {
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
        });
    if dependency_transferred {
        close_surface_window(owner, lease, window, cx);
    } else {
        close_live_undock_window_quietly(window, cx);
        settle_live_undock_dependency(owner, identity, Some(dependency), cx);
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
    close_surface_anchor_after_dependents(owner, &runtime, lease, cx);
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
    close_surface_anchor_after_dependents(owner, &runtime, lease, cx);
}

fn settle_surface_window_terminal(
    owner: &Entity<DockSurfaceOwner>,
    runtime: &DockViewportRuntimeHandle,
    lease: window_session::DockSurfaceWindowSessionLease,
    window_id: WindowId,
    disposition: window_session::DockSurfaceWindowSessionTerminalDisposition,
    cx: &mut App,
) {
    let runtime_settled = runtime.settle_surface_window_terminal(lease, window_id, cx);
    let runtime_empty = runtime.surface_generation_empty(lease);
    let live_effects = cx.update_entity(owner, |owner, owner_cx| {
        let (live_terminal, live_effects) = owner
            .settle_live_undock_window_terminal(window_id)
            .into_parts();
        debug_assert!(live_terminal.is_none_or(|terminal| terminal.lease() == lease));
        let terminal = owner
            .window_session_mut()
            .settle_terminal(lease, window_id, disposition);
        let shutting_down = owner.window_session().is_shutting_down(lease);
        let runtime = (shutting_down && runtime_empty)
            .then(|| owner.window_session_mut().mark_runtime_empty(lease));
        let convergence =
            shutting_down.then(|| owner.window_session_mut().complete_shutdown(lease));
        if runtime_settled
            || live_terminal.is_some()
            || matches!(
                terminal,
                window_session::DockSurfaceWindowSessionTerminalOutcome::Settled
            )
            || matches!(
                runtime,
                Some(window_session::DockSurfaceWindowSessionRuntimeEmptyOutcome::Marked)
            )
            || matches!(
                convergence,
                Some(window_session::DockSurfaceWindowSessionShutdownConvergenceOutcome::Closed)
            )
        {
            owner_cx.notify();
        }
        live_effects
    });
    let live_runtime = cx.read_entity(owner, |owner, _| owner.live_undock_runtime());
    live_runtime.enqueue_effects(live_effects, cx);
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

    let mut entered_window_update = false;
    let dispatch = catch_unwind(AssertUnwindSafe(|| {
        window.update(cx, |_, window, cx| {
            entered_window_update = true;
            window.remove_window(cx);
        })
    }));

    if entered_window_update {
        mark_surface_window_close_dispatched(owner, lease, window_id, cx);
    } else if retry_surface_window_close_dispatch(owner, lease, window_id, cx)
        && cx.windows().contains(&window)
    {
        defer_surface_window_close_retry(owner, lease, window, cx);
    }

    if let Err(payload) = dispatch {
        resume_unwind(payload);
    }
}

fn close_surface_anchor_after_dependents(
    owner: &Entity<DockSurfaceOwner>,
    runtime: &DockViewportRuntimeHandle,
    lease: window_session::DockSurfaceWindowSessionLease,
    cx: &mut App,
) {
    let (pending, has_pending_dependencies) = cx.read_entity(owner, |owner, _| {
        (
            owner.window_session().pending_terminal_window_ids(lease),
            owner.window_session().has_pending_dependencies(lease),
        )
    });
    if has_pending_dependencies {
        return;
    }
    let Some(pending) = pending else {
        return;
    };
    if pending.iter().any(|window_id| *window_id != lease.anchor()) {
        return;
    }
    if !pending.contains(&lease.anchor()) {
        return;
    }

    let anchor = runtime
        .windows_for_surface(lease)
        .into_iter()
        .find_map(|(role, window)| {
            (role == DockViewportWindowRole::PrimaryAnchor).then_some(window)
        });
    if let Some(anchor) = anchor {
        if !cx.windows().contains(&anchor) {
            return;
        }
        close_surface_window(owner, lease, anchor, cx);
    }
}

struct DockSurfaceShutdownCloseEffects {
    runtime: DockViewportRuntimeHandle,
    lease: window_session::DockSurfaceWindowSessionLease,
    reservation: DockViewportSurfaceShutdownReservation,
    fallback_windows: Vec<(DockViewportWindowRole, open_gpui::AnyWindowHandle)>,
    activation_settlements: DockSurfaceActivationSettlements,
    first_panic: Option<DockSurfaceShutdownPanic>,
}

fn prepare_surface_shutdown(
    owner: &Entity<DockSurfaceOwner>,
    lease: window_session::DockSurfaceWindowSessionLease,
    reason: DockSurfaceWindowSessionShutdownReason,
    cx: &mut App,
) -> Option<DockSurfaceShutdownCloseEffects> {
    let runtime = cx.read_entity(owner, |owner, _| owner.runtime());
    let mut snapshot = runtime.windows_for_surface(lease);
    let (begin, activation_settlements, live_shutdown, live_effects) =
        cx.update_entity(owner, |owner, owner_cx| {
            let live_shutdown = owner
                .window_session()
                .admits(lease)
                .then(|| owner.live_undock_shutdown_snapshot(lease))
                .flatten();
            let begin = owner.window_session_mut().begin_shutdown_with_dependencies(
                lease,
                reason,
                snapshot.iter().map(|(_, window)| window.window_id()).chain(
                    live_shutdown
                        .and_then(|snapshot| snapshot.window())
                        .map(|window| window.window_id()),
                ),
                live_shutdown.map(|snapshot| snapshot.dependency()),
            );
            let (activation_settlements, live_effects) = if matches!(
                begin,
                window_session::DockSurfaceWindowSessionBeginShutdownOutcome::Started { .. }
            ) {
                let (frozen, effects) = owner.freeze_live_undock_for_shutdown(lease).into_parts();
                assert_eq!(
                    frozen, live_shutdown,
                    "live-undock shutdown snapshot must remain exact inside one owner update"
                );
                owner_cx.notify();
                (owner.activation_mut().freeze_lease(lease), Some(effects))
            } else {
                (DockSurfaceActivationSettlements::default(), None)
            };
            (begin, activation_settlements, live_shutdown, live_effects)
        });
    if let Some(effects) = live_effects {
        let live_runtime = cx.read_entity(owner, |owner, _| owner.live_undock_runtime());
        live_runtime.enqueue_effects(effects, cx);
    }

    if let Some(window) = live_shutdown.and_then(|snapshot| snapshot.window())
        && !snapshot
            .iter()
            .any(|(_, current)| current.window_id() == window.window_id())
    {
        snapshot.push((
            DockViewportWindowRole::ProvisionalViewport(
                live_shutdown
                    .expect("a live-undock window must belong to one shutdown snapshot")
                    .opening(),
            ),
            window,
        ));
    }

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
    Some(DockSurfaceShutdownCloseEffects {
        runtime,
        lease,
        reservation,
        fallback_windows: snapshot,
        activation_settlements,
        first_panic: None,
    })
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
        fallback_windows,
        activation_settlements,
        mut first_panic,
    } = effects;
    if !cx.read_entity(owner, |owner, _| {
        owner.window_session().is_shutting_down(lease)
    }) {
        return;
    }

    let mut windows = match catch_unwind(AssertUnwindSafe(|| {
        runtime.commit_surface_shutdown(reservation, cx)
    })) {
        Ok(windows) => windows,
        Err(payload) => {
            retain_first_surface_shutdown_panic(
                &mut first_panic,
                Err(payload),
                "viewport runtime shutdown commit",
            );
            Vec::new()
        }
    };
    windows.extend(fallback_windows);

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
        fallback_windows,
        activation_settlements,
        mut first_panic,
    } = effects;
    if !cx.read_entity(owner, |owner, _| {
        owner.window_session().is_shutting_down(lease)
    }) {
        return first_panic;
    }

    let mut windows = match catch_unwind(AssertUnwindSafe(|| {
        runtime.retire_frozen_surface_after_capture_failure(reservation, cx)
    })) {
        Ok(windows) => windows,
        Err(payload) => {
            retain_first_surface_shutdown_panic(
                &mut first_panic,
                Err(payload),
                "viewport runtime capture-failure retirement",
            );
            Vec::new()
        }
    };
    windows.extend(fallback_windows);

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
            close_surface_anchor_after_dependents(owner, runtime, lease, cx);
        })),
        "primary anchor close dispatch",
    );
}

fn finish_scheduled_surface_shutdown(
    owner: &Entity<DockSurfaceOwner>,
    lease: window_session::DockSurfaceWindowSessionLease,
    coordinator: &DockSurfaceShutdownCoordinator,
    release_outcome: DockNativeCapturedSurfaceReleaseOutcome,
    first_panic: &mut Option<DockSurfaceShutdownPanic>,
    cx: &mut App,
) {
    let capture_terminal = DockSurfaceShutdownCaptureTerminal::from(release_outcome);
    let Some(mut pending) = coordinator.take_after_capture_terminal(lease, capture_terminal) else {
        return;
    };
    match capture_terminal {
        DockSurfaceShutdownCaptureTerminal::Released => {
            if let Some(effects) = pending.effects.take() {
                retain_first_surface_shutdown_panic(
                    first_panic,
                    catch_unwind(AssertUnwindSafe(|| {
                        apply_surface_shutdown_close_effects(owner, effects, cx);
                    })),
                    "capture-terminal surface close effects",
                );
            }
        }
        DockSurfaceShutdownCaptureTerminal::Failed => {
            let mut prior_panic = first_panic.take();
            if let Some(effects) = pending.effects.take() {
                match catch_unwind(AssertUnwindSafe(|| {
                    apply_surface_capture_failure_retirement(owner, effects, cx)
                })) {
                    Ok(Some(payload)) | Err(payload) => retain_first_surface_shutdown_panic(
                        &mut prior_panic,
                        Err(payload),
                        "capture-failure surface retirement",
                    ),
                    Ok(None) => {}
                }
            }
            *first_panic = Some(Box::new(DockSurfaceCaptureReleaseFailure {
                lease,
                prior_panic,
            }));
        }
        DockSurfaceShutdownCaptureTerminal::Awaiting => {
            unreachable!("an awaiting capture barrier cannot finish surface shutdown")
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
        move |release_outcome, first_panic, cx| {
            finish_scheduled_surface_shutdown(
                &owner,
                lease,
                &completion_coordinator,
                release_outcome,
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
) -> Option<DockViewportRegistrationKey> {
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
        if let Some(effects) = prepare_surface_shutdown(
            owner,
            lease,
            DockSurfaceWindowSessionShutdownReason::AnchorDestroyed,
            cx,
        ) {
            schedule_surface_shutdown_close_effects(owner, effects, None, cx);
        }
        return None;
    }

    let committed_registration = cx.read_entity(owner, |owner, _| {
        owner.live_undock_committed_destination_registration_for_logical_close(window_id)
    });
    if committed_registration.is_some() {
        return committed_registration;
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
    settle_surface_window_terminal(
        owner,
        &runtime,
        lease,
        window_id,
        window_session::DockSurfaceWindowSessionTerminalDisposition::ObservedClosed,
        cx,
    );
    let shutting_down = cx.read_entity(owner, |owner, _| {
        owner.window_session().is_shutting_down(lease)
    });
    let anchor_is_logically_registered = cx
        .windows()
        .iter()
        .any(|window| window.window_id() == lease.anchor());
    if shutting_down && window_id != lease.anchor() && anchor_is_logically_registered {
        let owner = owner.clone();
        cx.defer_shutdown_critical_before_window_registry_clear(move |cx| {
            let runtime = cx.read_entity(&owner, |owner, _| owner.runtime());
            close_surface_anchor_after_dependents(&owner, &runtime, lease, cx);
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
            && let Some(effects) = prepare_surface_shutdown(
                owner,
                active_lease,
                DockSurfaceWindowSessionShutdownReason::AppShutdown,
                cx,
            )
        {
            schedule_surface_shutdown_close_effects(owner, effects, Some(round.clone()), cx);
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
    window.on_window_should_close(cx, move |_, cx| {
        let lease = cx.read_entity(&close_owner, |owner, _| {
            owner.window_session().active_lease_for_anchor(anchor)
        });
        if let Some(lease) = lease {
            if let Some(effects) = prepare_surface_shutdown(
                &close_owner,
                lease,
                DockSurfaceWindowSessionShutdownReason::AnchorCloseRequested,
                cx,
            ) {
                schedule_surface_shutdown_close_effects(&close_owner, effects, None, cx);
            }
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
            if let Some(effects) = prepare_surface_shutdown(
                &presentation_owner,
                lease,
                DockSurfaceWindowSessionShutdownReason::PresentationFailed,
                cx,
            ) {
                schedule_surface_shutdown_close_effects(&presentation_owner, effects, None, cx);
            }
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
