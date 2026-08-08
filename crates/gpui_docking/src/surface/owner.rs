use super::{
    DockSurfaceActivationState,
    live_undock::{
        DockLiveUndockEffects, DockLiveUndockFact, DockLiveUndockIdentity,
        DockLiveUndockOpenFailureOutcome, DockLiveUndockOpenReturnOutcome,
        DockLiveUndockOpeningKey, DockLiveUndockPromotionCommitDisposition, DockLiveUndockSession,
        DockLiveUndockShutdownSnapshot, DockLiveUndockTransition,
        DockLiveUndockWindowTerminalOutcome,
    },
    live_undock_runtime::DockLiveUndockRuntime,
    payload_recovery::{
        DockPayloadRecoveryAuthority, DockPayloadRecoveryCommitError,
        DockPayloadRecoveryCommitReceipt, DockPayloadRecoveryEntry, DockPayloadRecoveryFocus,
        DockPayloadRecoveryPrepareError, DockPayloadRecoveryPrepared,
        DockPayloadRecoveryPresentationOrigin, DockPayloadRecoveryReason,
        DockPayloadRecoveryRegistry, DockPayloadRecoveryRestoreAction,
        DockPayloadRecoveryRestoreError, DockPayloadRecoveryRestorePrepared,
        DockPayloadRecoveryRestoreReceipt,
    },
    payload_recovery_executor::{
        DockPayloadRecoveryExecutionKey, DockPayloadRecoveryExecutor,
        DockPayloadRecoveryFinalization, DockPayloadRecoveryTransfer,
    },
    window_session::{
        DockSurfaceWindowSession, DockSurfaceWindowSessionDependencyId,
        DockSurfaceWindowSessionDependencyTerminalOutcome, DockSurfaceWindowSessionLease,
    },
};
use crate::{
    DockController, DockSpaceId, DockViewportProvisionalOpenAttemptCompletion,
    DockViewportRuntimeHandle, locked_drop_identity::DockLockedPayloadIdentity,
};
use open_gpui::{
    AnyWindowHandle, App, AppContext, Context, Entity, EntityId, EventEmitter, Subscription,
    WeakEntity, WindowId, view_presentation_window,
};
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

/// A durable category of committed docking-surface change.
///
/// Categories describe which snapshot domains may have changed. They intentionally exclude
/// transient focus, styling, and viewport-dispatch state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DockSurfaceChangeCategory {
    /// The logical docking layout changed.
    Layout,
    /// The selected panel changed.
    Selection,
    /// A panel was opened, closed, attached, detached, or otherwise changed lifecycle state.
    PanelLifecycle,
    /// The set or routing of platform viewports changed.
    ViewportTopology,
    /// Committed platform observation changed a viewport's placement.
    ObservedViewportPlacement,
}

impl DockSurfaceChangeCategory {
    const ALL: [Self; 5] = [
        Self::Layout,
        Self::Selection,
        Self::PanelLifecycle,
        Self::ViewportTopology,
        Self::ObservedViewportPlacement,
    ];

    const fn bit(self) -> u8 {
        match self {
            Self::Layout => 1 << 0,
            Self::Selection => 1 << 1,
            Self::PanelLifecycle => 1 << 2,
            Self::ViewportTopology => 1 << 3,
            Self::ObservedViewportPlacement => 1 << 4,
        }
    }
}

/// Named lifecycle transitions carried by a committed surface change event.
///
/// Categories remain the stable persistence/debounce domains. Transitions identify sparse
/// user-visible lifecycle boundaries that callers must not infer from a category combination.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DockSurfaceTransition {
    /// A durable promotion outlived the viewport that was presenting its payload.
    ViewportLostAfterPromotion,
    /// A user restore transaction re-homed that payload into the current primary anchor.
    ViewportRecovered,
}

impl DockSurfaceTransition {
    const ALL: [Self; 2] = [Self::ViewportLostAfterPromotion, Self::ViewportRecovered];

    const fn bit(self) -> u8 {
        match self {
            Self::ViewportLostAfterPromotion => 1 << 0,
            Self::ViewportRecovered => 1 << 1,
        }
    }
}

/// Metadata emitted after one docking-surface transaction commits.
///
/// The event does not contain a layout or viewport snapshot. Applications can debounce these
/// lightweight events and explicitly export a revision-consistent snapshot when their persistence
/// policy requires one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DockSurfaceChangeEvent {
    revision: u64,
    categories: Vec<DockSurfaceChangeCategory>,
    transitions: Vec<DockSurfaceTransition>,
}

impl DockSurfaceChangeEvent {
    fn new(
        revision: u64,
        categories: Vec<DockSurfaceChangeCategory>,
        transitions: Vec<DockSurfaceTransition>,
    ) -> Self {
        debug_assert!(!categories.is_empty());
        Self {
            revision,
            categories,
            transitions,
        }
    }

    /// Returns the committed surface revision.
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Returns the deduplicated change categories in stable declaration order.
    pub fn categories(&self) -> &[DockSurfaceChangeCategory] {
        &self.categories
    }

    /// Returns whether this commit contains `category`.
    pub fn contains(&self, category: DockSurfaceChangeCategory) -> bool {
        self.categories.contains(&category)
    }

    /// Returns named lifecycle transitions committed by this transaction.
    pub fn transitions(&self) -> &[DockSurfaceTransition] {
        &self.transitions
    }

    /// Returns whether this commit contains `transition`.
    pub fn contains_transition(&self, transition: DockSurfaceTransition) -> bool {
        self.transitions.contains(&transition)
    }
}

/// Internal identity for one explicit root surface transaction.
///
/// This identity is threaded through controller and viewport-runtime commits, but it is never
/// exposed as part of the application-facing mutation API or change event.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct DockSurfaceTransactionId(u64);

/// Completion evidence for one exact root surface transaction.
///
/// The receipt is filled before event delivery. A caller that owns a larger commit journal can
/// therefore distinguish an aborted transaction from a transaction whose event subscriber
/// panicked after the revision became durable.
#[derive(Clone, Debug)]
pub(crate) struct DockSurfaceTransactionReceipt {
    committed_revision: Rc<Cell<Option<u64>>>,
    aborted: Rc<Cell<bool>>,
}

/// One committed surface event whose delivery is deferred to an enclosing authority boundary.
///
/// The surface revision and transaction receipt are already durable when this value is created.
/// Consuming publication prevents the same event from being delivered twice by a retrying commit
/// journal. Empty transactions carry no event and publish nothing.
#[must_use = "a deferred surface publication must be settled after its enclosing authority commits"]
#[derive(Clone, Debug)]
pub(crate) struct DockSurfaceDeferredPublication {
    inner: Rc<DockSurfaceDeferredPublicationInner>,
}

#[derive(Debug)]
struct DockSurfaceDeferredPublicationInner {
    owner: WeakEntity<DockSurfaceOwner>,
    event: Option<DockSurfaceChangeEvent>,
    state: Cell<DockSurfaceDeferredPublicationState>,
}

#[derive(Debug)]
struct PendingDockSurfacePublication {
    event: DockSurfaceChangeEvent,
    ready: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockSurfaceDeferredPublicationState {
    Pending,
    Delivering,
    Settled,
}

impl DockSurfaceDeferredPublication {
    fn new(owner: Entity<DockSurfaceOwner>, event: Option<DockSurfaceChangeEvent>) -> Self {
        let state = if event.is_some() {
            DockSurfaceDeferredPublicationState::Pending
        } else {
            DockSurfaceDeferredPublicationState::Settled
        };
        Self {
            inner: Rc::new(DockSurfaceDeferredPublicationInner {
                owner: owner.downgrade(),
                event,
                state: Cell::new(state),
            }),
        }
    }

