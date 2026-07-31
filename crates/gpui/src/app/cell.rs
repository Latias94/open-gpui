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
    PlatformPointerCaptureReleaseOutcome, PlatformWindowCommand, PlatformWindowCommandDispatcher,
    PlatformWindowCommandOutcome, PointerCancelReason, WindowControlArea, WindowId,
};

type OpenUrlsHandler = dyn FnMut(Vec<String>, &mut App);
type AppHandler = dyn FnMut(&mut App);

const POINTER_CAPTURE_RELEASE_RETRY_DELAYS: [Duration; 3] = [
    Duration::from_millis(8),
    Duration::from_millis(32),
    Duration::from_millis(128),
];

fn retain_shutdown_panic(
    first: &mut Option<Box<dyn std::any::Any + Send>>,
    candidate: Option<Box<dyn std::any::Any + Send>>,
) {
    if first.is_none() {
        *first = candidate;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativePointerCaptureReleaseState {
    AwaitingLogicalTerminal,
    Queued,
    RetryPending,
    AwaitingNativeWindowTerminal,
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
    Queued,
    RetryPending,
    AwaitingNativeTerminal,
}

struct NativeShutdownFence {
    generation: u64,
    terminate_ingress: bool,
    preparation_complete: bool,
    registry_cleared: bool,
    was_quitting: bool,
    first_panic: Option<Box<dyn std::any::Any + Send>>,
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
    native_window_retirements: RefCell<HashMap<WindowId, NativeWindowRetirementState>>,
    observed_native_window_terminals: RefCell<HashSet<WindowId>>,
    next_shutdown_generation: Cell<u64>,
    shutdown_fence: RefCell<Option<NativeShutdownFence>>,
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
            observed_native_window_terminals: RefCell::new(HashSet::new()),
            next_shutdown_generation: Cell::new(0),
            shutdown_fence: RefCell::new(None),
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
        terminal: NativeCapturedDragReleaseTerminal,
        continuations: Vec<NativeCapturedDragReleaseContinuation>,
    ) {
        if continuations.is_empty() {
            return;
        }
        self.native_events.enqueue_captured_drag_release_completion(
            NativeCapturedDragReleaseCompletion::new(barrier, terminal, continuations),
        );
    }

    fn complete_native_pointer_capture_release(
        &self,
        token: NativePointerCaptureReleaseToken,
        terminal: NativeCapturedDragReleaseTerminal,
    ) {
        let continuations = {
            let mut barriers = self.pointer_capture_releases.borrow_mut();
            let Some(barrier) = barriers.get(&token.release_generation()) else {
                return;
            };
            if barrier.token != token {
                return;
            }
            barriers
                .remove(&token.release_generation())
                .expect("matching pointer-capture release barrier must remain present")
                .continuations
        };
        self.pointer_capture_release_retries
            .borrow_mut()
            .retain(|release| release.token() != token);
        if let Some(barrier) = NativeCapturedDragReleaseBarrier::from_release_token(token) {
            self.enqueue_native_captured_drag_release_completion(barrier, terminal, continuations);
        }
        self.request_active_shutdown_completion();
    }

    fn complete_native_pointer_capture_releases_for_native_window_terminal(
        &self,
        window_id: WindowId,
    ) {
        let released = {
            let mut barriers = self.pointer_capture_releases.borrow_mut();
            let release_generations = barriers
                .iter()
                .filter_map(|(release_generation, barrier)| {
                    (barrier.token.window_id() == window_id).then_some(*release_generation)
                })
                .collect::<Vec<_>>();
            release_generations
                .into_iter()
                .filter_map(|release_generation| barriers.remove(&release_generation))
                .collect::<Vec<_>>()
        };
        self.pointer_capture_release_retries
            .borrow_mut()
            .retain(|release| release.token().window_id() != window_id);
        for barrier in released {
            if let Some(release_barrier) =
                NativeCapturedDragReleaseBarrier::from_release_token(barrier.token)
            {
                self.enqueue_native_captured_drag_release_completion(
                    release_barrier,
                    NativeCapturedDragReleaseTerminal::NativeWindowTerminal,
                    barrier.continuations,
                );
            }
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
            self.enqueue_native_captured_drag_release_completion(
                barrier,
                NativeCapturedDragReleaseTerminal::NativeWindowTerminal,
                vec![continuation],
            );
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
        barrier.state = NativePointerCaptureReleaseState::RetryPending;
        let retry_delay = POINTER_CAPTURE_RELEASE_RETRY_DELAYS
            .get(usize::from(barrier.retry_attempts))
            .copied();
        barrier.retry_attempts = barrier.retry_attempts.saturating_add(1);
        let retry_epoch = barrier.retry_attempts;
        drop(barriers);
        self.pointer_capture_release_retries
            .borrow_mut()
            .push_back(release);
        if let Some(retry_delay) = retry_delay {
            self.schedule_pointer_capture_release_retry(retry_delay, token, retry_epoch);
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
        let previous = self
            .native_window_retirements
            .borrow_mut()
            .insert(window_id, NativeWindowRetirementState::Queued);
        debug_assert!(previous.is_none(), "native window retirement queued twice");
        self.native_events
            .enqueue_window_retirement(NativeWindowRetirement::new(window_id, window));
    }

    fn defer_native_window_retirement_retry(&self, mut retirement: NativeWindowRetirement) {
        let window_id = retirement.window_id();
        let retry_pending = self
            .native_window_retirements
            .borrow_mut()
            .get_mut(&window_id)
            .is_some_and(|state| {
                if *state != NativeWindowRetirementState::AwaitingNativeTerminal {
                    return false;
                }
                *state = NativeWindowRetirementState::RetryPending;
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
        if self.native_window_terminal_was_observed(window_id) {
            self.settle_native_window_terminal(window_id);
            return;
        }
        let dispatchable = self
            .native_window_retirements
            .borrow_mut()
            .get_mut(&window_id)
            .is_some_and(|state| {
                if *state != NativeWindowRetirementState::RetryPending {
                    return false;
                }
                *state = NativeWindowRetirementState::Queued;
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
        self.native_window_retirements
            .borrow_mut()
            .remove(&window_id);
        self.complete_native_pointer_capture_releases_for_native_window_terminal(window_id);
    }

    fn pointer_capture_release_barriers_are_clear(&self) -> bool {
        self.pointer_capture_releases.borrow().is_empty()
    }

    fn native_window_retirement_barriers_are_clear(&self) -> bool {
        self.native_window_retirements.borrow().is_empty()
    }

    pub(super) fn begin_shutdown_fence(
        &self,
        terminate_ingress: bool,
        was_quitting: bool,
    ) -> (u64, bool) {
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
        *fence = Some(NativeShutdownFence {
            generation,
            terminate_ingress,
            preparation_complete: false,
            registry_cleared: false,
            was_quitting,
            first_panic: None,
        });
        (generation, true)
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

        if !fence.registry_cleared {
            let Ok(mut app) = self.app.try_borrow_mut() else {
                *self.shutdown_fence.borrow_mut() = Some(fence);
                return NativeShutdownCompletionAction::Retry;
            };
            if super::window_registry::has_checked_out_window(&app) {
                drop(app);
                *self.shutdown_fence.borrow_mut() = Some(fence);
                return NativeShutdownCompletionAction::Retry;
            }

            let window_cleanup = catch_unwind(AssertUnwindSafe(|| {
                app.prepare_shutdown_pointer_sessions();
            }));
            retain_shutdown_panic(&mut fence.first_panic, window_cleanup.err());

            loop {
                match catch_unwind(AssertUnwindSafe(|| app.flush_effects())) {
                    Ok(()) => break,
                    Err(payload) => retain_shutdown_panic(&mut fence.first_panic, Some(payload)),
                }
            }

            let clear = catch_unwind(AssertUnwindSafe(|| {
                super::window_registry::clear(&mut app);
            }));
            retain_shutdown_panic(&mut fence.first_panic, clear.err());
            fence.registry_cleared = true;
            drop(app);
        }

        if !self.pointer_capture_release_barriers_are_clear() {
            *self.shutdown_fence.borrow_mut() = Some(fence);
            return NativeShutdownCompletionAction::Retry;
        }

        if fence.terminate_ingress && !self.native_window_retirement_barriers_are_clear() {
            *self.shutdown_fence.borrow_mut() = Some(fence);
            return NativeShutdownCompletionAction::Retry;
        }

        let Ok(mut app) = self.app.try_borrow_mut() else {
            *self.shutdown_fence.borrow_mut() = Some(fence);
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
            let (should_close, disposition) = if app.quitting {
                (true, NativeBoundaryDisposition::Closed)
            } else {
                let should_close = terminal.run_callback(|| {
                    app.update(|app| {
                        app.update_window_id_from_native(
                            window_id,
                            ingress_sequence,
                            |_, window, cx| window.should_close(cx),
                        )
                        .unwrap_or(false)
                    })
                });
                (should_close, NativeBoundaryDisposition::DELIVERED)
            };
            drop(app);
            terminal.run_callback(|| self.drain_native_captured_drags());
            terminal.settle(disposition);
            log::trace!(
                "native event sequence={sequence} window={window_id:?} disposition=DeliveredInline"
            );
            self.drain_native_work(None);
            return should_close;
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
        if self.native_callback_lease_active() || !self.app_is_idle() {
            return;
        }
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
                            let dispatchable =
                                !quitting && self.native_queries.committed(window_id).is_some();
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
                                        terminal.settle(NativeBoundaryDisposition::DELIVERED);
                                        if completes_initial_presentation {
                                            self.enqueue_native_window_event(
                                                window_id,
                                                NativeWindowEvent::InitialPresentationCompleted,
                                            );
                                        }
                                    }
                                    PlatformWindowCommandOutcome::Rejected => {
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
                            terminal.run_callback(|| self.drain_native_captured_drags());
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
                                drain.block_on_app();
                                return;
                            };
                            let (barrier, release_terminal, continuations) =
                                completion.into_parts();
                            let mut first_panic = None;
                            for continuation in continuations {
                                if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
                                    app.update(|app| continuation(barrier, release_terminal, app));
                                })) {
                                    retain_shutdown_panic(&mut first_panic, Some(payload));
                                }
                            }
                            drop(app);
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
                                .is_some_and(|state| *state == NativeWindowRetirementState::Queued);
                            if !dispatchable {
                                terminal.settle(NativeBoundaryDisposition::Stale);
                                continue;
                            }
                            self.native_window_retirements.borrow_mut().insert(
                                window_id,
                                NativeWindowRetirementState::AwaitingNativeTerminal,
                            );
                            let attempt = catch_unwind(AssertUnwindSafe(|| retirement.retire()));
                            match attempt {
                                Ok(NativeWindowRetirementAttempt::Accepted) => {
                                    if self.native_window_terminal_was_observed(window_id) {
                                        self.native_window_retirements
                                            .borrow_mut()
                                            .remove(&window_id);
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
        self.cell.app_borrow_released();
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
        self.cell.app_borrow_released();
        if option_env!("TRACK_THREAD_BORROWS").is_some() {
            let thread_id = std::thread::current().id();
            eprintln!("dropped {thread_id:?}");
        }
    }
}
