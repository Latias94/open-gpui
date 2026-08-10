use crate::{
    Pixels, PlatformWindowDispatch, PlatformWindowMutationTerminal, Point, Size, Subscription,
    WindowActivationPolicy, WindowBackgroundAppearance, WindowMutationDomain,
    WindowPhysicalPlacementRequest, WindowPlacementRequest, WindowPlacementState,
    WindowPlatformFacts,
};
use std::{
    any::Any,
    cell::RefCell,
    collections::BTreeMap,
    fmt, mem,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
    sync::Arc,
};

/// The request associated with a window mutation ticket.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindowMutationRequest {
    /// A coherent window placement request.
    Placement(WindowPlacementRequest),
    /// A windowed client-area placement in physical desktop coordinates.
    PhysicalPlacement(WindowPhysicalPlacementRequest),
    /// A pointer-input acceptance request.
    PointerInput(bool),
    /// A coherent lifetime activation-policy request.
    ActivationPolicy(WindowActivationPolicy),
    /// A native background or alpha-treatment request.
    Alpha(WindowBackgroundAppearance),
    /// A topmost-window request.
    Topmost(bool),
    /// A taskbar-visibility request.
    TaskbarVisibility(bool),
}

impl WindowMutationRequest {
    /// Returns the conflict domain this request belongs to.
    pub const fn domain(self) -> WindowMutationDomain {
        match self {
            Self::Placement(_) | Self::PhysicalPlacement(_) => WindowMutationDomain::Placement,
            Self::PointerInput(_) => WindowMutationDomain::PointerInput,
            Self::ActivationPolicy(_) => WindowMutationDomain::ActivationPolicy,
            Self::Alpha(_) => WindowMutationDomain::Alpha,
            Self::Topmost(_) => WindowMutationDomain::Topmost,
            Self::TaskbarVisibility(_) => WindowMutationDomain::TaskbarVisibility,
        }
    }

    pub(crate) fn matches_facts(self, facts: &WindowPlatformFacts) -> bool {
        match self {
            Self::Placement(request) => placement_request_matches_facts(request, facts),
            Self::PhysicalPlacement(request) => {
                placement_state_from_facts(facts) == WindowPlacementState::Windowed
                    && facts
                        .physical_geometry
                        .is_some_and(|geometry| geometry.client_bounds() == request.client_bounds())
            }
            Self::PointerInput(requested) => facts.accepts_pointer_input == requested,
            Self::ActivationPolicy(requested) => {
                facts.accepts_activation == requested.accepts_activation
                    && facts.focus_on_click == requested.focus_on_click
            }
            Self::Alpha(requested) => facts.background_appearance == requested,
            Self::Topmost(requested) => facts.topmost == requested,
            Self::TaskbarVisibility(requested) => facts.taskbar_visible == requested,
        }
    }
}

/// The terminal outcome recorded for a window mutation ticket.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WindowMutationOutcome {
    /// Observed facts exactly match the request's controllable properties.
    Exact,
    /// Observed facts differ from the requested controllable properties.
    Adjusted,
    /// A newer request in the same conflict domain replaced this request.
    Superseded,
    /// The backend rejected the request before a matching observation arrived.
    Rejected,
    /// The backend cannot perform this mutation for an already-open window.
    Unsupported,
    /// The window closed before the request received an observation.
    WindowClosed,
}

/// A terminal observation for one generation-bound window mutation request.
///
/// The contained facts are the committed platform snapshot at the moment GPUI settled the
/// request. They are never synthesized from the request intent.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowMutationObservation {
    /// The conflict domain that owns this request.
    pub domain: WindowMutationDomain,
    /// The monotonic generation within [`Self::domain`].
    pub generation: u64,
    /// The original request.
    pub request: WindowMutationRequest,
    /// The terminal result.
    pub outcome: WindowMutationOutcome,
    /// The committed platform facts used to settle the request.
    pub facts: WindowPlatformFacts,
}

impl WindowMutationObservation {
    /// Returns whether the observed controllable properties exactly match the request.
    pub const fn is_exact(&self) -> bool {
        matches!(self.outcome, WindowMutationOutcome::Exact)
    }
}