    pub(crate) fn has_event(&self) -> bool {
        self.inner.event.is_some()
    }

    pub(crate) fn is_settled(&self) -> bool {
        self.inner.state.get() == DockSurfaceDeferredPublicationState::Settled
    }

    /// Publishes the exact event committed by the tracked transaction.
    ///
    /// The shared receipt is idempotent. A borrow failure before delivery leaves it pending for a
    /// retry. Once subscriber delivery begins it is settled even if a subscriber panics, preserving
    /// at-most-once event delivery while the transaction receipt remains the durable authority.
    pub(crate) fn publish(&self, cx: &mut App) -> bool {
        if self.inner.state.get() != DockSurfaceDeferredPublicationState::Pending {
            return false;
        }
        let Some(event) = self.inner.event.clone() else {
            self.inner
                .state
                .set(DockSurfaceDeferredPublicationState::Settled);
            return false;
        };
        let Some(owner) = self.inner.owner.upgrade() else {
            self.inner
                .state
                .set(DockSurfaceDeferredPublicationState::Settled);
            return false;
        };

        self.inner
            .state
            .set(DockSurfaceDeferredPublicationState::Delivering);
        let delivery_started = Rc::new(Cell::new(false));
        let started_in_delivery = delivery_started.clone();
        let inner = self.inner.clone();
        let result = catch_unwind(AssertUnwindSafe(|| {
            cx.update_entity(&owner, move |_owner, owner_cx| {
                started_in_delivery.set(true);
                inner
                    .state
                    .set(DockSurfaceDeferredPublicationState::Settled);
                _owner.publish_committed_event(event.revision(), owner_cx);
            });
        }));
        match result {
            Ok(()) => true,
            Err(payload) => {
                if !delivery_started.get() {
                    self.inner
                        .state
                        .set(DockSurfaceDeferredPublicationState::Pending);
                }
                resume_unwind(payload)
            }
        }
    }
}

impl DockSurfaceTransactionReceipt {
    fn pending() -> Self {
        Self {
            committed_revision: Rc::new(Cell::new(None)),
            aborted: Rc::new(Cell::new(false)),
        }
    }

    pub(crate) fn committed_revision(&self) -> Option<u64> {
        self.committed_revision.get()
    }

    pub(crate) fn is_aborted(&self) -> bool {
        self.aborted.get()
    }

    fn commit(&self, revision: u64) {
        debug_assert!(!self.aborted.get());
        self.committed_revision.set(Some(revision));
    }

    fn abort(&self) {
        debug_assert!(self.committed_revision.get().is_none());
        self.aborted.set(true);
    }
}

#[derive(Debug)]
struct PendingDockSurfaceTransaction {
    id: DockSurfaceTransactionId,
    category_bits: u8,
    transition_bits: u8,
    receipt: Option<DockSurfaceTransactionReceipt>,
}

impl PendingDockSurfaceTransaction {
    fn new(id: DockSurfaceTransactionId, receipt: Option<DockSurfaceTransactionReceipt>) -> Self {
        Self {
            id,
            category_bits: 0,
            transition_bits: 0,
            receipt,
        }
    }

    fn record(&mut self, category: DockSurfaceChangeCategory) {
        self.category_bits |= category.bit();
    }

    fn categories(&self) -> Vec<DockSurfaceChangeCategory> {
        DockSurfaceChangeCategory::ALL
            .into_iter()
            .filter(|category| self.category_bits & category.bit() != 0)
            .collect()
    }

    fn record_transition(&mut self, transition: DockSurfaceTransition) {
        self.transition_bits |= transition.bit();
    }

    fn transitions(&self) -> Vec<DockSurfaceTransition> {
        DockSurfaceTransition::ALL
            .into_iter()
            .filter(|transition| self.transition_bits & transition.bit() != 0)
            .collect()
    }
}

/// Private application-level owner for one dock controller and viewport runtime.
///
/// Every `DockSurface` clone points at the same entity, so revision and transaction state cannot
/// diverge between handles.
#[derive(Debug)]
pub(crate) struct DockSurfaceOwner {
    controller: Entity<DockController>,
    viewport_runtime: DockViewportRuntimeHandle,
    primary_space: DockSpaceId,
    window_session: DockSurfaceWindowSession,
    live_undock: DockLiveUndockSession,
    live_undock_runtime: DockLiveUndockRuntime,
    payload_recovery: DockPayloadRecoveryRegistry,
    payload_recovery_executor: DockPayloadRecoveryExecutor,
    activation: DockSurfaceActivationState,
    revision: u64,
    last_published_revision: u64,
    pending_publications: BTreeMap<u64, PendingDockSurfacePublication>,
    publication_flush_active: bool,
    last_transaction_id: u64,
    pending_transaction: Option<PendingDockSurfaceTransaction>,
}

#[derive(Debug)]
pub(crate) struct DockPayloadRecoveryRestoreFinalCommit {
    key: DockPayloadRecoveryExecutionKey,
    prepared: DockPayloadRecoveryRestorePrepared,
}

impl DockSurfaceOwner {
    /// Creates an owner around one controller/runtime pair.
    pub(crate) fn new(
        controller: Entity<DockController>,
        viewport_runtime: DockViewportRuntimeHandle,
        primary_space: DockSpaceId,
        entity_id: EntityId,
    ) -> Self {
        Self {
            controller,
            viewport_runtime,
            primary_space,
            window_session: DockSurfaceWindowSession::new(entity_id),
            live_undock: DockLiveUndockSession::new(),
            live_undock_runtime: DockLiveUndockRuntime::new(),
            payload_recovery: DockPayloadRecoveryRegistry::new(),
            payload_recovery_executor: DockPayloadRecoveryExecutor::new(),
            activation: DockSurfaceActivationState::new(),
            revision: 0,
            last_published_revision: 0,
            pending_publications: BTreeMap::new(),
            publication_flush_active: false,
            last_transaction_id: 0,
            pending_transaction: None,
        }
    }

    /// Returns the shared controller entity.
    pub(crate) fn controller(&self) -> Entity<DockController> {
        self.controller.clone()
    }

    /// Returns the shared viewport-runtime handle.
    pub(crate) fn runtime(&self) -> DockViewportRuntimeHandle {
        self.viewport_runtime.clone()
    }

    /// Returns the default logical dock space.
    pub(crate) fn primary_space(&self) -> &DockSpaceId {
        &self.primary_space
    }

    pub(crate) fn window_session(&self) -> &DockSurfaceWindowSession {
        &self.window_session
    }

    pub(crate) fn window_session_mut(&mut self) -> &mut DockSurfaceWindowSession {
        &mut self.window_session
    }

    pub(crate) fn reduce_live_undock_fact(
        &mut self,
        fact: DockLiveUndockFact,
    ) -> Option<DockLiveUndockEffects> {
        if let DockLiveUndockFact::Trigger { lease, .. } = &fact
            && !self.window_session.admits(*lease)
        {
            return None;
        }
        Some(self.live_undock.apply(fact))
    }

    pub(crate) fn live_undock_runtime(&self) -> DockLiveUndockRuntime {
        self.live_undock_runtime.clone()
    }

    pub(crate) fn live_undock_committed_destination_logical_close_authority(
        &self,
        window_id: WindowId,
    ) -> Option<super::live_undock_runtime::DockLiveUndockLogicalCloseAuthority> {
        let identity = self.live_undock.current_identity()?;
        self.live_undock_runtime
            .committed_destination_logical_close_authority(identity, window_id)
    }

    pub(crate) fn accepts_live_undock_identity(&self, identity: DockLiveUndockIdentity) -> bool {
        self.live_undock.current_identity() == Some(identity)
    }

