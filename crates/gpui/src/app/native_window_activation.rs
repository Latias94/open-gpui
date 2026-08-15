use std::{
    any::Any,
    cell::{Cell, RefCell},
    collections::{BTreeMap, VecDeque},
    fmt, mem,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::{Rc, Weak},
};

use crate::{Subscription, WindowId};

use super::cell::AppCell;

/// Terminal result of one programmatic native-window activation request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowActivationTerminal {
    /// An exact native focus or foreground observation identified the target window.
    Activated,
    /// The platform rejected the activation command.
    Rejected,
    /// The target backend cannot provide observed programmatic activation.
    Unsupported,
    /// A newer owned-window activation won before this request completed.
    Superseded,
    /// The coherent activation policy changed while this request was pending.
    PolicyChanged,
    /// The semantic target associated with the request was replaced.
    TargetReplaced,
    /// The caller explicitly cancelled this request.
    Cancelled,
    /// The exact target window or application became terminal.
    WindowClosed,
}

/// Observable state of one programmatic native-window activation request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowActivationStatus {
    /// The request owns a queued native command.
    Queued,
    /// The backend command is executing and may synchronously pump native callbacks.
    Dispatching,
    /// The backend accepted the command; exact native focus is still unproven.
    Dispatched,
    /// The first terminal result.
    Terminal(WindowActivationTerminal),
}

impl WindowActivationStatus {
    /// Returns the terminal result, if one has been recorded.
    pub const fn terminal(self) -> Option<WindowActivationTerminal> {
        match self {
            Self::Terminal(terminal) => Some(terminal),
            Self::Queued | Self::Dispatching | Self::Dispatched => None,
        }
    }

    /// Returns whether this state is terminal.
    pub const fn is_terminal(self) -> bool {
        self.terminal().is_some()
    }
}

/// Immutable observation of one programmatic activation ticket.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowActivationSnapshot {
    target: WindowId,
    request_generation: u64,
    activation_policy_generation: u64,
    status: WindowActivationStatus,
}

impl WindowActivationSnapshot {
    /// Exact target window bound to this request.
    pub const fn target(self) -> WindowId {
        self.target
    }

    /// Monotonic application-wide activation request generation.
    pub const fn request_generation(self) -> u64 {
        self.request_generation
    }

    /// Coherent committed activation-policy generation bound to this request.
    pub const fn activation_policy_generation(self) -> u64 {
        self.activation_policy_generation
    }

    /// Current request state.
    pub const fn status(self) -> WindowActivationStatus {
        self.status
    }
}

/// Result of explicitly terminating a programmatic activation ticket.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowActivationCancellationOutcome {
    /// This call installed the contained terminal result.
    Installed(WindowActivationTerminal),
    /// The ticket already owned the contained first terminal result.
    AlreadyTerminal(WindowActivationTerminal),
}

/// Generation-bound observation handle for one programmatic native-window activation.
///
/// Platform command acceptance is only dispatch evidence. The ticket reaches
/// [`WindowActivationTerminal::Activated`] only after GPUI observes the exact target as the native
/// focus or foreground owner.
#[must_use = "programmatic activation must observe or explicitly discard its terminal ticket"]
#[derive(Clone)]
pub struct WindowActivationTicket {
    app: Weak<AppCell>,
    target: WindowId,
    request_generation: u64,
    activation_policy_generation: u64,
    state: Rc<RefCell<WindowActivationTicketState>>,
}

impl WindowActivationTicket {
    pub(super) fn new(
        app: Weak<AppCell>,
        target: WindowId,
        request_generation: u64,
        activation_policy_generation: u64,
    ) -> Self {
        Self {
            app,
            target,
            request_generation,
            activation_policy_generation,
            state: Rc::new(RefCell::new(WindowActivationTicketState::default())),
        }
    }

    pub(crate) fn terminal(
        app: Weak<AppCell>,
        target: WindowId,
        request_generation: u64,
        activation_policy_generation: u64,
        terminal: WindowActivationTerminal,
    ) -> Self {
        let ticket = Self::new(
            app,
            target,
            request_generation,
            activation_policy_generation,
        );
        let _ = ticket.settle(terminal);
        ticket
    }

