use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::{Rc, Weak},
    sync::Arc,
};

use parking_lot::Mutex;

#[cfg(any(test, feature = "test-support"))]
use super::native_callback_diagnostics::{
    NativeBoundaryDiagnosticCursor, NativeBoundaryDiagnosticsSnapshot,
};
use super::{
    AppCell,
    native_callback_diagnostics::{
        NativeBoundaryDiagnostic, NativeBoundaryDiagnostics, NativeBoundaryDisposition,
        NativeBoundaryGeneration, NativeBoundaryKind, NativeBoundaryTarget, NativeCallbackKind,
    },
    native_captured_drag::{
        NativeCapturedDragGeneration, NativeCapturedDragReleaseCompletion, NativeIngressSequence,
    },
    native_platform_commands::{
        NativePlatformCommand, NativePointerCaptureRelease, NativePointerCaptureReleaseToken,
        NativeShutdownCompletion, NativeWindowRetirement,
    },
};
use crate::{
    Action, App, ForegroundExecutor, ModifiersChangedEvent, PlatformInput,
    PlatformWindowActiveStatusObservation, PlatformWindowCommand, PlatformWindowCommandDispatcher,
    PlatformWindowMutationObservation, PointerCancelEvent, PointerCancelReason,
    RequestFrameOptions, SystemNotificationResponse, SystemWindowTabController, Task, Window,
    WindowId,
};

pub(super) const MAX_NATIVE_WORK_PER_DRAIN: u8 = 64;

// Callback threads and foreground reservations share this authority so sequence assignment is
// linearizable without moving App-bound work across threads.
struct NativeSequenceAuthority {
    next_sequence: u64,
    terminated: bool,
    shutdown_generation: Option<u64>,
    staged_cross_thread: VecDeque<StagedNativeEnvelope>,
}

impl NativeSequenceAuthority {
    fn new() -> Self {
        Self {
            next_sequence: 0,
            terminated: false,
            shutdown_generation: None,
            staged_cross_thread: VecDeque::new(),
        }
    }

    fn reserve_sequence(&mut self) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence = sequence
            .checked_add(1)
            .expect("native work ingress sequence overflowed");
        sequence
    }

    fn take_staged_before_reserving(&mut self) -> (VecDeque<StagedNativeEnvelope>, u64) {
        let staged = std::mem::take(&mut self.staged_cross_thread);
        let sequence = self.reserve_sequence();
        (staged, sequence)
    }

    fn take_staged(&mut self) -> VecDeque<StagedNativeEnvelope> {
        std::mem::take(&mut self.staged_cross_thread)
    }

    fn terminate(&mut self) -> VecDeque<StagedNativeEnvelope> {
        self.terminated = true;
        std::mem::take(&mut self.staged_cross_thread)
    }
}

#[derive(Clone)]
pub(crate) struct NativeCrossThreadIngress {
    sequencer: Arc<Mutex<NativeSequenceAuthority>>,
    wake_sender: async_channel::Sender<()>,
}

impl NativeCrossThreadIngress {
    fn new(
        sequencer: Arc<Mutex<NativeSequenceAuthority>>,
        wake_sender: async_channel::Sender<()>,
    ) -> Self {
        Self {
            sequencer,
            wake_sender,
        }
    }

    pub(crate) fn system_wake(&self) {
        self.stage_app_event(StagedNativeAppEvent::SystemWake);
    }

    pub(crate) fn system_sleep(&self) {
        self.stage_app_event(StagedNativeAppEvent::SystemSleep);
    }

    pub(crate) fn thermal_state_changed(&self) {
        self.stage_app_event(StagedNativeAppEvent::ThermalStateChanged);
    }

    pub(crate) fn power_mode_changed(&self) {
        self.stage_app_event(StagedNativeAppEvent::PowerModeChanged);
    }

    fn stage_app_event(&self, event: StagedNativeAppEvent) {
        {
            let mut sequencer = self.sequencer.lock();
            if sequencer.terminated {
                return;
            }
            let sequence = sequencer.reserve_sequence();
            sequencer
                .staged_cross_thread
                .push_back(StagedNativeEnvelope {
                    sequence,
                    event: StagedNativeEvent::App(event),
                });
        }
        self.wake_importer();
    }

    #[cfg(not(target_family = "wasm"))]
    pub(crate) fn activated(
        &self,
        window_id: WindowId,
        active_state: &std::sync::atomic::AtomicU64,
    ) -> bool {
        self.stage_active_state(window_id, active_state, true)
    }

    #[cfg(not(target_family = "wasm"))]
    pub(crate) fn deactivated(
        &self,
        window_id: WindowId,
        active_state: &std::sync::atomic::AtomicU64,
    ) {
        self.stage_active_state(window_id, active_state, false);
    }

    #[cfg(not(target_family = "wasm"))]
    fn stage_active_state(
        &self,
        window_id: WindowId,
        active_state: &std::sync::atomic::AtomicU64,
        active: bool,
    ) -> bool {
        {
            let mut sequencer = self.sequencer.lock();
            if sequencer.terminated || (active && sequencer.shutdown_generation.is_some()) {
                return false;
            }
            let sequence = sequencer.reserve_sequence();
            crate::window::a11y::set_requested_active(active_state, active);
            let activation_generation = crate::window::a11y::requested_generation(active_state);
            sequencer
                .staged_cross_thread
                .push_back(StagedNativeEnvelope {
                    sequence,
                    event: StagedNativeEvent::Accessibility {
                        window_id,
                        event: StagedNativeAccessibilityEvent::ActiveChanged {
                            active,
                            activation_generation,
                        },
                    },
                });
        }
        self.wake_importer();
        true
    }

    #[cfg(not(target_family = "wasm"))]
    pub(crate) fn action(
        &self,
        window_id: WindowId,
        active_state: &std::sync::atomic::AtomicU64,
        request: accesskit::ActionRequest,
    ) {
        {
            let mut sequencer = self.sequencer.lock();
            if sequencer.terminated {
                return;
            }
            let sequence = sequencer.reserve_sequence();
            let activation_generation = crate::window::a11y::requested_generation(active_state);
            sequencer
                .staged_cross_thread
                .push_back(StagedNativeEnvelope {
                    sequence,
                    event: StagedNativeEvent::Accessibility {
                        window_id,
                        event: StagedNativeAccessibilityEvent::Action {
                            activation_generation,
                            request,
                        },
                    },
                });
        }
        self.wake_importer();
    }

    fn wake_importer(&self) {
        // Capacity one coalesces wakeups, while the sequencer retains every callback payload.
        let _ = self.wake_sender.try_send(());
    }
}

struct StagedNativeEnvelope {
    sequence: u64,
    event: StagedNativeEvent,
}

enum StagedNativeEvent {
    App(StagedNativeAppEvent),
    #[cfg(not(target_family = "wasm"))]
    Accessibility {
        window_id: WindowId,
        event: StagedNativeAccessibilityEvent,
    },
}

enum StagedNativeAppEvent {
    SystemSleep,
    SystemWake,
    ThermalStateChanged,
    PowerModeChanged,
}

#[cfg(not(target_family = "wasm"))]
enum StagedNativeAccessibilityEvent {
    ActiveChanged {
        active: bool,
        activation_generation: u64,
    },
    Action {
        activation_generation: u64,
        request: accesskit::ActionRequest,
    },
}

impl StagedNativeEnvelope {
    fn into_native_event(self) -> NativeEventEnvelope {
        let target = match self.event {
            StagedNativeEvent::App(StagedNativeAppEvent::SystemSleep) => {
                NativeEventTarget::App(NativeAppEvent::SystemSleep)
            }
            StagedNativeEvent::App(StagedNativeAppEvent::SystemWake) => {
                NativeEventTarget::App(NativeAppEvent::SystemWake)
            }
            StagedNativeEvent::App(StagedNativeAppEvent::ThermalStateChanged) => {
                NativeEventTarget::App(NativeAppEvent::ThermalStateChanged)
            }
            StagedNativeEvent::App(StagedNativeAppEvent::PowerModeChanged) => {
                NativeEventTarget::App(NativeAppEvent::PowerModeChanged)
            }
            #[cfg(not(target_family = "wasm"))]
            StagedNativeEvent::Accessibility { window_id, event } => {
                let event = match event {
                    StagedNativeAccessibilityEvent::ActiveChanged {
                        active,
                        activation_generation,
                    } => NativeWindowEvent::AccessibilityActiveChanged {
                        active,
                        activation_generation,
                    },
                    StagedNativeAccessibilityEvent::Action {
                        activation_generation,
                        request,
                    } => NativeWindowEvent::AccessibilityAction {
                        activation_generation,
                        request,
                    },
                };
                NativeEventTarget::Window { window_id, event }
            }
        };
        NativeEventEnvelope {
            sequence: self.sequence,
            target,
        }
    }

    fn pending_diagnostic(&self) -> NativeBoundaryDiagnostic {
        match &self.event {
            StagedNativeEvent::App(event) => {
                let kind = match event {
                    StagedNativeAppEvent::SystemSleep => NativeCallbackKind::SystemSleep,
                    StagedNativeAppEvent::SystemWake => NativeCallbackKind::SystemWake,
                    StagedNativeAppEvent::ThermalStateChanged => {
                        NativeCallbackKind::ThermalStateChanged
                    }
                    StagedNativeAppEvent::PowerModeChanged => NativeCallbackKind::PowerModeChanged,
                };
                NativeBoundaryDiagnostic::pending(
                    self.sequence,
                    NativeBoundaryTarget::Application,
                    NativeBoundaryKind::Callback(kind),
                    None,
                )
            }
            #[cfg(not(target_family = "wasm"))]
            StagedNativeEvent::Accessibility { window_id, event } => {
                let (kind, generation) = match event {
                    StagedNativeAccessibilityEvent::ActiveChanged {
                        active,
                        activation_generation,
                    } => (
                        if *active {
                            NativeCallbackKind::AccessibilityActivated
                        } else {
                            NativeCallbackKind::AccessibilityDeactivated
                        },
                        *activation_generation,
                    ),
                    StagedNativeAccessibilityEvent::Action {
                        activation_generation,
                        ..
                    } => (
                        NativeCallbackKind::AccessibilityAction,
                        *activation_generation,
                    ),
                };
                NativeBoundaryDiagnostic::pending(
                    self.sequence,
                    NativeBoundaryTarget::Window(*window_id),
                    NativeBoundaryKind::Callback(kind),
                    Some(NativeBoundaryGeneration::AccessibilityActivation(
                        generation,
                    )),
                )
            }
        }
    }
}