    pub(crate) fn live_undock_shutdown_snapshot(
        &self,
        lease: DockSurfaceWindowSessionLease,
    ) -> Option<DockLiveUndockShutdownSnapshot> {
        self.live_undock.shutdown_snapshot(lease)
    }

    pub(crate) fn freeze_live_undock_for_shutdown(
        &mut self,
        lease: DockSurfaceWindowSessionLease,
        promotion_commit: DockLiveUndockPromotionCommitDisposition,
    ) -> DockLiveUndockTransition<Option<DockLiveUndockShutdownSnapshot>> {
        self.live_undock
            .freeze_for_shutdown(lease, promotion_commit)
    }

    pub(crate) fn complete_live_undock_opening(
        &mut self,
        opening: DockLiveUndockOpeningKey,
        window: AnyWindowHandle,
        runtime: DockViewportProvisionalOpenAttemptCompletion,
    ) -> DockLiveUndockTransition<DockLiveUndockOpenReturnOutcome> {
        self.live_undock.complete_opening(opening, window, runtime)
    }

    pub(crate) fn can_admit_live_undock_open_return(
        &self,
        opening: DockLiveUndockOpeningKey,
        window_id: WindowId,
    ) -> bool {
        self.live_undock.can_admit_open_return(opening, window_id)
    }

    pub(crate) fn fail_live_undock_opening(
        &mut self,
        opening: DockLiveUndockOpeningKey,
    ) -> DockLiveUndockTransition<DockLiveUndockOpenFailureOutcome> {
        self.live_undock.fail_opening(opening)
    }

    pub(crate) fn transfer_live_undock_dependency_to_window(
        &mut self,
        opening: DockLiveUndockOpeningKey,
        dependency: DockSurfaceWindowSessionDependencyId,
    ) -> DockSurfaceWindowSessionDependencyTerminalOutcome {
        if !self
            .live_undock
            .has_shutdown_dependency(opening, dependency)
        {
            return DockSurfaceWindowSessionDependencyTerminalOutcome::UnknownDependency;
        }
        let outcome = self
            .window_session
            .settle_dependency(opening.lease(), dependency);
        if matches!(
            outcome,
            DockSurfaceWindowSessionDependencyTerminalOutcome::Settled
                | DockSurfaceWindowSessionDependencyTerminalOutcome::AlreadyTerminal
        ) {
            assert!(
                self.live_undock
                    .transfer_shutdown_dependency_to_window(opening, dependency),
                "validated live-undock dependency must remain transferable inside one owner update"
            );
        }
        outcome
    }

    pub(crate) fn live_undock_lease_for_window(
        &self,
        window_id: WindowId,
    ) -> Option<DockSurfaceWindowSessionLease> {
        self.live_undock.lease_for_window(window_id)
    }

    pub(crate) fn settle_live_undock_window_terminal(
        &mut self,
        window_id: WindowId,
    ) -> DockLiveUndockTransition<Option<DockLiveUndockWindowTerminalOutcome>> {
        let transition = self.live_undock.settle_window_terminal(window_id);
        if let Some(terminal) = transition.outcome()
            && let Some(dependency) = terminal.dependency()
        {
            let outcome = self
                .window_session
                .settle_dependency(terminal.lease(), dependency);
            debug_assert!(matches!(
                outcome,
                DockSurfaceWindowSessionDependencyTerminalOutcome::Settled
                    | DockSurfaceWindowSessionDependencyTerminalOutcome::AlreadyTerminal
            ));
        }
        transition
    }

    pub(crate) fn prepare_payload_recovery(
        &mut self,
        authority: DockPayloadRecoveryAuthority,
        payload_identity: &DockLockedPayloadIdentity,
        reason: DockPayloadRecoveryReason,
        cx: &mut Context<Self>,
    ) -> Result<DockPayloadRecoveryPrepared, DockPayloadRecoveryPrepareError> {
        let graph = cx.read_entity(&self.controller, |controller, _| controller.graph().clone());
        self.payload_recovery
            .prepare(&graph, self.revision, authority, payload_identity, reason)
    }

    pub(crate) fn prepare_payload_recovery_with_focus_and_origin(
        &mut self,
        authority: DockPayloadRecoveryAuthority,
        payload_identity: &DockLockedPayloadIdentity,
        reason: DockPayloadRecoveryReason,
        focus: DockPayloadRecoveryFocus,
        presentation_origin: DockPayloadRecoveryPresentationOrigin,
        cx: &mut Context<Self>,
    ) -> Result<DockPayloadRecoveryPrepared, DockPayloadRecoveryPrepareError> {
        let graph = cx.read_entity(&self.controller, |controller, _| controller.graph().clone());
        self.payload_recovery.prepare_with_focus_and_origin(
            &graph,
            self.revision,
            authority,
            payload_identity,
            reason,
            Some(focus),
            Some(presentation_origin),
        )
    }

    pub(crate) fn prepare_payload_recovery_with_origin(
        &mut self,
        authority: DockPayloadRecoveryAuthority,
        payload_identity: &DockLockedPayloadIdentity,
        reason: DockPayloadRecoveryReason,
        presentation_origin: DockPayloadRecoveryPresentationOrigin,
        cx: &mut Context<Self>,
    ) -> Result<DockPayloadRecoveryPrepared, DockPayloadRecoveryPrepareError> {
        let graph = cx.read_entity(&self.controller, |controller, _| controller.graph().clone());
        self.payload_recovery.prepare_with_focus_and_origin(
            &graph,
            self.revision,
            authority,
            payload_identity,
            reason,
            None,
            Some(presentation_origin),
        )
    }

    pub(crate) fn prepare_unresolved_payload_recovery(
        &mut self,
        authority: DockPayloadRecoveryAuthority,
        payload_identity: &DockLockedPayloadIdentity,
        reason: DockPayloadRecoveryReason,
        unresolved: DockPayloadRecoveryPrepareError,
        cx: &mut Context<Self>,
    ) -> Result<DockPayloadRecoveryPrepared, DockPayloadRecoveryPrepareError> {
        let graph = cx.read_entity(&self.controller, |controller, _| controller.graph().clone());
        self.payload_recovery.prepare_unresolved(
            &graph,
            self.revision,
            authority,
            payload_identity,
            reason,
            unresolved,
        )
    }

    pub(crate) fn prepare_unresolved_payload_recovery_with_origin(
        &mut self,
        authority: DockPayloadRecoveryAuthority,
        payload_identity: &DockLockedPayloadIdentity,
        reason: DockPayloadRecoveryReason,
        unresolved: DockPayloadRecoveryPrepareError,
        presentation_origin: DockPayloadRecoveryPresentationOrigin,
        cx: &mut Context<Self>,
    ) -> Result<DockPayloadRecoveryPrepared, DockPayloadRecoveryPrepareError> {
        let graph = cx.read_entity(&self.controller, |controller, _| controller.graph().clone());
        self.payload_recovery.prepare_unresolved_with_origin(
            &graph,
            self.revision,
            authority,
            payload_identity,
            reason,
            unresolved,
            Some(presentation_origin),
        )
    }

    pub(crate) fn can_commit_payload_recovery(
        &self,
        prepared: &DockPayloadRecoveryPrepared,
        cx: &mut Context<Self>,
    ) -> bool {
        let graph = cx.read_entity(&self.controller, |controller, _| controller.graph().clone());
        self.payload_recovery
            .can_commit(&graph, self.revision, prepared)
    }

