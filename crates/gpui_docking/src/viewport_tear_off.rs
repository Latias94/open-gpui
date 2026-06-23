use crate::{
    DockActionApplyError, DockActionOutcome, DockGraph, DockItemId, DockNodeId, DockSpaceId,
    DockViewportActivationTransaction, DockViewportFocusRequest, DockViewportWindowEffects,
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

/// Runtime state for a tear-off request that is waiting for a platform viewport.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportTearOffPending {
    /// Original release request that started the transaction.
    request: DockViewportTearOffRequest,
    /// Empty logical dock space that will receive the torn-off payload.
    target_space: DockSpaceId,
    /// Runtime window that hosted the source space when the tear-off was prepared.
    source_window: Option<AnyWindowHandle>,
    /// Panel item that should receive GPUI focus after the tear-off completes.
    focus_item: Option<DockItemId>,
}

impl DockViewportTearOffPending {
    pub(crate) fn request(&self) -> &DockViewportTearOffRequest {
        &self.request
    }

    pub(crate) fn target_space(&self) -> &DockSpaceId {
        &self.target_space
    }

    pub(crate) fn source_window(&self) -> Option<AnyWindowHandle> {
        self.source_window
    }

    pub(crate) fn focus_item(&self) -> Option<&DockItemId> {
        self.focus_item.as_ref()
    }
}

/// Graph mutation for a pending tear-off has been committed and is waiting for viewport
/// registration.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportCommittedTearOffMove {
    pending: DockViewportTearOffPending,
    action: crate::DockActionOutcome,
}

pub(crate) struct DockViewportCommittedTearOffMoveCommit {
    pub(crate) pending: DockViewportTearOffPending,
    pub(crate) action: crate::DockActionOutcome,
}

impl DockViewportCommittedTearOffMove {
    fn new(pending: DockViewportTearOffPending, action: crate::DockActionOutcome) -> Self {
        Self { pending, action }
    }

    pub(crate) fn into_commit(self) -> DockViewportCommittedTearOffMoveCommit {
        DockViewportCommittedTearOffMoveCommit {
            pending: self.pending,
            action: self.action,
        }
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
    /// The recorded source payload can no longer be committed.
    SourceUnavailable,
}

/// Cancelled tear-off request and the reason it could not complete.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportTearOffCancelled {
    /// Pending request that was removed from the tear-off queue.
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

/// Completed graph-first tear-off request after platform viewport registration.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportTearOffCompleted {
    /// Pending request that completed.
    pending: DockViewportTearOffPending,
    /// Runtime viewport registration outcome.
    registration: crate::DockViewportRegisterOutcome,
    /// Runtime-owned windows superseded while completing this tear-off.
    replaced_windows: Vec<AnyWindowHandle>,
    /// Surviving runtime windows affected by registration cleanup.
    registration_affected_windows: Vec<AnyWindowHandle>,
    /// Runtime-owned source windows whose logical space became empty after the tear-off move.
    vacated_source_windows: Vec<AnyWindowHandle>,
    /// Surviving runtime windows affected while vacating the source viewport.
    vacated_source_affected_windows: Vec<AnyWindowHandle>,
    /// Graph transaction outcome that committed before the platform window was opened.
    action: crate::DockActionOutcome,
}

impl DockViewportTearOffCompleted {
    pub(crate) fn new(
        pending: DockViewportTearOffPending,
        registration: crate::DockViewportRegisterOutcome,
        replaced_windows: Vec<AnyWindowHandle>,
        registration_affected_windows: Vec<AnyWindowHandle>,
        vacated_source_windows: Vec<AnyWindowHandle>,
        vacated_source_affected_windows: Vec<AnyWindowHandle>,
        action: crate::DockActionOutcome,
    ) -> Self {
        Self {
            pending,
            registration,
            replaced_windows,
            registration_affected_windows,
            vacated_source_windows,
            vacated_source_affected_windows,
            action,
        }
    }

    pub(crate) fn pending(&self) -> &DockViewportTearOffPending {
        &self.pending
    }

    pub(crate) fn registration(&self) -> &crate::DockViewportRegisterOutcome {
        &self.registration
    }