    /// Returns the latest immutable ticket observation.
    pub fn snapshot(&self) -> WindowActivationSnapshot {
        WindowActivationSnapshot {
            target: self.target,
            request_generation: self.request_generation,
            activation_policy_generation: self.activation_policy_generation,
            status: self.state.borrow().status,
        }
    }

    /// Subscribes to the ticket's first terminal observation.
    ///
    /// Delivery is always deferred through the application-owned activation authority while the
    /// application exists. An already-terminal subscription therefore cannot reenter a live App or
    /// Window borrow synchronously.
    pub fn subscribe(
        &self,
        callback: impl FnOnce(WindowActivationSnapshot) + 'static,
    ) -> Subscription {
        let mut callback = Some(Box::new(callback) as WindowActivationObserver);
        let cancelled = Rc::new(Cell::new(false));
        let (immediate, observer_id) = {
            let mut state = self.state.borrow_mut();
            if let Some(terminal) = state.status.terminal() {
                (
                    Some(WindowActivationTicketDelivery {
                        snapshot: self
                            .snapshot_with_status(WindowActivationStatus::Terminal(terminal)),
                        observers: vec![WindowActivationObserverEntry {
                            callback: callback
                                .take()
                                .expect("activation callback must be present"),
                            cancelled: cancelled.clone(),
                        }],
                    }),
                    None,
                )
            } else {
                let observer_id = state.next_observer_id;
                state.next_observer_id = state
                    .next_observer_id
                    .checked_add(1)
                    .expect("window activation observer identity space exhausted");
                state.observers.insert(
                    observer_id,
                    WindowActivationObserverEntry {
                        callback: callback
                            .take()
                            .expect("activation callback must be present"),
                        cancelled: cancelled.clone(),
                    },
                );
                (None, Some(observer_id))
            }
        };

        if let Some(delivery) = immediate {
            self.schedule_delivery(delivery);
            return Subscription::new(move || cancelled.set(true));
        }

        let state = Rc::downgrade(&self.state);
        let observer_id = observer_id.expect("pending activation observer must have an identity");
        Subscription::new(move || {
            cancelled.set(true);
            if let Some(state) = state.upgrade() {
                state.borrow_mut().observers.remove(&observer_id);
            }
        })
    }

    /// Explicitly cancels this exact activation request.
    pub fn cancel(&self) -> WindowActivationCancellationOutcome {
        self.cancel_with(WindowActivationTerminal::Cancelled)
    }

    /// Marks this ticket terminal because its semantic target was replaced.
    #[doc(hidden)]
    pub fn cancel_for_target_replacement(&self) -> WindowActivationCancellationOutcome {
        self.cancel_with(WindowActivationTerminal::TargetReplaced)
    }

    pub(super) fn request_generation(&self) -> u64 {
        self.request_generation
    }

    pub(super) fn target(&self) -> WindowId {
        self.target
    }

    pub(super) fn activation_policy_generation(&self) -> u64 {
        self.activation_policy_generation
    }

    fn cancel_with(
        &self,
        terminal: WindowActivationTerminal,
    ) -> WindowActivationCancellationOutcome {
        if let Some(existing) = self.snapshot().status().terminal() {
            return WindowActivationCancellationOutcome::AlreadyTerminal(existing);
        }
        if let Some(app) = self.app.upgrade() {
            return app.cancel_native_window_activation(self, terminal);
        }
        match self.settle(WindowActivationTerminal::WindowClosed) {
            Some(delivery) => {
                delivery.deliver();
                WindowActivationCancellationOutcome::Installed(
                    WindowActivationTerminal::WindowClosed,
                )
            }
            None => WindowActivationCancellationOutcome::AlreadyTerminal(
                self.snapshot()
                    .status()
                    .terminal()
                    .expect("a failed activation settlement must already be terminal"),
            ),
        }
    }

