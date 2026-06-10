use crate::{
    DockActionApplyError, DockActionOutcome, DockItemId, DockNodeId, DockSpaceId,
    interaction::DockRuntimeDragSession, workspace_transaction::DockWorkspaceDropPayload,
};
use open_gpui::{AnyWindowHandle, Pixels, Point, WindowBounds};
use std::collections::BTreeMap;

/// Drag payload carried by a viewport-routed drop release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DockViewportDropPayload {
    /// One tab item.
    Item(DockItemId),
    /// The entire source tabs stack.
    Tabs,
}

impl DockViewportDropPayload {
    pub(crate) fn as_workspace_payload(
        &self,
        source_tabs: DockNodeId,
    ) -> DockWorkspaceDropPayload<'_> {
        match self {
            DockViewportDropPayload::Item(item) => {
                DockWorkspaceDropPayload::Item { source_tabs, item }
            }
            DockViewportDropPayload::Tabs => DockWorkspaceDropPayload::Tabs { source_tabs },
        }
    }

    pub(crate) fn key(
        &self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
    ) -> DockViewportTearOffKey {
        match self {
            DockViewportDropPayload::Item(item) => DockViewportTearOffKey::Item(item.clone()),
            DockViewportDropPayload::Tabs => DockViewportTearOffKey::Tabs {
                source_space: source_space.clone(),
                source_tabs: source_tabs.as_u64(),
            },
        }
    }

    pub(crate) fn label(&self) -> String {
        match self {
            DockViewportDropPayload::Item(item) => item.as_str().to_string(),
            DockViewportDropPayload::Tabs => "tabs".to_string(),
        }
    }
}

/// Request to open a new platform viewport for a payload released outside known dock viewports.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportTearOffRequest {
    /// Source dock space containing the dragged payload.
    source_space: DockSpaceId,
    /// Source tabs node where the drag started.
    source_tabs: DockNodeId,
    /// Payload being torn off.
    payload: DockViewportDropPayload,
    /// Runtime drag session that produced this tear-off, when known.
    drag_session: Option<DockRuntimeDragSession>,
    /// Release position in screen coordinates.
    release_position: Point<Pixels>,
    /// Suggested platform window bounds for the new viewport, when known.
    suggested_window_bounds: Option<WindowBounds>,
}

impl DockViewportTearOffRequest {
    pub(crate) fn new(
        source_space: impl Into<DockSpaceId>,
        source_tabs: DockNodeId,
        payload: DockViewportDropPayload,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
    ) -> Self {
        Self {
            source_space: source_space.into(),
            source_tabs,
            payload,
            drag_session: None,
            release_position,
            suggested_window_bounds,
        }
    }

    pub(crate) fn with_drag_session(
        mut self,
        drag_session: Option<DockRuntimeDragSession>,
    ) -> Self {
        self.drag_session = drag_session;
        self
    }

    pub(crate) fn source_space(&self) -> &DockSpaceId {
        &self.source_space
    }

    pub(crate) fn source_tabs(&self) -> DockNodeId {
        self.source_tabs
    }

    pub(crate) fn payload(&self) -> &DockViewportDropPayload {
        &self.payload
    }

    pub(crate) fn drag_session(&self) -> Option<&DockRuntimeDragSession> {
        self.drag_session.as_ref()
    }

    pub(crate) fn release_position(&self) -> Point<Pixels> {
        self.release_position
    }

    pub(crate) fn suggested_window_bounds(&self) -> Option<WindowBounds> {
        self.suggested_window_bounds
    }

    pub(crate) fn key(&self) -> DockViewportTearOffKey {
        self.payload.key(&self.source_space, self.source_tabs)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct DockViewportTearOffTick(u64);

impl DockViewportTearOffTick {
    #[cfg(test)]
    pub const fn new(tick: u64) -> Self {
        Self(tick)
    }

    pub(crate) const fn as_u64(self) -> u64 {
        self.0
    }

    pub const fn saturating_add(self, ticks: u64) -> Self {
        Self(self.0.saturating_add(ticks))
    }

    fn age_since(self, earlier: Self) -> u64 {
        self.0.saturating_sub(earlier.0)
    }
}

/// Runtime state for a tear-off request that is waiting for a platform viewport.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportTearOffPending {
    /// Original release request that started the transaction.
    request: DockViewportTearOffRequest,
    /// Empty logical dock space that will receive the torn-off payload.
    target_space: DockSpaceId,
    /// Panel item that should receive GPUI focus after the tear-off completes.
    focus_item: Option<DockItemId>,
    requested_at: DockViewportTearOffTick,
    expires_after_ticks: u64,
}

impl DockViewportTearOffPending {
    pub(crate) fn request(&self) -> &DockViewportTearOffRequest {
        &self.request
    }

