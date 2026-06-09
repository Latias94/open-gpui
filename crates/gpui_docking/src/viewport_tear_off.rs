use crate::{
    DockActionApplyError, DockActionOutcome, DockItemId, DockNodeId, DockSpaceId,
    workspace_transaction::DockWorkspaceDropPayload,
};
use open_gpui::{AnyWindowHandle, Pixels, Point, WindowBounds};
use std::collections::BTreeMap;

/// Drag payload carried by a viewport-routed drop release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DockViewportDropPayload {
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
pub struct DockViewportTearOffRequest {
    /// Source dock space containing the dragged payload.
    pub source_space: DockSpaceId,
    /// Source tabs node where the drag started.
    pub source_tabs: DockNodeId,
    /// Payload being torn off.
    pub payload: DockViewportDropPayload,
    /// Release position in screen coordinates.
    pub release_position: Point<Pixels>,
    /// Suggested platform window bounds for the new viewport, when known.
    pub suggested_window_bounds: Option<WindowBounds>,
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
pub struct DockViewportTearOffPending {
    /// Original release request that started the transaction.
    pub request: DockViewportTearOffRequest,
    /// Empty logical dock space that will receive the torn-off payload.
    pub target_space: DockSpaceId,
    requested_at: DockViewportTearOffTick,
    expires_after_ticks: u64,
}

impl DockViewportTearOffPending {
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
pub struct DockViewportTearOffCancelled {
    /// Pending request that was removed.
    pub pending: DockViewportTearOffPending,
    /// Reason the request was removed before commit.
    pub reason: DockViewportTearOffCancelReason,
}

/// Completed tear-off request after viewport registration and graph move commit.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportTearOffCompleted {
    /// Pending request that completed.
    pub pending: DockViewportTearOffPending,
    /// Runtime viewport registration outcome.
    pub registration: crate::DockViewportRegisterOutcome,
    /// Graph transaction outcome.
    pub action: crate::DockActionOutcome,
}

/// Tear-off request whose viewport opened but graph commit failed afterward.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportTearOffCommitFailure {
    /// Pending request that reached commit.
    pub pending: DockViewportTearOffPending,
    /// Runtime viewport registration outcome before cleanup.
    pub registration: crate::DockViewportRegisterOutcome,
    /// Commit error returned by the docking workspace.
    pub error: crate::DockActionApplyError,
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
pub enum DockViewportTearOffOpenOutcome {
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
pub enum DockViewportDropRouteOutcome {
    /// The route resolved to a normal workspace action.
    Action(DockViewportDropActionOutcome),
    /// The route opened or reused a platform viewport through the tear-off runtime transaction.
    TearOff(DockViewportTearOffOpenOutcome),
}

/// Workspace action outcome plus viewport-side effects requested by a routed drop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockViewportDropActionOutcome {
    /// Graph transaction outcome.
    pub action: DockActionOutcome,
    /// Runtime viewport that should become active after the drop, when known.
    pub activation: Option<DockViewportActivationTarget>,
}

/// Runtime viewport activation target selected by a successful drop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockViewportActivationTarget {
    /// Logical dock space to activate.
    pub space: DockSpaceId,
    /// GPUI window rendering the logical dock space.
    pub window: AnyWindowHandle,
}

impl DockViewportDropRouteOutcome {
    /// Returns the runtime viewport that should be activated after the drop, when known.
    pub fn activation_target(&self) -> Option<DockViewportActivationTarget> {
        match self {
            DockViewportDropRouteOutcome::Action(outcome) => outcome.activation.clone(),
            DockViewportDropRouteOutcome::TearOff(DockViewportTearOffOpenOutcome::Completed(
                completed,
            )) => Some(DockViewportActivationTarget {
                space: completed.pending.target_space.clone(),
                window: completed.registration.window,
            }),
            DockViewportDropRouteOutcome::TearOff(
                DockViewportTearOffOpenOutcome::Duplicate(_)
                | DockViewportTearOffOpenOutcome::Cancelled(_)
                | DockViewportTearOffOpenOutcome::CommitFailed(_),
            ) => None,
        }
    }