pub(super) struct NativeEventIngress {
    sequencer: Arc<Mutex<NativeSequenceAuthority>>,
    pending: RefCell<VecDeque<NativeWorkEnvelope>>,
    phase: Cell<NativeWorkPhase>,
    next_drain_generation: Cell<u64>,
    next_wake_ticket: Cell<u64>,
    scheduled_wake: Cell<Option<u64>>,
    unwind_recovery_scheduled: Cell<bool>,
    post_borrow_convergence_scheduled: Cell<bool>,
    terminated: Cell<bool>,
    delayed_wake_tasks: RefCell<Vec<NativeOwnedLocalTask>>,
    cross_thread_wake_sender: async_channel::Sender<()>,
    cross_thread_wake_task: RefCell<Option<NativeOwnedLocalTask>>,
    owned_local_authority_count: Rc<Cell<usize>>,
    foreground_executor: ForegroundExecutor,
    app: Weak<AppCell>,
    diagnostics: NativeBoundaryDiagnostics,
}

impl NativeEventIngress {
    pub(super) fn new(foreground_executor: ForegroundExecutor, app: Weak<AppCell>) -> Self {
        let sequencer = Arc::new(Mutex::new(NativeSequenceAuthority::new()));
        let owned_local_authority_count = Rc::new(Cell::new(0));
        let (cross_thread_wake_sender, cross_thread_wake_receiver) =
            async_channel::bounded::<()>(1);
        let cross_thread_wake_task = {
            let app = app.clone();
            let (lease, terminal) = NativeOwnedLocalTaskAuthorityLease::acquire(Rc::clone(
                &owned_local_authority_count,
            ));
            let task = foreground_executor.spawn(async move {
                let _lease = lease;
                while cross_thread_wake_receiver.recv().await.is_ok() {
                    let Some(app) = app.upgrade() else {
                        break;
                    };
                    app.import_staged_native_events();
                }
            });
            NativeOwnedLocalTask::new(task, terminal)
        };
        Self {
            sequencer,
            pending: RefCell::new(VecDeque::new()),
            phase: Cell::new(NativeWorkPhase::Idle),
            next_drain_generation: Cell::new(0),
            next_wake_ticket: Cell::new(0),
            scheduled_wake: Cell::new(None),
            unwind_recovery_scheduled: Cell::new(false),
            post_borrow_convergence_scheduled: Cell::new(false),
            terminated: Cell::new(false),
            delayed_wake_tasks: RefCell::new(Vec::new()),
            cross_thread_wake_sender,
            cross_thread_wake_task: RefCell::new(Some(cross_thread_wake_task)),
            owned_local_authority_count,
            foreground_executor,
            app,
            diagnostics: NativeBoundaryDiagnostics::default(),
        }
    }

    pub(super) fn cross_thread_ingress(&self) -> NativeCrossThreadIngress {
        NativeCrossThreadIngress::new(
            self.sequencer.clone(),
            self.cross_thread_wake_sender.clone(),
        )
    }

    pub(super) fn enqueue(&self, window_id: WindowId, event: NativeWindowEvent) {
        let permits_pointer_capture_release_retry = event.permits_pointer_capture_release_retry();
        let envelope = self.prepare(window_id, event);
        self.enqueue_envelope(envelope);
        if permits_pointer_capture_release_retry && let Some(app) = self.app.upgrade() {
            app.retry_native_pointer_capture_release_for_native_window_progress(window_id);
        }
    }

    pub(super) fn enqueue_app(&self, event: NativeAppEvent) {
        let envelope = self.prepare_target(NativeEventTarget::App(event));
        self.enqueue_envelope(envelope);
    }

    pub(super) fn enqueue_command(
        &self,
        window_id: WindowId,
        dispatcher: PlatformWindowCommandDispatcher,
        command: PlatformWindowCommand,
    ) -> Option<u64> {
        self.enqueue_native_command(NativePlatformCommand::new(window_id, dispatcher, command))
    }

    pub(super) fn enqueue_provisional_reveal(
        &self,
        window_id: WindowId,
        dispatcher: PlatformWindowCommandDispatcher,
        command: PlatformWindowCommand,
        ticket: crate::WindowProvisionalRevealTicket,
    ) {
        let _ = self.enqueue_native_command(NativePlatformCommand::new_provisional_reveal(
            window_id, dispatcher, command, ticket,
        ));
    }

    pub(super) fn enqueue_native_command(&self, command: NativePlatformCommand) -> Option<u64> {
        let sequence = self.reserve_sequence();
        self.enqueue_work(NativeWorkEnvelope::Command { sequence, command })
            .then_some(sequence)
    }

    pub(super) fn enqueue_window_activation_readback(
        &self,
        window_id: WindowId,
        request_generation: u64,
    ) {
        let sequence = self.reserve_sequence();
        let _ = self.enqueue_work(NativeWorkEnvelope::WindowActivationReadback {
            sequence,
            window_id,
            request_generation,
        });
    }

    pub(super) fn enqueue_pointer_capture_release(&self, release: NativePointerCaptureRelease) {
        let sequence = self.reserve_sequence();
        self.enqueue_work(NativeWorkEnvelope::PointerCaptureRelease { sequence, release });
    }

    pub(super) fn schedule_pointer_capture_release_retry(
        &self,
        timer: Task<()>,
        token: NativePointerCaptureReleaseToken,
        retry_epoch: u8,
    ) {
        let app = self.app.clone();
        let (lease, terminal) = self.acquire_owned_local_task();
        let wake = self.foreground_executor.spawn(async move {
            let _lease = lease;
            timer.await;
            if let Some(app) = app.upgrade() {
                app.retry_native_pointer_capture_release_from_wake(token, retry_epoch);
            }
        });
        self.retain_delayed_wake(NativeOwnedLocalTask::new(wake, terminal));
    }

    pub(super) fn schedule_window_retirement_retry(
        &self,
        timer: Task<()>,
        retirement: NativeWindowRetirement,
    ) {
        let app = self.app.clone();
        let (lease, terminal) = self.acquire_owned_local_task();
        let wake = self.foreground_executor.spawn(async move {
            let _lease = lease;
            timer.await;
            if let Some(app) = app.upgrade() {
                app.retry_native_window_retirement(retirement);
            }
        });
        self.retain_delayed_wake(NativeOwnedLocalTask::new(wake, terminal));
    }

    pub(super) fn schedule_shutdown_completion_retry(
        &self,
        timer: Task<()>,
        generation: u64,
        retry_epoch: u8,
    ) {
        let app = self.app.clone();
        let (lease, terminal) = self.acquire_owned_local_task();
        let wake = self.foreground_executor.spawn(async move {
            let _lease = lease;
            timer.await;
            if let Some(app) = app.upgrade() {
                app.retry_shutdown_completion_from_wake(generation, retry_epoch);
            }
        });
        self.retain_delayed_wake(NativeOwnedLocalTask::new(wake, terminal));
    }

    fn retain_delayed_wake(&self, wake: NativeOwnedLocalTask) {
        let mut wakes = self.delayed_wake_tasks.borrow_mut();
        wakes.retain(|wake| !wake.is_ready());
        wakes.push(wake);
    }

    fn acquire_owned_local_task(
        &self,
    ) -> (
        NativeOwnedLocalTaskAuthorityLease,
        NativeOwnedLocalTaskAuthorityTerminal,
    ) {
        NativeOwnedLocalTaskAuthorityLease::acquire(Rc::clone(&self.owned_local_authority_count))
    }

    pub(super) fn owned_local_authorities_are_settled(&self) -> bool {
        let mut wakes = self.delayed_wake_tasks.borrow_mut();
        wakes.retain(|wake| !wake.is_ready());
        wakes.is_empty() && self.owned_local_authority_count.get() == 0
    }

    pub(super) fn enqueue_captured_drag_release_completion(
        &self,
        completion: NativeCapturedDragReleaseCompletion,
    ) {
        let sequence = self.reserve_sequence();
        self.enqueue_work(NativeWorkEnvelope::CapturedDragReleaseCompletion {
            sequence,
            completion,
        });
    }

    pub(super) fn enqueue_window_retirement(&self, retirement: NativeWindowRetirement) {
        let sequence = self.reserve_sequence();
        self.enqueue_work(NativeWorkEnvelope::WindowRetirement {
            sequence,
            retirement,
        });
    }

    pub(super) fn enqueue_shutdown_completion(&self, completion: NativeShutdownCompletion) {
        let sequence = self.reserve_sequence();
        self.enqueue_work(NativeWorkEnvelope::ShutdownCompletion {
            sequence,
            completion,
        });
    }

    pub(super) fn prepare(
        &self,
        window_id: WindowId,
        event: NativeWindowEvent,
    ) -> NativeEventEnvelope {
        self.prepare_target(NativeEventTarget::Window { window_id, event })
    }

    pub(super) fn prepare_presequenced(
        &self,
        sequence: NativeIngressSequence,
        window_id: WindowId,
        event: NativeWindowEvent,
    ) -> NativeEventEnvelope {
        NativeEventEnvelope {
            sequence: sequence.value(),
            target: NativeEventTarget::Window { window_id, event },
        }
    }

    fn prepare_target(&self, target: NativeEventTarget) -> NativeEventEnvelope {
        let sequence = self.reserve_sequence();
        NativeEventEnvelope { sequence, target }
    }

    pub(super) fn reserve_input_sequence(&self) -> NativeIngressSequence {
        NativeIngressSequence::new(self.reserve_sequence())
    }

    fn reserve_sequence(&self) -> u64 {
        // Earlier callback entries must enter the foreground queue before this local sequence.
        let (staged, sequence) = self.sequencer.lock().take_staged_before_reserving();
        self.enqueue_staged_cross_thread(staged);
        sequence
    }

    pub(super) fn enqueue_envelope(&self, envelope: NativeEventEnvelope) {
        self.enqueue_work(NativeWorkEnvelope::Event(envelope));
    }

    pub(super) fn import_staged_native_events(&self) {
        let staged = self.sequencer.lock().take_staged();
        self.enqueue_staged_cross_thread(staged);
    }

    fn enqueue_staged_cross_thread(&self, staged: VecDeque<StagedNativeEnvelope>) {
        if staged.is_empty() {
            return;
        }
        if self.terminated.get() {
            for event in &staged {
                self.diagnostics.record_terminal(
                    event.pending_diagnostic(),
                    NativeBoundaryDisposition::Closed,
                );
            }
            drop(staged);
            return;
        }
        for staged in staged {
            self.enqueue_work(NativeWorkEnvelope::Event(staged.into_native_event()));
        }
    }

