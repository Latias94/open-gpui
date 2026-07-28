use std::{
    cell::{BorrowMutError, Ref, RefCell, RefMut},
    ops::{Deref, DerefMut},
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

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
    native_event_ingress::{
        NativeAppEvent, NativeEventDisposition, NativeEventDrainControl, NativeEventIngress,
        NativeEventPrefixPop, NativeWindowEvent, NativeWorkEnvelope, NativeWorkPop,
    },
    native_platform_commands::NativePlatformCommandRejection,
    native_query_snapshot::{NativeQuerySnapshots, NativeWindowLifecycle},
};
use crate::{
    Action, DispatchEventResult, NativeInputInvariantViolation, PlatformInput,
    PlatformWindowCommand, PlatformWindowCommandDispatcher, PlatformWindowCommandOutcome,
    WindowControlArea, WindowId,
};

type OpenUrlsHandler = dyn FnMut(Vec<String>, &mut App);
type AppHandler = dyn FnMut(&mut App);

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
        match catch_unwind(AssertUnwindSafe(callback)) {
            Ok(result) => result,
            Err(payload) => {
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
    native_events: NativeEventIngress,
    native_queries: NativeQuerySnapshots,
    open_urls_handler: NativeAppHandlerSlot<OpenUrlsHandler>,
    reopen_handler: NativeAppHandlerSlot<AppHandler>,
    system_wake_handler: NativeAppHandlerSlot<AppHandler>,
    platform_input_leases: RefCell<Vec<u64>>,
    input_handler_leases: RefCell<Vec<u64>>,
}

impl AppCell {
    pub(super) fn new(app: App) -> Self {
        let foreground_executor = app.foreground_executor.clone();
        let this = app.this.clone();
        Self {
            app: RefCell::new(app),
            native_events: NativeEventIngress::new(foreground_executor, this),
            native_queries: NativeQuerySnapshots::default(),
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
            sequence_cutoff,
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
            match barrier.pop_event_before(sequence_cutoff) {
                NativeEventPrefixPop::Event(envelope) => {
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
                    let delivery =
                        event_terminal.run_callback(|| app.update(|app| envelope.deliver(app)));
                    drop(app);
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
        let result = terminal.run_callback(|| {
            app.update_window_id(window_id, |_, window, cx| window.dispatch_event(event, cx))
        });
        drop(app);

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
            sequence_cutoff,
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
            match barrier.pop_event_before(sequence_cutoff) {
                NativeEventPrefixPop::Event(envelope) => {
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
                    let delivery =
                        event_terminal.run_callback(|| app.update(|app| envelope.deliver(app)));
                    drop(app);
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
        let result = terminal
            .run_callback(|| app.update_window_id(window_id, |_, window, cx| callback(window, cx)));
        drop(app);

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
            sequence,
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
            let mut terminal = NativeBoundaryTerminalGuard::new(
                &self.native_events,
                envelope.pending_diagnostic(),
            );
            let should_close = if app.quitting {
                terminal.settle(NativeBoundaryDisposition::Closed);
                true
            } else {
                let should_close = terminal.run_callback(|| {
                    app.update(|app| {
                        app.update_window_id(window_id, |_, window, cx| window.should_close(cx))
                            .unwrap_or(false)
                    })
                });
                terminal.settle(NativeBoundaryDisposition::DELIVERED);
                should_close
            };
            drop(app);
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
                                terminal.settle(NativeBoundaryDisposition::Closed);
                                drain.terminate();
                                return;
                            }
                            let delivery =
                                terminal.run_callback(|| app.update(|app| event.deliver(app)));
                            drop(app);
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
                    }
                }
                NativeWorkPop::Empty | NativeWorkPop::BudgetExhausted => return,
            }
        }
    }

    fn app_borrow_released(&self) {
        if self.native_callback_lease_active() {
            return;
        }
        if !self.app_is_idle() {
            return;
        }
        self.native_events.resume_after_app_borrow();
        self.drain_native_work(None);
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
        if !app.native_callback_lease_active() && app.app_is_idle() {
            app.native_events.resume_after_app_borrow();
            app.drain_native_work(None);
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
