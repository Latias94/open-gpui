use super::{DockSurface, DockSurfaceOwner};
use crate::{
    DockHost, DockItemId, DockSpaceId, DockViewportActivationTransaction, DockViewportFocusRequest,
    viewport_activation::{
        DockViewportActivationApplyOutcome, apply_viewport_activation_transaction,
    },
};
use open_gpui::{
    AnyWindowHandle, App, AppContext, EntityId, FocusClaimOutcome, Subscription, WeakEntity,
    Window, WindowId,
};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fmt,
    rc::{Rc, Weak as RcWeak},
};

type DockSurfaceActivationCallback =
    Box<dyn FnOnce(DockSurfaceActivationOutcome, &mut App) + 'static>;

/// Stable identity for one panel activation request.
///
/// Request identities are monotonic within a [`DockSurface`](super::DockSurface). A newer request
/// receives a distinct identity even when it targets the same panel as the current request.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DockSurfaceActivationRequestId(u64);

impl DockSurfaceActivationRequestId {
    /// Returns the surface-local monotonic request sequence.
    pub const fn sequence(self) -> u64 {
        self.0
    }
}

/// Terminal result of a stable-item panel activation request.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DockSurfaceActivationOutcome {
    /// GPUI committed focus to the requested panel or one of its descendants.
    Committed,
    /// GPUI or subtree-presentation authority rejected the focus claim.
    Rejected,
    /// A newer activation or focus mutation superseded the request.
    Superseded,
    /// The panel, activation host, or descendant focus target was unavailable.
    Unavailable,
    /// More than one live host claimed the target dock space.
    DuplicateHostConflict,
    /// The activation host window closed before focus committed.
    WindowClosed,
}

impl From<FocusClaimOutcome> for DockSurfaceActivationOutcome {
    fn from(outcome: FocusClaimOutcome) -> Self {
        match outcome {
            FocusClaimOutcome::Committed => Self::Committed,
            FocusClaimOutcome::Rejected => Self::Rejected,
            FocusClaimOutcome::Superseded => Self::Superseded,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct DockSurfaceActivationHostGeneration(u64);

impl DockSurfaceActivationHostGeneration {
    const fn sequence(self) -> u64 {
        self.0
    }
}

struct DockSurfaceActivationObserver {
    callback: Rc<RefCell<Option<DockSurfaceActivationCallback>>>,
}

impl DockSurfaceActivationObserver {
    fn new(
        callback: impl FnOnce(DockSurfaceActivationOutcome, &mut App) + 'static,
    ) -> (Self, Subscription) {
        let callback = Rc::new(RefCell::new(Some(
            Box::new(callback) as DockSurfaceActivationCallback
        )));
        let subscription_callback = callback.clone();
        let subscription = Subscription::new(move || {
            subscription_callback.borrow_mut().take();
        });
        (Self { callback }, subscription)
    }

    fn take_callback(&self) -> Option<DockSurfaceActivationCallback> {
        self.callback.borrow_mut().take()
    }
}

impl fmt::Debug for DockSurfaceActivationObserver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DockSurfaceActivationObserver")
            .field("observed", &self.callback.borrow().is_some())
            .finish()
    }
}

impl Drop for DockSurfaceActivationObserver {
    fn drop(&mut self) {
        self.callback.borrow_mut().take();
    }
}

/// A terminal callback detached from owner state.
///
/// The owner must return this handle from its entity update and deliver it only after that update
/// ends. This permits a callback to issue another activation without re-entering an owner borrow.
#[must_use = "activation settlements must be delivered after the owner update ends"]
#[derive(Debug)]
pub(crate) struct DockSurfaceActivationSettlement {
    outcome: DockSurfaceActivationOutcome,
    observer: DockSurfaceActivationObserver,
}

impl DockSurfaceActivationSettlement {
    fn new(outcome: DockSurfaceActivationOutcome, observer: DockSurfaceActivationObserver) -> Self {
        Self { outcome, observer }
    }