    fn enqueue_work(&self, mut envelope: NativeWorkEnvelope) -> bool {
        if self.terminated.get() {
            self.diagnostics.record_terminal(
                envelope.pending_diagnostic(),
                NativeBoundaryDisposition::Closed,
            );
            log::trace!(
                "native work sequence={} disposition=AfterQuitBarrier",
                envelope.sequence()
            );
            return false;
        }
        if self.sequencer.lock().shutdown_generation.is_some() && !envelope.is_shutdown_critical() {
            self.diagnostics.record_terminal(
                envelope.pending_diagnostic(),
                NativeBoundaryDisposition::Closed,
            );
            return false;
        }
        let mut pending = self.pending.borrow_mut();
        if let NativeWorkEnvelope::Event(current) = &mut envelope
            && let Some(coalesce_key) = current.coalesce_key()
            && let Some(NativeWorkEnvelope::Event(previous)) = pending.back()
            && previous.coalesce_key() == Some(coalesce_key)
        {
            let NativeWorkEnvelope::Event(mut replaced) = pending
                .pop_back()
                .expect("checked pending native work must still exist")
            else {
                unreachable!("coalescing only inspects an event envelope");
            };
            if matches!(
                &current.target,
                NativeEventTarget::Window {
                    event: NativeWindowEvent::RequestFrame(_),
                    ..
                }
            ) {
                std::mem::swap(current, &mut replaced);
            }
            current.merge_from(&replaced);
            self.diagnostics.record_terminal(
                replaced.pending_diagnostic(),
                NativeBoundaryDisposition::Coalesced {
                    into_sequence: current.sequence(),
                },
            );
            log::trace!(
                "native event sequence={} coalesced_by={}",
                replaced.sequence(),
                current.sequence()
            );
        }
        pending.push_back(envelope);
        drop(pending);
        self.schedule_wake_if_needed();
        true
    }

    pub(super) fn can_deliver_inline(&self) -> bool {
        !self.terminated.get()
            && self.pending.borrow().is_empty()
            && self.phase.get() == NativeWorkPhase::Idle
            && self.scheduled_wake.get().is_none()
    }

    /// Returns whether an earlier native work item still owns ordering before `sequence`.
    ///
    /// Captured drag facts are produced after their source transaction but are not themselves
    /// native ingress work. They must therefore wait behind every older queued callback or
    /// platform command before reaching an application-level consumer.
    pub(super) fn has_pending_before(&self, sequence: NativeIngressSequence) -> bool {
        self.pending
            .borrow()
            .iter()
            .any(|pending| pending.sequence() < sequence.value())
    }

    pub(super) fn has_pending_shutdown_critical_work(&self) -> bool {
        self.pending
            .borrow()
            .iter()
            .any(NativeWorkEnvelope::is_shutdown_critical)
    }

    pub(super) fn is_terminated(&self) -> bool {
        self.terminated.get()
    }

    pub(super) fn begin_shutdown(&self, generation: u64) {
        {
            let mut sequencer = self.sequencer.lock();
            match sequencer.shutdown_generation {
                Some(current) => {
                    debug_assert_eq!(
                        current, generation,
                        "shutdown generation changed while fenced"
                    );
                    return;
                }
                None => sequencer.shutdown_generation = Some(generation),
            }
        }

        let retired = {
            let mut pending = self.pending.borrow_mut();
            let mut retained = VecDeque::with_capacity(pending.len());
            let mut retired = VecDeque::new();
            while let Some(envelope) = pending.pop_front() {
                if envelope.is_shutdown_critical() {
                    retained.push_back(envelope);
                } else {
                    retired.push_back(envelope);
                }
            }
            *pending = retained;
            retired
        };
        for envelope in &retired {
            self.diagnostics.record_terminal(
                envelope.pending_diagnostic(),
                NativeBoundaryDisposition::Closed,
            );
        }
        drop(retired);
    }

    pub(super) fn end_shutdown(&self, generation: u64) {
        let ended = {
            let mut sequencer = self.sequencer.lock();
            if sequencer.shutdown_generation == Some(generation) {
                sequencer.shutdown_generation = None;
                true
            } else {
                false
            }
        };
        if ended {
            self.schedule_wake_if_needed();
        }
    }

    pub(super) fn reopen_window(&self, window_id: WindowId) {
        self.diagnostics.reopen_window(window_id);
    }

    pub(super) fn close_window(&self, window_id: WindowId) {
        self.diagnostics.close_window(window_id);
    }

    pub(super) fn record_immediate(
        &self,
        sequence: u64,
        window_id: WindowId,
        kind: NativeCallbackKind,
        domain_generation: Option<NativeBoundaryGeneration>,
        disposition: NativeBoundaryDisposition,
    ) {
        self.diagnostics.record_terminal(
            NativeBoundaryDiagnostic::pending(
                sequence,
                NativeBoundaryTarget::Window(window_id),
                NativeBoundaryKind::Callback(kind),
                domain_generation,
            ),
            disposition,
        );
    }

    pub(super) fn record_terminal(
        &self,
        diagnostic: NativeBoundaryDiagnostic,
        disposition: NativeBoundaryDisposition,
    ) {
        self.diagnostics.record_terminal(diagnostic, disposition);
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn diagnostics_snapshot_since(
        &self,
        cursor: NativeBoundaryDiagnosticCursor,
    ) -> NativeBoundaryDiagnosticsSnapshot {
        let mut pending = {
            self.sequencer
                .lock()
                .staged_cross_thread
                .iter()
                .map(StagedNativeEnvelope::pending_diagnostic)
                .collect::<Vec<_>>()
        };
        pending.extend(
            self.pending
                .borrow()
                .iter()
                .map(NativeWorkEnvelope::pending_diagnostic),
        );
        self.diagnostics.snapshot_since(cursor, pending)
    }

    pub(super) fn try_begin_drain(&self, wake_ticket: Option<u64>) -> Option<NativeDrainLease<'_>> {
        self.try_begin_drain_inner(wake_ticket, None)
    }

    pub(super) fn try_begin_drain_with_budget<'a>(
        &'a self,
        budget: &'a NativeWorkDrainBudget,
    ) -> Option<NativeDrainLease<'a>> {
        self.try_begin_drain_inner(None, Some(budget))
    }

    fn try_begin_drain_inner<'a>(
        &'a self,
        wake_ticket: Option<u64>,
        budget: Option<&'a NativeWorkDrainBudget>,
    ) -> Option<NativeDrainLease<'a>> {
        let remaining = budget
            .map(NativeWorkDrainBudget::remaining)
            .unwrap_or(MAX_NATIVE_WORK_PER_DRAIN);
        if remaining == 0 {
            return None;
        }
        if self.terminated.get() || self.pending.borrow().is_empty() {
            if wake_ticket.is_some() {
                self.consume_wake_ticket(wake_ticket?);
            }
            return None;
        }
        if let Some(wake_ticket) = wake_ticket {
            if !self.consume_wake_ticket(wake_ticket) {
                return None;
            }
        } else {
            self.scheduled_wake.take();
        }
        if self.phase.get() != NativeWorkPhase::Idle {
            return None;
        }
        let generation = self.next_generation();
        self.phase.set(NativeWorkPhase::Draining {
            generation,
            remaining,
            executing_command: false,
        });
        Some(NativeDrainLease {
            ingress: self,
            generation,
            budget,
            active: true,
        })
    }

    pub(super) fn begin_input_barrier(
        &self,
    ) -> Result<NativeInputBarrier<'_>, NativeInputBarrierError> {
        match self.phase.get() {
            NativeWorkPhase::Idle | NativeWorkPhase::BlockedOnApp => {
                self.scheduled_wake.take();
                let generation = self.next_generation();
                self.phase.set(NativeWorkPhase::Input {
                    generation,
                    remaining: MAX_NATIVE_WORK_PER_DRAIN,
                });
                Ok(NativeInputBarrier {
                    ingress: self,
                    generation,
                    root: true,
                    active: true,
                })
            }
            NativeWorkPhase::Draining {
                generation,
                executing_command: true,
                ..
            } => Ok(NativeInputBarrier {
                ingress: self,
                generation,
                root: false,
                active: true,
            }),
            NativeWorkPhase::Draining {
                executing_command: false,
                ..
            }
            | NativeWorkPhase::Input { .. } => Err(NativeInputBarrierError::EventTransaction),
        }
    }

    pub(super) fn resume_after_app_borrow(&self) -> bool {
        if self.phase.get() != NativeWorkPhase::BlockedOnApp {
            return false;
        }
        self.phase.set(NativeWorkPhase::Idle);
        true
    }

    pub(super) fn postpone_drain(&self, wake_ticket: Option<u64>) {
        if let Some(wake_ticket) = wake_ticket {
            self.consume_wake_ticket(wake_ticket);
        } else {
            self.scheduled_wake.take();
        }
    }

    pub(super) fn resume_after_post_borrow_convergence(&self) {
        self.schedule_wake_if_needed();
    }

    pub(super) fn schedule_post_borrow_convergence(&self) {
        if self.post_borrow_convergence_scheduled.replace(true) {
            return;
        }
        let app = self.app.clone();
        self.foreground_executor
            .spawn(async move {
                if let Some(app) = app.upgrade() {
                    app.run_post_borrow_convergence_from_wake();
                }
            })
            .detach();
    }

    pub(super) fn finish_post_borrow_convergence_wake(&self) {
        self.post_borrow_convergence_scheduled.set(false);
    }

    pub(super) fn schedule_drain_after_unwind(&self) {
        if self.terminated.get() || self.unwind_recovery_scheduled.replace(true) {
            return;
        }
        let app = self.app.clone();
        self.foreground_executor
            .spawn(async move {
                if let Some(app) = app.upgrade() {
                    app.recover_native_work_after_unwind();
                }
            })
            .detach();
    }

    pub(super) fn finish_unwind_recovery_wake(&self) {
        self.unwind_recovery_scheduled.set(false);
    }

    fn next_generation(&self) -> u64 {
        let generation = self.next_drain_generation.get();
        self.next_drain_generation.set(
            generation
                .checked_add(1)
                .expect("native work drain generation overflowed"),
        );
        generation
    }

    fn consume_wake_ticket(&self, wake_ticket: u64) -> bool {
        if self.scheduled_wake.get() != Some(wake_ticket) {
            return false;
        }
        self.scheduled_wake.set(None);
        true
    }

    fn terminate(&self) {
        if self.terminated.replace(true) {
            return;
        }
        // Delayed native wakes own local futures. Cancel them on the application thread before
        // the platform message loop can return; otherwise a timer thread could become the final
        // owner of a !Send future after the main-thread dispatch destination has gone away.
        self.delayed_wake_tasks.borrow_mut().clear();
        self.cross_thread_wake_task.borrow_mut().take();
        let staged = self.sequencer.lock().terminate();
        let pending = {
            let mut queue = self.pending.borrow_mut();
            std::mem::take(&mut *queue)
        };
        self.scheduled_wake.set(None);
        self.unwind_recovery_scheduled.set(false);

        for event in &staged {
            self.diagnostics.record_terminal(
                event.pending_diagnostic(),
                NativeBoundaryDisposition::Closed,
            );
        }
        for envelope in &pending {
            self.diagnostics.record_terminal(
                envelope.pending_diagnostic(),
                NativeBoundaryDisposition::Closed,
            );
        }
        drop(staged);
        drop(pending);
    }

    fn schedule_wake_if_needed(&self) {
        if self.terminated.get()
            || self.pending.borrow().is_empty()
            || self.phase.get() != NativeWorkPhase::Idle
            || self.scheduled_wake.get().is_some()
        {
            return;
        }
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if app.native_work_has_release_custody() {
            return;
        }
        let ticket = self.next_wake_ticket.get();
        self.next_wake_ticket.set(
            ticket
                .checked_add(1)
                .expect("native work wake ticket overflowed"),
        );
        self.scheduled_wake.set(Some(ticket));
        let app = self.app.clone();
        self.foreground_executor
            .spawn(async move {
                if let Some(app) = app.upgrade() {
                    app.drain_native_work_from_wake(ticket);
                }
            })
            .detach();
    }

    fn finish_drain(&self, generation: u64) {
        if matches!(
            self.phase.get(),
            NativeWorkPhase::Draining {
                generation: current,
                ..
            } if current == generation
        ) {
            self.phase.set(NativeWorkPhase::Idle);
            self.schedule_wake_if_needed();
        }
    }

    fn finish_input(&self, generation: u64, schedule_wake: bool) {
        if matches!(
            self.phase.get(),
            NativeWorkPhase::Input {
                generation: current,
                ..
            } if current == generation
        ) {
            self.phase.set(NativeWorkPhase::Idle);
            if schedule_wake {
                self.schedule_wake_if_needed();
            }
        }
    }

    fn consume_budget(&self, generation: u64) -> bool {
        let phase = self.phase.get();
        let next = match phase {
            NativeWorkPhase::Draining {
                generation: current,
                remaining,
                executing_command,
            } if current == generation && remaining > 0 => NativeWorkPhase::Draining {
                generation: current,
                remaining: remaining - 1,
                executing_command,
            },
            NativeWorkPhase::Input {
                generation: current,
                remaining,
            } if current == generation && remaining > 0 => NativeWorkPhase::Input {
                generation: current,
                remaining: remaining - 1,
            },
            _ => return false,
        };
        self.phase.set(next);
        true
    }

    fn pop_front(&self, generation: u64) -> NativeWorkPop {
        if self.pending.borrow().is_empty() {
            return NativeWorkPop::Empty;
        }
        if !self.consume_budget(generation) {
            return NativeWorkPop::BudgetExhausted;
        }
        NativeWorkPop::Work(
            self.pending
                .borrow_mut()
                .pop_front()
                .expect("checked pending native work must still exist"),
        )
    }

    fn event_before_index(&self, sequence_cutoff: u64) -> Option<usize> {
        let pending = self.pending.borrow();
        pending
            .iter()
            .take_while(|envelope| envelope.sequence() < sequence_cutoff)
            .position(|envelope| matches!(envelope, NativeWorkEnvelope::Event(_)))
    }

    fn checkout_event(&self, queue_index: usize) -> NativeEventCheckout {
        let Some(NativeWorkEnvelope::Event(event)) = self.pending.borrow_mut().remove(queue_index)
        else {
            unreachable!("eligible native event must remain at its queue position");
        };
        NativeEventCheckout { event, queue_index }
    }

    fn pop_event_before(&self, generation: u64, sequence_cutoff: u64) -> NativeEventPrefixPop {
        let Some(queue_index) = self.event_before_index(sequence_cutoff) else {
            return NativeEventPrefixPop::NoPriorEvent;
        };
        if !self.consume_budget(generation) {
            return NativeEventPrefixPop::BudgetExhausted;
        }
        NativeEventPrefixPop::Event(self.checkout_event(queue_index))
    }

    fn pop_event_before_unbounded(&self, sequence_cutoff: u64) -> NativeEventPrefixPop {
        let Some(queue_index) = self.event_before_index(sequence_cutoff) else {
            return NativeEventPrefixPop::NoPriorEvent;
        };
        NativeEventPrefixPop::Event(self.checkout_event(queue_index))
    }

    fn restore_event(&self, checkout: NativeEventCheckout) {
        let mut pending = self.pending.borrow_mut();
        assert!(
            checkout.queue_index <= pending.len(),
            "checked-out native event queue position must remain restorable"
        );
        pending.insert(
            checkout.queue_index,
            NativeWorkEnvelope::Event(checkout.event),
        );
    }

    fn push_front(&self, envelope: NativeWorkEnvelope) {
        self.pending.borrow_mut().push_front(envelope);
    }

    fn block_on_app(&self, generation: u64) {
        if matches!(
            self.phase.get(),
            NativeWorkPhase::Draining {
                generation: current,
                ..
            } if current == generation
        ) {
            self.phase.set(NativeWorkPhase::BlockedOnApp);
            self.scheduled_wake.set(None);
        }
    }

    fn enter_command(&self, generation: u64) -> NativeCommandGuard<'_> {
        let NativeWorkPhase::Draining {
            generation: current,
            remaining,
            executing_command: false,
        } = self.phase.get()
        else {
            panic!("native command dispatch requires the active non-recursive drain");
        };
        assert_eq!(
            current, generation,
            "native command dispatch used a stale drain generation"
        );
        self.phase.set(NativeWorkPhase::Draining {
            generation,
            remaining,
            executing_command: true,
        });
        NativeCommandGuard {
            ingress: self,
            generation,
        }
    }

    fn leave_command(&self, generation: u64) {
        if let NativeWorkPhase::Draining {
            generation: current,
            remaining,
            executing_command: true,
        } = self.phase.get()
            && current == generation
        {
            self.phase.set(NativeWorkPhase::Draining {
                generation,
                remaining,
                executing_command: false,
            });
        }
    }
}

