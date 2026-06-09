use crate::{
    DockItemId, DockNodeId, DockPolicy, DockPolicyError, DockSpaceId, DockViewportAdapter,
    DockViewportHit, DockViewportTargetContext,
};
use open_gpui::{Pixels, Point, WindowBounds};
use std::collections::BTreeMap;

/// Request to open a new platform viewport for a tab released outside known dock viewports.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportTearOffRequest {
    /// Source dock space containing the dragged item.
    pub source_space: DockSpaceId,
    /// Source tabs node where the drag started.
    pub source_tabs: DockNodeId,
    /// Item being torn off.
    pub item: DockItemId,
    /// Release position in screen coordinates.
    pub release_position: Point<Pixels>,
    /// Suggested platform window bounds for the new viewport, when known.
    pub suggested_window_bounds: Option<WindowBounds>,
}

/// Result of resolving a tab release against registered platform viewports.
#[derive(Debug, Clone, PartialEq)]
pub enum DockViewportTearOffOutcome {
    /// The release landed inside a known viewport; normal drop handling should continue.
    KnownViewport(DockViewportHit),
    /// The release can open a new platform viewport.
    Requested(DockViewportTearOffRequest),
    /// The request was rejected by docking policy.
    Rejected(DockPolicyError),
}

/// Logical clock value used by the viewport runtime to expire stale tear-off requests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DockViewportTearOffTick(u64);

impl DockViewportTearOffTick {
    /// Creates a logical tear-off clock value.
    pub const fn new(tick: u64) -> Self {
        Self(tick)
    }

    /// Returns the underlying logical tick value.
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Returns a tick advanced by `ticks`, saturating at `u64::MAX`.
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
    /// Empty logical dock space that will receive the torn-off item.
    pub target_space: DockSpaceId,
    /// Logical tick when the request was recorded.
    pub requested_at: DockViewportTearOffTick,
    /// Number of logical ticks after which the pending request is considered stale.
    pub expires_after_ticks: u64,
}

impl DockViewportTearOffPending {
    /// Returns true when this pending request is stale at `now`.
    pub fn is_expired_at(&self, now: DockViewportTearOffTick) -> bool {
        now.age_since(self.requested_at) > self.expires_after_ticks
    }
}

/// Outcome of recording a tear-off request.
#[derive(Debug, Clone, PartialEq)]
pub enum DockViewportTearOffBeginOutcome {
    /// A new pending request was recorded.
    Pending(DockViewportTearOffPending),
    /// The dragged item already has a pending request, so no duplicate request was created.
    Duplicate(DockViewportTearOffPending),
}

/// Reason a pending tear-off request was cancelled before commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportTearOffCancelReason {
    /// The caller explicitly cancelled the pending request.
    Cancelled,
    /// The pending request exceeded its logical time-to-live.
    Expired,
    /// The source item no longer exists in the recorded source dock space.
    SourceMissing,
    /// The source item still exists, but no longer belongs to the recorded source tabs node.
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

/// Outcome of completing a pending tear-off request with an opened viewport window.
#[derive(Debug, Clone, PartialEq)]
pub enum DockViewportTearOffCompletionOutcome {
    /// Viewport registration and graph move both completed.
    Completed(DockViewportTearOffCompleted),
    /// The pending request was cancelled before graph mutation.
    Cancelled(DockViewportTearOffCancelled),
    /// No pending request existed for the item.
    MissingPending {
        /// Item whose pending request was requested.
        item: DockItemId,
    },
    /// The viewport registered, but the graph move failed and runtime mapping was cleaned up.
    CommitFailed(DockViewportTearOffCommitFailure),
}

/// Outcome of opening a viewport for a tear-off request through the runtime.
#[derive(Debug, Clone, PartialEq)]
pub enum DockViewportTearOffOpenOutcome {
    /// The dragged item already had a pending request, so no duplicate window was opened.
    Duplicate(DockViewportTearOffPending),
    /// Viewport registration and graph move both completed.
    Completed(DockViewportTearOffCompleted),
    /// The request was cancelled before graph mutation.
    Cancelled(DockViewportTearOffCancelled),
    /// The viewport registered, but the graph move failed and runtime mapping was cleaned up.
    CommitFailed(DockViewportTearOffCommitFailure),
}