    fn schedule_delivery(&self, delivery: WindowActivationTicketDelivery) {
        if let Some(app) = self.app.upgrade() {
            app.schedule_native_window_activation_delivery(delivery);
        } else {
            delivery.deliver();
        }
    }

    fn snapshot_with_status(&self, status: WindowActivationStatus) -> WindowActivationSnapshot {
        WindowActivationSnapshot {
            target: self.target,
            request_generation: self.request_generation,
            activation_policy_generation: self.activation_policy_generation,
            status,
        }
    }

    fn transition(&self, expected: WindowActivationStatus, next: WindowActivationStatus) -> bool {
        debug_assert!(!next.is_terminal());
        let mut state = self.state.borrow_mut();
        if state.status != expected {
            return false;
        }
        state.status = next;
        true
    }

    fn settle(&self, terminal: WindowActivationTerminal) -> Option<WindowActivationTicketDelivery> {
        let observers = {
            let mut state = self.state.borrow_mut();
            if state.status.is_terminal() {
                return None;
            }
            state.status = WindowActivationStatus::Terminal(terminal);
            mem::take(&mut state.observers)
                .into_values()
                .collect::<Vec<_>>()
        };
        Some(WindowActivationTicketDelivery {
            snapshot: self.snapshot_with_status(WindowActivationStatus::Terminal(terminal)),
            observers,
        })
    }
}

impl fmt::Debug for WindowActivationTicket {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WindowActivationTicket")
            .field("snapshot", &self.snapshot())
            .finish()
    }
}

type WindowActivationObserver = Box<dyn FnOnce(WindowActivationSnapshot) + 'static>;

struct WindowActivationTicketState {
    status: WindowActivationStatus,
    observers: BTreeMap<usize, WindowActivationObserverEntry>,
    next_observer_id: usize,
}

impl Default for WindowActivationTicketState {
    fn default() -> Self {
        Self {
            status: WindowActivationStatus::Queued,
            observers: BTreeMap::new(),
            next_observer_id: 0,
        }
    }
}

pub(super) struct WindowActivationTicketDelivery {
    snapshot: WindowActivationSnapshot,
    observers: Vec<WindowActivationObserverEntry>,
}

impl WindowActivationTicketDelivery {
    pub(super) fn deliver(self) {
        let mut first_panic: Option<Box<dyn Any + Send>> = None;
        for observer in self.observers {
            if observer.cancelled.get() {
                continue;
            }
            if let Err(payload) =
                catch_unwind(AssertUnwindSafe(|| (observer.callback)(self.snapshot)))
            {
                if first_panic.is_none() {
                    first_panic = Some(payload);
                } else {
                    log::error!(
                        "suppressed secondary panic while delivering window activation observers"
                    );
                }
            }
        }
        if let Some(payload) = first_panic {
            resume_unwind(payload);
        }
    }
}

struct WindowActivationObserverEntry {
    callback: WindowActivationObserver,
    cancelled: Rc<Cell<bool>>,
}