struct NativeOwnedLocalTask {
    task: Option<Task<()>>,
    terminal: NativeOwnedLocalTaskAuthorityTerminal,
}

impl NativeOwnedLocalTask {
    fn new(task: Task<()>, terminal: NativeOwnedLocalTaskAuthorityTerminal) -> Self {
        Self {
            task: Some(task),
            terminal,
        }
    }

    fn is_ready(&self) -> bool {
        self.task.as_ref().is_none_or(Task::is_ready)
    }
}

impl Drop for NativeOwnedLocalTask {
    fn drop(&mut self) {
        // Cancel the future before settling its logical App authority. async-task may defer
        // destroying the future's storage until the cancellation runnable reaches the main
        // thread, so the future's lease shares the same idempotent terminal token.
        drop(self.task.take());
        self.terminal.settle();
    }
}

struct NativeOwnedLocalTaskAuthorityLease {
    terminal: NativeOwnedLocalTaskAuthorityTerminal,
}

#[derive(Clone)]
struct NativeOwnedLocalTaskAuthorityTerminal {
    state: Rc<NativeOwnedLocalTaskAuthorityState>,
}

struct NativeOwnedLocalTaskAuthorityState {
    active: Rc<Cell<usize>>,
    settled: Cell<bool>,
}

impl NativeOwnedLocalTaskAuthorityLease {
    fn acquire(
        active: Rc<Cell<usize>>,
    ) -> (
        NativeOwnedLocalTaskAuthorityLease,
        NativeOwnedLocalTaskAuthorityTerminal,
    ) {
        active.set(
            active
                .get()
                .checked_add(1)
                .expect("native ingress owned-local-task count overflowed"),
        );
        let terminal = NativeOwnedLocalTaskAuthorityTerminal {
            state: Rc::new(NativeOwnedLocalTaskAuthorityState {
                active,
                settled: Cell::new(false),
            }),
        };
        (
            Self {
                terminal: terminal.clone(),
            },
            terminal,
        )
    }
}

impl NativeOwnedLocalTaskAuthorityTerminal {
    fn settle(&self) {
        if self.state.settled.replace(true) {
            return;
        }
        self.state.active.set(
            self.state
                .active
                .get()
                .checked_sub(1)
                .expect("native ingress owned-local-task count underflowed"),
        );
    }
}

impl Drop for NativeOwnedLocalTaskAuthorityLease {
    fn drop(&mut self) {
        self.terminal.settle();
    }
}

impl Drop for NativeEventIngress {
    fn drop(&mut self) {
        self.terminate();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeWorkPhase {
    Idle,
    BlockedOnApp,
    Draining {
        generation: u64,
        remaining: u8,
        executing_command: bool,
    },
    Input {
        generation: u64,
        remaining: u8,
    },
}

pub(super) enum NativeWorkEnvelope {
    Event(NativeEventEnvelope),
    Command {
        sequence: u64,
        command: NativePlatformCommand,
    },
    WindowActivationReadback {
        sequence: u64,
        window_id: WindowId,
        request_generation: u64,
    },
    PointerCaptureRelease {
        sequence: u64,
        release: NativePointerCaptureRelease,
    },
    CapturedDragReleaseCompletion {
        sequence: u64,
        completion: NativeCapturedDragReleaseCompletion,
    },
    WindowRetirement {
        sequence: u64,
        retirement: NativeWindowRetirement,
    },
    ShutdownCompletion {
        sequence: u64,
        completion: NativeShutdownCompletion,
    },
}

impl NativeWorkEnvelope {
    pub(super) fn sequence(&self) -> u64 {
        match self {
            Self::Event(event) => event.sequence(),
            Self::Command { sequence, .. } => *sequence,
            Self::WindowActivationReadback { sequence, .. }
            | Self::PointerCaptureRelease { sequence, .. }
            | Self::CapturedDragReleaseCompletion { sequence, .. }
            | Self::WindowRetirement { sequence, .. }
            | Self::ShutdownCompletion { sequence, .. } => *sequence,
        }
    }

    pub(super) fn pending_diagnostic(&self) -> NativeBoundaryDiagnostic {
        match self {
            Self::Event(event) => event.pending_diagnostic(),
            Self::Command { sequence, command } => command.pending_diagnostic(*sequence),
            Self::WindowActivationReadback {
                sequence,
                window_id,
                request_generation,
            } => NativeBoundaryDiagnostic::pending(
                *sequence,
                NativeBoundaryTarget::Window(*window_id),
                NativeBoundaryKind::Command(
                    super::native_callback_diagnostics::NativePlatformCommandKind::Activate,
                ),
                Some(NativeBoundaryGeneration::WindowActivation(
                    *request_generation,
                )),
            ),
            Self::PointerCaptureRelease { sequence, release } => {
                release.pending_diagnostic(*sequence)
            }
            Self::CapturedDragReleaseCompletion {
                sequence,
                completion,
            } => completion.pending_diagnostic(*sequence),
            Self::WindowRetirement {
                sequence,
                retirement,
            } => retirement.pending_diagnostic(*sequence),
            Self::ShutdownCompletion {
                sequence,
                completion,
            } => completion.pending_diagnostic(*sequence),
        }
    }

    fn is_shutdown_critical(&self) -> bool {
        match self {
            Self::Event(event) => event.is_shutdown_critical(),
            Self::PointerCaptureRelease { .. }
            | Self::CapturedDragReleaseCompletion { .. }
            | Self::WindowRetirement { .. }
            | Self::ShutdownCompletion { .. } => true,
            Self::Command { .. } | Self::WindowActivationReadback { .. } => false,
        }
    }
}

impl NativeCapturedDragReleaseCompletion {
    pub(super) fn pending_diagnostic(&self, sequence: u64) -> NativeBoundaryDiagnostic {
        let barrier = self.barrier();
        NativeBoundaryDiagnostic::pending(
            sequence,
            NativeBoundaryTarget::Window(barrier.source_window()),
            NativeBoundaryKind::Command(
                super::native_callback_diagnostics::NativePlatformCommandKind::CompleteCapturedDragRelease,
            ),
            Some(NativeBoundaryGeneration::PointerCaptureRelease {
                captured_drag: Some(barrier.drag_generation()),
                release: barrier.release_generation(),
            }),
        )
    }
}

pub(super) enum NativeWorkPop {
    Work(NativeWorkEnvelope),
    Empty,
    BudgetExhausted,
}

pub(super) struct NativeWorkDrainBudget {
    remaining: Cell<u8>,
}

impl NativeWorkDrainBudget {
    /// Creates one bounded budget shared by every native-work phase in a convergence turn.
    pub(super) fn new() -> Self {
        Self {
            remaining: Cell::new(MAX_NATIVE_WORK_PER_DRAIN),
        }
    }

    fn remaining(&self) -> u8 {
        self.remaining.get()
    }

    fn update(&self, remaining: u8) {
        self.remaining.set(remaining);
    }
}

pub(super) struct NativeDrainLease<'a> {
    ingress: &'a NativeEventIngress,
    generation: u64,
    budget: Option<&'a NativeWorkDrainBudget>,
    active: bool,
}

impl NativeDrainLease<'_> {
    pub(super) fn pop_front(&self) -> NativeWorkPop {
        self.ingress.pop_front(self.generation)
    }

