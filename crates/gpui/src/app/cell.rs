use std::{
    cell::{BorrowMutError, Cell, Ref, RefCell, RefMut},
    collections::{HashMap, HashSet, VecDeque},
    ops::{Deref, DerefMut},
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
    time::Duration,
};

use futures::channel::oneshot;

#[cfg(any(test, feature = "test-support"))]
use super::native_callback_diagnostics::{
    NativeBoundaryDiagnosticCursor, NativeBoundaryDiagnosticsSnapshot,
};
use super::{
    App,
    native_callback_diagnostics::{
        NativeBoundaryDiagnostic, NativeBoundaryDisposition, NativeBoundaryGeneration,
        NativeBoundaryKind, NativeBoundaryTarget, NativeCallbackKind, NativeInputBoundary,
        NativeInputHandlerOperation, NativeInvariantFailure,
    },
    native_captured_drag::{
        ConsumerKey, NativeCapturedDragAuthoritySnapshot, NativeCapturedDragConsumer,
        NativeCapturedDragEvent, NativeCapturedDragGeneration, NativeCapturedDragOutbox,
        NativeCapturedDragReleaseBarrier, NativeCapturedDragReleaseCompletion,
        NativeCapturedDragReleaseContinuation, NativeCapturedDragReleaseTerminal,
    },
    native_event_ingress::{
        NativeAppEvent, NativeEventDisposition, NativeEventDrainControl, NativeEventIngress,
        NativeEventPrefixPop, NativeWindowEvent, NativeWorkEnvelope, NativeWorkPop,
        ReservedPointerCancel,
    },
    native_platform_commands::{
        NativePlatformCommandRejection, NativePointerCaptureRelease,
        NativePointerCaptureReleaseToken, NativeShutdownCompletion, NativeWindowRetirement,
        NativeWindowRetirementAttempt,
    },
    native_query_snapshot::{NativeQuerySnapshots, NativeWindowLifecycle},
};
use crate::{
    Action, BackgroundExecutor, DispatchEventResult, NativeInputInvariantViolation, PlatformInput,
    PlatformPointerCaptureReleaseOutcome, PlatformPresentationShutdownOutcome, PlatformWindow,
    PlatformWindowCommand, PlatformWindowCommandDispatcher, PlatformWindowCommandOutcome,
    PointerCancelReason, PreparedPlatformPresentationShutdown, WindowControlArea, WindowId,
};

type OpenUrlsHandler = dyn FnMut(Vec<String>, &mut App);
type AppHandler = dyn FnMut(&mut App);
type NativeShutdownCriticalCallback = Box<dyn FnOnce(&mut App)>;

#[derive(Default)]
struct PreShutdownCriticalState {
    next_ticket: u64,
    callbacks: VecDeque<(u64, NativeShutdownCriticalCallback)>,
}

impl PreShutdownCriticalState {
    fn protect(&mut self, callback: NativeShutdownCriticalCallback) -> u64 {
        self.next_ticket = self
            .next_ticket
            .checked_add(1)
            .expect("pre-shutdown critical ticket space exhausted");
        let ticket = self.next_ticket;
        self.callbacks.push_back((ticket, callback));
        ticket
    }

    fn take(&mut self, ticket: u64) -> Option<NativeShutdownCriticalCallback> {
        let index = self
            .callbacks
            .iter()
            .position(|(current, _)| *current == ticket)?;
        self.callbacks.remove(index).map(|(_, callback)| callback)
    }

    fn take_all(&mut self) -> VecDeque<NativeShutdownCriticalCallback> {
        std::mem::take(&mut self.callbacks)
            .into_iter()
            .map(|(_, callback)| callback)
            .collect()
    }
}

const POINTER_CAPTURE_RELEASE_RETRY_DELAYS: [Duration; 3] = [
    Duration::from_millis(8),
    Duration::from_millis(32),
    Duration::from_millis(128),
];
const SHUTDOWN_COMPLETION_RETRY_DELAYS: [Duration; 5] = [
    Duration::ZERO,
    Duration::from_millis(8),
    Duration::from_millis(32),
    Duration::from_millis(128),
    Duration::from_millis(512),
];
// Shutdown cleanup is best-effort after a panic. Bound synchronous unwinding before native
// teardown proceeds and the first panic is resumed to the caller.
const SHUTDOWN_EFFECT_FLUSH_PANIC_BUDGET: usize = 8;
// Bound synchronous convergence when terminal effects and critical participants enqueue each
// other. Exhaustion yields to the existing shutdown retry schedule instead of spinning forever.
const SHUTDOWN_TERMINAL_CONVERGENCE_WAVE_BUDGET: usize = 8;