    pub(crate) fn commit_payload_recovery(
        &mut self,
        transaction: DockSurfaceTransactionId,
        prepared: &DockPayloadRecoveryPrepared,
        cx: &mut Context<Self>,
    ) -> Result<DockPayloadRecoveryCommitReceipt, DockPayloadRecoveryCommitError> {
        self.assert_active_transaction(transaction);
        let graph = cx.read_entity(&self.controller, |controller, _| controller.graph().clone());
        let primary_space_is_live = self.window_session.active_lease().is_some();
        let receipt = self.payload_recovery.commit(
            &graph,
            self.revision,
            &self.primary_space,
            primary_space_is_live,
            prepared,
        )?;
        if prepared.reason() == DockPayloadRecoveryReason::LostViewportRecovery
            && receipt.disposition()
                == super::payload_recovery::DockPayloadRecoveryDisposition::VisibleRecoveryEntry
        {
            let bound = self
                .payload_recovery
                .bind_visible_entry_focus(receipt, cx.focus_handle());
            debug_assert!(
                bound,
                "a new visible recovery record must bind one focus handle"
            );
            cx.notify();
        }
        self.record_change(transaction, DockSurfaceChangeCategory::PanelLifecycle);
        if prepared.reason() == DockPayloadRecoveryReason::LostViewportRecovery {
            self.record_change(transaction, DockSurfaceChangeCategory::ViewportTopology);
            self.record_transition(
                transaction,
                DockSurfaceTransition::ViewportLostAfterPromotion,
            );
        }
        Ok(receipt)
    }

    pub(crate) fn prepare_payload_recovery_restore(
        &self,
        action: DockPayloadRecoveryRestoreAction,
        cx: &mut Context<Self>,
    ) -> Result<DockPayloadRecoveryRestorePrepared, DockPayloadRecoveryRestoreError> {
        let active_anchor = self.window_session.active_lease();
        let graph = cx.read_entity(&self.controller, |controller, _| controller.graph().clone());
        self.payload_recovery.prepare_restore(
            &graph,
            self.revision,
            &self.primary_space,
            active_anchor,
            action,
        )
    }

    pub(crate) fn reserve_payload_recovery_restore(
        &mut self,
        action: DockPayloadRecoveryRestoreAction,
        cx: &mut Context<Self>,
    ) -> Result<
        (
            DockPayloadRecoveryExecutionKey,
            DockPayloadRecoveryRestorePrepared,
        ),
        DockPayloadRecoveryRestoreError,
    > {
        let key = self.payload_recovery_executor.reserve(action)?;
        match self.prepare_payload_recovery_restore(action, cx) {
            Ok(prepared) => Ok((key, prepared)),
            Err(error) => {
                assert!(
                    self.payload_recovery_executor.finish(key),
                    "a failed restore preparation must retire its exact reservation"
                );
                Err(error)
            }
        }
    }

    pub(crate) fn install_payload_recovery_transfer(
        &mut self,
        key: DockPayloadRecoveryExecutionKey,
        transfer: DockPayloadRecoveryTransfer,
        session: &mut Option<view_presentation_window::RehostSession>,
    ) -> bool {
        self.payload_recovery_executor
            .install_transfer(key, transfer, session)
    }

    pub(crate) fn accept_payload_recovery_source_proxy_frame(
        &mut self,
        key: crate::host::DockHostRecoveryPresentationKey,
        accepted_frame: u64,
    ) -> Option<
        Result<
            view_presentation_window::RehostSourceProxyCommit,
            view_presentation_window::TransitionError,
        >,
    > {
        self.payload_recovery_executor
            .accept_source_proxy_frame(key, accepted_frame)
    }

    pub(crate) fn accept_payload_recovery_destination_frame(
        &mut self,
        key: crate::host::DockHostRecoveryPresentationKey,
        accepted_frame: u64,
        cx: &mut Context<Self>,
    ) -> Option<
        Result<
            view_presentation_window::RehostDestinationExposure,
            view_presentation_window::TransitionError,
        >,
    > {
        self.payload_recovery_executor
            .accept_destination_frame(key, accepted_frame, cx)
    }

    pub(crate) fn accept_payload_recovery_destination_presentation_frame(
        &self,
        key: crate::host::DockHostRecoveryPresentationKey,
        accepted_frame: u64,
        cx: &Context<Self>,
    ) -> Option<
        Result<
            view_presentation_window::RehostDestinationPresentation,
            view_presentation_window::TransitionError,
        >,
    > {
        self.payload_recovery_executor
            .accept_destination_presentation_frame(key, accepted_frame, cx)
    }

    pub(crate) fn settle_payload_recovery_source(
        &mut self,
        key: DockPayloadRecoveryExecutionKey,
        cx: &mut Context<Self>,
    ) -> Option<
        Result<
            view_presentation_window::SourceSettlement,
            view_presentation_window::TransitionError,
        >,
    > {
        self.payload_recovery_executor.settle_source(key, cx)
    }

    pub(crate) fn accept_payload_recovery_source_restoration_frame(
        &mut self,
        key: crate::host::DockHostRecoveryPresentationKey,
        accepted_frame: u64,
        cx: &mut Context<Self>,
    ) -> Option<
        Result<
            view_presentation_window::SourcePresentationFinish,
            view_presentation_window::TransitionError,
        >,
    > {
        self.payload_recovery_executor
            .accept_source_restoration_frame(key, accepted_frame, cx)
    }

    pub(crate) fn abandon_payload_recovery_rehost_after_source_loss(
        &mut self,
        key: DockPayloadRecoveryExecutionKey,
        cx: &mut Context<Self>,
    ) -> Option<
        Result<
            view_presentation_window::RehostAbandonmentOutcome,
            view_presentation_window::TransitionError,
        >,
    > {
        self.payload_recovery_executor
            .abandon_after_source_loss(key, cx)
    }

    pub(crate) fn prepare_payload_recovery_destination_terminal(
        &mut self,
        key: DockPayloadRecoveryExecutionKey,
        presented: &view_presentation_window::RehostDestinationPresentation,
        cx: &mut Context<Self>,
    ) -> Option<
        Result<
            view_presentation_window::RehostTerminalPreparation,
            view_presentation_window::TransitionError,
        >,
    > {
        self.payload_recovery_executor.prepare_terminal(
            key,
            cx,
            view_presentation_window::RehostTerminalIntent::CommitDestination(presented),
        )
    }

    pub(crate) fn payload_recovery_transfer(
        &self,
        key: DockPayloadRecoveryExecutionKey,
    ) -> Option<DockPayloadRecoveryTransfer> {
        self.payload_recovery_executor.transfer(key).cloned()
    }

    pub(crate) fn payload_recovery_transfer_for_presentation(
        &self,
        key: crate::host::DockHostRecoveryPresentationKey,
    ) -> Option<DockPayloadRecoveryTransfer> {
        self.payload_recovery_executor
            .transfer_for_presentation(key)
            .cloned()
    }

    pub(crate) fn queue_payload_recovery_finalization(
        &mut self,
        key: crate::host::DockHostRecoveryPresentationKey,
        presented: view_presentation_window::RehostDestinationPresentation,
    ) -> Option<DockPayloadRecoveryTransfer> {
        self.payload_recovery_executor
            .queue_finalization(key, presented)
    }

    pub(crate) fn payload_recovery_finalization(
        &self,
        key: DockPayloadRecoveryExecutionKey,
    ) -> Option<(DockPayloadRecoveryTransfer, DockPayloadRecoveryFinalization)> {
        self.payload_recovery_executor.finalization(key)
    }

    pub(crate) fn payload_recovery_rehost_terminal_disposition(
        &self,
        key: DockPayloadRecoveryExecutionKey,
    ) -> Option<view_presentation_window::RehostTerminalDisposition> {
        self.payload_recovery_executor
            .session_terminal_disposition(key)
    }

    pub(crate) fn cancel_payload_recovery_execution(
        &mut self,
        key: DockPayloadRecoveryExecutionKey,
    ) -> bool {
        self.payload_recovery_executor.finish(key)
            || self.payload_recovery_executor.finish_terminal(key)
    }