/// The result of requesting a mutation through [`crate::Window`].
///
/// A queued result carries a generation-bound ticket. All other variants are synchronous
/// dispatch facts and do not claim that a platform change was committed.
#[must_use = "window mutation dispatch outcomes must be handled"]
#[derive(Debug)]
pub enum WindowMutationDispatch {
    /// The backend accepted the request path and an observation ticket was created.
    Queued(WindowMutationTicket),
    /// The committed value already matched the request.
    Unchanged,
    /// The backend does not support this mutation for an already-open window.
    Unsupported,
    /// The backend rejected the request before it could be observed.
    Rejected,
    /// The window closed before the request could be dispatched.
    WindowClosed,
}

impl WindowMutationDispatch {
    /// Returns the queued observation ticket, if the backend accepted the request path.
    pub fn ticket(&self) -> Option<&WindowMutationTicket> {
        match self {
            Self::Queued(ticket) => Some(ticket),
            Self::Unchanged | Self::Unsupported | Self::Rejected | Self::WindowClosed => None,
        }
    }
}

/// A generation-bound handle for observing a queued window mutation.
///
/// Cloning a ticket only clones observation access. It does not issue another backend request.
/// Dropping a subscription returned by [`Self::subscribe`] cancels callback delivery, while the
/// ticket itself still records its eventual terminal observation.
#[derive(Clone)]
pub struct WindowMutationTicket {
    authority: Arc<WindowPlatformMutationAuthority>,
    request: WindowMutationRequest,
    generation: u64,
    state: Rc<RefCell<WindowMutationTicketState>>,
}

impl WindowMutationTicket {
    /// Returns the conflict domain that owns this ticket.
    pub const fn domain(&self) -> WindowMutationDomain {
        self.request.domain()
    }

    /// Returns the request that created this ticket.
    pub const fn request(&self) -> WindowMutationRequest {
        self.request
    }

    /// Returns the generation within [`Self::domain`].
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the terminal observation, if the ticket has settled.
    pub fn observation(&self) -> Option<WindowMutationObservation> {
        self.state.borrow().terminal.clone()
    }

    /// Subscribes to this ticket's terminal observation.
    ///
    /// If the ticket is already terminal, the callback runs immediately. Otherwise it runs at
    /// most once when the authority settles the ticket. Dropping the returned subscription stops
    /// callback delivery without cancelling the queued backend request or terminal recording.
    pub fn subscribe(
        &self,
        callback: impl FnOnce(WindowMutationObservation) + 'static,
    ) -> Subscription {
        if let Some(observation) = self.observation() {
            callback(observation);
            return Subscription::new(|| {});
        }

        let observer_id = {
            let mut state = self.state.borrow_mut();
            // GPUI window mutation state is UI-thread confined, so no other thread can settle
            // between the observation above and this registration.
            if let Some(observation) = state.terminal.clone() {
                drop(state);
                callback(observation);
                return Subscription::new(|| {});
            }
            let observer_id = state.next_observer_id;
            state.next_observer_id = state
                .next_observer_id
                .checked_add(1)
                .expect("window mutation observer identity space exhausted");
            state.observers.insert(observer_id, Box::new(callback));
            observer_id
        };

        let state = Rc::downgrade(&self.state);
        Subscription::new(move || {
            if let Some(state) = state.upgrade() {
                state.borrow_mut().observers.remove(&observer_id);
            }
        })
    }

    pub(crate) fn new(
        authority: Arc<WindowPlatformMutationAuthority>,
        request: WindowMutationRequest,
        generation: u64,
    ) -> Self {
        Self {
            authority,
            request,
            generation,
            state: Rc::new(RefCell::new(WindowMutationTicketState::default())),
        }
    }

    fn belongs_to(
        &self,
        authority: &Arc<WindowPlatformMutationAuthority>,
        request: WindowMutationRequest,
        generation: u64,
    ) -> bool {
        Arc::ptr_eq(&self.authority, authority)
            && self.request == request
            && self.generation == generation
    }

    fn belongs_to_generation(
        &self,
        authority: &Arc<WindowPlatformMutationAuthority>,
        generation: u64,
    ) -> bool {
        Arc::ptr_eq(&self.authority, authority) && self.generation == generation
    }