    pub(crate) fn action_result(&self) -> Result<DockActionOutcome, DockActionApplyError> {
        match self {
            DockViewportDropRouteOutcome::Action(outcome) => Ok(outcome.action),
            DockViewportDropRouteOutcome::TearOff(DockViewportTearOffOpenOutcome::Completed(
                completed,
            )) => Ok(completed.action),
            DockViewportDropRouteOutcome::TearOff(DockViewportTearOffOpenOutcome::Duplicate(_))
            | DockViewportDropRouteOutcome::TearOff(DockViewportTearOffOpenOutcome::Cancelled(_)) => {
                Ok(DockActionOutcome::Unchanged)
            }
            DockViewportDropRouteOutcome::TearOff(
                DockViewportTearOffOpenOutcome::CommitFailed(failure),
            ) => Err(failure.error.clone()),
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
        now: DockViewportTearOffTick,
    ) -> DockViewportTearOffBeginOutcome {
        self.expire(now);
        let key = request
            .payload
            .key(&request.source_space, request.source_tabs);
        if let Some(pending) = self.pending_by_key.get(&key) {
            return DockViewportTearOffBeginOutcome::Duplicate(pending.clone());
        }

        let pending = DockViewportTearOffPending {
            request,
            target_space,
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
            .map(|pending| DockViewportTearOffCancelled { pending, reason })
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
        let request = DockViewportTearOffRequest {
            source_space: space("main"),
            source_tabs: DockNodeId::null(),
            payload: DockViewportDropPayload::Item(item("a")),
            release_position: point(px(900.0), px(900.0)),
            suggested_window_bounds: None,
        };

        let first = machine.begin(
            request.clone(),
            space("detached"),
            DockViewportTearOffTick::new(1),
        );
        let second = machine.begin(request, space("other"), DockViewportTearOffTick::new(2));

        assert!(matches!(first, DockViewportTearOffBeginOutcome::Pending(_)));
        let DockViewportTearOffBeginOutcome::Duplicate(existing) = second else {
            panic!("second begin should be idempotent");
        };
        assert_eq!(existing.target_space, space("detached"));
        assert_eq!(machine.len(), 1);
    }

    #[test]
    fn tear_off_machine_deduplicates_pending_tab_stacks() {
        let mut machine = DockViewportTearOffMachine::default();
        let source = space("main");
        let request = DockViewportTearOffRequest {
            source_space: source.clone(),
            source_tabs: DockNodeId::null(),
            payload: DockViewportDropPayload::Tabs,
            release_position: point(px(900.0), px(900.0)),
            suggested_window_bounds: None,
        };

        let first = machine.begin(
            request.clone(),
            space("detached"),
            DockViewportTearOffTick::new(1),
        );
        let second = machine.begin(request, space("other"), DockViewportTearOffTick::new(2));

        assert!(matches!(first, DockViewportTearOffBeginOutcome::Pending(_)));
        let DockViewportTearOffBeginOutcome::Duplicate(existing) = second else {
            panic!("second stack begin should be idempotent");
        };
        assert_eq!(existing.request.source_space, source);
        assert_eq!(existing.target_space, space("detached"));
        assert_eq!(machine.len(), 1);
    }

    #[test]
    fn tear_off_machine_expires_stale_pending_requests() {
        let mut machine = DockViewportTearOffMachine::default();
        let request = DockViewportTearOffRequest {
            source_space: space("main"),
            source_tabs: DockNodeId::null(),
            payload: DockViewportDropPayload::Item(item("a")),
            release_position: point(px(900.0), px(900.0)),
            suggested_window_bounds: None,
        };

        machine.begin(request, space("detached"), DockViewportTearOffTick::new(1));
        assert!(machine.expire(DockViewportTearOffTick::new(601)).is_empty());
        let expired = machine.expire(DockViewportTearOffTick::new(602));

        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].reason, DockViewportTearOffCancelReason::Expired);
        assert_eq!(machine.len(), 0);
    }
}