    pub(crate) fn record_payload_recovery_source_native_terminal(
        &mut self,
        window_id: WindowId,
    ) -> Option<DockPayloadRecoveryTransfer> {
        self.payload_recovery_executor
            .record_source_native_terminal(window_id)
    }

    pub(crate) fn record_payload_recovery_source_logical_close(
        &mut self,
        window_id: WindowId,
    ) -> Option<DockPayloadRecoveryTransfer> {
        self.payload_recovery_executor
            .record_source_logical_close(window_id)
    }

    pub(crate) fn payload_recovery_source_close_state(
        &self,
        key: DockPayloadRecoveryExecutionKey,
    ) -> Option<(bool, bool)> {
        self.payload_recovery_executor.source_close_state(key)
    }

    pub(crate) fn payload_recovery_source_native_terminal_seen(
        &self,
        key: DockPayloadRecoveryExecutionKey,
    ) -> bool {
        self.payload_recovery_executor
            .source_native_terminal_seen(key)
    }

    pub(crate) fn payload_recovery_source_settlement_started(
        &self,
        key: DockPayloadRecoveryExecutionKey,
    ) -> bool {
        self.payload_recovery_executor
            .source_settlement_started(key)
    }

    #[cfg(test)]
    pub(crate) fn pause_payload_recovery_after_source_release_once_for_test(&mut self) {
        self.payload_recovery_executor
            .pause_after_source_release_once_for_test();
    }

    #[cfg(test)]
    pub(crate) fn take_payload_recovery_pause_after_source_release_for_test(&mut self) -> bool {
        self.payload_recovery_executor
            .take_pause_after_source_release_for_test()
    }

    #[cfg(test)]
    pub(crate) fn pause_payload_recovery_after_source_restoration_once_for_test(&mut self) {
        self.payload_recovery_executor
            .pause_after_source_restoration_once_for_test();
    }

    #[cfg(test)]
    pub(crate) fn take_payload_recovery_pause_after_source_restoration_for_test(&mut self) -> bool {
        self.payload_recovery_executor
            .take_pause_after_source_restoration_for_test()
    }

    #[cfg(test)]
    pub(crate) fn replace_payload_recovery_source_host_after_finish_once_for_test(&mut self) {
        self.payload_recovery_executor
            .replace_source_host_after_finish_once_for_test();
    }

    #[cfg(test)]
    pub(crate) fn take_payload_recovery_replace_source_host_after_finish_for_test(
        &mut self,
    ) -> bool {
        self.payload_recovery_executor
            .take_replace_source_host_after_finish_for_test()
    }

    #[cfg(test)]
    pub(crate) fn reject_next_payload_recovery_transfer_install_for_test(&mut self) {
        self.payload_recovery_executor
            .reject_next_transfer_install_once_for_test();
    }

    #[cfg(test)]
    pub(crate) fn panic_after_payload_recovery_finalization_stage_once_for_test(
        &mut self,
        stage: super::payload_recovery_executor::DockPayloadRecoveryFinalizationPanicStage,
    ) {
        self.payload_recovery_executor
            .panic_after_finalization_stage_once_for_test(stage);
    }

    #[cfg(test)]
    pub(crate) fn panic_after_payload_recovery_finalization_stage_for_test(
        &mut self,
        stage: super::payload_recovery_executor::DockPayloadRecoveryFinalizationPanicStage,
        attempts: u8,
    ) {
        self.payload_recovery_executor
            .panic_after_finalization_stage_for_test(stage, attempts);
    }

    #[cfg(test)]
    pub(crate) fn take_payload_recovery_finalization_panic_for_test(
        &mut self,
        stage: super::payload_recovery_executor::DockPayloadRecoveryFinalizationPanicStage,
    ) -> bool {
        self.payload_recovery_executor
            .take_finalization_panic_for_test(stage)
    }

    #[cfg(test)]
    pub(crate) fn pause_before_payload_recovery_finalization_once_for_test(&mut self) {
        self.payload_recovery_executor
            .pause_before_finalization_once_for_test();
    }

    #[cfg(test)]
    pub(crate) fn take_payload_recovery_finalization_pause_for_test(&mut self) -> bool {
        self.payload_recovery_executor
            .take_pause_before_finalization_for_test()
    }

    #[cfg(test)]
    pub(crate) fn pause_payload_recovery_finalization_retry_once_for_test(&mut self) {
        self.payload_recovery_executor
            .pause_finalization_retry_once_for_test();
    }

    #[cfg(test)]
    pub(crate) fn take_payload_recovery_finalization_retry_pause_for_test(&mut self) -> bool {
        self.payload_recovery_executor
            .take_pause_finalization_retry_for_test()
    }

    #[cfg(test)]
    pub(crate) fn panic_after_payload_recovery_installation_stage_once_for_test(
        &mut self,
        stage: super::payload_recovery_executor::DockPayloadRecoveryInstallationPanicStage,
    ) {
        self.payload_recovery_executor
            .panic_after_installation_stage_once_for_test(stage);
    }

    #[cfg(test)]
    pub(crate) fn take_payload_recovery_installation_panic_for_test(
        &mut self,
        stage: super::payload_recovery_executor::DockPayloadRecoveryInstallationPanicStage,
    ) -> bool {
        self.payload_recovery_executor
            .take_installation_panic_for_test(stage)
    }

    #[cfg(test)]
    pub(crate) fn payload_recovery_execution_snapshot_for_test(
        &self,
    ) -> Option<(
        DockPayloadRecoveryExecutionKey,
        open_gpui::view_presentation_window::RehostPhase,
        bool,
        WindowId,
        WindowId,
        bool,
    )> {
        self.payload_recovery_executor.execution_snapshot_for_test()
    }

    pub(crate) fn prepare_payload_recovery_restore_final(
        &self,
        key: DockPayloadRecoveryExecutionKey,
        prepared: DockPayloadRecoveryRestorePrepared,
        cx: &mut Context<Self>,
    ) -> Result<DockPayloadRecoveryRestoreFinalCommit, DockPayloadRecoveryRestoreError> {
        let execution_is_current = self.payload_recovery_executor.is_reserved(key)
            || self.payload_recovery_executor.transfer(key).is_some();
        if !execution_is_current || prepared.action() != key.action() {
            return Err(DockPayloadRecoveryRestoreError::StaleAction);
        }
        if !self.can_commit_prepared_payload_recovery_restore(&prepared, cx) {
            return Err(DockPayloadRecoveryRestoreError::StaleAction);
        }
        Ok(DockPayloadRecoveryRestoreFinalCommit { key, prepared })
    }

    pub(crate) fn can_commit_payload_recovery_restore_final(
        &self,
        prepared: &DockPayloadRecoveryRestoreFinalCommit,
        cx: &mut Context<Self>,
    ) -> bool {
        let execution_is_current = self.payload_recovery_executor.is_reserved(prepared.key)
            || self
                .payload_recovery_executor
                .transfer(prepared.key)
                .is_some();
        execution_is_current
            && prepared.prepared.action() == prepared.key.action()
            && self.can_commit_prepared_payload_recovery_restore(&prepared.prepared, cx)
    }

