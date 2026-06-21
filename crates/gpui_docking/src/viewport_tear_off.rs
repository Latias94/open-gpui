use crate::{
    DockActionApplyError, DockActionOutcome, DockGraph, DockItemId, DockNodeId, DockSpaceId,
    DockViewportActivationTransaction, DockViewportFocusRequest,
    drag::{DockDragPayload, DockDragPayloadKind, DockDragTearOffGeometry},
    interaction::DockRuntimeDragSession,
    workspace_transaction::DockWorkspaceDropPayload,
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
    /// An in-window floating subtree.
    Floating(DockNodeId),
}

impl DockViewportDropPayload {
    pub(crate) fn from_drag_payload(payload: &DockDragPayload) -> Self {
        match &payload.kind {
            DockDragPayloadKind::Item { item } => Self::Item(item.clone()),
            DockDragPayloadKind::Tabs => Self::Tabs,
            DockDragPayloadKind::Floating { floating } => Self::Floating(*floating),
        }
    }

    pub(crate) fn as_workspace_payload(
        &self,
        source_node: DockNodeId,
    ) -> DockWorkspaceDropPayload<'_> {
        match self {
            DockViewportDropPayload::Item(item) => DockWorkspaceDropPayload::Item {
                source_tabs: source_node,
                item,
            },
            DockViewportDropPayload::Tabs => DockWorkspaceDropPayload::Tabs {
                source_tabs: source_node,
            },
            DockViewportDropPayload::Floating(floating) => DockWorkspaceDropPayload::Floating {
                floating: *floating,
            },
        }
    }

    pub(crate) fn excluded_nodes_for_drop_scene(
        &self,
        graph: &DockGraph,
        source_node: DockNodeId,
    ) -> Vec<DockNodeId> {
        let source_node = match self {
            DockViewportDropPayload::Item(_) => return Vec::new(),
            DockViewportDropPayload::Tabs => source_node,
            DockViewportDropPayload::Floating(floating) => *floating,
        };
        let nodes = graph.nodes_in_subtree(source_node);
        if nodes.is_empty() {
            vec![source_node]
        } else {
            nodes
        }
    }

    pub(crate) fn key(
        &self,
        source_space: &DockSpaceId,
        source_node: DockNodeId,
    ) -> DockViewportTearOffKey {
        match self {
            DockViewportDropPayload::Item(item) => DockViewportTearOffKey::Item(item.clone()),
            DockViewportDropPayload::Tabs => DockViewportTearOffKey::Tabs {
                source_space: source_space.clone(),
                source_tabs: source_node.as_u64(),
            },
            DockViewportDropPayload::Floating(floating) => DockViewportTearOffKey::Floating {
                source_space: source_space.clone(),
                floating: *floating,
            },
        }
    }

    pub(crate) fn label(&self) -> String {
        match self {
            DockViewportDropPayload::Item(item) => item.as_str().to_string(),
            DockViewportDropPayload::Tabs => "tabs".to_string(),
            DockViewportDropPayload::Floating(_) => "floating".to_string(),
        }
    }
}

/// Request to open a new platform viewport for a payload released outside known dock viewports.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportTearOffRequest {
    /// Source dock space containing the dragged payload.
    source_space: DockSpaceId,
    /// Source graph node that owns the dragged payload.
    source_node: DockNodeId,
    /// Payload being torn off.
    payload: DockViewportDropPayload,
    /// Runtime drag session that produced this tear-off, when known.
    drag_session: Option<DockRuntimeDragSession>,
    /// Release position, if it is authoritative in screen coordinates.
    release_position: Option<Point<Pixels>>,
    /// Geometry captured from the drag source, when the source published it.
    tear_off_geometry: Option<DockDragTearOffGeometry>,
    /// Suggested platform window bounds for the new viewport, when known.
    suggested_window_bounds: Option<WindowBounds>,
}