fn retain_shutdown_panic(
    first: &mut Option<Box<dyn std::any::Any + Send>>,
    candidate: Option<Box<dyn std::any::Any + Send>>,
) {
    if first.is_none() {
        *first = candidate;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeShutdownEffectFlushTerminal {
    Drained,
    Failed { panic_count: usize },
}

fn settle_shutdown_effect_flush(
    first_panic: &mut Option<Box<dyn std::any::Any + Send>>,
    mut flush: impl FnMut() -> Result<(), Box<dyn std::any::Any + Send>>,
) -> NativeShutdownEffectFlushTerminal {
    for panic_count in 1..=SHUTDOWN_EFFECT_FLUSH_PANIC_BUDGET {
        match flush() {
            Ok(()) => return NativeShutdownEffectFlushTerminal::Drained,
            Err(payload) => {
                retain_shutdown_panic(first_panic, Some(payload));
                if panic_count == SHUTDOWN_EFFECT_FLUSH_PANIC_BUDGET {
                    return NativeShutdownEffectFlushTerminal::Failed { panic_count };
                }
            }
        }
    }
    unreachable!("shutdown effect-flush panic budget must be non-zero")
}

fn settle_shutdown_effect_wave(
    app: &mut App,
    generation: u64,
    phase: &'static str,
    first_panic: &mut Option<Box<dyn std::any::Any + Send>>,
) -> NativeShutdownEffectFlushTerminal {
    let terminal = settle_shutdown_effect_flush(first_panic, || {
        catch_unwind(AssertUnwindSafe(|| app.flush_effects()))
    });
    if let NativeShutdownEffectFlushTerminal::Failed { panic_count } = terminal {
        log::error!(
            "shutdown generation {generation} abandoned {phase} effect flushing after {panic_count} consecutive panics",
        );
        app.abandon_pending_effects_after_shutdown_failure();
    }
    terminal
}

fn merge_presentation_shutdowns(
    shutdowns: &mut HashMap<WindowId, PreparedPlatformPresentationShutdown>,
    prepared: impl IntoIterator<Item = PreparedPlatformPresentationShutdown>,
) -> bool {
    let mut added = false;
    for prepared in prepared {
        let snapshot = prepared.snapshot();
        let window_id = snapshot.window_id();
        if let Some(existing) = shutdowns.get(&window_id) {
            assert!(
                existing.same_authority(&prepared),
                "one full window id cannot own multiple presentation-shutdown authorities"
            );
            continue;
        }
        shutdowns.insert(window_id, prepared);
        added = true;
    }
    added
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativePointerCaptureReleaseState {
    AwaitingLogicalTerminal,
    Queued,
    RetryPending,
    AwaitingNativeWindowTerminal,
    CompletionQueued(NativeCapturedDragReleaseTerminal),
    Delivering(NativeCapturedDragReleaseTerminal),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativePointerCaptureReleaseRetryTrigger {
    Delayed {
        token: NativePointerCaptureReleaseToken,
        retry_epoch: u8,
    },
    NativeWindowProgress(WindowId),
}

struct NativePointerCaptureReleaseBarrier {
    token: NativePointerCaptureReleaseToken,
    state: NativePointerCaptureReleaseState,
    retry_attempts: u8,
    continuations: Vec<NativeCapturedDragReleaseContinuation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeWindowRetirementState {
    WaitingForDependencies,
    Queued,
    RetryPending,
    AwaitingNativeTerminal,
}

struct NativeWindowRetirementBarrier {
    state: NativeWindowRetirementState,
    presentation_shutdown: PreparedPlatformPresentationShutdown,
    pending_retirement: Option<NativeWindowRetirement>,
}

fn native_retirement_dependency_reaches(
    groups: &HashMap<WindowId, HashSet<WindowId>>,
    start: WindowId,
    target: WindowId,
) -> bool {
    let mut pending = vec![start];
    let mut visited = HashSet::new();
    while let Some(window_id) = pending.pop() {
        if window_id == target {
            return true;
        }
        if !visited.insert(window_id) {
            continue;
        }
        if let Some(dependencies) = groups.get(&window_id) {
            pending.extend(dependencies.iter().copied());
        }
    }
    false
}

struct NativeShutdownFence {
    generation: u64,
    terminate_ingress: bool,
    preparation_complete: bool,
    initial_effect_flush_terminal: Option<NativeShutdownEffectFlushTerminal>,
    presentation_shutdowns: Option<HashMap<WindowId, PreparedPlatformPresentationShutdown>>,
    retry_epoch: u8,
    registry_cleared: bool,
    was_quitting: bool,
    first_panic: Option<Box<dyn std::any::Any + Send>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeShutdownCriticalPhase {
    BeforeWindowRegistryClear,
    AfterWindowRegistryClear,
}

pub(super) enum NativeShutdownCriticalEnqueueError {
    Inactive(NativeShutdownCriticalCallback),
    PhasePassed(NativeShutdownCriticalCallback),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeShutdownCriticalWaveOutcome {
    Idle,
    Drained,
    Retry,
}

#[derive(Default)]
struct NativeShutdownCriticalState {
    generation: Option<u64>,
    registry_cleared: bool,
    before_registry_clear: VecDeque<NativeShutdownCriticalCallback>,
    after_registry_clear: VecDeque<NativeShutdownCriticalCallback>,
}

impl NativeShutdownCriticalState {
    fn begin(
        &mut self,
        generation: u64,
        before_registry_clear: VecDeque<NativeShutdownCriticalCallback>,
    ) {
        assert!(
            self.generation.is_none()
                && self.before_registry_clear.is_empty()
                && self.after_registry_clear.is_empty(),
            "a new shutdown generation cannot replace pending critical participants"
        );
        self.generation = Some(generation);
        self.registry_cleared = false;
        self.before_registry_clear = before_registry_clear;
    }

    fn enqueue(
        &mut self,
        generation: u64,
        phase: NativeShutdownCriticalPhase,
        callback: NativeShutdownCriticalCallback,
    ) -> Result<(), NativeShutdownCriticalEnqueueError> {
        if self.generation != Some(generation) {
            return Err(NativeShutdownCriticalEnqueueError::Inactive(callback));
        }
        match phase {
            NativeShutdownCriticalPhase::BeforeWindowRegistryClear => {
                if self.registry_cleared {
                    return Err(NativeShutdownCriticalEnqueueError::PhasePassed(callback));
                }
                self.before_registry_clear.push_back(callback);
            }
            NativeShutdownCriticalPhase::AfterWindowRegistryClear => {
                self.after_registry_clear.push_back(callback);
            }
        }
        Ok(())
    }

    fn take_wave(
        &mut self,
        generation: u64,
        phase: NativeShutdownCriticalPhase,
    ) -> VecDeque<NativeShutdownCriticalCallback> {
        if self.generation != Some(generation) {
            return VecDeque::new();
        }
        match phase {
            NativeShutdownCriticalPhase::BeforeWindowRegistryClear => {
                std::mem::take(&mut self.before_registry_clear)
            }
            NativeShutdownCriticalPhase::AfterWindowRegistryClear => {
                std::mem::take(&mut self.after_registry_clear)
            }
        }
    }

    fn has_pending(&self, generation: u64, phase: NativeShutdownCriticalPhase) -> bool {
        if self.generation != Some(generation) {
            return false;
        }
        match phase {
            NativeShutdownCriticalPhase::BeforeWindowRegistryClear => {
                !self.before_registry_clear.is_empty()
            }
            NativeShutdownCriticalPhase::AfterWindowRegistryClear => {
                !self.after_registry_clear.is_empty()
            }
        }
    }

    fn mark_registry_cleared(&mut self, generation: u64) {
        assert_eq!(
            self.generation,
            Some(generation),
            "registry-clear acknowledgement must match the active shutdown generation"
        );
        assert!(
            self.before_registry_clear.is_empty(),
            "registry clear cannot pass pending pre-clear critical participants"
        );
        self.registry_cleared = true;
    }

    fn finish(&mut self, generation: u64) {
        assert_eq!(
            self.generation,
            Some(generation),
            "shutdown completion must match its critical participant generation"
        );
        assert!(
            self.before_registry_clear.is_empty() && self.after_registry_clear.is_empty(),
            "shutdown cannot finish with pending critical participants"
        );
        self.generation = None;
        self.registry_cleared = false;
    }
}

struct NativeShutdownEffectFlushOwnership<'a> {
    active_generation: &'a Cell<Option<u64>>,
}

impl<'a> NativeShutdownEffectFlushOwnership<'a> {
    fn begin(active_generation: &'a Cell<Option<u64>>, generation: u64) -> Self {
        debug_assert!(active_generation.replace(Some(generation)).is_none());
        Self { active_generation }
    }
}

impl Drop for NativeShutdownEffectFlushOwnership<'_> {
    fn drop(&mut self) {
        self.active_generation.set(None);
    }
}

enum NativeShutdownCompletionAction {
    Retry,
    Complete {
        terminate_ingress: bool,
        panic: Option<Box<dyn std::any::Any + Send>>,
    },
}

struct NativeBoundaryTerminalGuard<'a> {
    ingress: &'a NativeEventIngress,
    pending: Option<NativeBoundaryDiagnostic>,
}

impl<'a> NativeBoundaryTerminalGuard<'a> {
    fn new(ingress: &'a NativeEventIngress, pending: NativeBoundaryDiagnostic) -> Self {
        Self {
            ingress,
            pending: Some(pending),
        }
    }

    fn immediate_input(
        ingress: &'a NativeEventIngress,
        sequence: u64,
        window_id: WindowId,
        kind: NativeCallbackKind,
    ) -> Self {
        Self::new(
            ingress,
            NativeBoundaryDiagnostic::pending(
                sequence,
                NativeBoundaryTarget::Window(window_id),
                NativeBoundaryKind::Callback(kind),
                None,
            ),
        )
    }

    fn set_generation(&mut self, generation: NativeBoundaryGeneration) {
        self.pending
            .as_mut()
            .expect("pending native boundary terminal must remain unsettled")
            .domain_generation = Some(generation);
    }

    fn settle(&mut self, disposition: NativeBoundaryDisposition) {
        let pending = self
            .pending
            .take()
            .expect("native boundary terminal must settle exactly once");
        self.ingress.record_terminal(pending, disposition);
    }

    fn settle_event(&mut self, disposition: NativeEventDisposition) {
        let disposition = match disposition {
            NativeEventDisposition::Delivered => NativeBoundaryDisposition::DELIVERED,
            NativeEventDisposition::StaleWindow => NativeBoundaryDisposition::Stale,
        };
        self.settle(disposition);
    }

    fn reject_input(
        &mut self,
        window_id: WindowId,
        boundary: NativeInputBoundary,
        slot_generation: Option<u64>,
        failure: NativeInvariantFailure,
    ) -> NativeInputInvariantViolation {
        self.settle(NativeBoundaryDisposition::InvariantFailure(failure));
        NativeInputInvariantViolation::new(window_id, boundary, slot_generation, failure)
    }

    fn run_callback<R>(&mut self, callback: impl FnOnce() -> R) -> R {
        self.run_callback_with_panic_cleanup(callback, || {})
    }

    fn run_callback_with_panic_cleanup<R>(
        &mut self,
        callback: impl FnOnce() -> R,
        cleanup: impl FnOnce(),
    ) -> R {
        match catch_unwind(AssertUnwindSafe(callback)) {
            Ok(result) => result,
            Err(payload) => {
                cleanup();
                self.settle(NativeBoundaryDisposition::InvariantFailure(
                    NativeInvariantFailure::CallbackPanicked,
                ));
                resume_unwind(payload)
            }
        }
    }
}

impl Drop for NativeBoundaryTerminalGuard<'_> {
    fn drop(&mut self) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        debug_assert!(
            std::thread::panicking(),
            "native boundary terminal guard was abandoned without a disposition"
        );
        self.ingress.record_terminal(
            pending,
            NativeBoundaryDisposition::InvariantFailure(NativeInvariantFailure::CallbackPanicked),
        );
    }
}

/// Temporary wrapper around [`RefCell<App>`] to help debug double borrows.
/// Strongly consider removing after stabilization.
#[doc(hidden)]
pub struct AppCell {
    app: RefCell<App>,
    app_borrow_waiters: RefCell<Vec<oneshot::Sender<()>>>,
    native_events: NativeEventIngress,
    native_captured_drags: NativeCapturedDragOutbox,
    native_queries: NativeQuerySnapshots,
    background_executor: BackgroundExecutor,
    next_pointer_capture_release_generation: Cell<u64>,
    pointer_capture_releases: RefCell<HashMap<u64, NativePointerCaptureReleaseBarrier>>,
    pointer_capture_release_retries: RefCell<VecDeque<NativePointerCaptureRelease>>,
    native_window_retirements: RefCell<HashMap<WindowId, NativeWindowRetirementBarrier>>,
    native_window_retirement_dependencies: RefCell<HashMap<WindowId, HashSet<WindowId>>>,
    observed_native_window_terminals: RefCell<HashSet<WindowId>>,
    next_shutdown_generation: Cell<u64>,
    shutdown_fence: RefCell<Option<NativeShutdownFence>>,
    shutdown_critical: RefCell<NativeShutdownCriticalState>,
    pre_shutdown_critical: RefCell<PreShutdownCriticalState>,
    active_shutdown_completion_generation: Cell<Option<u64>>,
    shutdown_terminate_ingress_requested: Cell<bool>,
    shutdown_completion_queued: Cell<Option<u64>>,
    open_urls_handler: NativeAppHandlerSlot<OpenUrlsHandler>,
    reopen_handler: NativeAppHandlerSlot<AppHandler>,
    system_wake_handler: NativeAppHandlerSlot<AppHandler>,
    platform_input_leases: RefCell<Vec<u64>>,
    input_handler_leases: RefCell<Vec<u64>>,
}

impl AppCell {
    pub(super) fn new(app: App) -> Self {
        let foreground_executor = app.foreground_executor.clone();
        let background_executor = app.background_executor.clone();
        let this = app.this.clone();
        Self {
            app: RefCell::new(app),
            app_borrow_waiters: RefCell::new(Vec::new()),
            native_events: NativeEventIngress::new(foreground_executor, this),
            native_captured_drags: NativeCapturedDragOutbox::default(),
            native_queries: NativeQuerySnapshots::default(),
            background_executor,
            next_pointer_capture_release_generation: Cell::new(0),
            pointer_capture_releases: RefCell::new(HashMap::new()),
            pointer_capture_release_retries: RefCell::new(VecDeque::new()),
            native_window_retirements: RefCell::new(HashMap::new()),
            native_window_retirement_dependencies: RefCell::new(HashMap::new()),
            observed_native_window_terminals: RefCell::new(HashSet::new()),
            next_shutdown_generation: Cell::new(0),
            shutdown_fence: RefCell::new(None),
            shutdown_critical: RefCell::new(NativeShutdownCriticalState::default()),
            pre_shutdown_critical: RefCell::new(PreShutdownCriticalState::default()),
            active_shutdown_completion_generation: Cell::new(None),
            shutdown_terminate_ingress_requested: Cell::new(false),
            shutdown_completion_queued: Cell::new(None),
            open_urls_handler: NativeAppHandlerSlot::default(),
            reopen_handler: NativeAppHandlerSlot::default(),
            system_wake_handler: NativeAppHandlerSlot::default(),
            platform_input_leases: RefCell::new(Vec::new()),
            input_handler_leases: RefCell::new(Vec::new()),
        }
    }

    pub(super) fn begin_platform_input_lease(
        self: &Rc<Self>,
        generation: u64,
    ) -> NativeCallbackLease {
        self.begin_native_callback_lease(NativeCallbackLeaseKind::PlatformInput, generation)
    }

    pub(super) fn begin_input_handler_lease(
        self: &Rc<Self>,
        generation: u64,
    ) -> NativeCallbackLease {
        self.begin_native_callback_lease(NativeCallbackLeaseKind::InputHandler, generation)
    }

    fn begin_native_callback_lease(
        self: &Rc<Self>,
        kind: NativeCallbackLeaseKind,
        generation: u64,
    ) -> NativeCallbackLease {
        kind.stack(self).borrow_mut().push(generation);
        NativeCallbackLease {
            app: Some(self.clone()),
            kind,
            generation,
        }
    }

    fn native_callback_lease_active(&self) -> bool {
        !self.platform_input_leases.borrow().is_empty()
            || !self.input_handler_leases.borrow().is_empty()
    }

    pub(super) fn set_open_urls_handler(&self, handler: Box<OpenUrlsHandler>) {
        self.open_urls_handler.set(handler);
    }

    pub(super) fn set_reopen_handler(&self, handler: Box<AppHandler>) {
        self.reopen_handler.set(handler);
    }

    pub(super) fn set_system_wake_handler(&self, handler: Box<AppHandler>) {
        self.system_wake_handler.set(handler);
    }

    pub(super) fn dispatch_open_urls(&self, urls: Vec<String>, app: &mut App) {
        if let Some(mut handler) = self.open_urls_handler.checkout() {
            (handler.callback_mut())(urls, app);
        }
    }

    pub(super) fn dispatch_reopen(&self, app: &mut App) {
        if let Some(mut handler) = self.reopen_handler.checkout() {
            (handler.callback_mut())(app);
        }
    }

    pub(super) fn dispatch_system_wake(&self, app: &mut App) {
        if let Some(mut handler) = self.system_wake_handler.checkout() {
            (handler.callback_mut())(app);
        }
    }

    pub(crate) fn validate_app_menu_action(&self, action: &dyn Action) -> bool {
        let action_type = action.as_any().type_id();
        if self.native_events.is_terminated() {
            return false;
        }
        if self.native_callback_lease_active() || !self.native_events.can_deliver_inline() {
            return self.native_queries.app_menu_action_available(action_type);
        }
        let Ok(mut app) = self.try_borrow_mut() else {
            return self.native_queries.app_menu_action_available(action_type);
        };
        if app.quitting {
            return false;
        }
        let available = app.update(|app| app.is_action_available(action));
        self.native_queries
            .commit_app_menu_action_availability(action_type, available);
        drop(app);
        available
    }

    pub(super) fn reserve_native_window(&self, window_id: WindowId) {
        self.native_events.reopen_window(window_id);
        self.native_queries.reserve_window(window_id);
        self.observed_native_window_terminals
            .borrow_mut()
            .remove(&window_id);
    }

    pub(super) fn commit_native_window(&self, window_id: WindowId) {
        self.native_queries.commit_window(window_id);
    }

    pub(super) fn remove_native_window(&self, window_id: WindowId) {
        self.native_events.close_window(window_id);
        self.native_queries.remove_window(window_id);
    }

    pub(super) fn clear_native_windows(&self) {
        self.native_queries.clear();
    }

    fn next_native_pointer_capture_release_token(
        &self,
        window_id: WindowId,
        captured_drag_generation: Option<NativeCapturedDragGeneration>,
    ) -> NativePointerCaptureReleaseToken {
        let release_generation = self.next_pointer_capture_release_generation.get();
        self.next_pointer_capture_release_generation.set(
            release_generation
                .checked_add(1)
                .expect("native pointer-capture release generation overflowed"),
        );
        NativePointerCaptureReleaseToken::new(
            window_id,
            captured_drag_generation,
            release_generation,
        )
    }

    fn native_window_terminal_was_observed(&self, window_id: WindowId) -> bool {
        self.observed_native_window_terminals
            .borrow()
            .contains(&window_id)
    }

    fn enqueue_native_captured_drag_release_completion(
        &self,
        barrier: NativeCapturedDragReleaseBarrier,
    ) {
        self.native_events.enqueue_captured_drag_release_completion(
            NativeCapturedDragReleaseCompletion::new(barrier),
        );
    }

    fn complete_native_pointer_capture_release(
        &self,
        token: NativePointerCaptureReleaseToken,
        terminal: NativeCapturedDragReleaseTerminal,
    ) {
        let captured_barrier = {
            let mut barriers = self.pointer_capture_releases.borrow_mut();
            let Some(barrier) = barriers.get_mut(&token.release_generation()) else {
                return;
            };
            if barrier.token != token {
                return;
            }
            let Some(captured_barrier) =
                NativeCapturedDragReleaseBarrier::from_release_token(token)
            else {
                barriers.remove(&token.release_generation());
                self.pointer_capture_release_retries
                    .borrow_mut()
                    .retain(|release| release.token() != token);
                self.request_active_shutdown_completion();
                return;
            };
            match &mut barrier.state {
                NativePointerCaptureReleaseState::CompletionQueued(existing_terminal) => {
                    if terminal == NativeCapturedDragReleaseTerminal::NativeWindowTerminal {
                        *existing_terminal = terminal;
                    }
                    return;
                }
                NativePointerCaptureReleaseState::Delivering(_) => return,
                state => *state = NativePointerCaptureReleaseState::CompletionQueued(terminal),
            }
            captured_barrier
        };
        self.pointer_capture_release_retries
            .borrow_mut()
            .retain(|release| release.token() != token);
        self.enqueue_native_captured_drag_release_completion(captured_barrier);
        self.request_active_shutdown_completion();
    }

    fn begin_native_captured_drag_release_completion(
        &self,
        release_barrier: NativeCapturedDragReleaseBarrier,
    ) -> Option<NativeCapturedDragReleaseTerminal> {
        let mut barriers = self.pointer_capture_releases.borrow_mut();
        let barrier = barriers.get_mut(&release_barrier.release_generation())?;
        if barrier.token.window_id() != release_barrier.source_window()
            || barrier.token.captured_drag_generation() != Some(release_barrier.drag_generation())
        {
            return None;
        }
        let NativePointerCaptureReleaseState::CompletionQueued(terminal) = barrier.state else {
            return None;
        };
        barrier.state = NativePointerCaptureReleaseState::Delivering(terminal);
        Some(terminal)
    }

    fn take_native_captured_drag_release_continuations(
        &self,
        release_barrier: NativeCapturedDragReleaseBarrier,
    ) -> Option<Vec<NativeCapturedDragReleaseContinuation>> {
        let mut barriers = self.pointer_capture_releases.borrow_mut();
        let barrier = barriers.get_mut(&release_barrier.release_generation())?;
        if barrier.token.window_id() != release_barrier.source_window()
            || barrier.token.captured_drag_generation() != Some(release_barrier.drag_generation())
            || !matches!(
                barrier.state,
                NativePointerCaptureReleaseState::Delivering(_)
            )
        {
            return None;
        }
        Some(std::mem::take(&mut barrier.continuations))
    }

    fn finish_native_captured_drag_release_completion(
        &self,
        release_barrier: NativeCapturedDragReleaseBarrier,
    ) -> bool {
        let mut barriers = self.pointer_capture_releases.borrow_mut();
        let removable = barriers
            .get(&release_barrier.release_generation())
            .is_some_and(|barrier| {
                barrier.token.window_id() == release_barrier.source_window()
                    && barrier.token.captured_drag_generation()
                        == Some(release_barrier.drag_generation())
                    && matches!(
                        barrier.state,
                        NativePointerCaptureReleaseState::Delivering(_)
                    )
                    && barrier.continuations.is_empty()
            });
        if removable {
            barriers.remove(&release_barrier.release_generation());
        }
        removable
    }

    fn complete_native_pointer_capture_releases_for_native_window_terminal(
        &self,
        window_id: WindowId,
    ) {
        let tokens = self
            .pointer_capture_releases
            .borrow()
            .values()
            .filter_map(|barrier| (barrier.token.window_id() == window_id).then_some(barrier.token))
            .collect::<Vec<_>>();
        for token in tokens {
            self.complete_native_pointer_capture_release(
                token,
                NativeCapturedDragReleaseTerminal::NativeWindowTerminal,
            );
        }
        self.request_active_shutdown_completion();
    }

    pub(super) fn reserve_native_pointer_capture_release(
        &self,
        window_id: WindowId,
        captured_drag_generation: Option<NativeCapturedDragGeneration>,
    ) -> NativePointerCaptureReleaseToken {
        let token =
            self.next_native_pointer_capture_release_token(window_id, captured_drag_generation);
        if !self.native_window_terminal_was_observed(window_id) {
            let previous = self.pointer_capture_releases.borrow_mut().insert(
                token.release_generation(),
                NativePointerCaptureReleaseBarrier {
                    token,
                    state: NativePointerCaptureReleaseState::AwaitingLogicalTerminal,
                    retry_attempts: 0,
                    continuations: Vec::new(),
                },
            );
            debug_assert!(previous.is_none());
        }
        token
    }

    pub(super) fn reserve_native_captured_drag_release(
        &self,
        window_id: WindowId,
        generation: NativeCapturedDragGeneration,
        continuation: NativeCapturedDragReleaseContinuation,
    ) -> (
        NativePointerCaptureReleaseToken,
        NativeCapturedDragReleaseBarrier,
    ) {
        let token = self.next_native_pointer_capture_release_token(window_id, Some(generation));
        let barrier = NativeCapturedDragReleaseBarrier::from_release_token(token)
            .expect("captured-drag release tokens must include their drag generation");
        if self.native_window_terminal_was_observed(window_id) {
            let previous = self.pointer_capture_releases.borrow_mut().insert(
                token.release_generation(),
                NativePointerCaptureReleaseBarrier {
                    token,
                    state: NativePointerCaptureReleaseState::CompletionQueued(
                        NativeCapturedDragReleaseTerminal::NativeWindowTerminal,
                    ),
                    retry_attempts: 0,
                    continuations: vec![continuation],
                },
            );
            debug_assert!(previous.is_none());
            self.enqueue_native_captured_drag_release_completion(barrier);
            return (token, barrier);
        }

        let previous = self.pointer_capture_releases.borrow_mut().insert(
            token.release_generation(),
            NativePointerCaptureReleaseBarrier {
                token,
                state: NativePointerCaptureReleaseState::AwaitingLogicalTerminal,
                retry_attempts: 0,
                continuations: vec![continuation],
            },
        );
        debug_assert!(previous.is_none());
        (token, barrier)
    }

    pub(super) fn attach_native_captured_drag_release_continuation(
        &self,
        window_id: WindowId,
        generation: NativeCapturedDragGeneration,
        continuation: &mut Option<NativeCapturedDragReleaseContinuation>,
    ) -> Option<NativeCapturedDragReleaseBarrier> {
        let mut barriers = self.pointer_capture_releases.borrow_mut();
        let barrier = barriers.values_mut().find(|barrier| {
            barrier.token.window_id() == window_id
                && barrier.token.captured_drag_generation() == Some(generation)
        })?;
        let release_barrier = NativeCapturedDragReleaseBarrier::from_release_token(barrier.token)
            .expect("captured-drag release tokens must include their drag generation");
        barrier.continuations.push(
            continuation
                .take()
                .expect("captured-drag release continuation must remain available"),
        );
        Some(release_barrier)
    }

    pub(super) fn settle_native_pointer_capture_release(
        &self,
        token: NativePointerCaptureReleaseToken,
        dispatcher: PlatformWindowCommandDispatcher,
        required: bool,
    ) {
        let terminal = if !required {
            Some(NativeCapturedDragReleaseTerminal::NotRequired)
        } else if self.native_window_terminal_was_observed(token.window_id()) {
            Some(NativeCapturedDragReleaseTerminal::NativeWindowTerminal)
        } else {
            None
        };
        if let Some(terminal) = terminal {
            self.complete_native_pointer_capture_release(token, terminal);
            self.app_borrow_released();
            return;
        }

        let mut barriers = self.pointer_capture_releases.borrow_mut();
        let Some(barrier) = barriers.get_mut(&token.release_generation()) else {
            return;
        };
        if barrier.token != token
            || barrier.state != NativePointerCaptureReleaseState::AwaitingLogicalTerminal
        {
            return;
        }
        barrier.state = NativePointerCaptureReleaseState::Queued;
        drop(barriers);
        self.native_events
            .enqueue_pointer_capture_release(NativePointerCaptureRelease::new(token, dispatcher));
    }

    fn defer_native_pointer_capture_release_retry(&self, release: NativePointerCaptureRelease) {
        let token = release.token();
        let mut barriers = self.pointer_capture_releases.borrow_mut();
        let Some(barrier) = barriers.get_mut(&token.release_generation()) else {
            return;
        };
        if barrier.token != token || barrier.state != NativePointerCaptureReleaseState::Queued {
            return;
        }
        let retry_delay = POINTER_CAPTURE_RELEASE_RETRY_DELAYS
            .get(usize::from(barrier.retry_attempts))
            .copied();
        barrier.retry_attempts = barrier.retry_attempts.saturating_add(1);
        let retry_epoch = barrier.retry_attempts;
        if retry_delay.is_some() {
            barrier.state = NativePointerCaptureReleaseState::RetryPending;
        }
        drop(barriers);
        if let Some(retry_delay) = retry_delay {
            self.pointer_capture_release_retries
                .borrow_mut()
                .push_back(release);
            self.schedule_pointer_capture_release_retry(retry_delay, token, retry_epoch);
        } else {
            log::error!(
                "native pointer-capture release window={:?} generation={} failed after {} attempts",
                token.window_id(),
                token.release_generation(),
                usize::from(retry_epoch),
            );
            self.complete_native_pointer_capture_release(
                token,
                NativeCapturedDragReleaseTerminal::Failed,
            );
        }
    }

    fn schedule_pointer_capture_release_retry(
        &self,
        retry_delay: Duration,
        token: NativePointerCaptureReleaseToken,
        retry_epoch: u8,
    ) {
        let timer = self.background_executor.timer(retry_delay);
        self.native_events
            .schedule_pointer_capture_release_retry(timer, token, retry_epoch);
    }

    fn retry_pending_pointer_capture_releases(
        &self,
        trigger: NativePointerCaptureReleaseRetryTrigger,
    ) {
        if self.native_events.is_terminated() {
            self.pointer_capture_release_retries.borrow_mut().clear();
            return;
        }
        let retries = std::mem::take(&mut *self.pointer_capture_release_retries.borrow_mut());
        let mut retained = VecDeque::new();
        for release in retries {
            let token = release.token();
            let trigger_matches_token = match trigger {
                NativePointerCaptureReleaseRetryTrigger::Delayed {
                    token: delayed_token,
                    ..
                } => token == delayed_token,
                NativePointerCaptureReleaseRetryTrigger::NativeWindowProgress(window_id) => {
                    token.window_id() == window_id
                }
            };
            if !trigger_matches_token {
                retained.push_back(release);
                continue;
            }
            let mut barriers = self.pointer_capture_releases.borrow_mut();
            let dispatchable =
                barriers
                    .get_mut(&token.release_generation())
                    .is_some_and(|barrier| {
                        if barrier.token != token
                            || barrier.state != NativePointerCaptureReleaseState::RetryPending
                        {
                            return false;
                        }
                        if let NativePointerCaptureReleaseRetryTrigger::Delayed {
                            retry_epoch, ..
                        } = trigger
                            && barrier.retry_attempts != retry_epoch
                        {
                            return false;
                        }
                        barrier.state = NativePointerCaptureReleaseState::Queued;
                        true
                    });
            let retain = barriers
                .get(&token.release_generation())
                .is_some_and(|barrier| {
                    barrier.token == token
                        && barrier.state == NativePointerCaptureReleaseState::RetryPending
                });
            drop(barriers);
            if dispatchable {
                self.native_events.enqueue_pointer_capture_release(release);
            } else if retain {
                retained.push_back(release);
            }
        }
        self.pointer_capture_release_retries
            .borrow_mut()
            .extend(retained);
    }

    pub(super) fn abandon_native_pointer_capture_release(
        &self,
        token: NativePointerCaptureReleaseToken,
    ) {
        if self.native_window_terminal_was_observed(token.window_id()) {
            self.complete_native_pointer_capture_release(
                token,
                NativeCapturedDragReleaseTerminal::NativeWindowTerminal,
            );
            return;
        }
        let mut barriers = self.pointer_capture_releases.borrow_mut();
        let Some(barrier) = barriers.get_mut(&token.release_generation()) else {
            return;
        };
        if barrier.token == token
            && barrier.state == NativePointerCaptureReleaseState::AwaitingLogicalTerminal
        {
            barrier.state = NativePointerCaptureReleaseState::AwaitingNativeWindowTerminal;
        }
    }

    pub(super) fn enqueue_native_window_retirement(
        &self,
        window_id: WindowId,
        window: Box<crate::Window>,
    ) {
        self.enqueue_window_retirement(window_id, NativeWindowRetirement::new(window_id, window));
    }

    pub(super) fn register_native_window_retirement_dependencies(
        &self,
        anchor: WindowId,
        dependencies: impl IntoIterator<Item = WindowId>,
    ) -> Result<(), crate::NativeWindowRetirementDependencyError> {
        let retirements = self.native_window_retirements.borrow();
        if retirements.contains_key(&anchor) {
            return Err(
                crate::NativeWindowRetirementDependencyError::AnchorAlreadyRetiring { anchor },
            );
        }
        if self.native_queries.lookup(anchor).is_none() {
            return Err(crate::NativeWindowRetirementDependencyError::UnknownAnchor { anchor });
        }
        let observed = self.observed_native_window_terminals.borrow();
        let dependencies = dependencies
            .into_iter()
            .filter(|dependency| !observed.contains(dependency))
            .collect::<HashSet<_>>();
        drop(observed);
        for dependency in dependencies.iter().copied() {
            if self.native_queries.lookup(dependency).is_none()
                && !retirements.contains_key(&dependency)
            {
                return Err(
                    crate::NativeWindowRetirementDependencyError::UnknownDependency {
                        anchor,
                        dependency,
                    },
                );
            }
        }
        drop(retirements);
        if dependencies.is_empty() {
            return Ok(());
        }
        let mut groups = self.native_window_retirement_dependencies.borrow_mut();
        for dependency in dependencies.iter().copied() {
            if dependency == anchor
                || native_retirement_dependency_reaches(&groups, dependency, anchor)
            {
                return Err(crate::NativeWindowRetirementDependencyError::Cycle {
                    anchor,
                    dependency,
                });
            }
        }
        groups.entry(anchor).or_default().extend(dependencies);
        Ok(())
    }

    pub(super) fn cancel_native_window_retirement_dependencies(&self, anchor: WindowId) -> bool {
        let removed = self
            .native_window_retirement_dependencies
            .borrow_mut()
            .remove(&anchor)
            .is_some();
        if !removed {
            return false;
        }

        let retirement = {
            let mut retirements = self.native_window_retirements.borrow_mut();
            let Some(barrier) = retirements.get_mut(&anchor) else {
                return true;
            };
            if barrier.state != NativeWindowRetirementState::WaitingForDependencies {
                return true;
            }
            barrier.state = NativeWindowRetirementState::Queued;
            Some(
                barrier
                    .pending_retirement
                    .take()
                    .expect("a dependency-blocked retirement must retain its platform owner"),
            )
        };
        if let Some(retirement) = retirement {
            self.native_events.enqueue_window_retirement(retirement);
        }
        true
    }

    pub(super) fn protect_pre_shutdown_critical(
        &self,
        callback: NativeShutdownCriticalCallback,
    ) -> u64 {
        self.pre_shutdown_critical.borrow_mut().protect(callback)
    }

    pub(super) fn take_pre_shutdown_critical(
        &self,
        ticket: u64,
    ) -> Option<NativeShutdownCriticalCallback> {
        self.pre_shutdown_critical.borrow_mut().take(ticket)
    }

    pub(crate) fn enqueue_platform_window_retirement(
        &self,
        window_id: WindowId,
        platform_window: Box<dyn PlatformWindow>,
        presentation_shutdown: PreparedPlatformPresentationShutdown,
    ) {
        self.enqueue_window_retirement(
            window_id,
            NativeWindowRetirement::from_platform_window(
                window_id,
                platform_window,
                presentation_shutdown,
            ),
        );
    }

    fn enqueue_window_retirement(&self, window_id: WindowId, retirement: NativeWindowRetirement) {
        if self.native_window_terminal_was_observed(window_id) {
            self.complete_native_pointer_capture_releases_for_native_window_terminal(window_id);
        } else {
            for barrier in self.pointer_capture_releases.borrow_mut().values_mut() {
                if barrier.token.window_id() == window_id
                    && barrier.state == NativePointerCaptureReleaseState::AwaitingLogicalTerminal
                {
                    barrier.state = NativePointerCaptureReleaseState::AwaitingNativeWindowTerminal;
                }
            }
        }
        let waiting_for_dependencies = self
            .native_window_retirement_dependencies
            .borrow()
            .get(&window_id)
            .is_some_and(|dependencies| !dependencies.is_empty());
        let presentation_shutdown = retirement.presentation_shutdown();
        let mut retirement = Some(retirement);
        let (state, pending_retirement) = if waiting_for_dependencies {
            (
                NativeWindowRetirementState::WaitingForDependencies,
                retirement.take(),
            )
        } else {
            (NativeWindowRetirementState::Queued, None)
        };
        let previous = self.native_window_retirements.borrow_mut().insert(
            window_id,
            NativeWindowRetirementBarrier {
                state,
                presentation_shutdown,
                pending_retirement,
            },
        );
        debug_assert!(previous.is_none(), "native window retirement queued twice");
        if !waiting_for_dependencies {
            self.native_events.enqueue_window_retirement(
                retirement.expect("queued retirement must retain its platform owner"),
            );
        }
    }

    fn release_native_window_retirement_dependencies(&self, terminal_window: WindowId) {
        let ready = {
            let mut groups = self.native_window_retirement_dependencies.borrow_mut();
            groups.remove(&terminal_window);
            let ready = groups
                .iter_mut()
                .filter_map(|(anchor, dependencies)| {
                    dependencies.remove(&terminal_window);
                    dependencies.is_empty().then_some(*anchor)
                })
                .collect::<Vec<_>>();
            for anchor in &ready {
                groups.remove(anchor);
            }
            ready
        };
        for anchor in ready {
            let retirement = {
                let mut retirements = self.native_window_retirements.borrow_mut();
                let Some(barrier) = retirements.get_mut(&anchor) else {
                    continue;
                };
                if barrier.state != NativeWindowRetirementState::WaitingForDependencies {
                    continue;
                }
                barrier.state = NativeWindowRetirementState::Queued;
                Some(
                    barrier
                        .pending_retirement
                        .take()
                        .expect("a dependency-blocked retirement must retain its platform owner"),
                )
            };
            if let Some(retirement) = retirement {
                self.native_events.enqueue_window_retirement(retirement);
            }
        }
    }

    fn defer_native_window_retirement_retry(&self, mut retirement: NativeWindowRetirement) {
        let window_id = retirement.window_id();
        let retry_pending = self
            .native_window_retirements
            .borrow_mut()
            .get_mut(&window_id)
            .is_some_and(|barrier| {
                if barrier.state != NativeWindowRetirementState::AwaitingNativeTerminal {
                    return false;
                }
                barrier.state = NativeWindowRetirementState::RetryPending;
                true
            });
        if !retry_pending {
            return;
        }
        let retry_delay = retirement.next_retry_delay();
        let timer = self.background_executor.timer(retry_delay);
        self.native_events
            .schedule_window_retirement_retry(timer, retirement);
    }

    pub(super) fn retry_native_window_retirement(&self, retirement: NativeWindowRetirement) {
        let window_id = retirement.window_id();
        let dispatchable = self
            .native_window_retirements
            .borrow_mut()
            .get_mut(&window_id)
            .is_some_and(|barrier| {
                if barrier.state != NativeWindowRetirementState::RetryPending {
                    return false;
                }
                barrier.state = NativeWindowRetirementState::Queued;
                true
            });
        if dispatchable {
            self.native_events.enqueue_window_retirement(retirement);
        }
    }

    pub(super) fn settle_native_window_terminal(&self, window_id: WindowId) {
        self.observed_native_window_terminals
            .borrow_mut()
            .insert(window_id);
        self.complete_native_pointer_capture_releases_for_native_window_terminal(window_id);
        let terminal = self
            .native_window_retirements
            .borrow()
            .get(&window_id)
            .map(|barrier| {
                let ticket = barrier.presentation_shutdown.ticket();
                let acknowledged = ticket.acknowledge_native_terminal();
                (acknowledged, ticket.snapshot())
            });
        match terminal {
            Some((true, _)) => {
                self.native_window_retirements
                    .borrow_mut()
                    .remove(&window_id);
            }
            Some((false, snapshot)) if snapshot.protocol_violation() => {
                log::error!(
                    "native window terminal preceded presentation quiescence for window {:?}, generation {}",
                    snapshot.window_id(),
                    snapshot.generation(),
                );
                self.native_window_retirements
                    .borrow_mut()
                    .remove(&window_id);
            }
            Some((false, snapshot)) => {
                log::error!(
                    "native window terminal could not settle presentation shutdown for window {:?}, generation {}",
                    snapshot.window_id(),
                    snapshot.generation(),
                );
            }
            None => {
                log::error!(
                    "native window terminal was observed without a registered presentation-shutdown authority for window {:?}",
                    window_id,
                );
            }
        }
        self.release_native_window_retirement_dependencies(window_id);
        self.request_active_shutdown_completion();
    }

    fn pending_native_window_presentation_shutdowns(
        &self,
    ) -> Vec<PreparedPlatformPresentationShutdown> {
        self.native_window_retirements
            .borrow()
            .values()
            .map(|barrier| barrier.presentation_shutdown.clone())
            .collect()
    }

    fn quiesce_shutdown_presentations(&self, fence: &mut NativeShutdownFence) -> bool {
        merge_presentation_shutdowns(
            fence
                .presentation_shutdowns
                .as_mut()
                .expect("shutdown preparation must own presentation authorities"),
            self.pending_native_window_presentation_shutdowns(),
        );

        let mut all_presentations_quiesced = true;
        for shutdown in fence
            .presentation_shutdowns
            .as_ref()
            .expect("shutdown preparation must own presentation authorities")
            .values()
        {
            if shutdown.snapshot().quiesced() {
                continue;
            }
            match catch_unwind(AssertUnwindSafe(|| shutdown.quiesce())) {
                Ok(PlatformPresentationShutdownOutcome::Quiesced)
                    if shutdown.snapshot().quiesced() => {}
                Ok(_) => all_presentations_quiesced = false,
                Err(payload) => {
                    retain_shutdown_panic(&mut fence.first_panic, Some(payload));
                    all_presentations_quiesced = false;
                }
            }
        }
        all_presentations_quiesced
    }

    fn pointer_capture_release_barriers_are_clear(&self) -> bool {
        self.pointer_capture_releases.borrow().is_empty()
    }

    pub(super) fn shutdown_fence_owns_effect_flush(&self) -> bool {
        self.active_shutdown_completion_generation.get().is_some()
            || self.shutdown_fence.borrow().is_some()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn native_exit_authority_is_settled_for_test(&self) -> bool {
        self.native_events.is_terminated()
            && !self.shutdown_fence_owns_effect_flush()
            && self.shutdown_completion_queued.get().is_none()
            && self.pointer_capture_release_barriers_are_clear()
            && self.native_window_retirement_barriers_are_clear()
            && self.native_events.owned_local_tasks_are_idle_for_test()
    }

    fn active_shutdown_generation(&self) -> Option<u64> {
        self.active_shutdown_completion_generation
            .get()
            .or_else(|| {
                self.shutdown_fence
                    .borrow()
                    .as_ref()
                    .map(|fence| fence.generation)
            })
    }

    pub(super) fn enqueue_shutdown_critical(
        &self,
        phase: NativeShutdownCriticalPhase,
        callback: NativeShutdownCriticalCallback,
    ) -> Result<(), NativeShutdownCriticalEnqueueError> {
        let Some(generation) = self.active_shutdown_generation() else {
            return Err(NativeShutdownCriticalEnqueueError::Inactive(callback));
        };
        self.shutdown_critical
            .borrow_mut()
            .enqueue(generation, phase, callback)?;
        self.request_shutdown_completion(generation);
        Ok(())
    }

    fn shutdown_critical_has_pending(
        &self,
        generation: u64,
        phase: NativeShutdownCriticalPhase,
    ) -> bool {
        self.shutdown_critical
            .borrow()
            .has_pending(generation, phase)
    }

    fn run_shutdown_critical_wave(
        &self,
        phase: NativeShutdownCriticalPhase,
        fence: &mut NativeShutdownFence,
    ) -> NativeShutdownCriticalWaveOutcome {
        if !self.shutdown_critical_has_pending(fence.generation, phase) {
            return NativeShutdownCriticalWaveOutcome::Idle;
        }
        let Ok(mut app) = self.app.try_borrow_mut() else {
            return NativeShutdownCriticalWaveOutcome::Retry;
        };
        let callbacks = self
            .shutdown_critical
            .borrow_mut()
            .take_wave(fence.generation, phase);
        for callback in callbacks {
            let result = catch_unwind(AssertUnwindSafe(|| callback(&mut app)));
            retain_shutdown_panic(&mut fence.first_panic, result.err());
        }
        drop(app);
        if self.shutdown_critical_has_pending(fence.generation, phase) {
            NativeShutdownCriticalWaveOutcome::Retry
        } else {
            NativeShutdownCriticalWaveOutcome::Drained
        }
    }

    fn native_window_retirement_barriers_are_clear(&self) -> bool {
        self.native_window_retirements.borrow().is_empty()
    }

    pub(super) fn begin_shutdown_fence(
        &self,
        terminate_ingress: bool,
        was_quitting: bool,
    ) -> (u64, bool) {
        if let Some(generation) = self.active_shutdown_completion_generation.get() {
            if terminate_ingress {
                self.shutdown_terminate_ingress_requested.set(true);
            }
            return (generation, false);
        }
        let mut fence = self.shutdown_fence.borrow_mut();
        if let Some(active) = fence.as_mut() {
            active.terminate_ingress |= terminate_ingress;
            return (active.generation, false);
        }
        let generation = self.next_shutdown_generation.get();
        self.next_shutdown_generation.set(
            generation
                .checked_add(1)
                .expect("native shutdown generation overflowed"),
        );
        self.shutdown_completion_queued.set(None);
        self.native_events.begin_shutdown(generation);
        let protected_before_registry_clear = self.pre_shutdown_critical.borrow_mut().take_all();
        self.shutdown_critical
            .borrow_mut()
            .begin(generation, protected_before_registry_clear);
        *fence = Some(NativeShutdownFence {
            generation,
            terminate_ingress,
            preparation_complete: false,
            initial_effect_flush_terminal: None,
            presentation_shutdowns: None,
            retry_epoch: 0,
            registry_cleared: false,
            was_quitting,
            first_panic: None,
        });
        (generation, true)
    }

    fn park_shutdown_fence(&self, mut fence: NativeShutdownFence) {
        fence.terminate_ingress |= self.shutdown_terminate_ingress_requested.replace(false);
        *self.shutdown_fence.borrow_mut() = Some(fence);
    }

    pub(super) fn finish_shutdown_preparation(
        &self,
        generation: u64,
        first_panic: Option<Box<dyn std::any::Any + Send>>,
    ) {
        let mut fence = self.shutdown_fence.borrow_mut();
        let Some(active) = fence.as_mut() else {
            return;
        };
        if active.generation != generation {
            return;
        }
        if active.first_panic.is_none() {
            active.first_panic = first_panic;
        }
        active.preparation_complete = true;
        drop(fence);
        self.request_shutdown_completion(generation);
    }

    fn request_active_shutdown_completion(&self) {
        let generation = self
            .shutdown_fence
            .borrow()
            .as_ref()
            .filter(|fence| fence.preparation_complete)
            .map(|fence| fence.generation);
        if let Some(generation) = generation {
            self.request_shutdown_completion(generation);
        }
    }

    fn request_shutdown_completion(&self, generation: u64) {
        let active = self
            .shutdown_fence
            .borrow()
            .as_ref()
            .is_some_and(|fence| fence.generation == generation && fence.preparation_complete);
        if !active || self.shutdown_completion_queued.get() == Some(generation) {
            return;
        }
        self.shutdown_completion_queued.set(Some(generation));
        self.native_events
            .enqueue_shutdown_completion(NativeShutdownCompletion::new(generation));
    }

    fn defer_shutdown_completion_retry(&self, generation: u64) {
        let retry_epoch = {
            let mut fence = self.shutdown_fence.borrow_mut();
            let Some(active) = fence.as_mut() else {
                return;
            };
            if active.generation != generation || !active.preparation_complete {
                return;
            }
            active.retry_epoch = active.retry_epoch.saturating_add(1);
            active.retry_epoch
        };
        let retry_delay = SHUTDOWN_COMPLETION_RETRY_DELAYS
            .get(usize::from(retry_epoch.saturating_sub(1)))
            .copied()
            .unwrap_or_else(|| {
                *SHUTDOWN_COMPLETION_RETRY_DELAYS
                    .last()
                    .expect("shutdown retry schedule must not be empty")
            });
        let timer = self.background_executor.timer(retry_delay);
        self.native_events
            .schedule_shutdown_completion_retry(timer, generation, retry_epoch);
    }

    pub(super) fn retry_shutdown_completion_from_wake(&self, generation: u64, retry_epoch: u8) {
        let active = self.shutdown_fence.borrow().as_ref().is_some_and(|fence| {
            fence.generation == generation
                && fence.preparation_complete
                && fence.retry_epoch == retry_epoch
        });
        if !active {
            return;
        }
        self.request_shutdown_completion(generation);
        self.drain_native_work(None);
    }

    fn mark_shutdown_completion_dequeued(&self, generation: u64) {
        if self.shutdown_completion_queued.get() == Some(generation) {
            self.shutdown_completion_queued.set(None);
        }
    }

    fn advance_shutdown_completion(
        &self,
        completion: NativeShutdownCompletion,
    ) -> NativeShutdownCompletionAction {
        let generation = completion.generation();
        let Some(mut fence) = self.shutdown_fence.borrow_mut().take() else {
            return NativeShutdownCompletionAction::Complete {
                terminate_ingress: false,
                panic: None,
            };
        };
        if fence.generation != generation {
            *self.shutdown_fence.borrow_mut() = Some(fence);
            return NativeShutdownCompletionAction::Complete {
                terminate_ingress: false,
                panic: None,
            };
        }
        let _effect_flush_ownership = NativeShutdownEffectFlushOwnership::begin(
            &self.active_shutdown_completion_generation,
            generation,
        );

        if fence.presentation_shutdowns.is_none() {
            let Ok(mut app) = self.app.try_borrow_mut() else {
                self.park_shutdown_fence(fence);
                return NativeShutdownCompletionAction::Retry;
            };
            if super::window_registry::has_checked_out_window(&app) {
                drop(app);
                self.park_shutdown_fence(fence);
                return NativeShutdownCompletionAction::Retry;
            }

            if fence.initial_effect_flush_terminal.is_none() {
                let window_cleanup = catch_unwind(AssertUnwindSafe(|| {
                    app.prepare_shutdown_pointer_sessions();
                }));
                retain_shutdown_panic(&mut fence.first_panic, window_cleanup.err());

                fence.initial_effect_flush_terminal = Some(settle_shutdown_effect_wave(
                    &mut app,
                    fence.generation,
                    "initial",
                    &mut fence.first_panic,
                ));
            }

            let prepared = catch_unwind(AssertUnwindSafe(|| {
                super::window_registry::prepare_presentation_shutdowns(&mut app)
            }));
            drop(app);
            let prepared = match prepared {
                Ok(prepared) => prepared,
                Err(payload) => {
                    retain_shutdown_panic(&mut fence.first_panic, Some(payload));
                    self.park_shutdown_fence(fence);
                    return NativeShutdownCompletionAction::Retry;
                }
            };
            let mut shutdowns = HashMap::with_capacity(prepared.len());
            merge_presentation_shutdowns(&mut shutdowns, prepared);
            fence.presentation_shutdowns = Some(shutdowns);
        }

        if !self.quiesce_shutdown_presentations(&mut fence) {
            self.park_shutdown_fence(fence);
            return NativeShutdownCompletionAction::Retry;
        }

        if self.run_shutdown_critical_wave(
            NativeShutdownCriticalPhase::BeforeWindowRegistryClear,
            &mut fence,
        ) == NativeShutdownCriticalWaveOutcome::Retry
        {
            self.park_shutdown_fence(fence);
            return NativeShutdownCompletionAction::Retry;
        }

        // Capture-release completions can enqueue pre-clear lifecycle work. The registry must
        // remain authoritative until both the release barrier and its critical continuation have
        // settled.
        if !self.pointer_capture_release_barriers_are_clear() {
            self.park_shutdown_fence(fence);
            return NativeShutdownCompletionAction::Retry;
        }

        // Pre-clear participants may have logically removed windows and transferred their exact
        // presentation tickets into the native-retirement coordinator. Claim and quiesce those
        // new authorities before the registry can be cleared.
        if !self.quiesce_shutdown_presentations(&mut fence) {
            self.park_shutdown_fence(fence);
            return NativeShutdownCompletionAction::Retry;
        }

        if !fence.registry_cleared {
            let Ok(mut app) = self.app.try_borrow_mut() else {
                self.park_shutdown_fence(fence);
                return NativeShutdownCompletionAction::Retry;
            };
            if super::window_registry::has_checked_out_window(&app) {
                drop(app);
                self.park_shutdown_fence(fence);
                return NativeShutdownCompletionAction::Retry;
            }

            let current = catch_unwind(AssertUnwindSafe(|| {
                super::window_registry::prepare_presentation_shutdowns(&mut app)
            }));
            let current = match current {
                Ok(current) => current,
                Err(payload) => {
                    retain_shutdown_panic(&mut fence.first_panic, Some(payload));
                    drop(app);
                    self.park_shutdown_fence(fence);
                    return NativeShutdownCompletionAction::Retry;
                }
            };
            let added = merge_presentation_shutdowns(
                fence
                    .presentation_shutdowns
                    .as_mut()
                    .expect("shutdown preparation must own presentation authorities"),
                current,
            );
            if added {
                drop(app);
                self.park_shutdown_fence(fence);
                return NativeShutdownCompletionAction::Retry;
            }

            if self.shutdown_critical_has_pending(
                fence.generation,
                NativeShutdownCriticalPhase::BeforeWindowRegistryClear,
            ) || !self.pointer_capture_release_barriers_are_clear()
            {
                drop(app);
                self.park_shutdown_fence(fence);
                return NativeShutdownCompletionAction::Retry;
            }

            let detached_windows = super::window_registry::take_all_for_shutdown(&mut app);
            fence.registry_cleared = true;
            self.shutdown_critical
                .borrow_mut()
                .mark_registry_cleared(fence.generation);
            drop(app);
            for (window_id, window) in detached_windows {
                let enqueue = catch_unwind(AssertUnwindSafe(|| {
                    self.enqueue_native_window_retirement(window_id, window);
                }));
                retain_shutdown_panic(&mut fence.first_panic, enqueue.err());
            }
        }

        if self.run_shutdown_critical_wave(
            NativeShutdownCriticalPhase::AfterWindowRegistryClear,
            &mut fence,
        ) == NativeShutdownCriticalWaveOutcome::Retry
        {
            self.park_shutdown_fence(fence);
            return NativeShutdownCompletionAction::Retry;
        }

        fence.terminate_ingress |= self.shutdown_terminate_ingress_requested.replace(false);

        if !self.pointer_capture_release_barriers_are_clear() {
            self.park_shutdown_fence(fence);
            return NativeShutdownCompletionAction::Retry;
        }

        if !self.native_window_retirement_barriers_are_clear() {
            self.park_shutdown_fence(fence);
            return NativeShutdownCompletionAction::Retry;
        }

        let mut terminal_converged = false;
        for _ in 0..SHUTDOWN_TERMINAL_CONVERGENCE_WAVE_BUDGET {
            let Ok(mut app) = self.app.try_borrow_mut() else {
                self.park_shutdown_fence(fence);
                return NativeShutdownCompletionAction::Retry;
            };
            settle_shutdown_effect_wave(
                &mut app,
                fence.generation,
                "terminal",
                &mut fence.first_panic,
            );
            drop(app);

            // Terminal effects may reenter shutdown, retire another presentation, or reserve a
            // new capture-release barrier. Revalidate every authority before the fence can
            // disappear.
            fence.terminate_ingress |= self.shutdown_terminate_ingress_requested.replace(false);
            if !self.quiesce_shutdown_presentations(&mut fence) {
                self.park_shutdown_fence(fence);
                return NativeShutdownCompletionAction::Retry;
            }
            match self.run_shutdown_critical_wave(
                NativeShutdownCriticalPhase::AfterWindowRegistryClear,
                &mut fence,
            ) {
                NativeShutdownCriticalWaveOutcome::Idle => {
                    terminal_converged = true;
                    break;
                }
                NativeShutdownCriticalWaveOutcome::Drained => {
                    // The participant may have produced ordinary effects. Flush them before
                    // deciding that shutdown is terminal.
                }
                NativeShutdownCriticalWaveOutcome::Retry => {
                    self.park_shutdown_fence(fence);
                    return NativeShutdownCompletionAction::Retry;
                }
            }
        }
        if !terminal_converged {
            self.park_shutdown_fence(fence);
            return NativeShutdownCompletionAction::Retry;
        }
        if !self.pointer_capture_release_barriers_are_clear() {
            self.park_shutdown_fence(fence);
            return NativeShutdownCompletionAction::Retry;
        }
        if !self.native_window_retirement_barriers_are_clear() {
            self.park_shutdown_fence(fence);
            return NativeShutdownCompletionAction::Retry;
        }

        let Ok(mut app) = self.app.try_borrow_mut() else {
            self.park_shutdown_fence(fence);
            return NativeShutdownCompletionAction::Retry;
        };
        app.quitting = fence.terminate_ingress;
        app.window_open_barrier_depth = app
            .window_open_barrier_depth
            .checked_sub(1)
            .expect("shutdown fence must retain one window-open barrier");
        if !fence.terminate_ingress {
            app.quitting = fence.was_quitting;
        }
        drop(app);

        self.shutdown_critical.borrow_mut().finish(fence.generation);

        NativeShutdownCompletionAction::Complete {
            terminate_ingress: fence.terminate_ingress,
            panic: fence.first_panic.take(),
        }
    }

    pub(super) fn register_native_captured_drag_observer(
        self: &Rc<Self>,
        callback: NativeCapturedDragConsumer,
    ) -> crate::Subscription {
        self.native_captured_drags
            .register_observer(Rc::downgrade(self), callback)
    }

    pub(super) fn register_native_captured_drag_route(
        self: &Rc<Self>,
        callback: NativeCapturedDragConsumer,
        release_locker: super::native_captured_drag::NativeCapturedDragReleaseLocker,
    ) -> crate::Subscription {
        self.native_captured_drags
            .register_route(Rc::downgrade(self), callback, release_locker)
    }

    pub(super) fn prepare_native_captured_drag_route(
        &self,
    ) -> Option<super::native_captured_drag::RegisteredConsumer> {
        self.native_captured_drags.prepared_route()
    }

    pub(super) fn lock_native_captured_drag_release(
        &self,
        event: &NativeCapturedDragEvent,
        source_window: &mut crate::Window,
        cx: &mut App,
    ) -> Option<std::sync::Arc<dyn std::any::Any>> {
        self.native_captured_drags
            .lock_release(event, source_window, cx)
    }

    pub(super) fn begin_native_captured_drag_generation(
        &self,
        authority: NativeCapturedDragAuthoritySnapshot,
        route_binding: super::native_captured_drag::NativeCapturedDragRouteBinding,
    ) {
        self.native_captured_drags
            .begin_generation(authority, route_binding);
    }

    pub(super) fn native_captured_drag_start_is_unfenced(
        &self,
        source_window: WindowId,
        source_sequence: Option<super::native_captured_drag::NativeIngressSequence>,
    ) -> bool {
        self.native_captured_drags
            .start_is_unfenced(source_window, source_sequence)
    }

    pub(crate) fn reserve_reentrant_pointer_cancel(
        self: &Rc<Self>,
        window_id: WindowId,
        slot_generation: u64,
        reason: PointerCancelReason,
    ) -> bool {
        if self.native_events.is_terminated() {
            return false;
        }
        let sequence = self.native_events.reserve_input_sequence();
        self.native_captured_drags
            .reserve_pointer_cancel(sequence, window_id, reason);
        let reservation = ReservedPointerCancel::platform_input(
            reason,
            sequence,
            slot_generation,
            Rc::downgrade(self),
        );
        let envelope = self.native_events.prepare_presequenced(
            sequence,
            window_id,
            NativeWindowEvent::PointerCanceled(reservation),
        );
        self.native_events.enqueue_envelope(envelope);
        true
    }

    pub(crate) fn reserve_native_captured_drag_cancel(
        self: &Rc<Self>,
        window_id: WindowId,
        generation: NativeCapturedDragGeneration,
        reason: PointerCancelReason,
    ) -> bool {
        if self.native_events.is_terminated() {
            return false;
        }
        let sequence = self.native_events.reserve_input_sequence();
        self.native_captured_drags
            .reserve_pointer_cancel(sequence, window_id, reason);
        let reservation = ReservedPointerCancel::captured_drag_terminal(
            reason,
            sequence,
            generation,
            Rc::downgrade(self),
        );
        let envelope = self.native_events.prepare_presequenced(
            sequence,
            window_id,
            NativeWindowEvent::PointerCanceled(reservation),
        );
        self.native_events.enqueue_envelope(envelope);
        true
    }

    pub(super) fn finish_reserved_pointer_cancel(
        &self,
        sequence: super::native_captured_drag::NativeIngressSequence,
    ) {
        self.native_captured_drags.finish_pointer_cancel(sequence);
    }

    pub(super) fn promote_reserved_pointer_cancel(
        &self,
        sequence: super::native_captured_drag::NativeIngressSequence,
    ) {
        self.native_captured_drags.promote_pointer_cancel(sequence);
    }

    pub(super) fn enqueue_native_captured_drag(&self, event: NativeCapturedDragEvent) {
        self.native_captured_drags.enqueue(event);
    }

    pub(super) fn retire_native_captured_drag_generation(
        &self,
        generation: NativeCapturedDragGeneration,
    ) {
        self.native_captured_drags.retire_generation(generation);
    }

    pub(super) fn unsubscribe_native_captured_drag(&self, key: ConsumerKey) {
        self.native_captured_drags.unsubscribe(key);
    }

    pub(super) fn set_native_window_control_area(
        &self,
        window_id: WindowId,
        area: Option<WindowControlArea>,
    ) {
        self.native_queries.set_window_control_area(window_id, area);
    }

    pub(super) fn native_window_control_area(
        &self,
        window_id: WindowId,
    ) -> Option<WindowControlArea> {
        self.native_queries.window_control_area(window_id)
    }

    pub(super) fn enqueue_native_window_event(
        &self,
        window_id: WindowId,
        event: NativeWindowEvent,
    ) {
        self.native_events.enqueue(window_id, event);
    }

    pub(super) fn enqueue_native_app_event(&self, event: NativeAppEvent) {
        self.native_events.enqueue_app(event);
    }

    #[cfg(not(target_family = "wasm"))]
    pub(crate) fn native_accessibility_ingress(
        &self,
    ) -> super::native_event_ingress::NativeAccessibilityIngress {
        self.native_events.accessibility_ingress()
    }

    #[cfg(not(target_family = "wasm"))]
    pub(super) fn import_native_accessibility_events(&self) {
        self.native_events.import_native_accessibility_events();
    }

    pub(crate) fn enqueue_will_open_app_menu(&self) {
        self.enqueue_native_app_event(NativeAppEvent::WillOpenAppMenu);
    }

    pub(crate) fn enqueue_app_menu_action(&self, action: Box<dyn Action>) {
        self.enqueue_native_app_event(NativeAppEvent::AppMenuAction(action));
    }

    #[cfg(test)]
    pub(crate) fn enqueue_keyboard_layout_changed_for_test(&self) {
        self.enqueue_native_app_event(NativeAppEvent::KeyboardLayoutChanged);
    }

    #[cfg(test)]
    pub(crate) fn enqueue_thermal_state_changed_for_test(&self) {
        self.enqueue_native_app_event(NativeAppEvent::ThermalStateChanged);
    }

    #[cfg(test)]
    pub(crate) fn enqueue_quit_for_test(&self) {
        self.enqueue_native_app_event(NativeAppEvent::Quit);
    }

    pub(crate) fn enqueue_platform_window_command(
        &self,
        window_id: WindowId,
        dispatcher: PlatformWindowCommandDispatcher,
        command: PlatformWindowCommand,
    ) {
        self.native_events
            .enqueue_command(window_id, dispatcher, command);
    }

    pub(crate) fn enqueue_provisional_window_reveal(
        &self,
        window_id: WindowId,
        dispatcher: PlatformWindowCommandDispatcher,
        command: PlatformWindowCommand,
        ticket: crate::WindowProvisionalRevealTicket,
    ) {
        self.native_events
            .enqueue_provisional_reveal(window_id, dispatcher, command, ticket);
    }

    pub(super) fn dispatch_native_window_input(
        &self,
        window_id: WindowId,
        event: PlatformInput,
    ) -> Result<DispatchEventResult, NativeInputInvariantViolation> {
        let sequence_cutoff = self.native_events.reserve_input_sequence();
        let boundary = NativeInputBoundary::PlatformInput;
        let mut terminal = NativeBoundaryTerminalGuard::immediate_input(
            &self.native_events,
            sequence_cutoff.value(),
            window_id,
            NativeCallbackKind::PlatformInput,
        );
        let Some(slot_generation) =
            self.current_native_callback_generation(NativeCallbackLeaseKind::PlatformInput)
        else {
            return Err(terminal.reject_input(
                window_id,
                boundary,
                None,
                NativeInvariantFailure::MissingLease,
            ));
        };
        terminal.set_generation(NativeBoundaryGeneration::InputSlot {
            boundary,
            generation: slot_generation,
        });

        match self.native_queries.lookup(window_id) {
            None => {
                return Err(terminal.reject_input(
                    window_id,
                    boundary,
                    Some(slot_generation),
                    NativeInvariantFailure::StaleWindow,
                ));
            }
            Some(snapshot) if snapshot.lifecycle() == NativeWindowLifecycle::Reserved => {
                return Err(terminal.reject_input(
                    window_id,
                    boundary,
                    Some(slot_generation),
                    NativeInvariantFailure::ReservedWindow,
                ));
            }
            Some(_) => {}
        }

        let mut barrier = match self.native_events.begin_input_barrier() {
            Ok(barrier) => barrier,
            Err(_) => {
                return Err(terminal.reject_input(
                    window_id,
                    boundary,
                    Some(slot_generation),
                    NativeInvariantFailure::EventTransactionReentry,
                ));
            }
        };
        loop {
            match barrier.pop_event_before(sequence_cutoff.value()) {
                NativeEventPrefixPop::Event(envelope) => {
                    let event_sequence = envelope.ingress_sequence();
                    let Ok(mut app) = self.app.try_borrow_mut() else {
                        barrier.push_front(NativeWorkEnvelope::Event(envelope));
                        return Err(terminal.reject_input(
                            window_id,
                            boundary,
                            Some(slot_generation),
                            NativeInvariantFailure::AppBorrowBusy,
                        ));
                    };
                    let mut event_terminal = NativeBoundaryTerminalGuard::new(
                        &self.native_events,
                        envelope.pending_diagnostic(),
                    );
                    let delivery = event_terminal.run_callback_with_panic_cleanup(
                        || app.update(|app| envelope.deliver(app)),
                        || self.native_captured_drags.retire_sequence(event_sequence),
                    );
                    drop(app);
                    event_terminal.run_callback(|| self.drain_native_captured_drags());
                    event_terminal.settle_event(delivery.disposition);
                    if delivery.control == NativeEventDrainControl::Terminate {
                        barrier.terminate();
                        break;
                    }
                }
                NativeEventPrefixPop::BlockedOrEmpty => break,
                NativeEventPrefixPop::BudgetExhausted => {
                    return Err(terminal.reject_input(
                        window_id,
                        boundary,
                        Some(slot_generation),
                        NativeInvariantFailure::BarrierBudgetExhausted,
                    ));
                }
            }
        }

        let Ok(mut app) = self.app.try_borrow_mut() else {
            return Err(terminal.reject_input(
                window_id,
                boundary,
                Some(slot_generation),
                NativeInvariantFailure::AppBorrowBusy,
            ));
        };
        if app.quitting {
            drop(app);
            let violation = terminal.reject_input(
                window_id,
                boundary,
                Some(slot_generation),
                NativeInvariantFailure::ApplicationQuitting,
            );
            let root = barrier.is_root();
            barrier.terminate();
            barrier.finish_without_wake();
            if root {
                self.drain_native_work(None);
            }
            return Err(violation);
        }
        let result = terminal.run_callback_with_panic_cleanup(
            || {
                app.update_window_id_from_native(window_id, sequence_cutoff, |_, window, cx| {
                    window.dispatch_event(event, cx)
                })
            },
            || self.native_captured_drags.retire_sequence(sequence_cutoff),
        );
        drop(app);
        terminal.run_callback(|| self.drain_native_captured_drags());

        let outcome = match result {
            Ok(result) => {
                terminal.settle(NativeBoundaryDisposition::delivered_input(result));
                Ok(result)
            }
            Err(_) => Err(terminal.reject_input(
                window_id,
                boundary,
                Some(slot_generation),
                NativeInvariantFailure::StaleWindow,
            )),
        };
        let root = barrier.is_root();
        barrier.finish_without_wake();
        if root {
            self.drain_native_work(None);
        }
        outcome
    }

    pub(super) fn dispatch_native_input_handler<R>(
        &self,
        window_id: WindowId,
        operation: NativeInputHandlerOperation,
        callback: impl FnOnce(&mut crate::Window, &mut App) -> R,
    ) -> Result<R, NativeInputInvariantViolation> {
        let sequence_cutoff = self.native_events.reserve_input_sequence();
        let kind = NativeCallbackKind::PlatformInputHandler(operation);
        let boundary = NativeInputBoundary::InputHandler;
        let mut terminal = NativeBoundaryTerminalGuard::immediate_input(
            &self.native_events,
            sequence_cutoff.value(),
            window_id,
            kind,
        );
        let Some(slot_generation) =
            self.current_native_callback_generation(NativeCallbackLeaseKind::InputHandler)
        else {
            return Err(terminal.reject_input(
                window_id,
                boundary,
                None,
                NativeInvariantFailure::MissingLease,
            ));
        };
        terminal.set_generation(NativeBoundaryGeneration::InputSlot {
            boundary,
            generation: slot_generation,
        });

        match self.native_queries.lookup(window_id) {
            None => {
                return Err(terminal.reject_input(
                    window_id,
                    boundary,
                    Some(slot_generation),
                    NativeInvariantFailure::StaleWindow,
                ));
            }
            Some(snapshot) if snapshot.lifecycle() == NativeWindowLifecycle::Reserved => {
                return Err(terminal.reject_input(
                    window_id,
                    boundary,
                    Some(slot_generation),
                    NativeInvariantFailure::ReservedWindow,
                ));
            }
            Some(_) => {}
        }

        let mut barrier = match self.native_events.begin_input_barrier() {
            Ok(barrier) => barrier,
            Err(_) => {
                return Err(terminal.reject_input(
                    window_id,
                    boundary,
                    Some(slot_generation),
                    NativeInvariantFailure::EventTransactionReentry,
                ));
            }
        };
        loop {
            match barrier.pop_event_before(sequence_cutoff.value()) {
                NativeEventPrefixPop::Event(envelope) => {
                    let event_sequence = envelope.ingress_sequence();
                    let Ok(mut app) = self.app.try_borrow_mut() else {
                        barrier.push_front(NativeWorkEnvelope::Event(envelope));
                        return Err(terminal.reject_input(
                            window_id,
                            boundary,
                            Some(slot_generation),
                            NativeInvariantFailure::AppBorrowBusy,
                        ));
                    };
                    let mut event_terminal = NativeBoundaryTerminalGuard::new(
                        &self.native_events,
                        envelope.pending_diagnostic(),
                    );
                    let delivery = event_terminal.run_callback_with_panic_cleanup(
                        || app.update(|app| envelope.deliver(app)),
                        || self.native_captured_drags.retire_sequence(event_sequence),
                    );
                    drop(app);
                    event_terminal.run_callback(|| self.drain_native_captured_drags());
                    event_terminal.settle_event(delivery.disposition);
                    if delivery.control == NativeEventDrainControl::Terminate {
                        barrier.terminate();
                        break;
                    }
                }
                NativeEventPrefixPop::BlockedOrEmpty => break,
                NativeEventPrefixPop::BudgetExhausted => {
                    return Err(terminal.reject_input(
                        window_id,
                        boundary,
                        Some(slot_generation),
                        NativeInvariantFailure::BarrierBudgetExhausted,
                    ));
                }
            }
        }

        let Ok(mut app) = self.app.try_borrow_mut() else {
            return Err(terminal.reject_input(
                window_id,
                boundary,
                Some(slot_generation),
                NativeInvariantFailure::AppBorrowBusy,
            ));
        };
        if app.quitting {
            drop(app);
            let violation = terminal.reject_input(
                window_id,
                boundary,
                Some(slot_generation),
                NativeInvariantFailure::ApplicationQuitting,
            );
            barrier.terminate();
            barrier.finish_without_wake();
            return Err(violation);
        }
        let result = terminal.run_callback_with_panic_cleanup(
            || {
                app.update_window_id_from_native(window_id, sequence_cutoff, |_, window, cx| {
                    callback(window, cx)
                })
            },
            || self.native_captured_drags.retire_sequence(sequence_cutoff),
        );
        drop(app);
        terminal.run_callback(|| self.drain_native_captured_drags());

        let outcome = match result {
            Ok(result) => {
                terminal.settle(NativeBoundaryDisposition::DELIVERED);
                Ok(result)
            }
            Err(_) => Err(terminal.reject_input(
                window_id,
                boundary,
                Some(slot_generation),
                NativeInvariantFailure::StaleWindow,
            )),
        };
        let root = barrier.is_root();
        barrier.finish_without_wake();
        if root {
            self.drain_native_work(None);
        }
        outcome
    }

    fn record_input_terminal(
        &self,
        sequence: u64,
        window_id: WindowId,
        kind: NativeCallbackKind,
        domain_generation: Option<NativeBoundaryGeneration>,
        disposition: NativeBoundaryDisposition,
    ) {
        self.native_events.record_immediate(
            sequence,
            window_id,
            kind,
            domain_generation,
            disposition,
        );
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn native_boundary_diagnostics(
        &self,
        cursor: NativeBoundaryDiagnosticCursor,
    ) -> NativeBoundaryDiagnosticsSnapshot {
        self.native_events.diagnostics_snapshot_since(cursor)
    }

    pub(crate) fn record_native_input_slot_invariant(
        &self,
        window_id: WindowId,
        boundary: NativeInputBoundary,
        generation: u64,
        failure: NativeInvariantFailure,
    ) {
        let sequence = self.native_events.reserve_input_sequence();
        let kind = match boundary {
            NativeInputBoundary::PlatformInput => NativeCallbackKind::PlatformInput,
            NativeInputBoundary::InputHandler => NativeCallbackKind::PlatformInputHandlerSlot,
        };
        self.record_input_terminal(
            sequence.value(),
            window_id,
            kind,
            Some(NativeBoundaryGeneration::InputSlot {
                boundary,
                generation,
            }),
            NativeBoundaryDisposition::InvariantFailure(failure),
        );
    }

    fn current_native_callback_generation(&self, kind: NativeCallbackLeaseKind) -> Option<u64> {
        kind.stack(self).borrow().last().copied()
    }

    #[cfg(test)]
    pub(crate) fn drain_native_work_for_test(&self) {
        self.drain_native_work(None);
    }

    pub(crate) fn dispatch_window_should_close(&self, window_id: WindowId) -> bool {
        let envelope = self
            .native_events
            .prepare(window_id, NativeWindowEvent::CloseRequested);
        let lifecycle = self
            .native_queries
            .lookup(window_id)
            .map(|snapshot| snapshot.lifecycle());
        if lifecycle.is_none() {
            self.native_events.record_terminal(
                envelope.pending_diagnostic(),
                NativeBoundaryDisposition::Stale,
            );
            log::trace!(
                "native close query window={window_id:?} disposition=StaleWindowAllowingClose"
            );
            return true;
        }
        if lifecycle == Some(NativeWindowLifecycle::Reserved) {
            self.native_events.enqueue_envelope(envelope);
            return false;
        }
        if self.native_events.can_deliver_inline()
            && let Ok(mut app) = self.try_borrow_mut()
        {
            let sequence = envelope.sequence();
            let ingress_sequence = envelope.ingress_sequence();
            let mut terminal = NativeBoundaryTerminalGuard::new(
                &self.native_events,
                envelope.pending_diagnostic(),
            );
            let disposition = if app.quitting {
                NativeBoundaryDisposition::Closed
            } else {
                terminal.run_callback(|| {
                    app.update(|app| {
                        app.update_window_id_from_native(
                            window_id,
                            ingress_sequence,
                            |_, window, cx| {
                                if window.should_close(cx) {
                                    window.remove_window(cx);
                                }
                            },
                        )
                        .ok();
                    })
                });
                NativeBoundaryDisposition::DELIVERED
            };
            drop(app);
            terminal.run_callback(|| self.drain_native_captured_drags());
            terminal.settle(disposition);
            log::trace!(
                "native event sequence={sequence} window={window_id:?} disposition=DeliveredInline"
            );
            self.drain_native_work(None);
            return false;
        }

        self.native_events.enqueue_envelope(envelope);
        false
    }

    pub(super) fn drain_native_work_from_wake(&self, wake_ticket: u64) {
        self.drain_native_work(Some(wake_ticket));
    }

    pub(super) fn recover_native_work_after_unwind(&self) {
        self.native_events.finish_unwind_recovery_wake();
        self.app_borrow_released();
    }

    pub(super) fn retry_native_pointer_capture_release_from_wake(
        &self,
        token: NativePointerCaptureReleaseToken,
        retry_epoch: u8,
    ) {
        self.retry_pending_pointer_capture_releases(
            NativePointerCaptureReleaseRetryTrigger::Delayed { token, retry_epoch },
        );
        self.drain_native_work(None);
    }

    pub(super) fn retry_native_pointer_capture_release_for_native_window_progress(
        &self,
        window_id: WindowId,
    ) {
        self.retry_pending_pointer_capture_releases(
            NativePointerCaptureReleaseRetryTrigger::NativeWindowProgress(window_id),
        );
    }

    fn drain_native_work(&self, wake_ticket: Option<u64>) {
        if self.native_callback_lease_active() {
            self.native_events.postpone_drain(wake_ticket);
            return;
        }
        let Some(mut drain) = self.native_events.try_begin_drain(wake_ticket) else {
            return;
        };

        loop {
            match drain.pop_front() {
                NativeWorkPop::Work(envelope) => {
                    let sequence = envelope.sequence();
                    match envelope {
                        NativeWorkEnvelope::Event(event) => {
                            let event_sequence = event.ingress_sequence();
                            let mut terminal = NativeBoundaryTerminalGuard::new(
                                &self.native_events,
                                event.pending_diagnostic(),
                            );
                            let Ok(mut app) = self.app.try_borrow_mut() else {
                                terminal
                                    .pending
                                    .take()
                                    .expect("blocked native event terminal must remain pending");
                                drain.push_front(NativeWorkEnvelope::Event(event));
                                drain.block_on_app();
                                return;
                            };
                            if app.quitting {
                                drop(app);
                                self.wake_app_borrow_waiters();
                                terminal.settle(NativeBoundaryDisposition::Closed);
                                drain.terminate();
                                return;
                            }
                            let delivery = terminal.run_callback_with_panic_cleanup(
                                || app.update(|app| event.deliver(app)),
                                || self.native_captured_drags.retire_sequence(event_sequence),
                            );
                            drop(app);
                            terminal.run_callback(|| self.drain_native_captured_drags());
                            self.wake_app_borrow_waiters();
                            terminal.settle_event(delivery.disposition);
                            if delivery.control == NativeEventDrainControl::Terminate {
                                drain.terminate();
                                return;
                            }
                        }
                        NativeWorkEnvelope::Command { command, .. } => {
                            let mut terminal = NativeBoundaryTerminalGuard::new(
                                &self.native_events,
                                command.pending_diagnostic(sequence),
                            );
                            let Ok(app) = self.app.try_borrow_mut() else {
                                terminal
                                    .pending
                                    .take()
                                    .expect("blocked native command terminal must remain pending");
                                drain.push_front(NativeWorkEnvelope::Command { sequence, command });
                                drain.block_on_app();
                                return;
                            };
                            let window_id = command.window_id();
                            let quitting = app.quitting;
                            let dispatchable = !quitting
                                && self.native_queries.committed(window_id).is_some()
                                && command.provisional_reveal_is_pending();
                            drop(app);
                            if dispatchable {
                                log::trace!(
                                    "native platform command sequence={sequence} window={window_id:?} disposition=Dispatching"
                                );
                                let completes_initial_presentation =
                                    command.completes_initial_presentation();
                                let command_guard = drain.enter_command();
                                let outcome = terminal.run_callback(|| command.dispatch());
                                drop(command_guard);
                                match outcome {
                                    PlatformWindowCommandOutcome::Accepted => {
                                        command.settle_provisional_reveal(
                                            crate::WindowProvisionalRevealOutcome::Revealed,
                                        );
                                        terminal.settle(NativeBoundaryDisposition::DELIVERED);
                                        if completes_initial_presentation {
                                            self.enqueue_native_window_event(
                                                window_id,
                                                NativeWindowEvent::InitialPresentationCompleted,
                                            );
                                        }
                                    }
                                    PlatformWindowCommandOutcome::Rejected => {
                                        command.settle_provisional_reveal(
                                            crate::WindowProvisionalRevealOutcome::Rejected,
                                        );
                                        terminal.settle(NativeBoundaryDisposition::Rejected);
                                        match command.settle_rejection() {
                                            NativePlatformCommandRejection::Retry(retry) => {
                                                self.native_events.enqueue_native_command(retry);
                                            }
                                            NativePlatformCommandRejection::InitialPresentationFailed => {
                                                self.enqueue_native_window_event(
                                                    window_id,
                                                    NativeWindowEvent::InitialPresentationFailed,
                                                );
                                            }
                                            NativePlatformCommandRejection::Terminal => {}
                                        }
                                    }
                                }
                            } else {
                                command.settle_provisional_reveal(if quitting {
                                    crate::WindowProvisionalRevealOutcome::WindowTerminal
                                } else {
                                    crate::WindowProvisionalRevealOutcome::Stale
                                });
                                terminal.settle(if quitting {
                                    NativeBoundaryDisposition::Closed
                                } else {
                                    NativeBoundaryDisposition::Stale
                                });
                                log::trace!(
                                    "native platform command sequence={sequence} window={window_id:?} disposition=StaleWindow"
                                );
                            }
                        }
                        NativeWorkEnvelope::PointerCaptureRelease { release, .. } => {
                            let mut terminal = NativeBoundaryTerminalGuard::new(
                                &self.native_events,
                                release.pending_diagnostic(sequence),
                            );
                            let token = release.token();
                            let dispatchable = self
                                .pointer_capture_releases
                                .borrow()
                                .get(&token.release_generation())
                                .is_some_and(|barrier| {
                                    barrier.token == token
                                        && barrier.state == NativePointerCaptureReleaseState::Queued
                                });
                            if !dispatchable {
                                terminal.settle(NativeBoundaryDisposition::Stale);
                                continue;
                            }

                            let retry = release.clone();
                            let command_guard = drain.enter_command();
                            let outcome = catch_unwind(AssertUnwindSafe(|| release.dispatch()));
                            drop(command_guard);
                            match outcome {
                                Ok(PlatformPointerCaptureReleaseOutcome::Released) => {
                                    self.complete_native_pointer_capture_release(
                                        token,
                                        NativeCapturedDragReleaseTerminal::Released,
                                    );
                                    terminal.settle(NativeBoundaryDisposition::DELIVERED);
                                }
                                Ok(PlatformPointerCaptureReleaseOutcome::NativeWindowTerminal) => {
                                    self.settle_native_window_terminal(token.window_id());
                                    terminal.settle(NativeBoundaryDisposition::DELIVERED);
                                }
                                Ok(PlatformPointerCaptureReleaseOutcome::Rejected) => {
                                    terminal.settle(NativeBoundaryDisposition::Rejected);
                                    self.defer_native_pointer_capture_release_retry(retry);
                                }
                                Err(payload) => {
                                    terminal.settle(NativeBoundaryDisposition::InvariantFailure(
                                        NativeInvariantFailure::CallbackPanicked,
                                    ));
                                    self.defer_native_pointer_capture_release_retry(retry);
                                    resume_unwind(payload);
                                }
                            }
                        }
                        NativeWorkEnvelope::CapturedDragReleaseCompletion {
                            completion, ..
                        } => {
                            let mut terminal = NativeBoundaryTerminalGuard::new(
                                &self.native_events,
                                completion.pending_diagnostic(sequence),
                            );
                            let barrier = completion.barrier();
                            let mut first_panic = catch_unwind(AssertUnwindSafe(|| {
                                self.drain_native_captured_drags()
                            }))
                            .err();
                            let Ok(mut app) = self.app.try_borrow_mut() else {
                                terminal.pending.take().expect(
                                    "blocked captured-drag release terminal must remain pending",
                                );
                                drain.push_front(
                                    NativeWorkEnvelope::CapturedDragReleaseCompletion {
                                        sequence,
                                        completion,
                                    },
                                );
                                if let Some(payload) = first_panic {
                                    self.native_events.schedule_drain_after_unwind();
                                    resume_unwind(payload);
                                } else {
                                    drain.block_on_app();
                                    return;
                                }
                            };
                            let Some(release_terminal) =
                                self.begin_native_captured_drag_release_completion(barrier)
                            else {
                                drop(app);
                                self.wake_app_borrow_waiters();
                                if let Some(payload) = first_panic {
                                    terminal.settle(NativeBoundaryDisposition::InvariantFailure(
                                        NativeInvariantFailure::CallbackPanicked,
                                    ));
                                    resume_unwind(payload);
                                }
                                terminal.settle(NativeBoundaryDisposition::Stale);
                                continue;
                            };
                            loop {
                                let continuations = self
                                    .take_native_captured_drag_release_continuations(barrier)
                                    .expect(
                                        "delivering captured-drag release barrier must remain present",
                                    );
                                if continuations.is_empty() {
                                    assert!(
                                        self.finish_native_captured_drag_release_completion(
                                            barrier
                                        ),
                                        "drained captured-drag release barrier must finish exactly once",
                                    );
                                    break;
                                }
                                for continuation in continuations {
                                    if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
                                        app.update(|app| {
                                            continuation(barrier, release_terminal, app)
                                        });
                                    })) {
                                        retain_shutdown_panic(&mut first_panic, Some(payload));
                                    }
                                }
                            }
                            drop(app);
                            self.request_active_shutdown_completion();
                            self.wake_app_borrow_waiters();
                            if let Some(payload) = first_panic {
                                terminal.settle(NativeBoundaryDisposition::InvariantFailure(
                                    NativeInvariantFailure::CallbackPanicked,
                                ));
                                resume_unwind(payload);
                            }
                            terminal.settle(NativeBoundaryDisposition::DELIVERED);
                        }
                        NativeWorkEnvelope::WindowRetirement { mut retirement, .. } => {
                            let mut terminal = NativeBoundaryTerminalGuard::new(
                                &self.native_events,
                                retirement.pending_diagnostic(sequence),
                            );
                            let window_id = retirement.window_id();
                            let dispatchable = self
                                .native_window_retirements
                                .borrow()
                                .get(&window_id)
                                .is_some_and(|barrier| {
                                    barrier.state == NativeWindowRetirementState::Queued
                                });
                            if !dispatchable {
                                terminal.settle(NativeBoundaryDisposition::Stale);
                                continue;
                            }
                            self.native_window_retirements
                                .borrow_mut()
                                .get_mut(&window_id)
                                .expect("dispatchable retirement must retain its exact barrier")
                                .state = NativeWindowRetirementState::AwaitingNativeTerminal;
                            let attempt = catch_unwind(AssertUnwindSafe(|| retirement.retire()));
                            match attempt {
                                Ok(NativeWindowRetirementAttempt::Accepted) => {
                                    if self.native_window_terminal_was_observed(window_id) {
                                        self.settle_native_window_terminal(window_id);
                                    }
                                    self.request_active_shutdown_completion();
                                    terminal.settle(NativeBoundaryDisposition::DELIVERED);
                                }
                                Ok(NativeWindowRetirementAttempt::NativeWindowTerminal) => {
                                    self.settle_native_window_terminal(window_id);
                                    terminal.settle(NativeBoundaryDisposition::DELIVERED);
                                }
                                Ok(NativeWindowRetirementAttempt::Rejected) => {
                                    self.defer_native_window_retirement_retry(retirement);
                                    self.request_active_shutdown_completion();
                                    terminal.settle(NativeBoundaryDisposition::Rejected);
                                }
                                Err(payload) => {
                                    if retirement.retains_window_owner() {
                                        self.defer_native_window_retirement_retry(retirement);
                                    }
                                    self.request_active_shutdown_completion();
                                    terminal.settle(NativeBoundaryDisposition::InvariantFailure(
                                        NativeInvariantFailure::CallbackPanicked,
                                    ));
                                    resume_unwind(payload);
                                }
                            }
                        }
                        NativeWorkEnvelope::ShutdownCompletion { completion, .. } => {
                            let mut terminal = NativeBoundaryTerminalGuard::new(
                                &self.native_events,
                                completion.pending_diagnostic(sequence),
                            );
                            self.mark_shutdown_completion_dequeued(completion.generation());
                            match self.advance_shutdown_completion(completion) {
                                NativeShutdownCompletionAction::Retry => {
                                    self.defer_shutdown_completion_retry(completion.generation());
                                    terminal.settle(NativeBoundaryDisposition::Rejected);
                                }
                                NativeShutdownCompletionAction::Complete {
                                    terminate_ingress,
                                    panic,
                                } => {
                                    terminal.settle(NativeBoundaryDisposition::DELIVERED);
                                    if terminate_ingress {
                                        drain.terminate();
                                    } else {
                                        self.native_events.end_shutdown(completion.generation());
                                    }
                                    if let Some(payload) = panic {
                                        resume_unwind(payload);
                                    }
                                }
                            }
                        }
                    }
                }
                NativeWorkPop::Empty => {
                    self.drain_native_captured_drags();
                    return;
                }
                NativeWorkPop::BudgetExhausted => return,
            }
        }
    }

    fn drain_native_captured_drags(&self) {
        let Some(next_sequence) = self.native_captured_drags.next_sequence() else {
            return;
        };
        if self.native_events.has_pending_before(next_sequence) {
            return;
        }
        if !self.native_captured_drags.begin_drain() {
            return;
        }

        let result = catch_unwind(AssertUnwindSafe(|| {
            while let Some(envelope) = self.native_captured_drags.pop_front() {
                if self
                    .native_events
                    .has_pending_before(envelope.event.sequence())
                {
                    self.native_captured_drags.push_front(envelope);
                    return;
                }
                let Ok(mut app) = self.app.try_borrow_mut() else {
                    self.native_captured_drags.push_front(envelope);
                    return;
                };
                if app.quitting {
                    drop(app);
                    return;
                }

                let delivery = catch_unwind(AssertUnwindSafe(|| {
                    app.update(|app| (envelope.callback)(envelope.event.clone(), app));
                }));
                match delivery {
                    Ok(()) => {
                        drop(app);
                        self.native_captured_drags.finish_delivery(&envelope);
                    }
                    Err(payload) => {
                        let already_unwinding = std::thread::panicking();
                        let cleanup = catch_unwind(AssertUnwindSafe(|| {
                            app.update(|app| {
                                app.settle_panicking_native_captured_drag(
                                    envelope.event.generation(),
                                    envelope.event.source_window(),
                                );
                            });
                        }));
                        drop(app);
                        self.native_captured_drags
                            .retire_panicking_consumer(envelope.key, envelope.event.generation());
                        if cleanup.is_err() {
                            log::error!(
                                "native captured-drag panic cleanup also panicked for generation {:?}",
                                envelope.event.generation()
                            );
                        }
                        if already_unwinding {
                            log::error!(
                                "suppressed a native captured-drag consumer panic while another panic was unwinding"
                            );
                            drop(payload);
                            break;
                        }
                        resume_unwind(payload);
                    }
                }
            }
        }));
        self.native_captured_drags.finish_drain();
        if let Err(payload) = result {
            resume_unwind(payload);
        }
    }

    fn app_borrow_released(&self) {
        if self.native_callback_lease_active() {
            return;
        }
        if !self.app_is_idle() {
            return;
        }
        self.request_active_shutdown_completion();
        self.drain_native_captured_drags();
        self.native_events.resume_after_app_borrow();
        self.drain_native_work(None);
        self.wake_app_borrow_waiters();
    }

    // Registering after a failed borrow and checking idle happen on the application thread, so a
    // release cannot be missed between those operations.
    pub(super) fn wait_for_app_borrow_release(&self) -> oneshot::Receiver<()> {
        let (sender, receiver) = oneshot::channel();
        if self.app_is_idle() {
            let _ = sender.send(());
        } else {
            self.app_borrow_waiters.borrow_mut().push(sender);
        }
        receiver
    }

    fn wake_app_borrow_waiters(&self) {
        let waiters = std::mem::take(&mut *self.app_borrow_waiters.borrow_mut());
        for waiter in waiters {
            let _ = waiter.send(());
        }
    }

    fn app_is_idle(&self) -> bool {
        let Ok(app) = self.app.try_borrow_mut() else {
            return false;
        };
        drop(app);
        true
    }

    #[doc(hidden)]
    #[track_caller]
    pub fn borrow(&self) -> AppRef<'_> {
        if option_env!("TRACK_THREAD_BORROWS").is_some() {
            let thread_id = std::thread::current().id();
            eprintln!("borrowed {thread_id:?}");
        }
        AppRef {
            app: Some(self.app.borrow()),
            cell: self,
        }
    }

    #[doc(hidden)]
    #[track_caller]
    pub fn borrow_mut(&self) -> AppRefMut<'_> {
        if option_env!("TRACK_THREAD_BORROWS").is_some() {
            let thread_id = std::thread::current().id();
            eprintln!("borrowed {thread_id:?}");
        }
        AppRefMut {
            app: Some(self.app.borrow_mut()),
            cell: self,
        }
    }

    #[doc(hidden)]
    #[track_caller]
    pub fn try_borrow_mut(&self) -> Result<AppRefMut<'_>, BorrowMutError> {
        if option_env!("TRACK_THREAD_BORROWS").is_some() {
            let thread_id = std::thread::current().id();
            eprintln!("borrowed {thread_id:?}");
        }
        Ok(AppRefMut {
            app: Some(self.app.try_borrow_mut()?),
            cell: self,
        })
    }
}