    pub(crate) fn commit_payload_recovery_restore_final(
        &mut self,
        transaction: DockSurfaceTransactionId,
        prepared: DockPayloadRecoveryRestoreFinalCommit,
        cx: &mut Context<Self>,
    ) -> DockPayloadRecoveryRestoreReceipt {
        self.assert_active_transaction(transaction);
        let already_committed = self
            .payload_recovery
            .committed_restore_receipt(prepared.prepared.action())
            .is_some();
        let projected_graph = prepared.prepared.projected_graph().clone();
        cx.update_entity(&self.controller, |controller, controller_cx| {
            controller.workspace_mut().set_graph(projected_graph);
            controller_cx.notify();
        });
        let receipt = self
            .payload_recovery
            .commit_prepared_restore(prepared.prepared);
        if !already_committed {
            self.record_changes(
                transaction,
                [
                    DockSurfaceChangeCategory::Layout,
                    DockSurfaceChangeCategory::Selection,
                    DockSurfaceChangeCategory::PanelLifecycle,
                ],
            );
            self.record_transition(transaction, DockSurfaceTransition::ViewportRecovered);
        }
        cx.notify();
        receipt
    }

    pub(crate) fn committed_payload_recovery_restore_receipt(
        &self,
        action: DockPayloadRecoveryRestoreAction,
    ) -> Option<DockPayloadRecoveryRestoreReceipt> {
        self.payload_recovery
            .committed_restore_receipt(action)
            .cloned()
    }

    pub(crate) fn retire_committed_payload_recovery_restore(
        &mut self,
        action: DockPayloadRecoveryRestoreAction,
        receipt: &DockPayloadRecoveryRestoreReceipt,
    ) -> bool {
        self.payload_recovery
            .retire_committed_restore(action, receipt)
    }

    #[cfg(test)]
    pub(crate) fn payload_recovery_committed_restore_count_for_test(&self) -> usize {
        self.payload_recovery.committed_restore_count_for_test()
    }

    pub(crate) fn can_commit_prepared_payload_recovery_restore(
        &self,
        prepared: &DockPayloadRecoveryRestorePrepared,
        cx: &mut Context<Self>,
    ) -> bool {
        let active_anchor = self.window_session.active_lease();
        let graph = cx.read_entity(&self.controller, |controller, _| controller.graph().clone());
        self.payload_recovery
            .can_commit_restore(&graph, self.revision, active_anchor, prepared)
    }

    pub(crate) fn commit_prepared_payload_recovery_restore(
        &mut self,
        transaction: DockSurfaceTransactionId,
        prepared: DockPayloadRecoveryRestorePrepared,
        cx: &mut Context<Self>,
    ) -> DockPayloadRecoveryRestoreReceipt {
        self.assert_active_transaction(transaction);
        assert!(
            self.can_commit_prepared_payload_recovery_restore(&prepared, cx),
            "prepared payload recovery restore must remain exact until commit"
        );
        let projected_graph = prepared.projected_graph().clone();
        cx.update_entity(&self.controller, |controller, _| {
            controller.workspace_mut().set_graph(projected_graph);
        });
        let receipt = self.payload_recovery.commit_prepared_restore(prepared);
        self.record_changes(
            transaction,
            [
                DockSurfaceChangeCategory::Layout,
                DockSurfaceChangeCategory::Selection,
                DockSurfaceChangeCategory::PanelLifecycle,
            ],
        );
        self.record_transition(transaction, DockSurfaceTransition::ViewportRecovered);
        cx.notify();
        receipt
    }

    pub(crate) fn restore_payload_recovery(
        &mut self,
        transaction: DockSurfaceTransactionId,
        action: DockPayloadRecoveryRestoreAction,
        cx: &mut Context<Self>,
    ) -> Result<DockPayloadRecoveryRestoreReceipt, DockPayloadRecoveryRestoreError> {
        self.assert_active_transaction(transaction);
        let prepared = self.prepare_payload_recovery_restore(action, cx)?;
        if !self.can_commit_prepared_payload_recovery_restore(&prepared, cx) {
            return Err(DockPayloadRecoveryRestoreError::StaleAction);
        }
        Ok(self.commit_prepared_payload_recovery_restore(transaction, prepared, cx))
    }

    pub(crate) fn committed_payload_recovery_receipt(
        &self,
        authority: DockPayloadRecoveryAuthority,
        reason: DockPayloadRecoveryReason,
    ) -> Option<DockPayloadRecoveryCommitReceipt> {
        self.payload_recovery.committed_receipt(authority, reason)
    }

    pub(crate) fn payload_recovery_restore_action(
        &self,
        recovery: DockPayloadRecoveryCommitReceipt,
    ) -> Option<DockPayloadRecoveryRestoreAction> {
        self.window_session
            .active_lease()
            .and_then(|anchor_lease| self.payload_recovery.restore_action(recovery, anchor_lease))
    }

    pub(crate) fn visible_payload_recovery_entries(&self) -> Vec<DockPayloadRecoveryEntry> {
        self.window_session
            .active_lease()
            .map(|anchor_lease| self.payload_recovery.visible_entries(anchor_lease))
            .unwrap_or_default()
    }

    pub(crate) fn settle_payload_recovery_entry_focus(
        &mut self,
        action: DockPayloadRecoveryRestoreAction,
        cx: &mut Context<Self>,
    ) -> bool {
        let changed = self.payload_recovery.settle_entry_focus(action);
        if changed {
            cx.notify();
        }
        changed
    }

    #[cfg(test)]
    pub(crate) fn live_undock_phase(&self) -> super::live_undock::DockLiveUndockPhase {
        self.live_undock.phase()
    }

    #[cfg(test)]
    pub(crate) fn current_live_undock_presentation_failure_for_test(
        &self,
    ) -> Option<DockLiveUndockFact> {
        self.live_undock.current_presentation_failure_for_test()
    }

    #[cfg(test)]
    pub(crate) fn visible_payload_recovery_count_for_test(
        &self,
        reason: DockPayloadRecoveryReason,
    ) -> usize {
        self.payload_recovery
            .visible_records()
            .filter(|record| record.reason() == reason)
            .count()
    }

    pub(crate) fn activation(&self) -> &DockSurfaceActivationState {
        &self.activation
    }

    pub(crate) fn activation_mut(&mut self) -> &mut DockSurfaceActivationState {
        &mut self.activation
    }

    /// Returns the latest committed surface revision.
    pub(crate) const fn revision(&self) -> u64 {
        self.revision
    }

    /// Begins a distinct root surface transaction.
    ///
    /// Nested work must carry the returned identity rather than beginning another root
    /// transaction.
    pub(crate) fn begin_root_transaction(&mut self) -> DockSurfaceTransactionId {
        assert!(
            self.pending_transaction.is_none(),
            "cannot begin a dock surface root transaction while another transaction is active"
        );

        self.last_transaction_id = self
            .last_transaction_id
            .checked_add(1)
            .expect("dock surface transaction identity space exhausted");
        let id = DockSurfaceTransactionId(self.last_transaction_id);
        self.pending_transaction = Some(PendingDockSurfaceTransaction::new(id, None));
        id
    }

    /// Begins a root transaction whose exact commit result must outlive synchronous re-entry.
    pub(crate) fn begin_tracked_root_transaction(
        &mut self,
    ) -> (DockSurfaceTransactionId, DockSurfaceTransactionReceipt) {
        assert!(
            self.pending_transaction.is_none(),
            "cannot begin a tracked dock surface transaction while another transaction is active"
        );

        self.last_transaction_id = self
            .last_transaction_id
            .checked_add(1)
            .expect("dock surface transaction identity space exhausted");
        let id = DockSurfaceTransactionId(self.last_transaction_id);
        let receipt = DockSurfaceTransactionReceipt::pending();
        self.pending_transaction = Some(PendingDockSurfaceTransaction::new(
            id,
            Some(receipt.clone()),
        ));
        (id, receipt)
    }

    /// Records one committed change category against the active root transaction.
    pub(crate) fn record_change(
        &mut self,
        transaction: DockSurfaceTransactionId,
        category: DockSurfaceChangeCategory,
    ) {
        self.assert_active_transaction(transaction);
        let pending = self.pending_transaction.as_mut().expect(
            "validated dock surface transaction must remain active while recording a change",
        );
        pending.record(category);
    }