impl DockViewportTearOffRequest {
    pub(crate) fn new(
        source_space: impl Into<DockSpaceId>,
        source_node: DockNodeId,
        payload: DockViewportDropPayload,
        release_position: impl Into<Option<Point<Pixels>>>,
        suggested_window_bounds: Option<WindowBounds>,
    ) -> Self {
        Self {
            source_space: source_space.into(),
            source_node,
            payload,
            drag_session: None,
            release_position: release_position.into(),
            tear_off_geometry: None,
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

    pub(crate) fn with_tear_off_geometry(
        mut self,
        tear_off_geometry: Option<DockDragTearOffGeometry>,
    ) -> Self {
        self.tear_off_geometry = tear_off_geometry;
        self
    }

    pub(crate) fn source_space(&self) -> &DockSpaceId {
        &self.source_space
    }

    pub(crate) fn source_node(&self) -> DockNodeId {
        self.source_node
    }

    pub(crate) fn payload(&self) -> &DockViewportDropPayload {
        &self.payload
    }

    pub(crate) fn drag_session(&self) -> Option<&DockRuntimeDragSession> {
        self.drag_session.as_ref()
    }

    pub(crate) fn release_position(&self) -> Option<Point<Pixels>> {
        self.release_position
    }

    pub(crate) fn tear_off_geometry(&self) -> Option<DockDragTearOffGeometry> {
        self.tear_off_geometry
    }

    pub(crate) fn suggested_window_bounds(&self) -> Option<WindowBounds> {
        self.suggested_window_bounds
    }

    pub(crate) fn key(&self) -> DockViewportTearOffKey {
        self.payload.key(&self.source_space, self.source_node)
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
    /// The recorded source payload can no longer be committed.
    SourceUnavailable,
}

/// Cancelled tear-off request and the reason it could not complete.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportTearOffCancelled {
    /// Reason the request was removed before commit.
    reason: DockViewportTearOffCancelReason,
}

impl DockViewportTearOffCancelled {
    pub(crate) fn new(reason: DockViewportTearOffCancelReason) -> Self {
        Self { reason }
    }

    pub(crate) fn reason(&self) -> DockViewportTearOffCancelReason {
        self.reason
    }
}

/// Completed graph-first tear-off request after platform viewport registration.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportTearOffCompleted {
    /// Pending request that completed.
    pending: DockViewportTearOffPending,
    /// Runtime viewport registration outcome.
    registration: crate::DockViewportRegisterOutcome,
    /// Runtime-owned windows superseded while completing this tear-off.
    replaced_windows: Vec<AnyWindowHandle>,
    /// Graph transaction outcome that committed before the platform window was opened.
    action: crate::DockActionOutcome,
}

impl DockViewportTearOffCompleted {
    pub(crate) fn new(
        pending: DockViewportTearOffPending,
        registration: crate::DockViewportRegisterOutcome,
        replaced_windows: Vec<AnyWindowHandle>,
        action: crate::DockActionOutcome,
    ) -> Self {
        Self {
            pending,
            registration,
            replaced_windows,
            action,
        }
    }

    pub(crate) fn pending(&self) -> &DockViewportTearOffPending {
        &self.pending
    }

    pub(crate) fn registration(&self) -> &crate::DockViewportRegisterOutcome {
        &self.registration
    }

    pub(crate) fn replaced_windows(&self) -> &[AnyWindowHandle] {
        &self.replaced_windows
    }

    pub(crate) fn action(&self) -> crate::DockActionOutcome {
        self.action
    }
}

/// Outcome of opening a platform viewport for an already validated tear-off request.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockViewportTearOffOpenOutcome {
    /// The dragged payload already had a pending request, so no duplicate window was opened.
    Duplicate(DockViewportTearOffPending),
    /// Graph move and viewport registration both completed.
    Completed(DockViewportTearOffCompleted),
}

/// Runtime outcome for a viewport-routed drop release.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockViewportDropRouteOutcome {
    /// The route resolved to a normal workspace action.
    Action(DockViewportDropActionOutcome),
    /// The route opened or reused a platform viewport through the tear-off runtime transaction.
    TearOff(Box<DockViewportTearOffOpenOutcome>),
}