    pub(crate) fn deliver(self, cx: &mut App) {
        let callback = self.observer.take_callback();
        if let Some(callback) = callback {
            callback(self.outcome, cx);
        }
    }
}

/// A small ordered batch of terminal callbacks produced by one owner operation.
///
/// A new immediately-failing request can produce two callbacks: first the superseded request,
/// then the immediate result of the new request.
#[must_use = "activation settlements must be delivered after the owner update ends"]
#[derive(Debug, Default)]
pub(crate) struct DockSurfaceActivationSettlements(Vec<DockSurfaceActivationSettlement>);

impl DockSurfaceActivationSettlements {
    fn push(&mut self, settlement: Option<DockSurfaceActivationSettlement>) {
        if let Some(settlement) = settlement {
            self.0.push(settlement);
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn deliver(self, cx: &mut App) {
        for settlement in self.0 {
            settlement.deliver(cx);
        }
    }
}

/// Generation-bound authority passed from the surface owner to a mounted host.
///
/// Both identities are validated against the owner's single current pending request. A stale
/// callback therefore cannot settle a replacement host generation or a newer equal-item request.
#[derive(Clone)]
pub(crate) struct DockSurfaceActivationBinding {
    request_id: DockSurfaceActivationRequestId,
    host_generation: DockSurfaceActivationHostGeneration,
    owner: WeakEntity<DockSurfaceOwner>,
    state: RcWeak<RefCell<DockSurfaceActivationCore>>,
}

impl DockSurfaceActivationBinding {
    fn new(
        request_id: DockSurfaceActivationRequestId,
        host_generation: DockSurfaceActivationHostGeneration,
        owner: WeakEntity<DockSurfaceOwner>,
        state: RcWeak<RefCell<DockSurfaceActivationCore>>,
    ) -> Self {
        Self {
            request_id,
            host_generation,
            owner,
            state,
        }
    }

    pub(crate) fn is_current<C>(&self, cx: &C) -> bool
    where
        C: AppContext,
    {
        let Some(owner) = self.owner.upgrade() else {
            return false;
        };
        cx.read_entity(&owner, |owner, _| {
            owner.activation().binding_is_current(self)
        })
    }

    /// Attempts to settle the request through its owning surface.
    ///
    /// The callback is invoked only after the owner entity update finishes. A released owner,
    /// replaced host generation, or already-settled request is an exact no-op.
    pub(crate) fn settle<C>(&self, outcome: DockSurfaceActivationOutcome, cx: &mut C) -> bool
    where
        C: AppContext,
    {
        let Some(state) = self.state.upgrade() else {
            return false;
        };
        let Some(owner) = self.owner.upgrade() else {
            return false;
        };
        let request_id = self.request_id;
        let host_generation = self.host_generation;
        cx.update_entity(&owner, move |_owner, owner_cx| {
            let settlement =
                state
                    .borrow_mut()
                    .settle_binding(request_id, host_generation, outcome);
            let did_settle = settlement.is_some();
            if let Some(settlement) = settlement {
                owner_cx.defer(move |cx| settlement.deliver(cx));
            }
            did_settle
        })
    }
}

impl fmt::Debug for DockSurfaceActivationBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DockSurfaceActivationBinding")
            .field("request_id", &self.request_id)
            .field("host_generation", &self.host_generation)
            .field("owner", &self.owner)
            .finish()
    }
}

impl PartialEq for DockSurfaceActivationBinding {
    fn eq(&self, other: &Self) -> bool {
        self.request_id == other.request_id
            && self.host_generation == other.host_generation
            && self.owner == other.owner
            && RcWeak::ptr_eq(&self.state, &other.state)
    }
}

impl Eq for DockSurfaceActivationBinding {}

/// A live host and generation selected for one activation request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DockSurfaceActivationTarget {
    host: WeakEntity<DockHost>,
    window: AnyWindowHandle,
    binding: DockSurfaceActivationBinding,
}

impl DockSurfaceActivationTarget {
    pub(crate) fn host(&self) -> &WeakEntity<DockHost> {
        &self.host
    }

    pub(crate) const fn window(&self) -> AnyWindowHandle {
        self.window
    }

    pub(crate) fn binding(&self) -> &DockSurfaceActivationBinding {
        &self.binding
    }
}

/// Dispatch decision produced while beginning an activation request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DockSurfaceActivationDispatch {
    Available(DockSurfaceActivationTarget),
    Immediate(DockSurfaceActivationOutcome),
}

/// Data returned by the activation owner when a request begins.
///
/// Callers must leave the owner entity update, deliver `settlements`, and only then dispatch an
/// available target. The returned subscription controls callback observation only.
#[must_use = "activation begin results contain a subscription and terminal callbacks"]
#[derive(Debug)]
pub(crate) struct DockSurfaceActivationBegin {
    request_id: DockSurfaceActivationRequestId,
    subscription: Subscription,
    dispatch: DockSurfaceActivationDispatch,
    settlements: DockSurfaceActivationSettlements,
}