pub(super) struct NativeWindowActivationBegin {
    pub(super) ticket: WindowActivationTicket,
    pub(super) displaced: Vec<WindowActivationTicketDelivery>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExactOwnedWindowGain {
    observed_window: WindowId,
    ingress_sequence: u64,
}

struct PendingActivationRecord {
    ticket: WindowActivationTicket,
    command_sequence: Option<u64>,
    first_positive: Option<ExactOwnedWindowGain>,
}

#[derive(Default)]
pub(super) struct NativeWindowActivationDispatchCompletion {
    pub(super) delivery: Option<WindowActivationTicketDelivery>,
    pub(super) needs_readback: bool,
}

/// Application-owned authority for generation-bound programmatic native activation.
///
/// Native activation APIs may synchronously pump focus callbacks. Records therefore remain in
/// this module while dispatch is in flight, and the first exact positive observation is combined
/// with the eventual dispatch outcome before any terminal ticket is published.
#[derive(Default)]
pub(super) struct NativeWindowActivationAuthority {
    next_generation: Cell<u64>,
    current_generation: Cell<Option<u64>>,
    records: RefCell<BTreeMap<u64, PendingActivationRecord>>,
    deliveries: RefCell<VecDeque<WindowActivationTicketDelivery>>,
    delivering: Cell<bool>,
}

impl NativeWindowActivationAuthority {
    pub(super) fn begin(
        &self,
        app: Weak<AppCell>,
        target: WindowId,
        activation_policy_generation: u64,
    ) -> NativeWindowActivationBegin {
        let request_generation = self
            .next_generation
            .get()
            .checked_add(1)
            .expect("window activation request generation overflowed");
        self.next_generation.set(request_generation);
        let ticket = WindowActivationTicket::new(
            app,
            target,
            request_generation,
            activation_policy_generation,
        );

        let mut displaced = Vec::new();
        let mut records = self.records.borrow_mut();
        if let Some(current_generation) = self.current_generation.get() {
            let retain_dispatching_evidence =
                records.get(&current_generation).is_some_and(|record| {
                    record.ticket.snapshot().status() == WindowActivationStatus::Dispatching
                        && record.first_positive.is_some()
                });
            if !retain_dispatching_evidence
                && let Some(record) = records.remove(&current_generation)
                && let Some(delivery) = record.ticket.settle(WindowActivationTerminal::Superseded)
            {
                displaced.push(delivery);
            }
        }
        records.insert(
            request_generation,
            PendingActivationRecord {
                ticket: ticket.clone(),
                command_sequence: None,
                first_positive: None,
            },
        );
        self.current_generation.set(Some(request_generation));
        drop(records);

        NativeWindowActivationBegin { ticket, displaced }
    }

    pub(super) fn bind_command_sequence(
        &self,
        ticket: &WindowActivationTicket,
        sequence: u64,
    ) -> bool {
        let mut records = self.records.borrow_mut();
        let Some(record) = records.get_mut(&ticket.request_generation()) else {
            return false;
        };
        if !same_ticket(&record.ticket, ticket) || record.command_sequence.is_some() {
            return false;
        }
        record.command_sequence = Some(sequence);
        true
    }

    pub(super) fn begin_dispatch(
        &self,
        target: WindowId,
        request_generation: u64,
        command_sequence: u64,
    ) -> bool {
        if self.current_generation.get() != Some(request_generation) {
            return false;
        }
        let mut records = self.records.borrow_mut();
        let Some(record) = records.get_mut(&request_generation) else {
            return false;
        };
        record.ticket.target() == target
            && record.command_sequence == Some(command_sequence)
            && record.ticket.transition(
                WindowActivationStatus::Queued,
                WindowActivationStatus::Dispatching,
            )
    }

    pub(super) fn finish_dispatch_accepted(
        &self,
        target: WindowId,
        request_generation: u64,
    ) -> NativeWindowActivationDispatchCompletion {
        let mut records = self.records.borrow_mut();
        let Some(record) = records.get(&request_generation) else {
            return NativeWindowActivationDispatchCompletion::default();
        };
        if record.ticket.target() != target
            || record.ticket.snapshot().status() != WindowActivationStatus::Dispatching
        {
            return NativeWindowActivationDispatchCompletion::default();
        }

        if let Some(positive) = record.first_positive {
            debug_assert!(
                record
                    .command_sequence
                    .is_some_and(|command_sequence| positive.ingress_sequence > command_sequence),
                "activation evidence must follow its native command"
            );
            let record = records
                .remove(&request_generation)
                .expect("checked activation record must remain present");
            self.clear_current_if(request_generation);
            drop(records);
            let terminal = if positive.observed_window == target {
                WindowActivationTerminal::Activated
            } else {
                WindowActivationTerminal::Superseded
            };
            return NativeWindowActivationDispatchCompletion {
                delivery: record.ticket.settle(terminal),
                needs_readback: false,
            };
        }

        let record = records
            .get_mut(&request_generation)
            .expect("checked activation record must remain present");
        let transitioned = record.ticket.transition(
            WindowActivationStatus::Dispatching,
            WindowActivationStatus::Dispatched,
        );
        debug_assert!(transitioned, "dispatching activation must advance once");
        NativeWindowActivationDispatchCompletion {
            delivery: None,
            needs_readback: transitioned,
        }
    }