    pub(super) fn push_front(&self, envelope: NativeWorkEnvelope) {
        self.ingress.push_front(envelope);
    }

    pub(super) fn block_on_app(&mut self) {
        self.sync_budget();
        self.ingress.block_on_app(self.generation);
        self.active = false;
    }

    pub(super) fn enter_command(&self) -> NativeCommandGuard<'_> {
        self.ingress.enter_command(self.generation)
    }

    pub(super) fn terminate(&mut self) {
        self.ingress.terminate();
    }

    fn sync_budget(&self) {
        let Some(budget) = self.budget else {
            return;
        };
        match self.ingress.phase.get() {
            NativeWorkPhase::Draining {
                generation,
                remaining,
                ..
            } if generation == self.generation => budget.update(remaining),
            _ => debug_assert!(
                false,
                "shared native-work budget requires the active drain generation"
            ),
        }
    }
}

impl Drop for NativeDrainLease<'_> {
    fn drop(&mut self) {
        if self.active {
            self.sync_budget();
            self.ingress.finish_drain(self.generation);
        }
    }
}

pub(super) struct NativeCommandGuard<'a> {
    ingress: &'a NativeEventIngress,
    generation: u64,
}

impl Drop for NativeCommandGuard<'_> {
    fn drop(&mut self) {
        self.ingress.leave_command(self.generation);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeInputBarrierError {
    EventTransaction,
}

pub(super) enum NativeEventPrefixPop {
    Event(NativeEventCheckout),
    NoPriorEvent,
    BudgetExhausted,
}

pub(super) struct NativeEventCheckout {
    event: NativeEventEnvelope,
    queue_index: usize,
}

impl NativeEventCheckout {
    pub(super) fn ingress_sequence(&self) -> NativeIngressSequence {
        self.event.ingress_sequence()
    }

    pub(super) fn pending_diagnostic(&self) -> NativeBoundaryDiagnostic {
        self.event.pending_diagnostic()
    }

    pub(super) fn deliver(self, app: &mut App) -> NativeEventDelivery {
        self.event.deliver(app)
    }
}

pub(super) struct NativeInputBarrier<'a> {
    ingress: &'a NativeEventIngress,
    generation: u64,
    root: bool,
    active: bool,
}

impl NativeInputBarrier<'_> {
    pub(super) fn pop_event_before(&self, sequence_cutoff: u64) -> NativeEventPrefixPop {
        if self.root {
            self.ingress.pop_event_before_unbounded(sequence_cutoff)
        } else {
            self.ingress
                .pop_event_before(self.generation, sequence_cutoff)
        }
    }

    pub(super) fn is_root(&self) -> bool {
        self.root
    }

    pub(super) fn restore_event(&self, checkout: NativeEventCheckout) {
        self.ingress.restore_event(checkout);
    }

    pub(super) fn finish_without_wake(&mut self) {
        if self.root && self.active {
            self.ingress.finish_input(self.generation, false);
            self.active = false;
        }
    }

    pub(super) fn terminate(&mut self) {
        self.ingress.terminate();
    }
}

impl Drop for NativeInputBarrier<'_> {
    fn drop(&mut self) {
        if self.root && self.active {
            self.ingress.finish_input(self.generation, true);
        }
    }
}

enum NativeEventTarget {
    App(NativeAppEvent),
    Window {
        window_id: WindowId,
        event: NativeWindowEvent,
    },
}

pub(super) struct NativeEventEnvelope {
    sequence: u64,
    target: NativeEventTarget,
}

impl NativeEventEnvelope {
    pub(super) fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(super) fn ingress_sequence(&self) -> NativeIngressSequence {
        NativeIngressSequence::new(self.sequence)
    }

    pub(super) fn pending_diagnostic(&self) -> NativeBoundaryDiagnostic {
        let (target, kind, domain_generation) = match &self.target {
            NativeEventTarget::App(event) => (
                NativeBoundaryTarget::Application,
                event.diagnostic_kind(),
                None,
            ),
            NativeEventTarget::Window { window_id, event } => {
                let (kind, domain_generation) = event.diagnostic_metadata();
                (
                    NativeBoundaryTarget::Window(*window_id),
                    kind,
                    domain_generation,
                )
            }
        };
        NativeBoundaryDiagnostic::pending(
            self.sequence,
            target,
            NativeBoundaryKind::Callback(kind),
            domain_generation,
        )
    }

    pub(super) fn deliver(self, app: &mut App) -> NativeEventDelivery {
        let Self { sequence, target } = self;
        let ingress_sequence = NativeIngressSequence::new(sequence);
        let (window_id, event) = match target {
            NativeEventTarget::App(event) => {
                let control = event.deliver(app);
                log::trace!(
                    "native event sequence={sequence} target=application disposition={control:?}"
                );
                return NativeEventDelivery {
                    control,
                    disposition: NativeEventDisposition::Delivered,
                };
            }
            NativeEventTarget::Window { window_id, event } => (window_id, event),
        };
        let disposition = match event {
            #[cfg(not(target_family = "wasm"))]
            NativeWindowEvent::AccessibilityAction {
                activation_generation,
                request,
            } => deliver_window_event_if(app, window_id, ingress_sequence, |window, cx| {
                window.with_input_transaction(cx, |window, cx| {
                    window.handle_a11y_action(activation_generation, request, cx)
                })
            }),
            #[cfg(not(target_family = "wasm"))]
            NativeWindowEvent::AccessibilityActiveChanged { .. } => {
                deliver_window_event(app, window_id, ingress_sequence, |window, _| {
                    window.refresh()
                })
            }
            NativeWindowEvent::ActiveChanged(observation) => {
                deliver_window_event(app, window_id, ingress_sequence, |window, cx| {
                    let admissible =
                        window.native_active_status_change_is_admissible(observation.active());
                    if admissible
                        && observation.active()
                        && observation.exact_native_positive()
                        && let Some(app_cell) = cx.this.upgrade()
                    {
                        app_cell.observe_native_window_activation_exact_positive(
                            window_id,
                            ingress_sequence.value(),
                        );
                    }
                    window.native_active_status_changed(observation.active(), cx);
                })
            }
            NativeWindowEvent::PointerCanceled(reservation) => {
                reservation.promote_terminal();
                let reason = reservation.reason;
                match reservation.delivery {
                    ReservedPointerCancelDelivery::PlatformInput { .. } => {
                        deliver_window_event_with_preclaimed_captured_drag(
                            app,
                            window_id,
                            ingress_sequence,
                            |window, cx| {
                                let _ = window.dispatch_event(
                                    PlatformInput::PointerCanceled(PointerCancelEvent { reason }),
                                    cx,
                                );
                            },
                        )
                    }
                    ReservedPointerCancelDelivery::CapturedDragTerminal { .. } => {
                        NativeEventDisposition::Delivered
                    }
                }
            }
            NativeWindowEvent::TouchCancelled { event, source } => {
                deliver_window_event_if(app, window_id, ingress_sequence, |window, cx| {
                    if !source.is_current() {
                        return false;
                    }
                    if window.user_interaction_is_admitted() {
                        let _ = window.dispatch_event(PlatformInput::Touch(event), cx);
                    } else {
                        window.dispatch_reserved_native_touch_cancel(event, cx);
                    }
                    true
                })
            }
            NativeWindowEvent::ExternalDrag { generation, event } => {
                deliver_window_event_if(app, window_id, ingress_sequence, |window, cx| {
                    window.dispatch_external_drag_event(generation, event, cx)
                })
            }
            NativeWindowEvent::ModifiersChanged(event) => {
                deliver_window_event(app, window_id, ingress_sequence, |window, cx| {
                    let _ = window.dispatch_event(PlatformInput::ModifiersChanged(event), cx);
                })
            }
            NativeWindowEvent::AppearanceChanged => {
                deliver_window_event(app, window_id, ingress_sequence, Window::appearance_changed)
            }
            NativeWindowEvent::InitialPresentationCompleted => deliver_window_event(
                app,
                window_id,
                ingress_sequence,
                Window::initial_presentation_completed,
            ),
            NativeWindowEvent::InitialPresentationFailed => deliver_window_event(
                app,
                window_id,
                ingress_sequence,
                Window::initial_presentation_failed,
            ),
            NativeWindowEvent::Resized
            | NativeWindowEvent::Moved
            | NativeWindowEvent::WindowStateChanged => {
                deliver_window_event(app, window_id, ingress_sequence, Window::bounds_changed)
            }
            NativeWindowEvent::DisplayChanged => {
                match app.update_window_id_from_native(
                    window_id,
                    ingress_sequence,
                    |_, window, _| window.update_display_association(),
                ) {
                    Ok(Some(display_id)) => {
                        if display_id.is_some_and(|id| !app.knows_display(id)) {
                            app.displays_changed();
                        }
                        deliver_window_event(
                            app,
                            window_id,
                            ingress_sequence,
                            Window::notify_display_observers,
                        )
                    }
                    Ok(None) => NativeEventDisposition::Delivered,
                    Err(_) => NativeEventDisposition::StaleWindow,
                }
            }
            NativeWindowEvent::VisibilityChanged(visibility) => {
                deliver_window_event(app, window_id, ingress_sequence, |window, app| {
                    window.update_visibility(visibility, app)
                })
            }
            NativeWindowEvent::ButtonLayoutChanged => deliver_window_event(
                app,
                window_id,
                ingress_sequence,
                Window::button_layout_changed,
            ),
            NativeWindowEvent::CloseRequested => {
                if app
                    .update_window_id_from_native(window_id, ingress_sequence, |_, window, cx| {
                        if window.should_close(cx) {
                            window.remove_window(cx);
                        }
                    })
                    .is_ok()
                {
                    NativeEventDisposition::Delivered
                } else {
                    NativeEventDisposition::StaleWindow
                }
            }
            NativeWindowEvent::CloseRequired => {
                deliver_window_event(app, window_id, ingress_sequence, Window::remove_window)
            }
            NativeWindowEvent::Closed => {
                let logical_close = catch_unwind(AssertUnwindSafe(|| {
                    app.update_window_id_from_native(
                        window_id,
                        ingress_sequence,
                        |_, window, cx| {
                            window.remove_window(cx);
                        },
                    )
                }));
                let disposition = match &logical_close {
                    Ok(Ok(())) => NativeEventDisposition::Delivered,
                    Ok(Err(_)) => NativeEventDisposition::StaleWindow,
                    Err(_) => NativeEventDisposition::Delivered,
                };

                if let Some(app_cell) = app.this.upgrade() {
                    app_cell.settle_native_window_terminal(window_id);
                }

                let mut first_panic = logical_close.err();
                let tab_cleanup = catch_unwind(AssertUnwindSafe(|| {
                    SystemWindowTabController::remove_tab(app, window_id);
                }));
                if first_panic.is_none() {
                    first_panic = tab_cleanup.err();
                }
                let native_terminal = catch_unwind(AssertUnwindSafe(|| {
                    app.notify_window_native_terminal(window_id);
                }));
                if first_panic.is_none() {
                    first_panic = native_terminal.err();
                }
                if let Some(payload) = first_panic {
                    resume_unwind(payload);
                }
                disposition
            }
            NativeWindowEvent::HoverChanged(hovered) => {
                deliver_window_event(app, window_id, ingress_sequence, |window, cx| {
                    window.native_hover_status_changed(hovered, cx);
                })
            }
            NativeWindowEvent::RequestFrame(options) => {
                deliver_window_event(app, window_id, ingress_sequence, |window, cx| {
                    window.native_frame_requested(options, cx);
                })
            }
            NativeWindowEvent::SystemTabCommand(command) => {
                deliver_window_event_if(app, window_id, ingress_sequence, |window, cx| {
                    if !window.user_interaction_is_admitted() {
                        return false;
                    }
                    match command {
                        NativeSystemTabCommand::MergeAll => {
                            SystemWindowTabController::merge_all_windows(cx, window_id);
                        }
                        NativeSystemTabCommand::MoveToNewWindow => {
                            SystemWindowTabController::move_tab_to_new_window(cx, window_id);
                        }
                        NativeSystemTabCommand::SelectNext => {
                            SystemWindowTabController::select_next_tab(cx, window_id);
                        }
                        NativeSystemTabCommand::SelectPrevious => {
                            SystemWindowTabController::select_previous_tab(cx, window_id);
                        }
                        NativeSystemTabCommand::ToggleBar => {
                            let visible = window.platform_window.tab_bar_visible();
                            SystemWindowTabController::set_visible(cx, visible);
                        }
                    }
                    true
                })
            }
            NativeWindowEvent::WindowMutationObserved(observation) => {
                deliver_window_event_if(app, window_id, ingress_sequence, |window, cx| {
                    window.window_mutation_observed(observation, cx)
                })
            }
        };
        log::trace!(
            "native event sequence={sequence} window={window_id:?} disposition={disposition:?}"
        );
        NativeEventDelivery {
            control: NativeEventDrainControl::Continue,
            disposition,
        }
    }