#[derive(Debug, Clone)]
pub(crate) struct DockViewportTearOffMachine {
    pending_by_item: BTreeMap<DockItemId, DockViewportTearOffPending>,
    ttl_ticks: u64,
}

impl Default for DockViewportTearOffMachine {
    fn default() -> Self {
        Self {
            pending_by_item: BTreeMap::new(),
            ttl_ticks: 600,
        }
    }
}

impl DockViewportTearOffMachine {
    pub(crate) fn len(&self) -> usize {
        self.pending_by_item.len()
    }

    pub(crate) fn pending(&self, item: &DockItemId) -> Option<&DockViewportTearOffPending> {
        self.pending_by_item.get(item)
    }

    pub(crate) fn pending_items(&self) -> Vec<DockItemId> {
        self.pending_by_item.keys().cloned().collect()
    }

    pub(crate) fn begin(
        &mut self,
        request: DockViewportTearOffRequest,
        target_space: DockSpaceId,
        now: DockViewportTearOffTick,
    ) -> DockViewportTearOffBeginOutcome {
        self.expire(now);
        if let Some(pending) = self.pending_by_item.get(&request.item) {
            return DockViewportTearOffBeginOutcome::Duplicate(pending.clone());
        }

        let pending = DockViewportTearOffPending {
            request,
            target_space,
            requested_at: now,
            expires_after_ticks: self.ttl_ticks,
        };
        self.pending_by_item
            .insert(pending.request.item.clone(), pending.clone());
        DockViewportTearOffBeginOutcome::Pending(pending)
    }

    pub(crate) fn cancel(
        &mut self,
        item: &DockItemId,
        reason: DockViewportTearOffCancelReason,
    ) -> Option<DockViewportTearOffCancelled> {
        self.pending_by_item
            .remove(item)
            .map(|pending| DockViewportTearOffCancelled { pending, reason })
    }

    pub(crate) fn expire(
        &mut self,
        now: DockViewportTearOffTick,
    ) -> Vec<DockViewportTearOffCancelled> {
        let expired = self
            .pending_by_item
            .iter()
            .filter_map(|(item, pending)| pending.is_expired_at(now).then_some(item.clone()))
            .collect::<Vec<_>>();

        expired
            .iter()
            .filter_map(|item| self.cancel(item, DockViewportTearOffCancelReason::Expired))
            .collect()
    }

    pub(crate) fn take_for_completion(
        &mut self,
        item: &DockItemId,
        now: DockViewportTearOffTick,
    ) -> DockViewportTearOffCompletionPending {
        let Some(pending) = self.pending_by_item.get(item) else {
            return DockViewportTearOffCompletionPending::Missing;
        };
        if pending.is_expired_at(now) {
            let cancelled = self
                .cancel(item, DockViewportTearOffCancelReason::Expired)
                .expect("pending item should still be present");
            return DockViewportTearOffCompletionPending::Cancelled(cancelled);
        }

        DockViewportTearOffCompletionPending::Pending(
            self.pending_by_item
                .remove(item)
                .expect("pending item should still be present"),
        )
    }
}

pub(crate) enum DockViewportTearOffCompletionPending {
    Pending(DockViewportTearOffPending),
    Cancelled(DockViewportTearOffCancelled),
    Missing,
}