impl DockSurfaceActivationBegin {
    pub(crate) fn into_parts(
        self,
    ) -> (
        DockSurfaceActivationRequestId,
        Subscription,
        DockSurfaceActivationDispatch,
        DockSurfaceActivationSettlements,
    ) {
        (
            self.request_id,
            self.subscription,
            self.dispatch,
            self.settlements,
        )
    }
}

/// Result of inspecting the current activation host for a dock space.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DockSurfaceActivationHostLookup {
    Available {
        host: WeakEntity<DockHost>,
        window: AnyWindowHandle,
        generation: u64,
    },
    Unavailable,
    DuplicateHostConflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockSurfaceActivationHostRegistrationStatus {
    Committed,
    DuplicateHostConflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockSurfaceActivationHostRegistrationKind {
    Committed {
        generation: DockSurfaceActivationHostGeneration,
    },
    Conflict {
        incumbent_generation: DockSurfaceActivationHostGeneration,
        conflict_sequence: u64,
    },
}

/// Exact lease used to release either a committed host or one recorded duplicate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DockSurfaceActivationHostRegistration {
    space: DockSpaceId,
    host_id: EntityId,
    window: AnyWindowHandle,
    kind: DockSurfaceActivationHostRegistrationKind,
}

impl DockSurfaceActivationHostRegistration {
    pub(crate) fn space(&self) -> &DockSpaceId {
        &self.space
    }

    pub(crate) const fn host_id(&self) -> EntityId {
        self.host_id
    }

    pub(crate) const fn window(&self) -> AnyWindowHandle {
        self.window
    }

    pub(crate) const fn status(&self) -> DockSurfaceActivationHostRegistrationStatus {
        match self.kind {
            DockSurfaceActivationHostRegistrationKind::Committed { .. } => {
                DockSurfaceActivationHostRegistrationStatus::Committed
            }
            DockSurfaceActivationHostRegistrationKind::Conflict { .. } => {
                DockSurfaceActivationHostRegistrationStatus::DuplicateHostConflict
            }
        }
    }
}

/// Host registration result plus callbacks invalidated by the registration.
#[must_use = "host registration can settle a pending activation"]
#[derive(Debug)]
pub(crate) struct DockSurfaceActivationHostRegistrationResult {
    registration: DockSurfaceActivationHostRegistration,
    settlements: DockSurfaceActivationSettlements,
}

impl DockSurfaceActivationHostRegistrationResult {
    pub(crate) fn into_parts(
        self,
    ) -> (
        DockSurfaceActivationHostRegistration,
        DockSurfaceActivationSettlements,
    ) {
        (self.registration, self.settlements)
    }
}

#[derive(Clone, Debug)]
struct DockSurfaceCommittedActivationHost {
    host: WeakEntity<DockHost>,
    window: AnyWindowHandle,
    generation: DockSurfaceActivationHostGeneration,
}

#[derive(Clone, Debug)]
struct DockSurfaceConflictingActivationHost {
    host: WeakEntity<DockHost>,
    window: AnyWindowHandle,
    incumbent_generation: DockSurfaceActivationHostGeneration,
    conflict_sequence: u64,
}

#[derive(Debug, Default)]
struct DockSurfaceActivationHostSlot {
    committed: Option<DockSurfaceCommittedActivationHost>,
    conflicts: BTreeMap<EntityId, DockSurfaceConflictingActivationHost>,
}

impl DockSurfaceActivationHostSlot {
    fn has_live_conflict(&self) -> bool {
        self.conflicts
            .values()
            .any(|conflict| conflict.host.upgrade().is_some())
    }

    fn is_empty(&self) -> bool {
        self.committed.is_none() && self.conflicts.is_empty()
    }
}

#[derive(Debug)]
struct PendingDockSurfaceActivation {
    request_id: DockSurfaceActivationRequestId,
    space: DockSpaceId,
    host_generation: DockSurfaceActivationHostGeneration,
    window: AnyWindowHandle,
    observer: DockSurfaceActivationObserver,
}