    pub(crate) fn target_space(&self) -> &DockSpaceId {
        &self.target_space
    }

    pub(crate) fn focus_item(&self) -> Option<&DockItemId> {
        self.focus_item.as_ref()
    }

    pub(crate) fn is_expired_at(&self, now: DockViewportTearOffTick) -> bool {
        now.age_since(self.requested_at) > self.expires_after_ticks
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockViewportTearOffBeginOutcome {
    Pending(DockViewportTearOffPending),
    Duplicate(DockViewportTearOffPending),
}

/// Reason a pending tear-off request was cancelled before commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportTearOffCancelReason {
    /// The caller explicitly cancelled the pending request.
    Cancelled,
    /// The pending request exceeded its logical time-to-live.
    Expired,
    /// The source payload no longer exists in the recorded source dock space.
    SourceMissing,
    /// The source payload still exists, but no longer belongs to the recorded source tabs node.
    SourceMoved,
}

/// Cancelled tear-off request and the reason it could not complete.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportTearOffCancelled {
    /// Pending request that was removed.
    pending: DockViewportTearOffPending,
    /// Reason the request was removed before commit.
    reason: DockViewportTearOffCancelReason,
}

impl DockViewportTearOffCancelled {
    pub(crate) fn new(
        pending: DockViewportTearOffPending,
        reason: DockViewportTearOffCancelReason,
    ) -> Self {
        Self { pending, reason }
    }

    pub(crate) fn pending(&self) -> &DockViewportTearOffPending {
        &self.pending
    }

    pub(crate) fn reason(&self) -> DockViewportTearOffCancelReason {
        self.reason
    }
}

/// Completed tear-off request after viewport registration and graph move commit.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportTearOffCompleted {
    /// Pending request that completed.
    pending: DockViewportTearOffPending,
    /// Runtime viewport registration outcome.
    registration: crate::DockViewportRegisterOutcome,
    /// Graph transaction outcome.
    action: crate::DockActionOutcome,
}

impl DockViewportTearOffCompleted {
    pub(crate) fn new(
        pending: DockViewportTearOffPending,
        registration: crate::DockViewportRegisterOutcome,
        action: crate::DockActionOutcome,
    ) -> Self {
        Self {
            pending,
            registration,
            action,
        }
    }

    pub(crate) fn pending(&self) -> &DockViewportTearOffPending {
        &self.pending
    }

    pub(crate) fn registration(&self) -> &crate::DockViewportRegisterOutcome {
        &self.registration
    }

    pub(crate) fn action(&self) -> crate::DockActionOutcome {
        self.action
    }
}

/// Tear-off request whose viewport opened but graph commit failed afterward.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportTearOffCommitFailure {
    /// Pending request that reached commit.
    pending: DockViewportTearOffPending,
    /// Runtime viewport registration outcome before cleanup.
    registration: crate::DockViewportRegisterOutcome,
    /// Commit error returned by the docking workspace.
    error: crate::DockActionApplyError,
}

impl DockViewportTearOffCommitFailure {
    pub(crate) fn new(
        pending: DockViewportTearOffPending,
        registration: crate::DockViewportRegisterOutcome,
        error: crate::DockActionApplyError,
    ) -> Self {
        Self {
            pending,
            registration,
            error,
        }
    }

    pub(crate) fn pending(&self) -> &DockViewportTearOffPending {
        &self.pending
    }

    pub(crate) fn error(&self) -> &crate::DockActionApplyError {
        &self.error
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockViewportTearOffCompletionOutcome {
    Completed(DockViewportTearOffCompleted),
    Cancelled(DockViewportTearOffCancelled),
    MissingPending { payload: DockViewportDropPayload },
    CommitFailed(DockViewportTearOffCommitFailure),
}

/// Outcome of opening a viewport for a tear-off request through the runtime.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockViewportTearOffOpenOutcome {
    /// The dragged payload already had a pending request, so no duplicate window was opened.
    Duplicate(DockViewportTearOffPending),
    /// Viewport registration and graph move both completed.
    Completed(DockViewportTearOffCompleted),
    /// The request was cancelled before graph mutation.
    Cancelled(DockViewportTearOffCancelled),
    /// The viewport registered, but the graph move failed and runtime mapping was cleaned up.
    CommitFailed(DockViewportTearOffCommitFailure),
}

/// Runtime outcome for a viewport-routed drop release.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockViewportDropRouteOutcome {
    /// The route resolved to a normal workspace action.
    Action(DockViewportDropActionOutcome),
    /// The route opened or reused a platform viewport through the tear-off runtime transaction.
    TearOff(DockViewportTearOffOpenOutcome),
}

/// Workspace action outcome plus viewport-side effects requested by a routed drop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportDropActionOutcome {
    /// Graph transaction outcome.
    action: DockActionOutcome,
    /// Runtime viewport that should become active after the drop, when known.
    activation: Option<DockViewportActivationTarget>,
}