    fn settle(
        &self,
        outcome: WindowMutationOutcome,
        facts: WindowPlatformFacts,
    ) -> Option<WindowMutationTicketDelivery> {
        let observation = WindowMutationObservation {
            domain: self.domain(),
            generation: self.generation,
            request: self.request,
            outcome,
            facts,
        };
        let callbacks = {
            let mut state = self.state.borrow_mut();
            if state.terminal.is_some() {
                return None;
            }
            state.terminal = Some(observation.clone());
            mem::take(&mut state.observers)
                .into_values()
                .collect::<Vec<_>>()
        };
        Some(WindowMutationTicketDelivery {
            observation,
            callbacks,
        })
    }
}

impl fmt::Debug for WindowMutationTicket {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WindowMutationTicket")
            .field("domain", &self.domain())
            .field("generation", &self.generation)
            .field("request", &self.request)
            .field("observation", &self.observation())
            .finish()
    }
}

type WindowMutationObserver = Box<dyn FnOnce(WindowMutationObservation) + 'static>;

#[derive(Default)]
struct WindowMutationTicketState {
    terminal: Option<WindowMutationObservation>,
    observers: BTreeMap<usize, WindowMutationObserver>,
    next_observer_id: usize,
}

/// Identity shared by a window and every ticket it creates.
///
/// It intentionally owns no window or platform handle. A ticket can therefore outlive its window
/// without retaining either backend resources or a stale window ID.
#[derive(Debug, Default)]
pub(crate) struct WindowPlatformMutationAuthority {
    _private: (),
}

#[derive(Default)]
pub(crate) struct WindowMutationState {
    domains: BTreeMap<WindowMutationDomain, WindowMutationDomainState>,
}

#[derive(Default)]
struct WindowMutationDomainState {
    last_generation: u64,
    pending: Option<PendingWindowMutation>,
}

struct PendingWindowMutation {
    ticket: WindowMutationTicket,
}

pub(crate) struct WindowMutationBegin {
    pub(crate) ticket: WindowMutationTicket,
    pub(crate) deliveries: Vec<WindowMutationTicketDelivery>,
}

/// A terminal delivery detached from mutable window state.
///
/// Callers must settle state first, then invoke [`Self::deliver`] after releasing their authority
/// bookkeeping so an observing callback cannot alter a half-settled request.
#[must_use = "window mutation ticket deliveries must be dispatched after settlement"]
pub(crate) struct WindowMutationTicketDelivery {
    observation: WindowMutationObservation,
    callbacks: Vec<WindowMutationObserver>,
}

impl WindowMutationTicketDelivery {
    pub(crate) fn deliver(self) {
        let mut first_panic: Option<Box<dyn Any + Send>> = None;
        for callback in self.callbacks {
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
                callback(self.observation.clone());
            })) {
                if first_panic.is_none() {
                    first_panic = Some(payload);
                } else {
                    log::error!(
                        "suppressed secondary panic while delivering window mutation ticket observers"
                    );
                }
            }
        }
        if let Some(payload) = first_panic {
            resume_unwind(payload);
        }
    }
}

impl WindowMutationState {
    pub(crate) fn begin(
        &mut self,
        authority: &Arc<WindowPlatformMutationAuthority>,
        request: WindowMutationRequest,
        facts: &WindowPlatformFacts,
    ) -> Option<WindowMutationBegin> {
        let generation = self.next_generation(request.domain())?;
        let deliveries = self.supersede_pending(authority, request.domain(), facts);
        let ticket = WindowMutationTicket::new(authority.clone(), request, generation);
        self.pending_slot_mut(request.domain())
            .replace(PendingWindowMutation {
                ticket: ticket.clone(),
            });

        // The ticket is already installed before calling into a backend. Some test and native
        // adapters synchronously invoke an observation callback while dispatching a request.
        Some(WindowMutationBegin { ticket, deliveries })
    }

    pub(crate) fn settle_unqueued(
        &mut self,
        authority: &Arc<WindowPlatformMutationAuthority>,
        ticket: &WindowMutationTicket,
        outcome: WindowMutationOutcome,
        facts: &WindowPlatformFacts,
    ) -> Vec<WindowMutationTicketDelivery> {
        self.take_matching(authority, ticket)
            .and_then(|pending| pending.ticket.settle(outcome, facts.clone()))
            .into_iter()
            .collect()
    }