    /// Records committed change categories against the active root transaction.
    pub(crate) fn record_changes(
        &mut self,
        transaction: DockSurfaceTransactionId,
        categories: impl IntoIterator<Item = DockSurfaceChangeCategory>,
    ) {
        for category in categories {
            self.record_change(transaction, category);
        }
    }

    /// Records one named lifecycle transition against the active root transaction.
    pub(crate) fn record_transition(
        &mut self,
        transaction: DockSurfaceTransactionId,
        transition: DockSurfaceTransition,
    ) {
        self.assert_active_transaction(transaction);
        let pending = self.pending_transaction.as_mut().expect(
            "validated dock surface transaction must remain active while recording a transition",
        );
        pending.record_transition(transition);
    }

    /// Commits a root transaction and returns its metadata event when it recorded durable changes.
    ///
    /// Empty transactions do not advance the revision. The pending transaction is cleared and a
    /// tracked receipt is filled before this method returns.
    fn commit_root_transaction(
        &mut self,
        transaction: DockSurfaceTransactionId,
    ) -> Option<DockSurfaceChangeEvent> {
        let pending = self
            .pending_transaction
            .as_ref()
            .expect("cannot commit a dock surface transaction that is not active");
        assert_eq!(
            pending.id, transaction,
            "attempted to commit a different dock surface transaction"
        );
        let pending = self
            .pending_transaction
            .take()
            .expect("validated dock surface transaction must remain active");

        let categories = pending.categories();
        let transitions = pending.transitions();
        if categories.is_empty() {
            if let Some(receipt) = pending.receipt {
                receipt.commit(self.revision);
            }
            return None;
        }

        self.revision = self
            .revision
            .checked_add(1)
            .expect("dock surface revision space exhausted");
        let event = DockSurfaceChangeEvent::new(self.revision, categories, transitions);
        assert!(
            self.pending_publications
                .insert(
                    self.revision,
                    PendingDockSurfacePublication {
                        event: event.clone(),
                        ready: false,
                    },
                )
                .is_none(),
            "a dock surface revision must own exactly one publication"
        );
        if let Some(receipt) = pending.receipt {
            receipt.commit(self.revision);
        }
        Some(event)
    }

    fn publish_committed_event(&mut self, revision: u64, cx: &mut Context<Self>) {
        let publication = self
            .pending_publications
            .get_mut(&revision)
            .expect("a committed dock surface event must retain its publication slot");
        publication.ready = true;
        self.flush_ready_publications(cx);
    }

    fn flush_ready_publications(&mut self, cx: &mut Context<Self>) {
        if self.publication_flush_active {
            return;
        }

        self.publication_flush_active = true;
        let mut first_panic = None;
        loop {
            let next_revision = self
                .last_published_revision
                .checked_add(1)
                .expect("dock surface publication revision space exhausted");
            let ready = self
                .pending_publications
                .get(&next_revision)
                .is_some_and(|publication| publication.ready);
            if !ready {
                break;
            }

            let publication = self
                .pending_publications
                .remove(&next_revision)
                .expect("a ready dock surface publication must remain queued");
            self.last_published_revision = next_revision;
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| cx.emit(publication.event))) {
                if first_panic.is_none() {
                    first_panic = Some(payload);
                } else {
                    log::error!("suppressed a secondary dock surface subscriber panic");
                }
            }
        }
        self.publication_flush_active = false;

        if let Some(payload) = first_panic {
            resume_unwind(payload);
        }
    }

    /// Finishes a root transaction and emits one metadata event when it recorded durable changes.
    ///
    /// Empty transactions do not advance the revision. The pending transaction is cleared before
    /// event publication so an event subscriber can synchronously issue another root command.
    pub(crate) fn finish_root_transaction(
        &mut self,
        transaction: DockSurfaceTransactionId,
        cx: &mut Context<Self>,
    ) -> Option<DockSurfaceChangeEvent> {
        let event = self.commit_root_transaction(transaction)?;
        self.publish_committed_event(event.revision(), cx);
        Some(event)
    }

    /// Aborts a root transaction whose update did not reach its commit boundary.
    fn abort_root_transaction(&mut self, transaction: DockSurfaceTransactionId) {
        let pending = self
            .pending_transaction
            .as_ref()
            .expect("cannot abort a dock surface transaction that is not active");
        assert_eq!(
            pending.id, transaction,
            "attempted to abort a different dock surface transaction"
        );
        let pending = self
            .pending_transaction
            .take()
            .expect("validated dock surface transaction must remain active");
        if let Some(receipt) = pending.receipt {
            receipt.abort();
        }
    }

    fn assert_active_transaction(&self, transaction: DockSurfaceTransactionId) {
        let pending = self
            .pending_transaction
            .as_ref()
            .expect("cannot use a dock surface transaction that is not active");
        assert_eq!(
            pending.id, transaction,
            "dock surface work belongs to a different transaction"
        );
    }
}

impl EventEmitter<DockSurfaceChangeEvent> for DockSurfaceOwner {}

/// Runs one explicit root transaction against a surface owner.
///
/// `update` should thread the supplied identity through nested controller/runtime operations and
/// record only categories backed by committed facts.
pub(crate) fn with_root_transaction<C, R>(
    owner: &Entity<DockSurfaceOwner>,
    cx: &mut C,
    update: impl FnOnce(
        &mut DockSurfaceOwner,
        DockSurfaceTransactionId,
        &mut Context<DockSurfaceOwner>,
    ) -> R,
) -> R
where
    C: AppContext,
{
    cx.update_entity(owner, |owner, cx| {
        let transaction = owner.begin_root_transaction();
        match catch_unwind(AssertUnwindSafe(|| update(owner, transaction, cx))) {
            Ok(result) => {
                owner.finish_root_transaction(transaction, cx);
                result
            }
            Err(payload) => {
                owner.abort_root_transaction(transaction);
                resume_unwind(payload);
            }
        }
    })
}

/// Runs a root transaction whose work may synchronously re-enter the app.
///
/// The owner borrow is released while `update` runs. Typed nested commit sinks can therefore
/// record against the explicit transaction identity without re-entering an active entity update.
pub(crate) fn with_detached_root_transaction<C, R>(
    owner: &Entity<DockSurfaceOwner>,
    cx: &mut C,
    update: impl FnOnce(DockSurfaceTransactionId, &mut C) -> R,
) -> R
where
    C: AppContext,
{
    let transaction = cx.update_entity(owner, |owner, _| owner.begin_root_transaction());
    match catch_unwind(AssertUnwindSafe(|| update(transaction, cx))) {
        Ok(result) => {
            cx.update_entity(owner, |owner, owner_cx| {
                owner.finish_root_transaction(transaction, owner_cx);
            });
            result
        }
        Err(payload) => {
            cx.update_entity(owner, |owner, _| {
                owner.abort_root_transaction(transaction);
            });
            resume_unwind(payload);
        }
    }
}

/// Commits a tracked detached transaction without synchronously publishing its event.
///
/// The returned publication is a linear continuation for event delivery. Its paired receipt is
/// committed before this function returns, allowing an enclosing runtime to install its own
/// durable authority before subscribers can synchronously re-enter.
pub(crate) fn with_detached_deferred_tracked_root_transaction<C, R>(
    owner: &Entity<DockSurfaceOwner>,
    cx: &mut C,
    update: impl FnOnce(DockSurfaceTransactionId, DockSurfaceTransactionReceipt, &mut C) -> R,
) -> (R, DockSurfaceDeferredPublication)
where
    C: AppContext,
{
    let (transaction, receipt) =
        cx.update_entity(owner, |owner, _| owner.begin_tracked_root_transaction());
    match catch_unwind(AssertUnwindSafe(|| {
        update(transaction, receipt.clone(), cx)
    })) {
        Ok(result) => {
            let event =
                cx.update_entity(owner, |owner, _| owner.commit_root_transaction(transaction));
            (
                result,
                DockSurfaceDeferredPublication::new(owner.clone(), event),
            )
        }
        Err(payload) => {
            cx.update_entity(owner, |owner, _| {
                owner.abort_root_transaction(transaction);
            });
            resume_unwind(payload);
        }
    }
}