impl DockViewportDropActionOutcome {
    pub(crate) fn new(
        action: DockActionOutcome,
        activation: Option<DockViewportActivationTarget>,
    ) -> Self {
        Self { action, activation }
    }

    pub(crate) fn action(&self) -> DockActionOutcome {
        self.action
    }

    pub(crate) fn activation(&self) -> Option<&DockViewportActivationTarget> {
        self.activation.as_ref()
    }
}

/// Runtime viewport activation target selected by a successful drop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportActivationTarget {
    /// Logical dock space to activate.
    space: DockSpaceId,
    /// GPUI window rendering the logical dock space.
    window: AnyWindowHandle,
    /// Active panel item that should receive focus after the window is active.
    focus_item: Option<DockItemId>,
}

impl DockViewportActivationTarget {
    pub(crate) fn new(
        space: impl Into<DockSpaceId>,
        window: impl Into<AnyWindowHandle>,
        focus_item: Option<DockItemId>,
    ) -> Self {
        Self {
            space: space.into(),
            window: window.into(),
            focus_item,
        }
    }

    pub(crate) fn space(&self) -> &DockSpaceId {
        &self.space
    }

    pub(crate) fn window(&self) -> AnyWindowHandle {
        self.window
    }

    pub(crate) fn focus_item(&self) -> Option<&DockItemId> {
        self.focus_item.as_ref()
    }
}

impl DockViewportDropRouteOutcome {
    /// Returns the runtime viewport that should be activated after the drop, when known.
    pub(crate) fn activation_target(&self) -> Option<DockViewportActivationTarget> {
        match self {
            DockViewportDropRouteOutcome::Action(outcome) => outcome.activation().cloned(),
            DockViewportDropRouteOutcome::TearOff(DockViewportTearOffOpenOutcome::Completed(
                completed,
            )) => Some(DockViewportActivationTarget::new(
                completed.pending().target_space().clone(),
                completed.registration().window(),
                completed.pending().focus_item().cloned(),
            )),
            DockViewportDropRouteOutcome::TearOff(
                DockViewportTearOffOpenOutcome::Duplicate(_)
                | DockViewportTearOffOpenOutcome::Cancelled(_)
                | DockViewportTearOffOpenOutcome::CommitFailed(_),
            ) => None,
        }
    }