struct NativeAppHandlerSlot<T: ?Sized> {
    state: RefCell<NativeAppHandlerState<T>>,
}

impl<T: ?Sized> Default for NativeAppHandlerSlot<T> {
    fn default() -> Self {
        Self {
            state: RefCell::new(NativeAppHandlerState {
                generation: 0,
                handler: None,
            }),
        }
    }
}

impl<T: ?Sized> NativeAppHandlerSlot<T> {
    fn set(&self, handler: Box<T>) {
        let replaced = {
            let mut state = self.state.borrow_mut();
            state.generation = state
                .generation
                .checked_add(1)
                .expect("native application handler generation overflowed");
            state.handler.replace(handler)
        };
        drop(replaced);
    }

    fn checkout(&self) -> Option<NativeAppHandlerCheckout<'_, T>> {
        let mut state = self.state.borrow_mut();
        let generation = state.generation;
        let handler = state.handler.take()?;
        Some(NativeAppHandlerCheckout {
            slot: self,
            generation,
            handler: Some(handler),
        })
    }
}

struct NativeAppHandlerState<T: ?Sized> {
    generation: u64,
    handler: Option<Box<T>>,
}

struct NativeAppHandlerCheckout<'a, T: ?Sized> {
    slot: &'a NativeAppHandlerSlot<T>,
    generation: u64,
    handler: Option<Box<T>>,
}