    fn is_shutdown_critical(&self) -> bool {
        match &self.target {
            NativeEventTarget::Window {
                event:
                    NativeWindowEvent::PointerCanceled(_)
                    | NativeWindowEvent::TouchCancelled { .. }
                    | NativeWindowEvent::ExternalDrag {
                        event: crate::FileDropEvent::Ended,
                        ..
                    }
                    | NativeWindowEvent::CloseRequired
                    | NativeWindowEvent::Closed,
                ..
            } => true,
            NativeEventTarget::App(NativeAppEvent::Quit) => true,
            _ => false,
        }
    }

    fn coalesce_key(&self) -> Option<NativeEventCoalesceKey> {
        match &self.target {
            NativeEventTarget::App(event) => {
                event.coalesce_domain().map(NativeEventCoalesceKey::App)
            }
            NativeEventTarget::Window { window_id, event } => {
                event
                    .coalesce_domain()
                    .map(|domain| NativeEventCoalesceKey::Window {
                        window_id: *window_id,
                        domain,
                    })
            }
        }
    }

    fn merge_from(&mut self, other: &Self) {
        if let (
            NativeEventTarget::Window {
                event: NativeWindowEvent::RequestFrame(current),
                ..
            },
            NativeEventTarget::Window {
                event: NativeWindowEvent::RequestFrame(later),
                ..
            },
        ) = (&mut self.target, &other.target)
        {
            current.merge(*later);
        }
    }
}

pub(super) enum NativeAppEvent {
    OpenUrls(Vec<String>),
    Reopen,
    SystemNotificationResponse(SystemNotificationResponse),
    SystemSleep,
    SystemWake,
    WillOpenAppMenu,
    AppMenuAction(Box<dyn Action>),
    KeyboardLayoutChanged,
    ThermalStateChanged,
    PowerModeChanged,
    DisplaysChanged,
    Quit,
}

impl NativeAppEvent {
    fn deliver(self, app: &mut App) -> NativeEventDrainControl {
        match self {
            Self::OpenUrls(urls) => {
                if let Some(app_cell) = app.this.upgrade() {
                    app_cell.dispatch_open_urls(urls, app);
                }
                NativeEventDrainControl::Continue
            }
            Self::Reopen => {
                if let Some(app_cell) = app.this.upgrade() {
                    app_cell.dispatch_reopen(app);
                }
                NativeEventDrainControl::Continue
            }
            Self::SystemNotificationResponse(response) => {
                if let Some(app_cell) = app.this.upgrade() {
                    app_cell.dispatch_system_notification_response(response, app);
                }
                NativeEventDrainControl::Continue
            }
            Self::SystemSleep => {
                app.system_sleep_observers
                    .clone()
                    .retain(&(), |callback| callback(app));
                NativeEventDrainControl::Continue
            }
            Self::SystemWake => {
                app.system_wake_observers
                    .clone()
                    .retain(&(), |callback| callback(app));
                NativeEventDrainControl::Continue
            }
            Self::WillOpenAppMenu => {
                app.clear_pending_keystrokes();
                NativeEventDrainControl::Continue
            }
            Self::AppMenuAction(action) => {
                app.dispatch_action(action.as_ref());
                NativeEventDrainControl::Continue
            }
            Self::KeyboardLayoutChanged => {
                app.keyboard_layout = app.platform.keyboard_layout();
                app.keyboard_mapper = app.platform.keyboard_mapper();
                app.keyboard_layout_observers
                    .clone()
                    .retain(&(), move |callback| (callback)(app));
                NativeEventDrainControl::Continue
            }
            Self::ThermalStateChanged => {
                app.thermal_state_observers
                    .clone()
                    .retain(&(), move |callback| (callback)(app));
                NativeEventDrainControl::Continue
            }
            Self::PowerModeChanged => {
                app.power_mode = app.platform.power_mode();
                app.power_mode_observers
                    .clone()
                    .retain(&(), |callback| callback(app));
                NativeEventDrainControl::Continue
            }
            Self::DisplaysChanged => {
                app.displays_changed();
                NativeEventDrainControl::Continue
            }
            Self::Quit => {
                app.quit_after_terminal_shutdown();
                NativeEventDrainControl::Continue
            }
        }
    }

    fn coalesce_domain(&self) -> Option<NativeAppEventCoalesceDomain> {
        match self {
            Self::KeyboardLayoutChanged => Some(NativeAppEventCoalesceDomain::KeyboardLayout),
            Self::ThermalStateChanged => Some(NativeAppEventCoalesceDomain::ThermalState),
            Self::PowerModeChanged => Some(NativeAppEventCoalesceDomain::PowerMode),
            Self::DisplaysChanged => Some(NativeAppEventCoalesceDomain::Displays),
            Self::OpenUrls(_)
            | Self::Reopen
            | Self::SystemNotificationResponse(_)
            | Self::SystemSleep
            | Self::SystemWake
            | Self::WillOpenAppMenu
            | Self::AppMenuAction(_)
            | Self::Quit => None,
        }
    }