impl PendingDockSurfaceActivation {
    fn into_settlement(
        self,
        outcome: DockSurfaceActivationOutcome,
    ) -> DockSurfaceActivationSettlement {
        DockSurfaceActivationSettlement::new(outcome, self.observer)
    }
}

#[derive(Debug, Default)]
struct DockSurfaceActivationCore {
    last_request_sequence: u64,
    last_conflict_sequence: u64,
    last_host_generation_by_space: BTreeMap<DockSpaceId, u64>,
    hosts: BTreeMap<DockSpaceId, DockSurfaceActivationHostSlot>,
    pending: Option<PendingDockSurfaceActivation>,
}

impl DockSurfaceActivationCore {
    fn next_request_id(&mut self) -> DockSurfaceActivationRequestId {
        self.last_request_sequence = self
            .last_request_sequence
            .checked_add(1)
            .expect("dock surface activation request identity space exhausted");
        DockSurfaceActivationRequestId(self.last_request_sequence)
    }

    fn next_conflict_sequence(&mut self) -> u64 {
        self.last_conflict_sequence = self
            .last_conflict_sequence
            .checked_add(1)
            .expect("dock surface activation host conflict identity space exhausted");
        self.last_conflict_sequence
    }

    fn next_host_generation(&mut self, space: &DockSpaceId) -> DockSurfaceActivationHostGeneration {
        let sequence = self
            .last_host_generation_by_space
            .entry(space.clone())
            .or_default();
        *sequence = sequence
            .checked_add(1)
            .expect("dock surface activation host generation space exhausted");
        DockSurfaceActivationHostGeneration(*sequence)
    }

    fn settle_binding(
        &mut self,
        request_id: DockSurfaceActivationRequestId,
        host_generation: DockSurfaceActivationHostGeneration,
        outcome: DockSurfaceActivationOutcome,
    ) -> Option<DockSurfaceActivationSettlement> {
        let matches = self.pending.as_ref().is_some_and(|pending| {
            pending.request_id == request_id && pending.host_generation == host_generation
        });
        if !matches {
            return None;
        }
        self.pending
            .take()
            .map(|pending| pending.into_settlement(outcome))
    }

    fn settle_pending_host(
        &mut self,
        space: &DockSpaceId,
        generation: DockSurfaceActivationHostGeneration,
        window: AnyWindowHandle,
        outcome: DockSurfaceActivationOutcome,
    ) -> Option<DockSurfaceActivationSettlement> {
        let matches = self.pending.as_ref().is_some_and(|pending| {
            pending.space == *space
                && pending.host_generation == generation
                && pending.window == window
        });
        if !matches {
            return None;
        }
        self.pending
            .take()
            .map(|pending| pending.into_settlement(outcome))
    }

    fn settle_pending_window(
        &mut self,
        window: WindowId,
        outcome: DockSurfaceActivationOutcome,
    ) -> Option<DockSurfaceActivationSettlement> {
        let matches = self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.window.window_id() == window);
        if !matches {
            return None;
        }
        self.pending
            .take()
            .map(|pending| pending.into_settlement(outcome))
    }

    fn prune_released_hosts(
        &mut self,
        space: &DockSpaceId,
    ) -> Option<DockSurfaceActivationSettlement> {
        let released_committed = {
            let slot = self.hosts.get_mut(space)?;
            slot.conflicts
                .retain(|_, conflict| conflict.host.upgrade().is_some());
            if slot
                .committed
                .as_ref()
                .is_some_and(|committed| committed.host.upgrade().is_none())
            {
                slot.committed.take()
            } else {
                None
            }
        };

        let settlement = released_committed.and_then(|committed| {
            self.settle_pending_host(
                space,
                committed.generation,
                committed.window,
                DockSurfaceActivationOutcome::Unavailable,
            )
        });
        if self
            .hosts
            .get(space)
            .is_some_and(DockSurfaceActivationHostSlot::is_empty)
        {
            self.hosts.remove(space);
        }
        settlement
    }

    fn lookup_host(&self, space: &DockSpaceId) -> DockSurfaceActivationHostLookup {
        let Some(slot) = self.hosts.get(space) else {
            return DockSurfaceActivationHostLookup::Unavailable;
        };
        if slot.has_live_conflict() {
            return DockSurfaceActivationHostLookup::DuplicateHostConflict;
        }
        let Some(committed) = slot
            .committed
            .as_ref()
            .filter(|committed| committed.host.upgrade().is_some())
        else {
            return DockSurfaceActivationHostLookup::Unavailable;
        };
        DockSurfaceActivationHostLookup::Available {
            host: committed.host.clone(),
            window: committed.window,
            generation: committed.generation.sequence(),
        }
    }
}