impl<T: ?Sized> NativeAppHandlerCheckout<'_, T> {
    fn callback_mut(&mut self) -> &mut T {
        self.handler
            .as_deref_mut()
            .expect("checked-out native application handler must remain present")
    }
}

impl<T: ?Sized> Drop for NativeAppHandlerCheckout<'_, T> {
    fn drop(&mut self) {
        let mut handler = self.handler.take();
        {
            let mut state = self.slot.state.borrow_mut();
            if state.generation == self.generation && state.handler.is_none() {
                state.handler = handler.take();
            }
        }
        drop(handler);
    }
}

pub(crate) struct NativeCallbackLease {
    app: Option<Rc<AppCell>>,
    kind: NativeCallbackLeaseKind,
    generation: u64,
}

#[derive(Clone, Copy)]
enum NativeCallbackLeaseKind {
    PlatformInput,
    InputHandler,
}

impl NativeCallbackLeaseKind {
    fn stack(self, app: &AppCell) -> &RefCell<Vec<u64>> {
        match self {
            Self::PlatformInput => &app.platform_input_leases,
            Self::InputHandler => &app.input_handler_leases,
        }
    }
}

impl Drop for NativeCallbackLease {
    fn drop(&mut self) {
        let Some(app) = self.app.take() else {
            return;
        };
        let generation = self
            .kind
            .stack(&app)
            .borrow_mut()
            .pop()
            .expect("native callback lease stack underflowed");
        assert_eq!(
            generation, self.generation,
            "native callback leases must retire in stack order"
        );
        if !app.native_callback_lease_active() {
            if std::thread::panicking() {
                app.native_events.schedule_drain_after_unwind();
            } else {
                app.app_borrow_released();
            }
        }
    }
}