/// Subscribes to committed metadata events from a surface owner.
pub(crate) fn subscribe(
    owner: &Entity<DockSurfaceOwner>,
    cx: &mut App,
    mut on_event: impl FnMut(&DockSurfaceChangeEvent, &mut App) + 'static,
) -> Subscription {
    cx.subscribe(owner, move |_owner, event, cx| on_event(event, cx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DockPanelPlacement, DockSurface};
    use open_gpui::{IntoElement, Render, Window, div};
    use std::{
        cell::RefCell,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };

    struct TestPanel;

    impl Render for TestPanel {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    fn test_panel(cx: &mut App) -> open_gpui::AnyView {
        cx.new(|_| TestPanel).into()
    }

    fn test_surface(cx: &mut App) -> DockSurface {
        DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel_factory("editor", "Editor", test_panel)
            .build(cx)
            .expect("surface layout should validate")
    }

    #[open_gpui::test]
    fn deferred_publication_commits_revision_and_receipt_before_delivery(
        cx: &mut open_gpui::TestAppContext,
    ) {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let observed_for_subscription = observed.clone();
        let (surface, receipt, publication, subscription) = cx.update(|cx| {
            let surface = test_surface(cx);
            let subscription = surface.subscribe_changes(cx, move |event, _| {
                observed_for_subscription
                    .borrow_mut()
                    .push(event.revision());
            });
            let owner = surface.owner().clone();
            let (receipt, publication) = with_detached_deferred_tracked_root_transaction(
                &owner,
                cx,
                |transaction, receipt, cx| {
                    cx.update_entity(&owner, |owner, _| {
                        owner.record_change(transaction, DockSurfaceChangeCategory::Layout);
                    });
                    receipt
                },
            );

            assert_eq!(surface.revision(cx), 1);
            assert_eq!(receipt.committed_revision(), Some(1));
            assert!(!receipt.is_aborted());
            assert!(publication.has_event());
            assert!(observed.borrow().is_empty());
            (surface, receipt, publication, subscription)
        });

        assert!(observed.borrow().is_empty());
        assert!(cx.update(|cx| publication.publish(cx)));
        assert!(publication.is_settled());
        assert!(!cx.update(|cx| publication.publish(cx)));
        assert_eq!(observed.borrow().as_slice(), &[1]);
        assert_eq!(receipt.committed_revision(), Some(1));
        assert_eq!(cx.read(|cx| surface.revision(cx)), 1);
        drop(subscription);
    }

    #[open_gpui::test]
    fn deferred_publication_orders_later_surface_events_by_revision(
        cx: &mut open_gpui::TestAppContext,
    ) {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let observed_for_subscription = observed.clone();
        let (surface, publication, subscription) = cx.update(|cx| {
            let surface = test_surface(cx);
            let subscription = surface.subscribe_changes(cx, move |event, _| {
                observed_for_subscription
                    .borrow_mut()
                    .push(event.revision());
            });
            let owner = surface.owner().clone();
            let (_, publication) = with_detached_deferred_tracked_root_transaction(
                &owner,
                cx,
                |transaction, _, cx| {
                    cx.update_entity(&owner, |owner, _| {
                        owner.record_change(transaction, DockSurfaceChangeCategory::Layout);
                    });
                },
            );
            with_root_transaction(&owner, cx, |owner, transaction, _| {
                owner.record_change(transaction, DockSurfaceChangeCategory::Selection);
            });

            assert_eq!(surface.revision(cx), 2);
            assert!(observed.borrow().is_empty());
            (surface, publication, subscription)
        });

        assert!(cx.update(|cx| publication.publish(cx)));
        assert_eq!(observed.borrow().as_slice(), &[1, 2]);
        assert_eq!(cx.read(|cx| surface.revision(cx)), 2);
        drop(subscription);
    }

    #[open_gpui::test]
    fn empty_deferred_transaction_commits_receipt_without_publication(
        cx: &mut open_gpui::TestAppContext,
    ) {
        let (surface, receipt, publication) = cx.update(|cx| {
            let surface = test_surface(cx);
            let owner = surface.owner().clone();
            let (receipt, publication) = with_detached_deferred_tracked_root_transaction(
                &owner,
                cx,
                |_transaction, receipt, _cx| receipt,
            );
            (surface, receipt, publication)
        });

        assert_eq!(receipt.committed_revision(), Some(0));
        assert!(!receipt.is_aborted());
        assert!(!publication.has_event());
        assert!(publication.is_settled());
        assert!(!cx.update(|cx| publication.publish(cx)));
        assert_eq!(cx.read(|cx| surface.revision(cx)), 0);
    }

    #[open_gpui::test]
    fn panic_before_deferred_commit_aborts_the_receipt(cx: &mut open_gpui::TestAppContext) {
        let surface = cx.update(test_surface);
        let receipt_slot = Rc::new(RefCell::new(None));
        let receipt_for_update = receipt_slot.clone();

        let panic = catch_unwind(AssertUnwindSafe(|| {
            cx.update(|cx| {
                let owner = surface.owner().clone();
                let _: ((), DockSurfaceDeferredPublication) =
                    with_detached_deferred_tracked_root_transaction(
                        &owner,
                        cx,
                        |transaction, receipt, cx| {
                            receipt_for_update.borrow_mut().replace(receipt);
                            cx.update_entity(&owner, |owner, _| {
                                owner.record_change(transaction, DockSurfaceChangeCategory::Layout);
                            });
                            panic!("injected deferred transaction panic");
                        },
                    );
            });
        }));

        assert!(panic.is_err());
        let receipt = receipt_slot
            .borrow()
            .clone()
            .expect("the tracked receipt should escape before the injected panic");
        assert!(receipt.is_aborted());
        assert_eq!(receipt.committed_revision(), None);
        assert_eq!(cx.read(|cx| surface.revision(cx)), 0);
    }

    #[open_gpui::test]
    fn deferred_subscriber_panic_preserves_committed_receipt(cx: &mut open_gpui::TestAppContext) {
        let subscriber_called = Rc::new(Cell::new(false));
        let called_by_subscription = subscriber_called.clone();
        let (surface, receipt, publication, subscription) = cx.update(|cx| {
            let surface = test_surface(cx);
            let subscription = surface.subscribe_changes(cx, move |_event, _| {
                called_by_subscription.set(true);
                panic!("injected deferred publication subscriber panic");
            });
            let owner = surface.owner().clone();
            let (receipt, publication) = with_detached_deferred_tracked_root_transaction(
                &owner,
                cx,
                |transaction, receipt, cx| {
                    cx.update_entity(&owner, |owner, _| {
                        owner.record_change(transaction, DockSurfaceChangeCategory::Layout);
                    });
                    receipt
                },
            );
            (surface, receipt, publication, subscription)
        });

        let panic = catch_unwind(AssertUnwindSafe(|| {
            cx.update(|cx| publication.publish(cx));
        }));
        assert!(subscriber_called.get());
        assert!(panic.is_err());
        assert!(publication.is_settled());
        assert!(!cx.update(|cx| publication.publish(cx)));
        assert_eq!(receipt.committed_revision(), Some(1));
        assert!(!receipt.is_aborted());
        drop(surface);
        drop(subscription);
    }
}