/// Workspace action outcome plus viewport-side effects requested by a routed drop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportDropActionOutcome {
    /// Graph transaction outcome.
    action: DockActionOutcome,
    /// Runtime activation transaction to apply after the drop, when known.
    activation: Option<DockViewportActivationTransaction>,
}

impl DockViewportDropActionOutcome {
    pub(crate) fn new(
        action: DockActionOutcome,
        activation: Option<DockViewportActivationTransaction>,
    ) -> Self {
        Self { action, activation }
    }

    pub(crate) fn action(&self) -> DockActionOutcome {
        self.action
    }

    pub(crate) fn activation(&self) -> Option<&DockViewportActivationTransaction> {
        self.activation.as_ref()
    }
}

impl DockViewportDropRouteOutcome {
    pub(crate) fn tear_off(outcome: DockViewportTearOffOpenOutcome) -> Self {
        Self::TearOff(Box::new(outcome))
    }

    /// Returns the runtime activation transaction that should be applied after the drop, when known.
    pub(crate) fn activation_transaction(&self) -> Option<DockViewportActivationTransaction> {
        match self {
            DockViewportDropRouteOutcome::Action(outcome) => outcome.activation().cloned(),
            DockViewportDropRouteOutcome::TearOff(outcome) => match outcome.as_ref() {
                DockViewportTearOffOpenOutcome::Completed(completed) => {
                    Some(DockViewportActivationTransaction::new(
                        completed.pending().target_space().clone(),
                        completed.registration().window(),
                        completed
                            .pending()
                            .focus_item()
                            .cloned()
                            .map(DockViewportFocusRequest::panel)
                            .unwrap_or_else(DockViewportFocusRequest::no_panel_focus),
                    ))
                }
                DockViewportTearOffOpenOutcome::Duplicate(_) => None,
            },
        }
    }

    pub(crate) fn action_result(&self) -> Result<DockActionOutcome, DockActionApplyError> {
        match self {
            DockViewportDropRouteOutcome::Action(outcome) => Ok(outcome.action()),
            DockViewportDropRouteOutcome::TearOff(outcome) => match outcome.as_ref() {
                DockViewportTearOffOpenOutcome::Completed(completed) => Ok(completed.action()),
                DockViewportTearOffOpenOutcome::Duplicate(_) => Ok(DockActionOutcome::Unchanged),
            },
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
    Floating {
        source_space: DockSpaceId,
        floating: DockNodeId,
    },
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
            .map(|_| DockViewportTearOffCancelled::new(reason))
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

    pub(crate) fn take_committed(
        &mut self,
        pending: &DockViewportTearOffPending,
    ) -> Option<DockViewportTearOffPending> {
        let key = pending.request().key();
        if self.pending_by_key.get(&key) != Some(pending) {
            return None;
        }
        self.pending_by_key.remove(&key)
    }
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

    #[test]
    fn tear_off_machine_only_commits_matching_pending_request() {
        let mut machine = DockViewportTearOffMachine::default();
        let request = DockViewportTearOffRequest::new(
            space("main"),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(900.0), px(900.0)),
            None,
        );
        let DockViewportTearOffBeginOutcome::Pending(expired_pending) = machine.begin(
            request.clone(),
            space("detached"),
            None,
            DockViewportTearOffTick::new(1),
        ) else {
            panic!("first request should become pending");
        };
        assert_eq!(machine.expire(DockViewportTearOffTick::new(602)).len(), 1);
        let DockViewportTearOffBeginOutcome::Pending(current_pending) = machine.begin(
            request,
            space("other"),
            None,
            DockViewportTearOffTick::new(603),
        ) else {
            panic!("new request after expiration should become pending");
        };

        assert_eq!(machine.take_committed(&expired_pending), None);
        assert_eq!(machine.len(), 1);
        assert_eq!(
            machine.take_committed(&current_pending).as_ref(),
            Some(&current_pending)
        );
        assert_eq!(machine.len(), 0);
    }
}