    pub(crate) fn settle_from_terminal_facts(
        &mut self,
        authority: &Arc<WindowPlatformMutationAuthority>,
        domain: WindowMutationDomain,
        generation: u64,
        terminal: PlatformWindowMutationTerminal,
        facts: &WindowPlatformFacts,
    ) -> Vec<WindowMutationTicketDelivery> {
        let pending = self.pending_slot_mut(domain).take();
        let Some(pending) = pending else {
            return Vec::new();
        };
        if !pending.ticket.belongs_to_generation(authority, generation) {
            *self.pending_slot_mut(domain) = Some(pending);
            return Vec::new();
        }
        let outcome = terminal_outcome(terminal, pending.ticket.request(), facts);
        pending
            .ticket
            .settle(outcome, facts.clone())
            .into_iter()
            .collect()
    }

    pub(crate) fn is_current_generation(
        &self,
        authority: &Arc<WindowPlatformMutationAuthority>,
        domain: WindowMutationDomain,
        generation: u64,
    ) -> bool {
        self.domains
            .get(&domain)
            .and_then(|state| state.pending.as_ref())
            .is_some_and(|pending| pending.ticket.belongs_to_generation(authority, generation))
    }

    pub(crate) fn last_generation(&self, domain: WindowMutationDomain) -> u64 {
        self.domains
            .get(&domain)
            .map_or(0, |state| state.last_generation)
    }

    pub(crate) fn settle_all(
        &mut self,
        authority: &Arc<WindowPlatformMutationAuthority>,
        outcome: WindowMutationOutcome,
        facts: &WindowPlatformFacts,
    ) -> Vec<WindowMutationTicketDelivery> {
        let mut deliveries = Vec::new();
        for domain in WindowMutationDomain::ALL {
            let pending = self.pending_slot_mut(domain).take();
            let Some(pending) = pending else {
                continue;
            };
            if !pending.ticket.belongs_to(
                authority,
                pending.ticket.request(),
                pending.ticket.generation(),
            ) {
                *self.pending_slot_mut(domain) = Some(pending);
                continue;
            }
            if let Some(delivery) = pending.ticket.settle(outcome, facts.clone()) {
                deliveries.push(delivery);
            }
        }
        deliveries
    }

    fn next_generation(&mut self, domain: WindowMutationDomain) -> Option<u64> {
        let state = self.domains.entry(domain).or_default();
        let next = state.last_generation.checked_add(1)?;
        state.last_generation = next;
        Some(next)
    }

    fn supersede_pending(
        &mut self,
        authority: &Arc<WindowPlatformMutationAuthority>,
        domain: WindowMutationDomain,
        facts: &WindowPlatformFacts,
    ) -> Vec<WindowMutationTicketDelivery> {
        let pending = self.pending_slot_mut(domain).take();
        let Some(pending) = pending else {
            return Vec::new();
        };
        if !pending.ticket.belongs_to(
            authority,
            pending.ticket.request(),
            pending.ticket.generation(),
        ) {
            *self.pending_slot_mut(domain) = Some(pending);
            return Vec::new();
        }
        pending
            .ticket
            .settle(WindowMutationOutcome::Superseded, facts.clone())
            .into_iter()
            .collect()
    }

    fn take_matching(
        &mut self,
        authority: &Arc<WindowPlatformMutationAuthority>,
        ticket: &WindowMutationTicket,
    ) -> Option<PendingWindowMutation> {
        let domain = ticket.domain();
        let pending = self.pending_slot_mut(domain).take()?;
        if pending
            .ticket
            .belongs_to(authority, ticket.request(), ticket.generation())
        {
            Some(pending)
        } else {
            *self.pending_slot_mut(domain) = Some(pending);
            None
        }
    }

    fn pending_slot_mut(
        &mut self,
        domain: WindowMutationDomain,
    ) -> &mut Option<PendingWindowMutation> {
        &mut self.domains.entry(domain).or_default().pending
    }
}