    pub(super) fn settle_generation(
        &self,
        target: WindowId,
        request_generation: u64,
        terminal: WindowActivationTerminal,
    ) -> Option<WindowActivationTicketDelivery> {
        let mut records = self.records.borrow_mut();
        let record = records.get(&request_generation)?;
        if record.ticket.target() != target {
            return None;
        }
        let record = records
            .remove(&request_generation)
            .expect("checked activation record must remain present");
        self.clear_current_if(request_generation);
        drop(records);
        record.ticket.settle(terminal)
    }

    pub(super) fn observe_exact_positive(
        &self,
        observed_window: WindowId,
        event_sequence: u64,
    ) -> Option<WindowActivationTicketDelivery> {
        let mut records = self.records.borrow_mut();
        let request_generation = records.iter().rev().find_map(|(generation, record)| {
            let command_sequence = record.command_sequence?;
            let status = record.ticket.snapshot().status();
            (event_sequence > command_sequence
                && matches!(
                    status,
                    WindowActivationStatus::Queued
                        | WindowActivationStatus::Dispatching
                        | WindowActivationStatus::Dispatched
                ))
            .then_some(*generation)
        })?;
        let record = records
            .get_mut(&request_generation)
            .expect("selected activation record must remain present");
        match record.ticket.snapshot().status() {
            WindowActivationStatus::Queued | WindowActivationStatus::Dispatching => {
                if record.first_positive.is_none() {
                    record.first_positive = Some(ExactOwnedWindowGain {
                        observed_window,
                        ingress_sequence: event_sequence,
                    });
                }
                None
            }
            WindowActivationStatus::Dispatched => {
                let record = records
                    .remove(&request_generation)
                    .expect("selected activation record must remain present");
                self.clear_current_if(request_generation);
                drop(records);
                let terminal = if record.ticket.target() == observed_window {
                    WindowActivationTerminal::Activated
                } else {
                    WindowActivationTerminal::Superseded
                };
                record.ticket.settle(terminal)
            }
            WindowActivationStatus::Terminal(_) => None,
        }
    }

    /// Settles an accepted activation only when the post-dispatch readback already names the
    /// exact target.
    ///
    /// A different focused window may simply be the pre-command source that has not emitted its
    /// loss edge yet. Only a later exact native positive observation can prove that another owned
    /// window won after the command.
    pub(super) fn observe_readback(
        &self,
        target: WindowId,
        request_generation: u64,
        focused_window: Option<WindowId>,
    ) -> Option<WindowActivationTicketDelivery> {
        if focused_window != Some(target) {
            return None;
        }

        let mut records = self.records.borrow_mut();
        let record = records.get(&request_generation)?;
        if record.ticket.target() != target
            || record.ticket.snapshot().status() != WindowActivationStatus::Dispatched
        {
            return None;
        }
        let record = records
            .remove(&request_generation)
            .expect("checked activation record must remain present");
        self.clear_current_if(request_generation);
        drop(records);
        record.ticket.settle(WindowActivationTerminal::Activated)
    }

    pub(super) fn activation_policy_changed(
        &self,
        window_id: WindowId,
        activation_policy_generation: u64,
        accepts_activation: Option<bool>,
    ) -> Vec<WindowActivationTicketDelivery> {
        let generations = self
            .records
            .borrow()
            .iter()
            .filter_map(|(generation, record)| {
                (record.ticket.target() == window_id
                    && (record.ticket.activation_policy_generation()
                        != activation_policy_generation
                        || accepts_activation == Some(false)))
                .then_some(*generation)
            })
            .collect::<Vec<_>>();
        self.settle_generations(generations, WindowActivationTerminal::PolicyChanged)
    }