/// Single-owner activation registry and request authority for one surface.
///
/// Store exactly one instance on `DockSurfaceOwner`. The core is shared weakly with bindings so a
/// focus callback can validate itself while still entering through the owner entity.
#[derive(Default)]
pub(crate) struct DockSurfaceActivationState {
    core: Rc<RefCell<DockSurfaceActivationCore>>,
}

impl DockSurfaceActivationState {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Registers one mounted host without replacing a live incumbent.
    ///
    /// Re-registering the same host entity in the same window is idempotent. Other host entities,
    /// whether in the same or another window, are deduplicated by entity id and recorded as
    /// conflicts until their exact registration is released.
    pub(crate) fn register_host(
        &mut self,
        space: DockSpaceId,
        host: WeakEntity<DockHost>,
        window: AnyWindowHandle,
    ) -> DockSurfaceActivationHostRegistrationResult {
        let host_id = host.entity_id();
        let mut state = self.core.borrow_mut();
        let mut settlements = DockSurfaceActivationSettlements::default();
        settlements.push(state.prune_released_hosts(&space));

        if let Some(committed) = state
            .hosts
            .get(&space)
            .and_then(|slot| slot.committed.as_ref())
            .filter(|committed| committed.host.entity_id() == host_id && committed.window == window)
            .cloned()
        {
            return DockSurfaceActivationHostRegistrationResult {
                registration: DockSurfaceActivationHostRegistration {
                    space,
                    host_id,
                    window,
                    kind: DockSurfaceActivationHostRegistrationKind::Committed {
                        generation: committed.generation,
                    },
                },
                settlements,
            };
        }

        let incumbent = state
            .hosts
            .get(&space)
            .and_then(|slot| slot.committed.clone());
        if let Some(incumbent) = incumbent {
            if let Some(conflict) = state
                .hosts
                .get(&space)
                .and_then(|slot| slot.conflicts.get(&host_id))
                .cloned()
            {
                return DockSurfaceActivationHostRegistrationResult {
                    registration: DockSurfaceActivationHostRegistration {
                        space,
                        host_id,
                        window: conflict.window,
                        kind: DockSurfaceActivationHostRegistrationKind::Conflict {
                            incumbent_generation: conflict.incumbent_generation,
                            conflict_sequence: conflict.conflict_sequence,
                        },
                    },
                    settlements,
                };
            }

            let conflict_sequence = state.next_conflict_sequence();
            state
                .hosts
                .entry(space.clone())
                .or_default()
                .conflicts
                .insert(
                    host_id,
                    DockSurfaceConflictingActivationHost {
                        host,
                        window,
                        incumbent_generation: incumbent.generation,
                        conflict_sequence,
                    },
                );
            settlements.push(state.settle_pending_host(
                &space,
                incumbent.generation,
                incumbent.window,
                DockSurfaceActivationOutcome::DuplicateHostConflict,
            ));
            return DockSurfaceActivationHostRegistrationResult {
                registration: DockSurfaceActivationHostRegistration {
                    space,
                    host_id,
                    window,
                    kind: DockSurfaceActivationHostRegistrationKind::Conflict {
                        incumbent_generation: incumbent.generation,
                        conflict_sequence,
                    },
                },
                settlements,
            };
        }

        let generation = state.next_host_generation(&space);
        let slot = state.hosts.entry(space.clone()).or_default();
        slot.conflicts.remove(&host_id);
        slot.committed = Some(DockSurfaceCommittedActivationHost {
            host,
            window,
            generation,
        });
        DockSurfaceActivationHostRegistrationResult {
            registration: DockSurfaceActivationHostRegistration {
                space,
                host_id,
                window,
                kind: DockSurfaceActivationHostRegistrationKind::Committed { generation },
            },
            settlements,
        }
    }