fn terminal_outcome(
    terminal: PlatformWindowMutationTerminal,
    request: WindowMutationRequest,
    facts: &WindowPlatformFacts,
) -> WindowMutationOutcome {
    match terminal {
        PlatformWindowMutationTerminal::Observed
            if matches!(request, WindowMutationRequest::PhysicalPlacement(_))
                && facts.physical_geometry.is_none() =>
        {
            WindowMutationOutcome::Rejected
        }
        PlatformWindowMutationTerminal::Observed => observation_outcome(request, facts),
        PlatformWindowMutationTerminal::Rejected => WindowMutationOutcome::Rejected,
        PlatformWindowMutationTerminal::Unsupported => WindowMutationOutcome::Unsupported,
        PlatformWindowMutationTerminal::WindowClosed => WindowMutationOutcome::WindowClosed,
    }
}

fn observation_outcome(
    request: WindowMutationRequest,
    facts: &WindowPlatformFacts,
) -> WindowMutationOutcome {
    if request.matches_facts(facts) {
        WindowMutationOutcome::Exact
    } else {
        WindowMutationOutcome::Adjusted
    }
}

pub(crate) fn placement_request_is_valid(
    request: WindowPlacementRequest,
    facts: &WindowPlatformFacts,
) -> bool {
    if request.is_empty() {
        return false;
    }
    if request
        .position
        .is_some_and(|position| !point_is_finite(position))
        || request
            .size
            .is_some_and(|size| !size_is_finite_and_positive(size))
        || request.restore_bounds.is_some_and(|bounds| {
            !point_is_finite(bounds.origin) || !size_is_finite_and_positive(bounds.size)
        })
    {
        return false;
    }

    let has_geometry = request.position.is_some() || request.size.is_some();
    match request.state {
        Some(WindowPlacementState::Windowed) => request.restore_bounds.is_none(),
        Some(
            WindowPlacementState::Maximized
            | WindowPlacementState::Fullscreen
            | WindowPlacementState::Minimized,
        ) => !has_geometry,
        None => {
            if has_geometry && placement_state_from_facts(facts) != WindowPlacementState::Windowed {
                return false;
            }
            request.restore_bounds.is_none()
                || placement_state_from_facts(facts) != WindowPlacementState::Windowed
        }
    }
}

fn point_is_finite(point: Point<Pixels>) -> bool {
    point.x.0.is_finite() && point.y.0.is_finite()
}

fn size_is_finite_and_positive(size: Size<Pixels>) -> bool {
    size.width.0.is_finite()
        && size.height.0.is_finite()
        && size.width.0 > 0.0
        && size.height.0 > 0.0
}

pub(crate) fn placement_request_matches_facts(
    request: WindowPlacementRequest,
    facts: &WindowPlatformFacts,
) -> bool {
    if let Some(state) = request.state
        && placement_state_from_facts(facts) != state
    {
        return false;
    }
    if let Some(position) = request.position
        && facts.bounds.origin != position
    {
        return false;
    }
    if let Some(size) = request.size
        && facts.bounds.size != size
    {
        return false;
    }
    request
        .restore_bounds
        .is_none_or(|restore_bounds| facts.window_bounds.get_bounds() == restore_bounds)
}

pub(crate) fn placement_state_from_facts(facts: &WindowPlatformFacts) -> WindowPlacementState {
    if facts.is_minimized {
        WindowPlacementState::Minimized
    } else if facts.is_fullscreen {
        WindowPlacementState::Fullscreen
    } else if facts.is_maximized {
        WindowPlacementState::Maximized
    } else {
        WindowPlacementState::Windowed
    }
}

pub(crate) fn platform_dispatch_outcome(
    dispatch: PlatformWindowDispatch,
) -> Option<WindowMutationOutcome> {
    match dispatch {
        PlatformWindowDispatch::Queued => None,
        PlatformWindowDispatch::Unchanged => None,
        PlatformWindowDispatch::Unsupported => Some(WindowMutationOutcome::Unsupported),
        PlatformWindowDispatch::Rejected => Some(WindowMutationOutcome::Rejected),
        PlatformWindowDispatch::WindowClosed => Some(WindowMutationOutcome::WindowClosed),
    }
}