    pub(crate) fn affected_windows(&self) -> Vec<AnyWindowHandle> {
        let mut windows = self.registration_affected_windows.clone();
        extend_unique_windows(
            &mut windows,
            self.vacated_source_affected_windows.iter().cloned(),
        );
        windows
    }

    #[cfg(test)]
    pub(crate) fn has_window_effects(&self) -> bool {
        self.window_effects().has_effects()
    }

    pub(crate) fn window_effects(&self) -> DockViewportWindowEffects {
        DockViewportWindowEffects::new(
            self.replaced_windows.clone(),
            self.affected_windows(),
            self.vacated_source_windows.clone(),
        )
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
    /// Viewport window effects requested by cleanup while committing the drop.
    window_effects: DockViewportWindowEffects,
}

impl DockViewportDropActionOutcome {
    pub(crate) fn new(
        action: DockActionOutcome,
        activation: Option<DockViewportActivationTransaction>,
    ) -> Self {
        Self {
            action,
            activation,
            window_effects: DockViewportWindowEffects::default(),
        }
    }

    pub(crate) fn with_window_effects(mut self, window_effects: DockViewportWindowEffects) -> Self {
        self.window_effects = window_effects;
        self
    }

    pub(crate) fn action(&self) -> DockActionOutcome {
        self.action
    }

    pub(crate) fn activation(&self) -> Option<&DockViewportActivationTransaction> {
        self.activation.as_ref()
    }

