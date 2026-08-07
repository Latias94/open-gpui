use super::{
    DockSurfaceActivationState,
    live_undock::{
        DockLiveUndockEffects, DockLiveUndockFact, DockLiveUndockIdentity,
        DockLiveUndockOpenFailureOutcome, DockLiveUndockOpenReturnOutcome,
        DockLiveUndockOpeningKey, DockLiveUndockSession, DockLiveUndockShutdownSnapshot,
        DockLiveUndockTransition, DockLiveUndockWindowTerminalOutcome,
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
        DockPayloadRecoveryExecutionKey, DockPayloadRecoveryExecutor, DockPayloadRecoveryTransfer,
    },
    window_session::{
        DockSurfaceWindowSession, DockSurfaceWindowSessionDependencyId,
        DockSurfaceWindowSessionDependencyTerminalOutcome, DockSurfaceWindowSessionLease,
    },
};
use crate::{
    DockController, DockSpaceId, DockViewportProvisionalOpenAttemptCompletion,
    DockViewportRuntimeHandle, locked_drop_identity::DockLockedPayloadIdentity,
    viewport_registry::DockViewportRegistrationKey,
};
use open_gpui::{
    AnyWindowHandle, App, AppContext, Context, Entity, EntityId, EventEmitter, Subscription,
    WindowId,
};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

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

#[derive(Debug)]
struct PendingDockSurfaceTransaction {
    id: DockSurfaceTransactionId,
    category_bits: u8,
    transition_bits: u8,
}

impl PendingDockSurfaceTransaction {
    fn new(id: DockSurfaceTransactionId) -> Self {
        Self {
            id,
            category_bits: 0,
            transition_bits: 0,
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

    pub(crate) fn live_undock_committed_destination_registration_for_logical_close(
        &self,
        window_id: WindowId,
    ) -> Option<DockViewportRegistrationKey> {
        let identity = self.live_undock.current_identity()?;
        self.live_undock_runtime
            .committed_destination_registration_for_logical_close(identity, window_id)
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
    ) -> DockLiveUndockTransition<Option<DockLiveUndockShutdownSnapshot>> {
        self.live_undock.freeze_for_shutdown(lease)
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
    ) -> bool {
        self.payload_recovery_executor
            .install_transfer(key, transfer)
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

    pub(crate) fn reserve_payload_recovery_finalization(
        &mut self,
        key: crate::host::DockHostRecoveryPresentationKey,
    ) -> Option<DockPayloadRecoveryTransfer> {
        self.payload_recovery_executor.reserve_finalization(key)
    }

    pub(crate) fn cancel_payload_recovery_execution(
        &mut self,
        key: DockPayloadRecoveryExecutionKey,
    ) -> bool {
        self.payload_recovery_executor.finish(key)
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
        assert!(
            self.payload_recovery_executor.finish(prepared.key),
            "prepared payload recovery execution must remain exact until final commit"
        );
        let receipt = self
            .payload_recovery
            .commit_prepared_restore(prepared.prepared);
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
        self.pending_transaction = Some(PendingDockSurfaceTransaction::new(id));
        id
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

    /// Finishes a root transaction and emits one metadata event when it recorded durable changes.
    ///
    /// Empty transactions do not advance the revision. The pending transaction is cleared before
    /// event publication so an event subscriber can synchronously issue another root command.
    pub(crate) fn finish_root_transaction(
        &mut self,
        transaction: DockSurfaceTransactionId,
        cx: &mut Context<Self>,
    ) -> Option<DockSurfaceChangeEvent> {
        let pending = self
            .pending_transaction
            .as_ref()
            .expect("cannot finish a dock surface transaction that is not active");
        assert_eq!(
            pending.id, transaction,
            "attempted to finish a different dock surface transaction"
        );
        let pending = self
            .pending_transaction
            .take()
            .expect("validated dock surface transaction must remain active");

        let categories = pending.categories();
        let transitions = pending.transitions();
        if categories.is_empty() {
            return None;
        }

        self.revision = self
            .revision
            .checked_add(1)
            .expect("dock surface revision space exhausted");
        let event = DockSurfaceChangeEvent::new(self.revision, categories, transitions);
        cx.emit(event.clone());
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
        self.pending_transaction = None;
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

/// Subscribes to committed metadata events from a surface owner.
pub(crate) fn subscribe(
    owner: &Entity<DockSurfaceOwner>,
    cx: &mut App,
    mut on_event: impl FnMut(&DockSurfaceChangeEvent, &mut App) + 'static,
) -> Subscription {
    cx.subscribe(owner, move |_owner, event, cx| on_event(event, cx))
}