    pub(super) fn window_closed(&self, window_id: WindowId) -> Vec<WindowActivationTicketDelivery> {
        let generations = self
            .records
            .borrow()
            .iter()
            .filter_map(|(generation, record)| {
                (record.ticket.target() == window_id).then_some(*generation)
            })
            .collect::<Vec<_>>();
        self.settle_generations(generations, WindowActivationTerminal::WindowClosed)
    }

    pub(super) fn cancel(
        &self,
        ticket: &WindowActivationTicket,
        terminal: WindowActivationTerminal,
    ) -> Option<WindowActivationTicketDelivery> {
        let mut records = self.records.borrow_mut();
        let Some(record) = records.get(&ticket.request_generation()) else {
            drop(records);
            return ticket.settle(terminal);
        };
        if !same_ticket(&record.ticket, ticket) {
            drop(records);
            return ticket.settle(terminal);
        }
        let record = records
            .remove(&ticket.request_generation())
            .expect("checked activation record must remain present");
        self.clear_current_if(ticket.request_generation());
        drop(records);
        record.ticket.settle(terminal)
    }

    pub(super) fn terminate(&self) -> Vec<WindowActivationTicketDelivery> {
        let generations = self.records.borrow().keys().copied().collect::<Vec<_>>();
        self.settle_generations(generations, WindowActivationTerminal::WindowClosed)
    }

    pub(super) fn enqueue_delivery(&self, delivery: WindowActivationTicketDelivery) {
        self.deliveries.borrow_mut().push_back(delivery);
    }

    pub(super) fn drain_deliveries(&self) {
        if self.delivering.replace(true) {
            return;
        }

        let mut first_panic = None;
        loop {
            let delivery = { self.deliveries.borrow_mut().pop_front() };
            let Some(delivery) = delivery else {
                break;
            };
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| delivery.deliver())) {
                if first_panic.is_none() {
                    first_panic = Some(payload);
                } else {
                    log::error!(
                        "suppressed secondary panic while draining window activation deliveries"
                    );
                }
            }
        }
        self.delivering.set(false);

        if let Some(payload) = first_panic {
            resume_unwind(payload);
        }
    }

    fn settle_generations(
        &self,
        generations: Vec<u64>,
        terminal: WindowActivationTerminal,
    ) -> Vec<WindowActivationTicketDelivery> {
        let mut records = self.records.borrow_mut();
        let mut deliveries = Vec::new();
        for generation in generations {
            let Some(record) = records.remove(&generation) else {
                continue;
            };
            self.clear_current_if(generation);
            if let Some(delivery) = record.ticket.settle(terminal) {
                deliveries.push(delivery);
            }
        }
        deliveries
    }

    fn clear_current_if(&self, request_generation: u64) {
        if self.current_generation.get() == Some(request_generation) {
            self.current_generation.set(None);
        }
    }
}

impl Drop for NativeWindowActivationAuthority {
    fn drop(&mut self) {
        let mut deliveries = Vec::new();
        for (_, record) in mem::take(self.records.get_mut()) {
            if let Some(delivery) = record.ticket.settle(WindowActivationTerminal::WindowClosed) {
                deliveries.push(delivery);
            }
        }
        for delivery in deliveries {
            if catch_unwind(AssertUnwindSafe(|| delivery.deliver())).is_err() {
                log::error!("suppressed panic while closing a dropped window activation authority");
            }
        }
    }
}