    pub(crate) fn action_result(&self) -> Result<DockActionOutcome, DockActionApplyError> {
        match self {
            DockViewportDropRouteOutcome::Action(outcome) => Ok(outcome.action()),
            DockViewportDropRouteOutcome::TearOff(DockViewportTearOffOpenOutcome::Completed(
                completed,
            )) => Ok(completed.action()),
            DockViewportDropRouteOutcome::TearOff(DockViewportTearOffOpenOutcome::Duplicate(_))
            | DockViewportDropRouteOutcome::TearOff(DockViewportTearOffOpenOutcome::Cancelled(_)) => {
                Ok(DockActionOutcome::Unchanged)
            }
            DockViewportDropRouteOutcome::TearOff(
                DockViewportTearOffOpenOutcome::CommitFailed(failure),
            ) => Err(failure.error().clone()),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DockViewportTearOffMachine {
    pending_by_key: BTreeMap<DockViewportTearOffKey, DockViewportTearOffPending>,
    ttl_ticks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DockViewportTearOffKey {
    Item(DockItemId),
    Tabs {
        source_space: DockSpaceId,
        source_tabs: u64,
    },
}

impl DockViewportTearOffKey {
    pub(crate) fn payload(&self) -> DockViewportDropPayload {
        match self {
            DockViewportTearOffKey::Item(item) => DockViewportDropPayload::Item(item.clone()),
            DockViewportTearOffKey::Tabs { .. } => DockViewportDropPayload::Tabs,
        }
    }
}

impl Default for DockViewportTearOffMachine {
    fn default() -> Self {
        Self {
            pending_by_key: BTreeMap::new(),
            ttl_ticks: 600,
        }
    }
}

impl DockViewportTearOffMachine {
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.pending_by_key.len()
    }

    pub(crate) fn pending(
        &self,
        key: &DockViewportTearOffKey,
    ) -> Option<&DockViewportTearOffPending> {
        self.pending_by_key.get(key)
    }

    pub(crate) fn begin(
        &mut self,
        request: DockViewportTearOffRequest,
        target_space: DockSpaceId,
        focus_item: Option<DockItemId>,
        now: DockViewportTearOffTick,
    ) -> DockViewportTearOffBeginOutcome {
        self.expire(now);
        let key = request.key();
        if let Some(pending) = self.pending_by_key.get(&key) {
            return DockViewportTearOffBeginOutcome::Duplicate(pending.clone());
        }

        let pending = DockViewportTearOffPending {
            request,
            target_space,
            focus_item,
            requested_at: now,
            expires_after_ticks: self.ttl_ticks,
        };
        self.pending_by_key.insert(key, pending.clone());
        DockViewportTearOffBeginOutcome::Pending(pending)
    }

    pub(crate) fn cancel(
        &mut self,
        key: &DockViewportTearOffKey,
        reason: DockViewportTearOffCancelReason,
    ) -> Option<DockViewportTearOffCancelled> {
        self.pending_by_key
            .remove(key)
            .map(|pending| DockViewportTearOffCancelled::new(pending, reason))
    }

    pub(crate) fn expire(
        &mut self,
        now: DockViewportTearOffTick,
    ) -> Vec<DockViewportTearOffCancelled> {
        let expired = self
            .pending_by_key
            .iter()
            .filter_map(|(key, pending)| pending.is_expired_at(now).then_some(key.clone()))
            .collect::<Vec<_>>();

        expired
            .iter()
            .filter_map(|key| self.cancel(key, DockViewportTearOffCancelReason::Expired))
            .collect()
    }

    pub(crate) fn take_for_completion(
        &mut self,
        key: &DockViewportTearOffKey,
        now: DockViewportTearOffTick,
    ) -> DockViewportTearOffCompletionPending {
        let Some(pending) = self.pending_by_key.get(key) else {
            return DockViewportTearOffCompletionPending::Missing;
        };
        if pending.is_expired_at(now) {
            let cancelled = self
                .cancel(key, DockViewportTearOffCancelReason::Expired)
                .expect("pending payload should still be present");
            return DockViewportTearOffCompletionPending::Cancelled(cancelled);
        }

        DockViewportTearOffCompletionPending::Pending(
            self.pending_by_key
                .remove(key)
                .expect("pending payload should still be present"),
        )
    }
}

pub(crate) enum DockViewportTearOffCompletionPending {
    Pending(DockViewportTearOffPending),
    Cancelled(DockViewportTearOffCancelled),
    Missing,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport_test_support::{item, space};
    use open_gpui::{point, px};
    use slotmap::Key;

    #[test]
    fn tear_off_machine_deduplicates_pending_items() {
        let mut machine = DockViewportTearOffMachine::default();
        let request = DockViewportTearOffRequest::new(
            space("main"),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(900.0), px(900.0)),
            None,
        );

        let first = machine.begin(
            request.clone(),
            space("detached"),
            None,
            DockViewportTearOffTick::new(1),
        );
        let second = machine.begin(
            request,
            space("other"),
            None,
            DockViewportTearOffTick::new(2),
        );

        assert!(matches!(first, DockViewportTearOffBeginOutcome::Pending(_)));
        let DockViewportTearOffBeginOutcome::Duplicate(existing) = second else {
            panic!("second begin should be idempotent");
        };
        assert_eq!(existing.target_space(), &space("detached"));
        assert_eq!(machine.len(), 1);
    }

    #[test]
    fn tear_off_machine_deduplicates_pending_tab_stacks() {
        let mut machine = DockViewportTearOffMachine::default();
        let source = space("main");
        let request = DockViewportTearOffRequest::new(
            source.clone(),
            DockNodeId::null(),
            DockViewportDropPayload::Tabs,
            point(px(900.0), px(900.0)),
            None,
        );

        let first = machine.begin(
            request.clone(),
            space("detached"),
            None,
            DockViewportTearOffTick::new(1),
        );
        let second = machine.begin(
            request,
            space("other"),
            None,
            DockViewportTearOffTick::new(2),
        );

        assert!(matches!(first, DockViewportTearOffBeginOutcome::Pending(_)));
        let DockViewportTearOffBeginOutcome::Duplicate(existing) = second else {
            panic!("second stack begin should be idempotent");
        };
        assert_eq!(existing.request.source_space(), &source);
        assert_eq!(existing.target_space(), &space("detached"));
        assert_eq!(machine.len(), 1);
    }

    #[test]
    fn tear_off_machine_expires_stale_pending_requests() {
        let mut machine = DockViewportTearOffMachine::default();
        let request = DockViewportTearOffRequest::new(
            space("main"),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(900.0), px(900.0)),
            None,
        );

        machine.begin(
            request,
            space("detached"),
            None,
            DockViewportTearOffTick::new(1),
        );
        assert!(machine.expire(DockViewportTearOffTick::new(601)).is_empty());
        let expired = machine.expire(DockViewportTearOffTick::new(602));

        assert_eq!(expired.len(), 1);
        assert_eq!(
            expired[0].reason(),
            DockViewportTearOffCancelReason::Expired
        );
        assert_eq!(machine.len(), 0);
    }
}