    /// Releases only the exact registration represented by `registration`.
    pub(crate) fn release_host(
        &mut self,
        registration: &DockSurfaceActivationHostRegistration,
    ) -> DockSurfaceActivationSettlements {
        let mut state = self.core.borrow_mut();
        let mut settlements = DockSurfaceActivationSettlements::default();
        match registration.kind {
            DockSurfaceActivationHostRegistrationKind::Committed { generation } => {
                let matches = state
                    .hosts
                    .get(&registration.space)
                    .and_then(|slot| slot.committed.as_ref())
                    .is_some_and(|committed| {
                        committed.host.entity_id() == registration.host_id
                            && committed.window == registration.window
                            && committed.generation == generation
                    });
                if matches {
                    if let Some(slot) = state.hosts.get_mut(&registration.space) {
                        slot.committed = None;
                    }
                    settlements.push(state.settle_pending_host(
                        &registration.space,
                        generation,
                        registration.window,
                        DockSurfaceActivationOutcome::Unavailable,
                    ));
                }
            }
            DockSurfaceActivationHostRegistrationKind::Conflict {
                conflict_sequence, ..
            } => {
                let matches = state
                    .hosts
                    .get(&registration.space)
                    .and_then(|slot| slot.conflicts.get(&registration.host_id))
                    .is_some_and(|conflict| {
                        conflict.window == registration.window
                            && conflict.conflict_sequence == conflict_sequence
                    });
                if matches {
                    if let Some(slot) = state.hosts.get_mut(&registration.space) {
                        slot.conflicts.remove(&registration.host_id);
                    }
                }
            }
        }
        if state
            .hosts
            .get(&registration.space)
            .is_some_and(DockSurfaceActivationHostSlot::is_empty)
        {
            state.hosts.remove(&registration.space);
        }
        settlements
    }

    /// Returns the unique live activation host without creating a request.
    #[cfg(test)]
    pub(crate) fn lookup_host(&self, space: &DockSpaceId) -> DockSurfaceActivationHostLookup {
        self.core.borrow().lookup_host(space)
    }

    /// Begins a surface-wide activation request and supersedes the prior request, if any.
    pub(crate) fn begin_request(
        &mut self,
        owner: WeakEntity<DockSurfaceOwner>,
        space: DockSpaceId,
        callback: impl FnOnce(DockSurfaceActivationOutcome, &mut App) + 'static,
    ) -> DockSurfaceActivationBegin {
        let (observer, subscription) = DockSurfaceActivationObserver::new(callback);
        let mut state = self.core.borrow_mut();
        let request_id = state.next_request_id();
        let mut settlements = DockSurfaceActivationSettlements::default();
        settlements.push(
            state
                .pending
                .take()
                .map(|pending| pending.into_settlement(DockSurfaceActivationOutcome::Superseded)),
        );

        let dispatch = match state.lookup_host(&space) {
            DockSurfaceActivationHostLookup::Available {
                host,
                window,
                generation,
            } => {
                let generation = DockSurfaceActivationHostGeneration(generation);
                let binding = DockSurfaceActivationBinding::new(
                    request_id,
                    generation,
                    owner,
                    Rc::downgrade(&self.core),
                );
                state.pending = Some(PendingDockSurfaceActivation {
                    request_id,
                    space,
                    host_generation: generation,
                    window,
                    observer,
                });
                DockSurfaceActivationDispatch::Available(DockSurfaceActivationTarget {
                    host,
                    window,
                    binding,
                })
            }
            DockSurfaceActivationHostLookup::Unavailable => {
                settlements.push(Some(DockSurfaceActivationSettlement::new(
                    DockSurfaceActivationOutcome::Unavailable,
                    observer,
                )));
                DockSurfaceActivationDispatch::Immediate(DockSurfaceActivationOutcome::Unavailable)
            }
            DockSurfaceActivationHostLookup::DuplicateHostConflict => {
                settlements.push(Some(DockSurfaceActivationSettlement::new(
                    DockSurfaceActivationOutcome::DuplicateHostConflict,
                    observer,
                )));
                DockSurfaceActivationDispatch::Immediate(
                    DockSurfaceActivationOutcome::DuplicateHostConflict,
                )
            }
        };

        DockSurfaceActivationBegin {
            request_id,
            subscription,
            dispatch,
            settlements,
        }
    }