#[doc(hidden)]
pub struct AppRef<'a> {
    app: Option<Ref<'a, App>>,
    cell: &'a AppCell,
}

impl Deref for AppRef<'_> {
    type Target = App;

    fn deref(&self) -> &Self::Target {
        self.app
            .as_deref()
            .expect("AppRef cannot be accessed while it is being dropped")
    }
}

impl Drop for AppRef<'_> {
    fn drop(&mut self) {
        drop(self.app.take());
        if std::thread::panicking() {
            self.cell.native_events.schedule_drain_after_unwind();
        } else {
            self.cell.app_borrow_released();
        }
        if option_env!("TRACK_THREAD_BORROWS").is_some() {
            let thread_id = std::thread::current().id();
            eprintln!("dropped borrow from {thread_id:?}");
        }
    }
}

#[doc(hidden)]
pub struct AppRefMut<'a> {
    app: Option<RefMut<'a, App>>,
    cell: &'a AppCell,
}

impl Deref for AppRefMut<'_> {
    type Target = App;

    fn deref(&self) -> &Self::Target {
        self.app
            .as_deref()
            .expect("AppRefMut cannot be accessed while it is being dropped")
    }
}

impl DerefMut for AppRefMut<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.app
            .as_deref_mut()
            .expect("AppRefMut cannot be accessed while it is being dropped")
    }
}