impl DockViewportAdapter {
    /// Resolves a tab release using explicit viewport target arbitration inputs.
    ///
    /// This method never mutates the docking graph. Callers should open/register a destination
    /// viewport first, then commit a move action after runtime setup succeeds.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_tear_off_request_with_context(
        &self,
        source_space: impl Into<DockSpaceId>,
        source_tabs: DockNodeId,
        item: impl Into<DockItemId>,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        policy: &DockPolicy,
        target_context: &DockViewportTargetContext,
    ) -> DockViewportTearOffOutcome {
        if let Some(hit) = self.hit_test_screen_with_context(release_position, target_context) {
            return DockViewportTearOffOutcome::KnownViewport(hit);
        }

        if let Err(reason) = policy.validate_platform_viewports() {
            return DockViewportTearOffOutcome::Rejected(reason);
        }

        DockViewportTearOffOutcome::Requested(DockViewportTearOffRequest {
            source_space: source_space.into(),
            source_tabs,
            item: item.into(),
            release_position,
            suggested_window_bounds,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockViewportAdapter,
        viewport_test_support::{bounds, handle, item, space},
    };
    use open_gpui::{DisplayId, point, px};
    use slotmap::Key;

    #[test]
    fn tear_off_release_inside_known_viewport_returns_hit() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        adapter.register_viewport(main.clone(), handle(1));
        adapter.update_snapshot(
            &main,
            Some(DisplayId::new(7)),
            WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0)),
            bounds(10.0, 20.0, 300.0, 200.0),
        );

        assert_eq!(
            adapter.resolve_tear_off_request_with_context(
                main.clone(),
                DockNodeId::null(),
                item("a"),
                point(px(115.0), px(225.0)),
                None,
                &DockPolicy::default(),
                &DockViewportTargetContext::new(),
            ),
            DockViewportTearOffOutcome::KnownViewport(DockViewportHit {
                space: main,
                host_position: point(px(5.0), px(5.0)),
            })
        );
    }

    #[test]
    fn tear_off_release_outside_viewports_respects_platform_policy() {
        let adapter = DockViewportAdapter::new();
        let main = space("main");

        assert_eq!(
            adapter.resolve_tear_off_request_with_context(
                main,
                DockNodeId::null(),
                item("a"),
                point(px(900.0), px(900.0)),
                None,
                &DockPolicy::default(),
                &DockViewportTargetContext::new(),
            ),
            DockViewportTearOffOutcome::Rejected(DockPolicyError::PlatformViewportsDisabled)
        );
    }

    #[test]
    fn tear_off_release_outside_viewports_emits_request_when_enabled() {
        let adapter = DockViewportAdapter::new();
        let main = space("main");
        let item = item("a");
        let release_position = point(px(900.0), px(900.0));
        let suggested_window_bounds = WindowBounds::Windowed(bounds(880.0, 880.0, 360.0, 240.0));
        let mut policy = DockPolicy::default();
        policy.set_allow_platform_viewports(true);

        assert_eq!(
            adapter.resolve_tear_off_request_with_context(
                main.clone(),
                DockNodeId::null(),
                item.clone(),
                release_position,
                Some(suggested_window_bounds),
                &policy,
                &DockViewportTargetContext::new(),
            ),
            DockViewportTearOffOutcome::Requested(DockViewportTearOffRequest {
                source_space: main,
                source_tabs: DockNodeId::null(),
                item,
                release_position,
                suggested_window_bounds: Some(suggested_window_bounds),
            })
        );
    }

    #[test]
    fn stale_viewport_bounds_do_not_block_tear_off_request() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        adapter.register_viewport(main.clone(), handle(1));
        let mut policy = DockPolicy::default();
        policy.set_allow_platform_viewports(true);

        assert!(matches!(
            adapter.resolve_tear_off_request_with_context(
                main,
                DockNodeId::null(),
                item("a"),
                point(px(115.0), px(225.0)),
                None,
                &policy,
                &DockViewportTargetContext::new(),
            ),
            DockViewportTearOffOutcome::Requested(_)
        ));
    }

    #[test]
    fn tear_off_machine_deduplicates_pending_items() {
        let mut machine = DockViewportTearOffMachine::default();
        let request = DockViewportTearOffRequest {
            source_space: space("main"),
            source_tabs: DockNodeId::null(),
            item: item("a"),
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
    fn tear_off_machine_expires_stale_pending_requests() {
        let mut machine = DockViewportTearOffMachine::default();
        let request = DockViewportTearOffRequest {
            source_space: space("main"),
            source_tabs: DockNodeId::null(),
            item: item("a"),
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