    pub(crate) fn begin_immediate_request(
        &mut self,
        outcome: DockSurfaceActivationOutcome,
        callback: impl FnOnce(DockSurfaceActivationOutcome, &mut App) + 'static,
    ) -> DockSurfaceActivationBegin {
        let (observer, subscription) = DockSurfaceActivationObserver::new(callback);
        let mut state = self.core.borrow_mut();
        let request_id = state.next_request_id();
        let mut settlements = DockSurfaceActivationSettlements::default();
        settlements.push(
            state
                .pending
                .take()
                .map(|pending| pending.into_settlement(DockSurfaceActivationOutcome::Superseded)),
        );
        settlements.push(Some(DockSurfaceActivationSettlement::new(
            outcome, observer,
        )));
        DockSurfaceActivationBegin {
            request_id,
            subscription,
            dispatch: DockSurfaceActivationDispatch::Immediate(outcome),
            settlements,
        }
    }

    pub(crate) fn binding_is_current(&self, binding: &DockSurfaceActivationBinding) -> bool {
        self.core.borrow().pending.as_ref().is_some_and(|pending| {
            pending.request_id == binding.request_id
                && pending.host_generation == binding.host_generation
        })
    }

    pub(crate) fn settle(
        &mut self,
        binding: &DockSurfaceActivationBinding,
        outcome: DockSurfaceActivationOutcome,
    ) -> DockSurfaceActivationSettlements {
        let mut settlements = DockSurfaceActivationSettlements::default();
        settlements.push(self.core.borrow_mut().settle_binding(
            binding.request_id,
            binding.host_generation,
            outcome,
        ));
        settlements
    }

    /// Removes registrations for a closed window and settles its pending request exactly once.
    pub(crate) fn window_closed(&mut self, window: WindowId) -> DockSurfaceActivationSettlements {
        let mut state = self.core.borrow_mut();
        let mut settlements = DockSurfaceActivationSettlements::default();
        settlements
            .push(state.settle_pending_window(window, DockSurfaceActivationOutcome::WindowClosed));
        state.hosts.retain(|_, slot| {
            if slot
                .committed
                .as_ref()
                .is_some_and(|committed| committed.window.window_id() == window)
            {
                slot.committed = None;
            }
            slot.conflicts
                .retain(|_, conflict| conflict.window.window_id() != window);
            !slot.is_empty()
        });
        settlements
    }
}

impl DockSurface {
    /// Issues a stable-item activation intent without retaining an outcome observer.
    ///
    /// The returned id identifies the intent. Dropping the internal observation subscription does
    /// not cancel platform activation or focus work. Use [`Self::activate_panel_from_window`]
    /// inside a window event callback.
    pub fn activate_panel(
        &self,
        item: impl Into<DockItemId>,
        cx: &mut App,
    ) -> DockSurfaceActivationRequestId {
        let (request_id, subscription) = self.activate_panel_with_completion(item, cx, |_, _cx| {});
        drop(subscription);
        request_id
    }

    /// Issues a stable-item activation intent from a window event callback.
    ///
    /// When the requested panel belongs to the current event-receiver window, activation is
    /// deferred until GPUI returns that window and the callback's entities to the app.
    pub fn activate_panel_from_window(
        &self,
        item: impl Into<DockItemId>,
        window: &Window,
        cx: &mut App,
    ) -> DockSurfaceActivationRequestId {
        let (request_id, subscription) =
            self.activate_panel_with_completion_from_window(item, window, cx, |_, _cx| {});
        drop(subscription);
        request_id
    }

    /// Activates a panel by stable item id and observes its exact terminal focus outcome.
    ///
    /// Selection is committed independently before focus is requested. The callback receives one
    /// of the typed terminal outcomes; dropping the returned subscription stops callback delivery
    /// but leaves the activation intent in flight. Use
    /// [`Self::activate_panel_with_completion_from_window`] inside a window event callback.
    pub fn activate_panel_with_completion(
        &self,
        item: impl Into<DockItemId>,
        cx: &mut App,
        callback: impl FnOnce(DockSurfaceActivationOutcome, &mut App) + 'static,
    ) -> (DockSurfaceActivationRequestId, Subscription) {
        self.activate_panel_with_completion_impl(item.into(), None, cx, callback)
    }

    /// Activates a panel from a window event callback and observes its exact terminal outcome.
    ///
    /// Use this entry point when the target may be hosted by the current event-receiver window.
    /// Other-window targets retain the synchronous app-level activation path.
    pub fn activate_panel_with_completion_from_window(
        &self,
        item: impl Into<DockItemId>,
        window: &Window,
        cx: &mut App,
        callback: impl FnOnce(DockSurfaceActivationOutcome, &mut App) + 'static,
    ) -> (DockSurfaceActivationRequestId, Subscription) {
        self.activate_panel_with_completion_impl(
            item.into(),
            Some(window.window_handle()),
            cx,
            callback,
        )
    }