impl Drop for AppRefMut<'_> {
    fn drop(&mut self) {
        drop(self.app.take());
        if std::thread::panicking() {
            self.cell.native_events.schedule_drain_after_unwind();
        } else {
            self.cell.app_borrow_released();
        }
        if option_env!("TRACK_THREAD_BORROWS").is_some() {
            let thread_id = std::thread::current().id();
            eprintln!("dropped {thread_id:?}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AnyWindowHandle, AppContext, Empty, QuitMode, TestAppContext, WindowOptions, px, size,
    };

    fn enqueue_persistent_panicking_shutdown_effect(attempts: Rc<Cell<usize>>, app: &mut App) {
        app.defer(move |app| {
            attempts.set(attempts.get() + 1);
            enqueue_persistent_panicking_shutdown_effect(attempts.clone(), app);
            panic!("injected persistent shutdown effect panic");
        });
    }

    #[test]
    fn shutdown_effect_flush_settles_after_its_panic_budget() {
        let attempts = Cell::new(0usize);
        let mut first_panic = None;

        let terminal = settle_shutdown_effect_flush(&mut first_panic, || {
            let attempt = attempts.get() + 1;
            attempts.set(attempt);
            Err(Box::new(attempt) as Box<dyn std::any::Any + Send>)
        });

        assert_eq!(
            terminal,
            NativeShutdownEffectFlushTerminal::Failed {
                panic_count: SHUTDOWN_EFFECT_FLUSH_PANIC_BUDGET,
            }
        );
        assert_eq!(attempts.get(), SHUTDOWN_EFFECT_FLUSH_PANIC_BUDGET);
        assert_eq!(
            first_panic
                .as_deref()
                .and_then(|payload| payload.downcast_ref::<usize>()),
            Some(&1),
            "shutdown must preserve the first panic while bounding later failures"
        );
    }

    #[test]
    fn shutdown_effect_flush_drains_after_transient_panics() {
        let attempts = Cell::new(0usize);
        let mut first_panic = None;

        let terminal = settle_shutdown_effect_flush(&mut first_panic, || {
            let attempt = attempts.get() + 1;
            attempts.set(attempt);
            if attempt < 3 {
                Err(Box::new(attempt) as Box<dyn std::any::Any + Send>)
            } else {
                Ok(())
            }
        });

        assert_eq!(terminal, NativeShutdownEffectFlushTerminal::Drained);
        assert_eq!(attempts.get(), 3);
        assert_eq!(
            first_panic
                .as_deref()
                .and_then(|payload| payload.downcast_ref::<usize>()),
            Some(&1),
        );
    }

    #[crate::test]
    fn shutdown_continues_after_persistent_effect_panics(cx: &mut TestAppContext) {
        let _: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let attempts = Rc::new(Cell::new(0usize));
        cx.update({
            let attempts = attempts.clone();
            move |app| {
                app.on_app_quit(move |app| {
                    enqueue_persistent_panicking_shutdown_effect(attempts.clone(), app);
                    std::future::ready(())
                })
                .detach();
            }
        });

        let shutdown = catch_unwind(AssertUnwindSafe(|| cx.quit()));

        assert!(shutdown.is_err());
        assert_eq!(attempts.get(), SHUTDOWN_EFFECT_FLUSH_PANIC_BUDGET);
        assert!(
            cx.windows().is_empty(),
            "effect-flush failure must not prevent native window teardown"
        );
        cx.update(|app| {
            assert!(app.pending_effects.is_empty());
            app.defer(|_| {});
        });
    }

    #[crate::test]
    fn shutdown_started_inside_app_update_cannot_double_panic_during_borrow_drop(
        cx: &mut TestAppContext,
    ) {
        let _: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let attempts = Rc::new(Cell::new(0usize));
        cx.update({
            let attempts = attempts.clone();
            move |app| {
                app.on_app_quit(move |app| {
                    enqueue_persistent_panicking_shutdown_effect(attempts.clone(), app);
                    std::future::ready(())
                })
                .detach();
            }
        });

        let shutdown = catch_unwind(AssertUnwindSafe(|| {
            cx.update(|app| app.shutdown());
        }));

        assert!(shutdown.is_err());
        assert_eq!(attempts.get(), SHUTDOWN_EFFECT_FLUSH_PANIC_BUDGET);
        assert!(cx.windows().is_empty());
    }

    #[crate::test]
    fn shutdown_critical_phases_survive_ordinary_effect_abandonment(cx: &mut TestAppContext) {
        let _: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let attempts = Rc::new(Cell::new(0usize));
        let phases = Rc::new(RefCell::new(Vec::new()));
        cx.update({
            let attempts = attempts.clone();
            let phases = phases.clone();
            move |app| {
                app.on_app_quit(move |app| {
                    enqueue_persistent_panicking_shutdown_effect(attempts.clone(), app);
                    app.defer_shutdown_critical_before_window_registry_clear({
                        let phases = phases.clone();
                        move |app| {
                            assert!(
                                !app.windows().is_empty(),
                                "pre-clear critical work must observe the live registry"
                            );
                            phases.borrow_mut().push("before");
                        }
                    });
                    app.defer_shutdown_critical_after_window_registry_clear({
                        let phases = phases.clone();
                        move |app| {
                            assert!(
                                app.windows().is_empty(),
                                "post-clear critical work must observe an empty registry"
                            );
                            phases.borrow_mut().push("after");
                        }
                    });
                    std::future::ready(())
                })
                .detach();
            }
        });

        let shutdown = catch_unwind(AssertUnwindSafe(|| cx.quit()));

        assert!(shutdown.is_err());
        assert_eq!(attempts.get(), SHUTDOWN_EFFECT_FLUSH_PANIC_BUDGET);
        assert_eq!(phases.borrow().as_slice(), &["before", "after"]);
        assert!(cx.windows().is_empty());
    }

    #[crate::test]
    fn late_pre_clear_critical_work_runs_synchronously_after_registry_clear(
        cx: &mut TestAppContext,
    ) {
        let _: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let nested_ran = Rc::new(Cell::new(false));
        let observed_synchronously = Rc::new(Cell::new(false));
        cx.update({
            let nested_ran = nested_ran.clone();
            let observed_synchronously = observed_synchronously.clone();
            move |app| {
                app.on_app_quit(move |app| {
                    let observed_synchronously = observed_synchronously.clone();
                    app.defer_shutdown_critical_after_window_registry_clear({
                        let nested_ran = nested_ran.clone();
                        move |app| {
                            assert!(
                                app.windows().is_empty(),
                                "post-clear critical work must observe an empty registry"
                            );
                            app.defer_shutdown_critical_before_window_registry_clear_or_run_now({
                                let nested_ran = nested_ran.clone();
                                move |app| {
                                    assert!(
                                        app.windows().is_empty(),
                                        "late pre-clear work must run in the post-clear App turn"
                                    );
                                    nested_ran.set(true);
                                }
                            });
                            assert!(
                                nested_ran.get(),
                                "late pre-clear work must execute synchronously after phase passage"
                            );
                            observed_synchronously.set(true);
                        }
                    });
                    std::future::ready(())
                })
                .detach();
            }
        });

        cx.quit();

        assert!(nested_ran.get());
        assert!(observed_synchronously.get());
        assert!(cx.windows().is_empty());
    }

    #[crate::test]
    fn late_delayed_lifecycle_work_yields_to_a_new_post_clear_wave(cx: &mut TestAppContext) {
        let _: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let nested_ran = Rc::new(Cell::new(false));
        let outer_returned = Rc::new(Cell::new(false));
        cx.update({
            let nested_ran = nested_ran.clone();
            let outer_returned = outer_returned.clone();
            move |app| {
                app.on_app_quit(move |app| {
                    let outer_returned = outer_returned.clone();
                    app.defer_shutdown_critical_after_window_registry_clear({
                        let nested_ran = nested_ran.clone();
                        move |app| {
                            assert!(app.windows().is_empty());
                            app.defer_after_or_shutdown_critical_before_window_registry_clear(
                                Duration::ZERO,
                                {
                                    let nested_ran = nested_ran.clone();
                                    let outer_returned = outer_returned.clone();
                                    move |app| {
                                        assert!(app.windows().is_empty());
                                        assert!(
                                            outer_returned.get(),
                                            "phase-passed delayed work must yield the current callback stack"
                                        );
                                        nested_ran.set(true);
                                    }
                                },
                            );
                            assert!(
                                !nested_ran.get(),
                                "phase-passed delayed work must not run synchronously"
                            );
                            outer_returned.set(true);
                        }
                    });
                    std::future::ready(())
                })
                .detach();
            }
        });

        cx.quit();

        assert!(nested_ran.get());
        assert!(outer_returned.get());
        assert!(cx.windows().is_empty());
    }

    #[crate::test]
    fn pre_shutdown_critical_work_survives_later_effect_abandonment(cx: &mut TestAppContext) {
        let _: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let attempts = Rc::new(Cell::new(0usize));
        let critical_ran = Rc::new(Cell::new(false));

        let shutdown = catch_unwind(AssertUnwindSafe(|| {
            cx.update({
                let attempts = attempts.clone();
                let critical_ran = critical_ran.clone();
                move |app| {
                    for _ in 0..SHUTDOWN_EFFECT_FLUSH_PANIC_BUDGET {
                        app.defer({
                            let attempts = attempts.clone();
                            move |_| {
                                attempts.set(attempts.get() + 1);
                                panic!("injected pre-existing shutdown effect panic");
                            }
                        });
                    }
                    app.defer_shutdown_critical_before_window_registry_clear(move |app| {
                        assert!(
                            !app.windows().is_empty(),
                            "protected pre-shutdown work must run before registry clear"
                        );
                        critical_ran.set(true);
                    });
                    app.shutdown();
                }
            });
            cx.run_until_parked();
        }));

        assert!(shutdown.is_err());
        assert_eq!(attempts.get(), SHUTDOWN_EFFECT_FLUSH_PANIC_BUDGET);
        assert!(critical_ran.get());
        assert!(cx.windows().is_empty());
    }

    #[crate::test]
    fn delayed_shutdown_critical_work_waits_for_the_outer_app_borrow(cx: &mut TestAppContext) {
        let callback_ran = Rc::new(Cell::new(false));
        cx.update({
            let callback_ran = callback_ran.clone();
            move |app| {
                let callback_ran_after_borrow = callback_ran.clone();
                app.defer_after_or_shutdown_critical_before_window_registry_clear(
                    Duration::ZERO,
                    move |_| callback_ran_after_borrow.set(true),
                );
                app.background_executor.run_until_parked();
                assert!(
                    !callback_ran.get(),
                    "a foreground timer must not reborrow App while the outer update is active"
                );
            }
        });

        cx.run_until_parked();
        assert!(callback_ran.get());
    }

    #[crate::test]
    fn delayed_shutdown_critical_work_transfers_into_active_shutdown(cx: &mut TestAppContext) {
        let _: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let callback_ran = Rc::new(Cell::new(false));
        cx.update({
            let callback_ran = callback_ran.clone();
            move |app| {
                app.defer_after_or_shutdown_critical_before_window_registry_clear(
                    Duration::from_secs(60),
                    move |app| {
                        assert!(
                            !app.windows().is_empty(),
                            "transferred delayed work must run before registry clear"
                        );
                        callback_ran.set(true);
                    },
                );
                app.shutdown();
            }
        });
        cx.run_until_parked();

        assert!(callback_ran.get());
        assert!(cx.windows().is_empty());
    }

    #[crate::test]
    fn post_terminal_critical_work_gets_a_follow_up_effect_wave(cx: &mut TestAppContext) {
        let _: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let follow_up_effect_ran = Rc::new(Cell::new(false));
        cx.update({
            let follow_up_effect_ran = follow_up_effect_ran.clone();
            move |app| {
                app.on_app_quit(move |app| {
                    app.defer_shutdown_critical_after_window_registry_clear({
                        let follow_up_effect_ran = follow_up_effect_ran.clone();
                        move |app| {
                            app.defer(move |app| {
                                app.defer_shutdown_critical_after_window_registry_clear(
                                    move |app| {
                                        app.defer(move |_| follow_up_effect_ran.set(true));
                                    },
                                );
                            });
                        }
                    });
                    std::future::ready(())
                })
                .detach();
            }
        });

        cx.quit();
        cx.run_until_parked();

        assert!(
            follow_up_effect_ran.get(),
            "effects created by a post-terminal critical participant must flush before completion"
        );
    }

    #[crate::test]
    fn native_retirement_dependencies_hold_anchor_until_dependents_are_terminal(
        cx: &mut TestAppContext,
    ) {
        cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
        let anchor: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let dependent: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let anchor_platform = cx.test_window(anchor);
        let dependent_platform = cx.test_window(dependent);
        let anchor_terminal = cx.hold_window_native_terminal(anchor);
        let dependent_terminal = cx.hold_window_native_terminal(dependent);

        cx.update(|app| {
            app.register_native_window_retirement_dependencies(
                anchor.window_id(),
                [dependent.window_id()],
            )
            .expect("one anchor-to-dependent edge must be acyclic");
            anchor
                .update(app, |_, window, cx| window.remove_window(cx))
                .expect("the anchor should remain logically registered");
            dependent
                .update(app, |_, window, cx| window.remove_window(cx))
                .expect("the dependent should remain logically registered");
        });
        cx.run_until_parked();

        assert_eq!(dependent_platform.presentation_shutdown_counts().2, 1);
        assert_eq!(
            anchor_platform.presentation_shutdown_counts().2,
            0,
            "anchor retirement must wait for the dependent native terminal"
        );

        assert!(dependent_terminal.release());
        cx.run_until_parked();
        assert_eq!(anchor_platform.presentation_shutdown_counts().2, 1);
        assert!(anchor_terminal.release());
        cx.run_until_parked();
        assert!(cx.app.native_window_retirement_barriers_are_clear());
    }

    #[crate::test]
    fn cancelling_native_retirement_dependencies_dispatches_a_waiting_anchor(
        cx: &mut TestAppContext,
    ) {
        cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
        let anchor: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let dependent: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let anchor_platform = cx.test_window(anchor);
        let anchor_terminal = cx.hold_window_native_terminal(anchor);
        let dependent_terminal = cx.hold_window_native_terminal(dependent);

        cx.update(|app| {
            app.register_native_window_retirement_dependencies(
                anchor.window_id(),
                [dependent.window_id()],
            )
            .expect("one anchor-to-dependent edge must be acyclic");
            anchor
                .update(app, |_, window, cx| window.remove_window(cx))
                .expect("the anchor should remain logically registered");
        });
        cx.run_until_parked();
        assert_eq!(
            anchor_platform.presentation_shutdown_counts().2,
            0,
            "the anchor must first enter dependency-blocked retirement"
        );

        cx.update(|app| {
            assert!(app.cancel_native_window_retirement_dependencies(anchor.window_id()));
            assert!(
                !app.cancel_native_window_retirement_dependencies(anchor.window_id()),
                "the exact dependency group must only be cancelled once"
            );
        });
        cx.run_until_parked();
        assert_eq!(
            anchor_platform.presentation_shutdown_counts().2,
            1,
            "cancellation must transfer the retained anchor owner to native retirement"
        );
        assert!(anchor_terminal.release());
        cx.run_until_parked();

        cx.update(|app| {
            dependent
                .update(app, |_, window, cx| window.remove_window(cx))
                .expect("the dependent should remain logically registered");
        });
        cx.run_until_parked();
        assert!(dependent_terminal.release());
        cx.run_until_parked();
        assert!(cx.app.native_window_retirement_barriers_are_clear());
    }

    #[crate::test]
    fn native_retirement_dependency_cycles_are_rejected(cx: &mut TestAppContext) {
        cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
        let anchor: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let dependent: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();

        cx.update(|app| {
            app.register_native_window_retirement_dependencies(
                anchor.window_id(),
                [dependent.window_id()],
            )
            .expect("the first dependency edge must be admitted");
            assert_eq!(
                app.register_native_window_retirement_dependencies(
                    dependent.window_id(),
                    [anchor.window_id()],
                ),
                Err(crate::NativeWindowRetirementDependencyError::Cycle {
                    anchor: dependent.window_id(),
                    dependency: anchor.window_id(),
                })
            );
            anchor
                .update(app, |_, window, cx| window.remove_window(cx))
                .expect("the anchor should remain logically registered");
            dependent
                .update(app, |_, window, cx| window.remove_window(cx))
                .expect("the dependent should remain logically registered");
        });
        cx.run_until_parked();

        assert!(cx.app.native_window_retirement_barriers_are_clear());
    }

    #[crate::test]
    fn native_retirement_dependencies_reject_unknown_identities_atomically(
        cx: &mut TestAppContext,
    ) {
        cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
        let anchor: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let dependent: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let anchor_platform = cx.test_window(anchor);
        let anchor_terminal = cx.hold_window_native_terminal(anchor);
        let dependent_terminal = cx.hold_window_native_terminal(dependent);
        let unknown_anchor = WindowId::from((9_001_u64 << 32) | 1);
        let unknown_dependency = WindowId::from((9_002_u64 << 32) | 1);
        let rolled_back_dependency = WindowId::from((9_003_u64 << 32) | 1);
        cx.app.reserve_native_window(rolled_back_dependency);
        cx.app.remove_native_window(rolled_back_dependency);

        cx.update(|app| {
            assert_eq!(
                app.register_native_window_retirement_dependencies(
                    unknown_anchor,
                    [dependent.window_id()],
                ),
                Err(
                    crate::NativeWindowRetirementDependencyError::UnknownAnchor {
                        anchor: unknown_anchor,
                    }
                )
            );
            assert_eq!(
                app.register_native_window_retirement_dependencies(
                    anchor.window_id(),
                    [rolled_back_dependency],
                ),
                Err(
                    crate::NativeWindowRetirementDependencyError::UnknownDependency {
                        anchor: anchor.window_id(),
                        dependency: rolled_back_dependency,
                    }
                )
            );
            assert_eq!(
                app.register_native_window_retirement_dependencies(
                    anchor.window_id(),
                    [dependent.window_id(), unknown_dependency],
                ),
                Err(
                    crate::NativeWindowRetirementDependencyError::UnknownDependency {
                        anchor: anchor.window_id(),
                        dependency: unknown_dependency,
                    }
                )
            );
            anchor
                .update(app, |_, window, cx| window.remove_window(cx))
                .expect("the anchor should remain logically registered");
            dependent
                .update(app, |_, window, cx| window.remove_window(cx))
                .expect("the dependent should remain logically registered");
        });
        cx.run_until_parked();

        assert_eq!(
            anchor_platform.presentation_shutdown_counts().2,
            1,
            "a rejected dependency batch must not partially retain the anchor"
        );
        assert!(dependent_terminal.release());
        assert!(anchor_terminal.release());
        cx.run_until_parked();
        assert!(cx.app.native_window_retirement_barriers_are_clear());
    }

    #[crate::test]
    fn nonterminating_shutdown_keeps_window_open_barrier_through_retirement_retry(
        cx: &mut TestAppContext,
    ) {
        cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
        let window: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let platform_window = cx.test_window(window);
        platform_window.reject_native_retirement_attempts(2);

        cx.update(|app| app.shutdown());
        cx.run_until_parked();

        let blocked = cx.update(|app| {
            app.open_window_detailed(WindowOptions::default(), |_, app| app.new(|_| Empty))
        });
        assert_eq!(
            blocked
                .expect_err("a retry-pending native owner must retain the window-open barrier")
                .stage(),
            crate::WindowOpenFailureStage::AppShutdown
        );
        assert_eq!(platform_window.presentation_shutdown_counts().2, 0);

        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(8));
        cx.run_until_parked();

        assert_eq!(platform_window.presentation_shutdown_counts().2, 1);
        let replacement: AnyWindowHandle = cx
            .update(|app| app.open_window(WindowOptions::default(), |_, app| app.new(|_| Empty)))
            .expect("reopen must wait for exact native retirement terminal")
            .into();
        assert!(replacement.update(cx, |_, _, _| ()).is_ok());
    }

    #[crate::test]
    fn terminating_shutdown_waits_for_queued_capture_release_completion(cx: &mut TestAppContext) {
        let source: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let platform_window = cx.test_window(source);
        let generation = cx.update(|app| {
            app.reserve_native_captured_drag_start()
                .token()
                .generation()
        });
        let completions = Rc::new(RefCell::new(Vec::new()));
        let (token, barrier) = cx.app.reserve_native_captured_drag_release(
            source.window_id(),
            generation,
            Box::new({
                let completions = completions.clone();
                move |barrier, terminal, _| {
                    completions.borrow_mut().push((barrier, terminal));
                }
            }),
        );

        let mut app = cx.app.borrow_mut();
        cx.app.settle_native_pointer_capture_release(
            token,
            platform_window.command_dispatcher(),
            true,
        );
        app.shutdown_from_native_quit();
        drop(app);
        cx.run_until_parked();

        assert_eq!(
            completions.borrow().as_slice(),
            &[(barrier, NativeCapturedDragReleaseTerminal::Released)],
            "terminating shutdown must not discard a release continuation queued behind its final native attempt"
        );
        assert!(cx.app.pointer_capture_release_barriers_are_clear());
        assert!(cx.windows().is_empty());
    }

    fn assert_shutdown_flushes_effects_created_by_release_completion(
        cx: &mut TestAppContext,
        terminate_ingress: bool,
    ) {
        let source: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let platform_window = cx.test_window(source);
        let generation = cx.update(|app| {
            app.reserve_native_captured_drag_start()
                .token()
                .generation()
        });
        let deferred_effect_ran = Rc::new(Cell::new(false));
        let (token, _) = cx.app.reserve_native_captured_drag_release(
            source.window_id(),
            generation,
            Box::new({
                let deferred_effect_ran = deferred_effect_ran.clone();
                move |_, terminal, app| {
                    assert_eq!(terminal, NativeCapturedDragReleaseTerminal::Released);
                    app.defer(move |_| deferred_effect_ran.set(true));
                }
            }),
        );

        let mut app = cx.app.borrow_mut();
        cx.app.settle_native_pointer_capture_release(
            token,
            platform_window.command_dispatcher(),
            true,
        );
        if terminate_ingress {
            app.shutdown_from_native_quit();
        } else {
            app.shutdown();
        }
        drop(app);
        cx.run_until_parked();

        assert!(
            deferred_effect_ran.get(),
            "shutdown must reach effect quiescence after release continuations settle"
        );
        assert!(cx.app.borrow().pending_effects.is_empty());
        assert!(cx.app.pointer_capture_release_barriers_are_clear());
        assert!(cx.windows().is_empty());
    }

    #[crate::test]
    fn terminating_shutdown_flushes_effects_created_by_release_completion(cx: &mut TestAppContext) {
        assert_shutdown_flushes_effects_created_by_release_completion(cx, true);
    }

    #[crate::test]
    fn non_terminating_shutdown_flushes_effects_created_by_release_completion(
        cx: &mut TestAppContext,
    ) {
        assert_shutdown_flushes_effects_created_by_release_completion(cx, false);
    }
}