    fn diagnostic_kind(&self) -> NativeCallbackKind {
        match self {
            Self::OpenUrls(_) => NativeCallbackKind::OpenUrls,
            Self::Reopen => NativeCallbackKind::Reopen,
            Self::SystemNotificationResponse(_) => NativeCallbackKind::SystemNotificationResponse,
            Self::SystemSleep => NativeCallbackKind::SystemSleep,
            Self::SystemWake => NativeCallbackKind::SystemWake,
            Self::WillOpenAppMenu => NativeCallbackKind::WillOpenAppMenu,
            Self::AppMenuAction(_) => NativeCallbackKind::AppMenuAction,
            Self::KeyboardLayoutChanged => NativeCallbackKind::KeyboardLayoutChanged,
            Self::ThermalStateChanged => NativeCallbackKind::ThermalStateChanged,
            Self::PowerModeChanged => NativeCallbackKind::PowerModeChanged,
            Self::DisplaysChanged => NativeCallbackKind::DisplaysChanged,
            Self::Quit => NativeCallbackKind::Quit,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NativeEventDrainControl {
    Continue,
    Terminate,
}

pub(super) struct ReservedPointerCancel {
    reason: PointerCancelReason,
    sequence: NativeIngressSequence,
    delivery: ReservedPointerCancelDelivery,
    app: Weak<AppCell>,
}

#[derive(Clone, Copy)]
enum ReservedPointerCancelDelivery {
    PlatformInput {
        slot_generation: u64,
    },
    CapturedDragTerminal {
        generation: NativeCapturedDragGeneration,
    },
}

impl ReservedPointerCancel {
    pub(super) fn platform_input(
        reason: PointerCancelReason,
        sequence: NativeIngressSequence,
        slot_generation: u64,
        app: Weak<AppCell>,
    ) -> Self {
        Self {
            reason,
            sequence,
            delivery: ReservedPointerCancelDelivery::PlatformInput { slot_generation },
            app,
        }
    }

    pub(super) fn captured_drag_terminal(
        reason: PointerCancelReason,
        sequence: NativeIngressSequence,
        generation: NativeCapturedDragGeneration,
        app: Weak<AppCell>,
    ) -> Self {
        Self {
            reason,
            sequence,
            delivery: ReservedPointerCancelDelivery::CapturedDragTerminal { generation },
            app,
        }
    }

    fn diagnostic_metadata(&self) -> (NativeCallbackKind, Option<NativeBoundaryGeneration>) {
        match self.delivery {
            ReservedPointerCancelDelivery::PlatformInput { slot_generation } => (
                NativeCallbackKind::PlatformInput,
                Some(NativeBoundaryGeneration::InputSlot {
                    boundary: crate::NativeInputBoundary::PlatformInput,
                    generation: slot_generation,
                }),
            ),
            ReservedPointerCancelDelivery::CapturedDragTerminal { generation } => (
                NativeCallbackKind::CapturedDragCancellation,
                Some(NativeBoundaryGeneration::CapturedDrag(generation)),
            ),
        }
    }

    fn promote_terminal(&self) {
        if let Some(app) = self.app.upgrade() {
            app.promote_reserved_pointer_cancel(self.sequence);
        }
    }
}

impl Drop for ReservedPointerCancel {
    fn drop(&mut self) {
        if let Some(app) = self.app.upgrade() {
            app.finish_reserved_pointer_cancel(self.sequence);
        }
    }
}

pub(super) enum NativeWindowEvent {
    #[cfg(not(target_family = "wasm"))]
    AccessibilityAction {
        activation_generation: u64,
        request: accesskit::ActionRequest,
    },
    #[cfg(not(target_family = "wasm"))]
    AccessibilityActiveChanged {
        active: bool,
        activation_generation: u64,
    },
    ActiveChanged(PlatformWindowActiveStatusObservation),
    VisibilityChanged(crate::WindowVisibility),
    PointerCanceled(ReservedPointerCancel),
    TouchCancelled {
        event: crate::TouchEvent,
        source: crate::platform::PlatformInputCallbackSource,
    },
    ExternalDrag {
        generation: NativeCapturedDragGeneration,
        event: crate::FileDropEvent,
    },
    ModifiersChanged(ModifiersChangedEvent),
    AppearanceChanged,
    InitialPresentationCompleted,
    InitialPresentationFailed,
    Resized,
    Moved,
    WindowStateChanged,
    DisplayChanged,
    ButtonLayoutChanged,
    CloseRequested,
    CloseRequired,
    Closed,
    HoverChanged(bool),
    RequestFrame(RequestFrameOptions),
    SystemTabCommand(NativeSystemTabCommand),
    WindowMutationObserved(PlatformWindowMutationObservation),
}

impl NativeWindowEvent {
    fn permits_pointer_capture_release_retry(&self) -> bool {
        matches!(
            self,
            Self::ActiveChanged(_)
                | Self::PointerCanceled(_)
                | Self::CloseRequested
                | Self::CloseRequired
                | Self::Closed
                | Self::HoverChanged(_)
        )
    }

    fn coalesce_domain(&self) -> Option<NativeWindowEventCoalesceDomain> {
        match self {
            Self::AppearanceChanged => Some(NativeWindowEventCoalesceDomain::Appearance),
            Self::Resized | Self::Moved | Self::WindowStateChanged => {
                Some(NativeWindowEventCoalesceDomain::Bounds)
            }
            Self::ButtonLayoutChanged => Some(NativeWindowEventCoalesceDomain::ButtonLayout),
            Self::DisplayChanged => Some(NativeWindowEventCoalesceDomain::Display),
            Self::RequestFrame(_) => Some(NativeWindowEventCoalesceDomain::Frame),
            #[cfg(not(target_family = "wasm"))]
            Self::AccessibilityAction { .. } | Self::AccessibilityActiveChanged { .. } => None,
            Self::ActiveChanged(_)
            | Self::VisibilityChanged(_)
            | Self::PointerCanceled(_)
            | Self::TouchCancelled { .. }
            | Self::ExternalDrag { .. }
            | Self::ModifiersChanged(_)
            | Self::InitialPresentationCompleted
            | Self::InitialPresentationFailed
            | Self::CloseRequested
            | Self::CloseRequired
            | Self::Closed
            | Self::HoverChanged(_)
            | Self::SystemTabCommand(_)
            | Self::WindowMutationObserved(_) => None,
        }
    }

    fn diagnostic_metadata(&self) -> (NativeCallbackKind, Option<NativeBoundaryGeneration>) {
        match self {
            #[cfg(not(target_family = "wasm"))]
            Self::AccessibilityAction {
                activation_generation,
                ..
            } => (
                NativeCallbackKind::AccessibilityAction,
                Some(NativeBoundaryGeneration::AccessibilityActivation(
                    *activation_generation,
                )),
            ),
            #[cfg(not(target_family = "wasm"))]
            Self::AccessibilityActiveChanged {
                active,
                activation_generation,
            } => (
                if *active {
                    NativeCallbackKind::AccessibilityActivated
                } else {
                    NativeCallbackKind::AccessibilityDeactivated
                },
                Some(NativeBoundaryGeneration::AccessibilityActivation(
                    *activation_generation,
                )),
            ),
            Self::ActiveChanged(_) => (NativeCallbackKind::ActiveChanged, None),
            Self::VisibilityChanged(_) => (NativeCallbackKind::VisibilityChanged, None),
            Self::PointerCanceled(reservation) => reservation.diagnostic_metadata(),
            Self::TouchCancelled { source, .. } => (
                NativeCallbackKind::PlatformInput,
                Some(NativeBoundaryGeneration::InputSlot {
                    boundary: crate::NativeInputBoundary::PlatformInput,
                    generation: source.generation(),
                }),
            ),
            Self::ExternalDrag { generation, .. } => (
                NativeCallbackKind::PlatformInput,
                Some(NativeBoundaryGeneration::CapturedDrag(*generation)),
            ),
            Self::ModifiersChanged(_) => (NativeCallbackKind::ModifiersChanged, None),
            Self::AppearanceChanged => (NativeCallbackKind::AppearanceChanged, None),
            Self::InitialPresentationCompleted => {
                (NativeCallbackKind::InitialPresentationCompleted, None)
            }
            Self::InitialPresentationFailed => {
                (NativeCallbackKind::InitialPresentationFailed, None)
            }
            Self::Resized => (NativeCallbackKind::Resized, None),
            Self::Moved => (NativeCallbackKind::Moved, None),
            Self::WindowStateChanged => (NativeCallbackKind::WindowStateChanged, None),
            Self::DisplayChanged => (NativeCallbackKind::DisplayChanged, None),
            Self::ButtonLayoutChanged => (NativeCallbackKind::ButtonLayoutChanged, None),
            Self::CloseRequested => (NativeCallbackKind::ShouldClose, None),
            Self::CloseRequired => (NativeCallbackKind::CloseRequired, None),
            Self::Closed => (NativeCallbackKind::Closed, None),
            Self::HoverChanged(_) => (NativeCallbackKind::HoverChanged, None),
            Self::RequestFrame(_) => (NativeCallbackKind::RequestFrame, None),
            Self::SystemTabCommand(command) => (command.diagnostic_kind(), None),
            Self::WindowMutationObserved(observation) => (
                NativeCallbackKind::WindowMutationObserved,
                Some(NativeBoundaryGeneration::WindowMutation {
                    domain: observation.domain,
                    generation: observation.generation,
                }),
            ),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum NativeSystemTabCommand {
    MergeAll,
    MoveToNewWindow,
    SelectNext,
    SelectPrevious,
    ToggleBar,
}

impl NativeSystemTabCommand {
    fn diagnostic_kind(self) -> NativeCallbackKind {
        match self {
            Self::MergeAll => NativeCallbackKind::SystemTabMergeAll,
            Self::MoveToNewWindow => NativeCallbackKind::SystemTabMoveToNewWindow,
            Self::SelectNext => NativeCallbackKind::SystemTabSelectNext,
            Self::SelectPrevious => NativeCallbackKind::SystemTabSelectPrevious,
            Self::ToggleBar => NativeCallbackKind::SystemTabToggleBar,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeEventCoalesceKey {
    App(NativeAppEventCoalesceDomain),
    Window {
        window_id: WindowId,
        domain: NativeWindowEventCoalesceDomain,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeAppEventCoalesceDomain {
    KeyboardLayout,
    ThermalState,
    PowerMode,
    Displays,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeWindowEventCoalesceDomain {
    Appearance,
    Bounds,
    Display,
    ButtonLayout,
    Frame,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NativeEventDisposition {
    Delivered,
    StaleWindow,
}

pub(super) struct NativeEventDelivery {
    pub(super) control: NativeEventDrainControl,
    pub(super) disposition: NativeEventDisposition,
}

fn deliver_window_event(
    app: &mut App,
    window_id: WindowId,
    sequence: NativeIngressSequence,
    event: impl FnOnce(&mut Window, &mut App),
) -> NativeEventDisposition {
    if app
        .update_window_id_from_native(window_id, sequence, |_, window, cx| event(window, cx))
        .is_ok()
    {
        NativeEventDisposition::Delivered
    } else {
        NativeEventDisposition::StaleWindow
    }
}

fn deliver_window_event_with_preclaimed_captured_drag(
    app: &mut App,
    window_id: WindowId,
    sequence: NativeIngressSequence,
    event: impl FnOnce(&mut Window, &mut App),
) -> NativeEventDisposition {
    if app
        .update_window_id_from_native_with_preclaimed_captured_drag(
            window_id,
            sequence,
            |_, window, cx| event(window, cx),
        )
        .is_ok()
    {
        NativeEventDisposition::Delivered
    } else {
        NativeEventDisposition::StaleWindow
    }
}

fn deliver_window_event_if(
    app: &mut App,
    window_id: WindowId,
    sequence: NativeIngressSequence,
    event: impl FnOnce(&mut Window, &mut App) -> bool,
) -> NativeEventDisposition {
    match app.update_window_id_from_native(window_id, sequence, |_, window, cx| event(window, cx)) {
        Ok(true) => NativeEventDisposition::Delivered,
        Ok(false) | Err(_) => NativeEventDisposition::StaleWindow,
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use crate::FrameRequestSource;
    use accesskit::{Action as AccessibleAction, ActionRequest, NodeId, TreeId};
    use open_gpui_scheduler::Instant;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    fn assert_send_sync<T: Send + Sync>() {}

    fn event_ingress() -> NativeEventIngress {
        let dispatcher = Arc::new(crate::TestDispatcher::new(0));
        NativeEventIngress::new(ForegroundExecutor::new(dispatcher), Weak::new())
    }

    #[test]
    fn nested_frame_coalescing_retains_the_first_sequence_signal_and_diagnostics() {
        let ingress = event_ingress();
        let barrier = ingress
            .begin_input_barrier()
            .expect("an idle ingress admits a native input transaction");
        let phase = ingress.phase.get();
        let window_id = WindowId::from(1);
        let first = Instant::now();
        ingress.enqueue(
            window_id,
            NativeWindowEvent::RequestFrame(RequestFrameOptions {
                require_presentation: true,
                signal_at: Some(first),
                signal_source: FrameRequestSource::LocalSchedule,
                ..Default::default()
            }),
        );
        let input_cutoff = ingress.reserve_input_sequence();
        for offset in [16, 32] {
            ingress.enqueue(
                window_id,
                NativeWindowEvent::RequestFrame(RequestFrameOptions {
                    force_render: true,
                    signal_at: Some(first + Duration::from_millis(offset)),
                    signal_source: FrameRequestSource::NativeCallback,
                    ..Default::default()
                }),
            );
        }

        assert_eq!(ingress.phase.get(), phase);
        assert!(ingress.has_pending_before(input_cutoff));
        let diagnostics =
            ingress.diagnostics_snapshot_since(NativeBoundaryDiagnosticCursor::default());
        assert_eq!(diagnostics.pending.len(), 1);
        assert_eq!(diagnostics.pending[0].sequence, 0);
        assert_eq!(
            diagnostics.pending[0].kind,
            NativeBoundaryKind::Callback(NativeCallbackKind::RequestFrame)
        );
        assert_eq!(diagnostics.terminal.len(), 2);
        for (diagnostic, sequence) in diagnostics.terminal.iter().zip([2, 3]) {
            assert_eq!(diagnostic.sequence, sequence);
            assert_eq!(diagnostic.target, NativeBoundaryTarget::Window(window_id));
            assert_eq!(
                diagnostic.disposition,
                NativeBoundaryDisposition::Coalesced { into_sequence: 0 }
            );
        }

        let NativeEventPrefixPop::Event(checkout) = barrier.pop_event_before(input_cutoff.value())
        else {
            panic!("coalesced frame work must remain before the later input reservation");
        };
        assert_eq!(checkout.event.sequence(), 0);
        let NativeEventTarget::Window {
            window_id: retained_window,
            event: NativeWindowEvent::RequestFrame(options),
        } = checkout.event.target
        else {
            panic!("the first frame callback must retain its window authority");
        };
        assert_eq!(retained_window, window_id);
        assert!(options.require_presentation);
        assert!(options.force_render);
        assert_eq!(options.signal_at, Some(first));
        assert_eq!(options.signal_source, FrameRequestSource::LocalSchedule);
    }

    #[test]
    fn frame_coalescing_respects_intervening_events_and_window_generations() {
        let ingress = event_ingress();
        let original_window = WindowId::from(1);
        let replacement_window = WindowId::from((7_u64 << 32) | 1);
        for (window_id, event) in [
            (
                original_window,
                NativeWindowEvent::RequestFrame(RequestFrameOptions::default()),
            ),
            (original_window, NativeWindowEvent::Resized),
            (
                original_window,
                NativeWindowEvent::RequestFrame(RequestFrameOptions::default()),
            ),
            (
                replacement_window,
                NativeWindowEvent::RequestFrame(RequestFrameOptions::default()),
            ),
            (
                original_window,
                NativeWindowEvent::RequestFrame(RequestFrameOptions::default()),
            ),
        ] {
            ingress.enqueue(window_id, event);
        }
        let diagnostics =
            ingress.diagnostics_snapshot_since(NativeBoundaryDiagnosticCursor::default());
        assert_eq!(
            diagnostics
                .pending
                .iter()
                .map(|diagnostic| diagnostic.sequence)
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4]
        );
        assert!(diagnostics.terminal.is_empty());
    }

    #[test]
    fn non_frame_coalescing_preserves_the_latest_callback_sequence() {
        let ingress = event_ingress();
        let window_id = WindowId::from(1);
        ingress.enqueue(window_id, NativeWindowEvent::AppearanceChanged);
        ingress.enqueue(window_id, NativeWindowEvent::AppearanceChanged);
        let diagnostics =
            ingress.diagnostics_snapshot_since(NativeBoundaryDiagnosticCursor::default());
        assert_eq!(diagnostics.pending.len(), 1);
        assert_eq!(diagnostics.pending[0].sequence, 1);
        assert_eq!(diagnostics.terminal.len(), 1);
        assert_eq!(diagnostics.terminal[0].sequence, 0);
        assert_eq!(
            diagnostics.terminal[0].disposition,
            NativeBoundaryDisposition::Coalesced { into_sequence: 1 }
        );
    }

    #[test]
    fn owned_local_task_authority_terminal_is_idempotent() {
        let active = Rc::new(Cell::new(0));
        let (lease, terminal) = NativeOwnedLocalTaskAuthorityLease::acquire(Rc::clone(&active));

        assert_eq!(active.get(), 1);
        terminal.settle();
        assert_eq!(active.get(), 0);

        drop(lease);
        terminal.settle();
        assert_eq!(
            active.get(),
            0,
            "future teardown and owner cancellation must share one terminal authority"
        );
    }

    fn action_request() -> ActionRequest {
        ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: NodeId(7),
            data: None,
        }
    }

    #[test]
    fn cross_thread_ingress_handle_is_send_and_sync() {
        assert_send_sync::<NativeCrossThreadIngress>();
    }

    #[test]
    fn cross_thread_app_notifications_are_sequenced_before_later_local_event() {
        let sequencer = Arc::new(Mutex::new(NativeSequenceAuthority::new()));
        let (wake_sender, _wake_receiver) = async_channel::bounded(1);
        let ingress = NativeCrossThreadIngress::new(sequencer.clone(), wake_sender);

        std::thread::spawn(move || {
            ingress.system_wake();
            ingress.thermal_state_changed();
        })
        .join()
        .expect("native notification worker should complete");

        let (mut staged, local_sequence) = sequencer.lock().take_staged_before_reserving();
        let system_wake = staged
            .pop_front()
            .expect("the raw system-wake notification must stage typed app work");
        let thermal_state_change = staged
            .pop_front()
            .expect("the raw thermal notification must stage typed app work");
        let later_event = NativeEventEnvelope {
            sequence: local_sequence,
            target: NativeEventTarget::App(NativeAppEvent::Reopen),
        };

        assert!(staged.is_empty());
        assert_eq!(
            [
                system_wake.sequence,
                thermal_state_change.sequence,
                later_event.sequence,
            ],
            [0, 1, 2]
        );
        assert!(matches!(
            system_wake.event,
            StagedNativeEvent::App(StagedNativeAppEvent::SystemWake)
        ));
        assert!(matches!(
            thermal_state_change.event,
            StagedNativeEvent::App(StagedNativeAppEvent::ThermalStateChanged)
        ));
        assert!(
            system_wake.sequence < thermal_state_change.sequence
                && thermal_state_change.sequence < later_event.sequence,
            "raw ingress must reserve both notification sequences before a later local event"
        );
    }

    #[test]
    fn accessibility_callbacks_are_sequenced_before_later_local_work_without_coalescing() {
        let sequencer = Arc::new(Mutex::new(NativeSequenceAuthority::new()));
        let (wake_sender, _wake_receiver) = async_channel::bounded(1);
        let ingress = NativeCrossThreadIngress::new(sequencer.clone(), wake_sender);
        let active_state = Arc::new(AtomicU64::new(0));
        let original_window = WindowId::from(1);
        let replacement_window = WindowId::from((7_u64 << 32) | 1);

        let worker_ingress = ingress.clone();
        let worker_state = active_state.clone();
        std::thread::spawn(move || {
            assert!(worker_ingress.activated(original_window, &worker_state));
        })
        .join()
        .expect("accessibility callback worker should complete");
        ingress.action(original_window, &active_state, action_request());
        ingress.deactivated(replacement_window, &active_state);

        let (staged, local_sequence) = sequencer.lock().take_staged_before_reserving();
        assert_eq!(local_sequence, 3);
        assert_eq!(staged.len(), 3, "callback payloads must not coalesce");
        assert_eq!(
            staged
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert!(matches!(
            &staged[0].event,
            StagedNativeEvent::Accessibility {
                window_id,
                event: StagedNativeAccessibilityEvent::ActiveChanged {
                    active: true,
                    activation_generation: 1,
                },
            } if *window_id == original_window
        ));
        assert!(matches!(
            &staged[1].event,
            StagedNativeEvent::Accessibility {
                window_id,
                event: StagedNativeAccessibilityEvent::Action {
                    activation_generation: 1,
                    ..
                },
            } if *window_id == original_window
        ));
        assert!(matches!(
            &staged[2].event,
            StagedNativeEvent::Accessibility {
                window_id,
                event: StagedNativeAccessibilityEvent::ActiveChanged {
                    active: false,
                    activation_generation: 2,
                },
            } if *window_id == replacement_window
        ));
        assert_eq!(
            active_state.load(Ordering::SeqCst),
            2 << 1,
            "activation generation changes must linearize with callback sequences"
        );
    }

    #[test]
    fn terminated_cross_thread_sequencer_discards_staged_and_future_callbacks() {
        let sequencer = Arc::new(Mutex::new(NativeSequenceAuthority::new()));
        let (wake_sender, _wake_receiver) = async_channel::bounded(1);
        let ingress = NativeCrossThreadIngress::new(sequencer.clone(), wake_sender);
        let active_state = AtomicU64::new(0);
        let window_id = WindowId::from(1);

        assert!(ingress.activated(window_id, &active_state));
        ingress.system_wake();
        sequencer.lock().terminate();
        assert!(!ingress.activated(window_id, &active_state));
        ingress.deactivated(window_id, &active_state);
        ingress.thermal_state_changed();

        let sequencer = sequencer.lock();
        assert!(sequencer.staged_cross_thread.is_empty());
        assert_eq!(
            sequencer.next_sequence, 2,
            "callbacks after termination must not reserve or stage native work"
        );
        assert_eq!(
            active_state.load(Ordering::SeqCst),
            (1 << 1) | 1,
            "callbacks after termination must not mutate requested activation state"
        );
    }

    #[test]
    fn shutdown_fence_refuses_activation_without_consuming_sequence_or_generation() {
        let sequencer = Arc::new(Mutex::new(NativeSequenceAuthority::new()));
        let (wake_sender, _wake_receiver) = async_channel::bounded(1);
        let ingress = NativeCrossThreadIngress::new(sequencer.clone(), wake_sender);
        let active_state = AtomicU64::new(0);
        let window_id = WindowId::from(1);
        sequencer.lock().shutdown_generation = Some(17);
        assert!(!ingress.activated(window_id, &active_state));
        assert_eq!(active_state.load(Ordering::SeqCst), 0);
        {
            let mut state = sequencer.lock();
            assert_eq!(state.next_sequence, 0);
            assert!(state.staged_cross_thread.is_empty());
            assert!(!state.terminated);
            state.shutdown_generation = None;
        }
        assert!(ingress.activated(window_id, &active_state));
        assert_eq!(sequencer.lock().next_sequence, 1);
        assert_eq!(active_state.load(Ordering::SeqCst), (1 << 1) | 1);
    }
}