fn same_ticket(lhs: &WindowActivationTicket, rhs: &WindowActivationTicket) -> bool {
    lhs.target() == rhs.target()
        && lhs.request_generation() == rhs.request_generation()
        && lhs.activation_policy_generation() == rhs.activation_policy_generation()
        && Rc::ptr_eq(&lhs.state, &rhs.state)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window_id(raw: u64) -> WindowId {
        WindowId::from(raw)
    }

    fn begin_bound(
        authority: &NativeWindowActivationAuthority,
        target: WindowId,
        policy_generation: u64,
        command_sequence: u64,
    ) -> WindowActivationTicket {
        let request = authority.begin(Weak::new(), target, policy_generation);
        assert!(request.displaced.is_empty());
        assert!(authority.bind_command_sequence(&request.ticket, command_sequence));
        request.ticket
    }

    #[test]
    fn stale_native_observation_before_command_sequence_cannot_settle_request() {
        let authority = NativeWindowActivationAuthority::default();
        let ticket = begin_bound(&authority, window_id(1), 3, 20);

        assert!(authority.observe_exact_positive(window_id(1), 19).is_none());
        assert_eq!(ticket.snapshot().status(), WindowActivationStatus::Queued);
        assert!(authority.begin_dispatch(window_id(1), ticket.request_generation(), 20));
        assert!(authority.observe_exact_positive(window_id(1), 19).is_none());

        let completion =
            authority.finish_dispatch_accepted(window_id(1), ticket.request_generation());
        assert!(completion.delivery.is_none());
        assert!(completion.needs_readback);
        assert_eq!(
            ticket.snapshot().status(),
            WindowActivationStatus::Dispatched
        );
    }

    #[test]
    fn synchronous_positive_is_combined_with_accepted_dispatch() {
        let authority = NativeWindowActivationAuthority::default();
        let ticket = begin_bound(&authority, window_id(1), 3, 20);
        assert!(authority.begin_dispatch(window_id(1), ticket.request_generation(), 20));
        assert!(authority.observe_exact_positive(window_id(1), 21).is_none());

        let completion =
            authority.finish_dispatch_accepted(window_id(1), ticket.request_generation());
        completion
            .delivery
            .expect("accepted dispatch must consume the in-flight positive")
            .deliver();
        assert!(!completion.needs_readback);
        assert_eq!(
            ticket.snapshot().status(),
            WindowActivationStatus::Terminal(WindowActivationTerminal::Activated)
        );
    }

    #[test]
    fn synchronous_positive_cannot_override_rejected_dispatch() {
        let authority = NativeWindowActivationAuthority::default();
        let ticket = begin_bound(&authority, window_id(1), 3, 20);
        assert!(authority.begin_dispatch(window_id(1), ticket.request_generation(), 20));
        assert!(authority.observe_exact_positive(window_id(1), 21).is_none());

        authority
            .settle_generation(
                window_id(1),
                ticket.request_generation(),
                WindowActivationTerminal::Rejected,
            )
            .expect("rejected dispatch must remain terminal")
            .deliver();
        assert_eq!(
            ticket.snapshot().status(),
            WindowActivationStatus::Terminal(WindowActivationTerminal::Rejected)
        );
    }

    #[test]
    fn nested_new_request_does_not_erase_in_flight_positive() {
        let authority = NativeWindowActivationAuthority::default();
        let first = begin_bound(&authority, window_id(1), 3, 20);
        assert!(authority.begin_dispatch(window_id(1), first.request_generation(), 20));
        assert!(authority.observe_exact_positive(window_id(1), 21).is_none());

        let second = authority.begin(Weak::new(), window_id(2), 4);
        assert!(
            second.displaced.is_empty(),
            "the dispatching request must retain its earlier native evidence"
        );
        assert!(authority.bind_command_sequence(&second.ticket, 22));

        authority
            .finish_dispatch_accepted(window_id(1), first.request_generation())
            .delivery
            .expect("the retained first request must settle")
            .deliver();
        assert_eq!(
            first.snapshot().status(),
            WindowActivationStatus::Terminal(WindowActivationTerminal::Activated)
        );
        assert_eq!(
            second.ticket.snapshot().status(),
            WindowActivationStatus::Queued
        );
    }

    #[test]
    fn newer_request_supersedes_queued_request_once() {
        let authority = NativeWindowActivationAuthority::default();
        let first = begin_bound(&authority, window_id(1), 0, 10);
        let second = authority.begin(Weak::new(), window_id(2), 0);
        assert_eq!(second.displaced.len(), 1);
        for delivery in second.displaced {
            delivery.deliver();
        }

        assert_eq!(
            first.snapshot().status(),
            WindowActivationStatus::Terminal(WindowActivationTerminal::Superseded)
        );
        assert_eq!(
            second.ticket.snapshot().status(),
            WindowActivationStatus::Queued
        );
    }

    #[test]
    fn other_owned_window_after_dispatch_supersedes_request() {
        let authority = NativeWindowActivationAuthority::default();
        let ticket = begin_bound(&authority, window_id(1), 0, 10);
        assert!(authority.begin_dispatch(window_id(1), ticket.request_generation(), 10));
        let completion =
            authority.finish_dispatch_accepted(window_id(1), ticket.request_generation());
        assert!(completion.needs_readback);

        authority
            .observe_exact_positive(window_id(2), 11)
            .expect("a newer owned winner must settle the request")
            .deliver();
        assert_eq!(
            ticket.snapshot().status(),
            WindowActivationStatus::Terminal(WindowActivationTerminal::Superseded)
        );
    }

    #[test]
    fn readback_keeps_the_pre_command_source_pending_until_exact_target_gain() {
        let authority = NativeWindowActivationAuthority::default();
        let target = window_id(1);
        let source = window_id(2);
        let ticket = begin_bound(&authority, target, 0, 10);
        assert!(authority.begin_dispatch(target, ticket.request_generation(), 10));
        let completion = authority.finish_dispatch_accepted(target, ticket.request_generation());
        assert!(completion.needs_readback);

        assert!(
            authority
                .observe_readback(target, ticket.request_generation(), Some(source))
                .is_none()
        );
        assert!(
            authority
                .observe_readback(target, ticket.request_generation(), None)
                .is_none()
        );
        assert_eq!(
            ticket.snapshot().status(),
            WindowActivationStatus::Dispatched
        );

        authority
            .observe_readback(target, ticket.request_generation(), Some(target))
            .expect("the exact target readback should settle activation")
            .deliver();
        assert_eq!(
            ticket.snapshot().status(),
            WindowActivationStatus::Terminal(WindowActivationTerminal::Activated)
        );
    }

    #[test]
    fn activation_policy_generation_change_is_terminal() {
        let authority = NativeWindowActivationAuthority::default();
        let ticket = begin_bound(&authority, window_id(1), 4, 10);
        let deliveries = authority.activation_policy_changed(window_id(1), 5, None);
        assert_eq!(deliveries.len(), 1);
        for delivery in deliveries {
            delivery.deliver();
        }
        assert_eq!(
            ticket.snapshot().status(),
            WindowActivationStatus::Terminal(WindowActivationTerminal::PolicyChanged)
        );
    }

    #[test]
    fn first_terminal_result_cannot_be_overwritten() {
        let authority = NativeWindowActivationAuthority::default();
        let ticket = begin_bound(&authority, window_id(1), 7, 40);
        authority
            .settle_generation(
                window_id(1),
                ticket.request_generation(),
                WindowActivationTerminal::Rejected,
            )
            .expect("rejection should settle the current request")
            .deliver();

        assert!(authority.window_closed(window_id(1)).is_empty());
        assert_eq!(
            ticket.snapshot().status(),
            WindowActivationStatus::Terminal(WindowActivationTerminal::Rejected)
        );
    }

    #[test]
    fn dropping_subscription_cancels_callback_after_terminal_was_queued() {
        let ticket = WindowActivationTicket::new(Weak::new(), window_id(1), 1, 0);
        let delivered = Rc::new(Cell::new(false));
        let subscription = ticket.subscribe({
            let delivered = delivered.clone();
            move |_| delivered.set(true)
        });
        let delivery = ticket
            .settle(WindowActivationTerminal::Rejected)
            .expect("the pending ticket should produce one terminal delivery");

        drop(subscription);
        delivery.deliver();

        assert!(!delivered.get());
        assert_eq!(
            ticket.snapshot().status(),
            WindowActivationStatus::Terminal(WindowActivationTerminal::Rejected)
        );
    }
}