    fn activate_panel_with_completion_impl(
        &self,
        item: DockItemId,
        current_window: Option<AnyWindowHandle>,
        cx: &mut App,
        callback: impl FnOnce(DockSurfaceActivationOutcome, &mut App) + 'static,
    ) -> (DockSurfaceActivationRequestId, Subscription) {
        let controller = self.controller(cx);
        let space = cx.read_entity(&controller, |controller, _| {
            controller.graph().spaces().into_iter().find(|space| {
                controller
                    .graph()
                    .find_item_in_space(space, &item)
                    .is_some()
            })
        });

        let Some(space) = space else {
            let owner = self.owner().clone();
            let begin = cx.update_entity(&owner, |owner, _| {
                owner
                    .activation_mut()
                    .begin_immediate_request(DockSurfaceActivationOutcome::Unavailable, callback)
            });
            let (request_id, subscription, _dispatch, settlements) = begin.into_parts();
            Self::defer_settlements(settlements, cx);
            return (request_id, subscription);
        };

        let owner = self.owner().clone();
        let owner_weak = owner.downgrade();
        let begin = cx.update_entity(&owner, |owner, _| {
            owner
                .activation_mut()
                .begin_request(owner_weak, space.clone(), callback)
        });
        let (request_id, subscription, dispatch, settlements) = begin.into_parts();
        Self::defer_settlements(settlements, cx);

        let DockSurfaceActivationDispatch::Available(target) = dispatch else {
            return (request_id, subscription);
        };
        if !target.binding().is_current(cx) {
            return (request_id, subscription);
        }
        let Some(target_host) = target.host().upgrade() else {
            self.settle_activation(
                target.binding(),
                DockSurfaceActivationOutcome::Unavailable,
                cx,
            );
            return (request_id, subscription);
        };
        let registration = cx.read_entity(&target_host, |host, _| {
            host.viewport_runtime()
                .registration_key_for_space_window(&space, target.window().window_id())
        });
        let Some(registration) = registration else {
            self.settle_activation(
                target.binding(),
                DockSurfaceActivationOutcome::Unavailable,
                cx,
            );
            return (request_id, subscription);
        };

        let transaction = DockViewportActivationTransaction::surface_activation(
            registration,
            target.window(),
            DockViewportFocusRequest::panel(item),
            target.binding().clone(),
            target.host().clone(),
        );
        if current_window == Some(target.window()) {
            let surface = self.clone();
            let binding = target.binding().clone();
            cx.defer(move |cx| {
                let outcome = apply_viewport_activation_transaction(Some(transaction), cx);
                surface.settle_failed_activation_apply(&binding, outcome, cx);
            });
        } else {
            let outcome = apply_viewport_activation_transaction(Some(transaction), cx);
            self.settle_failed_activation_apply(target.binding(), outcome, cx);
        }

        (request_id, subscription)
    }

    fn settle_failed_activation_apply(
        &self,
        binding: &DockSurfaceActivationBinding,
        outcome: DockViewportActivationApplyOutcome,
        cx: &mut App,
    ) {
        if matches!(
            outcome,
            DockViewportActivationApplyOutcome::NoTarget
                | DockViewportActivationApplyOutcome::WindowUnavailable
                | DockViewportActivationApplyOutcome::WrongRootView
                | DockViewportActivationApplyOutcome::SpaceMismatch
        ) {
            self.settle_activation(binding, DockSurfaceActivationOutcome::Unavailable, cx);
        }
    }

    fn settle_activation(
        &self,
        binding: &DockSurfaceActivationBinding,
        outcome: DockSurfaceActivationOutcome,
        cx: &mut App,
    ) {
        let owner = self.owner().clone();
        let settlements = cx.update_entity(&owner, |owner, _| {
            owner.activation_mut().settle(binding, outcome)
        });
        Self::defer_settlements(settlements, cx);
    }

    fn defer_settlements(settlements: DockSurfaceActivationSettlements, cx: &mut App) {
        if !settlements.is_empty() {
            cx.defer(move |cx| settlements.deliver(cx));
        }
    }
}

impl fmt::Debug for DockSurfaceActivationState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DockSurfaceActivationState")
            .finish_non_exhaustive()
    }
}