    pub(crate) fn window_effects(&self) -> DockViewportWindowEffects {
        self.window_effects.clone()
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

    #[cfg(test)]
    pub(crate) fn affected_windows(&self) -> Vec<AnyWindowHandle> {
        self.window_effects().refresh().to_vec()
    }

    pub(crate) fn has_window_effects(&self) -> bool {
        self.window_effects().has_effects()
    }

    pub(crate) fn window_effects(&self) -> DockViewportWindowEffects {
        match self {
            DockViewportDropRouteOutcome::Action(outcome) => outcome.window_effects(),
            DockViewportDropRouteOutcome::TearOff(outcome) => match outcome.as_ref() {
                DockViewportTearOffOpenOutcome::Completed(completed) => completed.window_effects(),
                DockViewportTearOffOpenOutcome::Duplicate(_) => {
                    DockViewportWindowEffects::default()
                }
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

fn extend_unique_windows(
    windows: &mut Vec<AnyWindowHandle>,
    next_windows: impl IntoIterator<Item = AnyWindowHandle>,
) {
    for window in next_windows {
        if windows
            .iter()
            .any(|existing| existing.window_id() == window.window_id())
        {
            continue;
        }
        windows.push(window);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DockViewportTearOffMachine {
    pending_by_key: BTreeMap<DockViewportTearOffKey, DockViewportTearOffPending>,
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
        source_window: Option<AnyWindowHandle>,
        focus_item: Option<DockItemId>,
    ) -> DockViewportTearOffBeginOutcome {
        let key = request.key();
        if let Some(pending) = self.pending_by_key.get(&key) {
            return DockViewportTearOffBeginOutcome::Duplicate(pending.clone());
        }

        let pending = DockViewportTearOffPending {
            request,
            target_space,
            source_window,
            focus_item,
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

    pub(crate) fn is_current_pending(&self, pending: &DockViewportTearOffPending) -> bool {
        let key = pending.request().key();
        self.pending_by_key.get(&key) == Some(pending)
    }

    pub(crate) fn take_committed(
        &mut self,
        pending: &DockViewportTearOffPending,
        action: crate::DockActionOutcome,
    ) -> Option<DockViewportCommittedTearOffMove> {
        if !self.is_current_pending(pending) {
            return None;
        }
        let key = pending.request().key();
        self.pending_by_key
            .remove(&key)
            .map(|pending| DockViewportCommittedTearOffMove::new(pending, action))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockActionOutcome, DockViewportAdapter,
        viewport_test_support::{handle, item, space},
    };
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

        let first = machine.begin(request.clone(), space("detached"), None, None);
        let second = machine.begin(request, space("other"), None, None);

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

        let first = machine.begin(request.clone(), space("detached"), None, None);
        let second = machine.begin(request, space("other"), None, None);

        assert!(matches!(first, DockViewportTearOffBeginOutcome::Pending(_)));
        let DockViewportTearOffBeginOutcome::Duplicate(existing) = second else {
            panic!("second stack begin should be idempotent");
        };
        assert_eq!(existing.request.source_space(), &source);
        assert_eq!(existing.target_space(), &space("detached"));
        assert_eq!(machine.len(), 1);
    }

    #[test]
    fn tear_off_machine_cancels_pending_requests() {
        let mut machine = DockViewportTearOffMachine::default();
        let request = DockViewportTearOffRequest::new(
            space("main"),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(900.0), px(900.0)),
            None,
        );
        let key = request.key();

        machine.begin(request, space("detached"), None, None);
        let cancelled = machine
            .cancel(&key, DockViewportTearOffCancelReason::Cancelled)
            .expect("pending request should cancel");

        assert_eq!(
            cancelled.reason(),
            DockViewportTearOffCancelReason::Cancelled
        );
        assert_eq!(cancelled.pending().target_space(), &space("detached"));
        assert_eq!(machine.len(), 0);
    }

    #[test]
    fn tear_off_machine_only_commits_matching_uncancelled_pending_request() {
        let mut machine = DockViewportTearOffMachine::default();
        let request = DockViewportTearOffRequest::new(
            space("main"),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(900.0), px(900.0)),
            None,
        );
        let key = request.key();
        let DockViewportTearOffBeginOutcome::Pending(cancelled_pending) =
            machine.begin(request.clone(), space("detached"), None, None)
        else {
            panic!("first request should become pending");
        };
        assert!(
            machine
                .cancel(&key, DockViewportTearOffCancelReason::Cancelled)
                .is_some()
        );
        let DockViewportTearOffBeginOutcome::Pending(current_pending) =
            machine.begin(request, space("other"), None, None)
        else {
            panic!("new request after cancellation should become pending");
        };

        assert!(!machine.is_current_pending(&cancelled_pending));
        assert_eq!(
            machine.take_committed(&cancelled_pending, DockActionOutcome::Unchanged),
            None
        );
        assert_eq!(machine.len(), 1);
        assert!(machine.is_current_pending(&current_pending));
        let committed = machine
            .take_committed(&current_pending, DockActionOutcome::Unchanged)
            .expect("current pending request should produce committed token");
        let commit = committed.into_commit();
        assert_eq!(commit.pending, current_pending);
        assert_eq!(commit.action, DockActionOutcome::Unchanged);
        assert_eq!(machine.len(), 0);
    }

    #[test]
    fn completed_tear_off_aggregates_window_effects_without_duplicates() {
        let mut machine = DockViewportTearOffMachine::default();
        let request = DockViewportTearOffRequest::new(
            space("main"),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(900.0), px(900.0)),
            None,
        );
        let DockViewportTearOffBeginOutcome::Pending(pending) =
            machine.begin(request, space("detached"), None, None)
        else {
            panic!("request should become pending");
        };
        let mut adapter = DockViewportAdapter::new();
        let registration = adapter.register_viewport_with_outcome(space("detached"), handle(1));
        let first_affected = handle(2);
        let second_affected = handle(3);
        let replaced_window = handle(4);
        let vacated_source_window = handle(5);

        let completed = DockViewportTearOffCompleted::new(
            pending,
            registration,
            vec![replaced_window],
            vec![first_affected, second_affected],
            vec![vacated_source_window],
            vec![second_affected, first_affected],
            DockActionOutcome::Changed,
        );
        let route = DockViewportDropRouteOutcome::tear_off(
            DockViewportTearOffOpenOutcome::Completed(completed.clone()),
        );

        assert!(completed.has_window_effects());
        assert_eq!(
            completed.affected_windows(),
            vec![first_affected, second_affected],
            "registration and vacated-source cleanup should refresh each affected window once"
        );
        let effects = completed.window_effects();
        assert_eq!(effects.close_now(), &[replaced_window]);
        assert_eq!(effects.refresh(), &[first_affected, second_affected]);
        assert_eq!(
            effects.close_after_current_effect(),
            &[vacated_source_window]
        );
        assert!(route.has_window_effects());
        assert_eq!(
            route.affected_windows(),
            vec![first_affected, second_affected]
        );
    }
}
