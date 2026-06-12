use crate::{
    DockActionApplyError, DockActionOutcome, DockController, DockGraphMutationError, DockItemId,
    DockNode, DockNodeId, DockPolicy, DockPolicyError, DockSpaceId, DockViewportActivationTarget,
    DockViewportAdapter, DockViewportCloseOutcome, DockViewportClosePolicy,
    DockViewportCloseStatus, DockViewportDropActionOutcome, DockViewportDropPayload,
    DockViewportDropRoute, DockViewportDropRouteCommit, DockViewportDropRouteOutcome,
    DockViewportDropRouteRequest, DockViewportIdentity, DockViewportPlacementLayout,
    DockViewportPlacementValidationError, DockViewportResolvedDropRoute,
    DockViewportRestoreOutcome, DockViewportRuntimeHandle, DockViewportRuntimeStatus,
    DockViewportShouldCloseOutcome, DockViewportShouldCloseStatus, DockViewportTargetContext,
    DockViewportTargetHit, DockViewportTearOffBeginOutcome, DockViewportTearOffCancelReason,
    DockViewportTearOffCancelled, DockViewportTearOffCommitFailure, DockViewportTearOffCompleted,
    DockViewportTearOffCompletionOutcome, DockViewportTearOffCompletionPending,
    DockViewportTearOffKey, DockViewportTearOffMachine, DockViewportTearOffOpenOutcome,
    DockViewportTearOffPending, DockViewportTearOffRequest, DockViewportTearOffTick,
    DockViewportWindowFacts,
    drag::DockDragPayload,
    drop_preview::DockDropPreview,
    drop_runtime::DockHostDropSceneFact,
    drop_target::{DockDropResolution, DockResolvedDropTarget, validate_resolved_drop_target},
    interaction::DockRuntimeDragSession,
    viewport_close_gate::DockViewportCloseGate,
    viewport_drop_scene::{
        DockViewportHostSceneFrame, DockViewportHostSceneRegistration,
        DockViewportHostSceneRegistry, DockViewportHostSceneSnapshot,
    },
    workspace_move_validation::dock_target_validator,
    workspace_transaction::DockWorkspacePayloadDropRequest,
};
use open_gpui::{
    AnyWindowHandle, App, Bounds, Entity, Pixels, Point, WindowBounds, WindowId, WindowOptions,
    point, px, size,
};
use std::collections::HashSet;

const DEFAULT_TEAR_OFF_WINDOW_SIZE: open_gpui::Size<Pixels> = size(px(360.0), px(240.0));
const DEFAULT_TEAR_OFF_CURSOR_OFFSET: Point<Pixels> = point(px(24.0), px(18.0));

/// Internal owner for controller-backed platform viewport lifecycle.
///
/// The runtime keeps the shared [`DockController`] together with the low-level
/// [`DockViewportAdapter`] so the handle does not have to pass the controller into every open call
/// or duplicate close-callback cleanup logic. The adapter remains the place for window mappings,
/// live window facts, and placement import/export.
#[derive(Debug)]
pub(crate) struct DockViewportRuntime {
    controller: Entity<DockController>,
    adapter: DockViewportAdapter,
    close_gate: DockViewportCloseGate,
    host_scenes: DockViewportHostSceneRegistry,
    tear_off: DockViewportTearOffMachine,
    tear_off_tick: DockViewportTearOffTick,
    drag_session: Option<DockRuntimeDragSession>,
    next_drag_session_id: u64,
    owned_windows: HashSet<WindowId>,
    last_hovered_window: Option<DockViewportLastHoveredWindow>,
    routed_drop_preview: Option<DockViewportRoutedDropPreview>,
    status: DockViewportRuntimeStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DockViewportLastHoveredWindow {
    window_id: WindowId,
    drag_session_id: Option<u64>,
}

enum DockViewportWorkspaceRouteTarget {
    Valid(Option<crate::DockViewportCachedDropTarget>),
    Rejected(DockPolicyError),
}

impl DockViewportLastHoveredWindow {
    fn new(window_id: WindowId, drag_session_id: Option<u64>) -> Self {
        Self {
            window_id,
            drag_session_id,
        }
    }

    fn matches_drag_session(&self, session: Option<&DockRuntimeDragSession>) -> bool {
        self.drag_session_id == session.map(DockRuntimeDragSession::id)
    }

    fn matches_drag_session_id(&self, drag_session_id: Option<u64>) -> bool {
        self.drag_session_id == drag_session_id
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportRoutedDropPreview {
    identity: DockViewportIdentity,
    pub(crate) preview: DockDropPreview,
    commit: DockViewportDropRouteCommit,
    pub(crate) payload_title: String,
}

impl DockViewportRoutedDropPreview {
    fn new(
        space: DockSpaceId,
        window_id: WindowId,
        preview: DockDropPreview,
        commit: DockViewportDropRouteCommit,
        payload_title: impl Into<String>,
    ) -> Self {
        Self {
            identity: DockViewportIdentity::new(space, window_id),
            preview,
            commit,
            payload_title: payload_title.into(),
        }
    }

    fn matches(&self, space: &DockSpaceId, window_id: WindowId) -> bool {
        self.identity.matches(space, window_id)
    }

    fn space(&self) -> &DockSpaceId {
        self.identity.space()
    }

    fn window_id(&self) -> WindowId {
        self.identity.window_id()
    }

    fn commit(&self) -> &DockViewportDropRouteCommit {
        &self.commit
    }
}

#[derive(Debug)]
pub(crate) struct DockViewportPreparedTearOffDrop {
    pub(crate) request: DockViewportTearOffRequest,
    pub(crate) target_space: DockSpaceId,
    pub(crate) options: WindowOptions,
}

#[derive(Debug)]
pub(crate) enum DockViewportTearOffCommitPreparation {
    Prepared(Box<DockViewportPreparedTearOffDrop>),
}

fn cached_route_target(
    frame: DockViewportHostSceneFrame,
    resolution: DockDropResolution,
) -> Result<crate::DockViewportCachedDropTarget, DockPolicyError> {
    match resolution {
        DockDropResolution::Valid(target) => {
            Ok(crate::DockViewportCachedDropTarget::new(frame, target))
        }
        DockDropResolution::Rejected(rejection) => Err(rejection.reason),
    }
}

fn push_unique_window(windows: &mut Vec<AnyWindowHandle>, window: Option<AnyWindowHandle>) {
    let Some(window) = window else {
        return;
    };
    if windows
        .iter()
        .any(|existing| existing.window_id() == window.window_id())
    {
        return;
    }
    windows.push(window);
}

impl DockViewportRuntime {
    /// Creates a runtime with the default close policy.
    pub(crate) fn new(controller: Entity<DockController>) -> Self {
        Self::with_close_policy(controller, DockViewportClosePolicy::default())
    }

    /// Creates a runtime with an explicit close policy.
    pub(crate) fn with_close_policy(
        controller: Entity<DockController>,
        close_policy: DockViewportClosePolicy,
    ) -> Self {
        Self {
            controller,
            adapter: DockViewportAdapter::new(),
            close_gate: DockViewportCloseGate::new(close_policy),
            host_scenes: DockViewportHostSceneRegistry::default(),
            tear_off: DockViewportTearOffMachine::default(),
            tear_off_tick: DockViewportTearOffTick::default(),
            drag_session: None,
            next_drag_session_id: 0,
            owned_windows: HashSet::new(),
            last_hovered_window: None,
            routed_drop_preview: None,
            status: DockViewportRuntimeStatus::default(),
        }
    }

    /// Creates a runtime from an existing adapter.
    #[cfg(test)]
    pub(crate) fn from_adapter(
        controller: Entity<DockController>,
        adapter: DockViewportAdapter,
        close_policy: DockViewportClosePolicy,
    ) -> Self {
        let close_gate = DockViewportCloseGate::new(close_policy);
        close_gate.sync_adapter(&adapter);
        Self {
            controller,
            adapter,
            close_gate,
            host_scenes: DockViewportHostSceneRegistry::default(),
            tear_off: DockViewportTearOffMachine::default(),
            tear_off_tick: DockViewportTearOffTick::default(),
            drag_session: None,
            next_drag_session_id: 0,
            owned_windows: HashSet::new(),
            last_hovered_window: None,
            routed_drop_preview: None,
            status: DockViewportRuntimeStatus::default(),
        }
    }

    /// Wraps this runtime in a cloneable handle for GPUI application callbacks.
    pub(crate) fn into_handle(self) -> DockViewportRuntimeHandle {
        DockViewportRuntimeHandle::from_runtime(self)
    }

    pub(crate) fn controller_entity(&self) -> Entity<DockController> {
        self.controller.clone()
    }

    /// Returns the low-level viewport adapter.
    pub(crate) fn adapter(&self) -> &DockViewportAdapter {
        &self.adapter
    }

    /// Returns the latest read-only runtime diagnostic snapshot.
    pub(crate) fn runtime_status(&self) -> DockViewportRuntimeStatus {
        self.status.clone()
    }

    pub(crate) fn begin_payload_drag(
        &mut self,
        payload: &DockDragPayload,
    ) -> DockRuntimeDragSession {
        let id = self.next_drag_session_id.wrapping_add(1);
        self.next_drag_session_id = id;
        let session = DockRuntimeDragSession::new(id, payload);
        self.drag_session = Some(session.clone());
        self.last_hovered_window = None;
        session
    }

    pub(crate) fn active_payload_drag_session(
        &self,
        payload: &DockDragPayload,
    ) -> Option<DockRuntimeDragSession> {
        self.drag_session
            .as_ref()
            .filter(|session| session.accepts_payload(payload))
            .cloned()
    }

    pub(crate) fn finish_payload_drag(&mut self, session: &DockRuntimeDragSession) -> bool {
        if self.drag_session.as_ref() != Some(session) {
            return false;
        }
        self.drag_session = None;
        self.clear_last_hovered_window_for_drag_session(Some(session));
        true
    }

    pub(crate) fn validate_payload_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Result<(), DockActionApplyError> {
        let Some(session) = session else {
            return Ok(());
        };
        if self.drag_session.as_ref() == Some(session) {
            return Ok(());
        }
        Err(DockActionApplyError::DropDragSessionStale {
            session: session.id(),
        })
    }

    #[cfg(test)]
    pub(crate) fn last_hovered_window(&self) -> Option<WindowId> {
        self.last_hovered_window.map(|hovered| hovered.window_id)
    }

    pub(crate) fn last_hovered_window_for_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<WindowId> {
        let hovered = self.last_hovered_window?;
        hovered
            .matches_drag_session(session)
            .then_some(hovered.window_id)
    }

    pub(crate) fn record_window_focus(&mut self, window_id: WindowId) {
        self.adapter.record_window_focus(window_id);
    }

    fn clear_last_hovered_window_if_matches(&mut self, window_id: WindowId) {
        if self
            .last_hovered_window
            .is_some_and(|hovered| hovered.window_id == window_id)
        {
            self.last_hovered_window = None;
        }
    }

    fn discard_owned_window(&mut self, window_id: WindowId) -> bool {
        self.owned_windows.remove(&window_id)
    }

    fn clear_last_hovered_window_for_drag_session(
        &mut self,
        session: Option<&DockRuntimeDragSession>,
    ) {
        if self
            .last_hovered_window
            .is_some_and(|hovered| hovered.matches_drag_session(session))
        {
            self.last_hovered_window = None;
        }
    }

    fn clear_last_hovered_window_for_drag_id(&mut self, drag_session_id: Option<u64>) {
        if self
            .last_hovered_window
            .is_some_and(|hovered| hovered.matches_drag_session_id(drag_session_id))
        {
            self.last_hovered_window = None;
        }
    }

    /// Returns the close policy used by [`handle_window_should_close`](Self::handle_window_should_close).
    pub(crate) fn close_policy(&self) -> DockViewportClosePolicy {
        self.close_gate.close_policy()
    }

    /// Replaces the close policy used by [`handle_window_should_close`](Self::handle_window_should_close).
    pub(crate) fn set_close_policy(&mut self, close_policy: DockViewportClosePolicy) {
        self.close_gate.set_close_policy(close_policy);
    }

    #[cfg(test)]
    pub(crate) fn pending_tear_off_len(&self) -> usize {
        self.tear_off.len()
    }

    /// Updates display, window, and host bounds for a registered viewport.
    ///
    /// Returns true when the stored runtime snapshot changed.
    pub(crate) fn update_viewport_snapshot(
        &mut self,
        space: &DockSpaceId,
        window_facts: DockViewportWindowFacts,
        host_bounds: Bounds<Pixels>,
    ) -> bool {
        self.adapter
            .update_snapshot(space, window_facts, host_bounds)
    }

    #[cfg(test)]
    pub(crate) fn begin_viewport_host_scene(
        &mut self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        host_bounds: Bounds<Pixels>,
        host_position: Point<Pixels>,
    ) -> bool {
        self.begin_viewport_host_scene_frame(
            space,
            window_id,
            window_facts,
            host_bounds,
            host_position,
        )
        .is_some_and(|registration| registration.changed)
    }

    pub(crate) fn begin_viewport_host_scene_frame(
        &mut self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        host_bounds: Bounds<Pixels>,
        host_position: Point<Pixels>,
    ) -> Option<DockViewportHostSceneRegistration> {
        let space = space.into();
        let window = self.adapter.window_for_space(&space)?;
        let current_identity = DockViewportIdentity::new(space.clone(), window.window_id());
        if !current_identity.matches(&space, window_id) {
            return None;
        }
        let changed = self.update_viewport_snapshot(&space, window_facts, host_bounds);
        let mut registration = self
            .host_scenes
            .register(DockViewportHostSceneSnapshot::new(
                space,
                window_id,
                window_facts.screen_bounds,
                host_bounds,
                host_position,
            ));
        registration.changed |= changed;
        Some(registration)
    }

    #[cfg(test)]
    pub(crate) fn push_viewport_host_scene_fact(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
        fact: DockHostDropSceneFact,
    ) -> bool {
        self.host_scenes.push_fact(space, window_id, fact)
    }

    pub(crate) fn push_viewport_host_scene_frame_fact(
        &mut self,
        frame: &DockViewportHostSceneFrame,
        fact: DockHostDropSceneFact,
    ) -> Option<DockViewportHostSceneFrame> {
        self.host_scenes.push_frame_fact(frame, fact)
    }

    pub(crate) fn routed_drop_preview_for(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<DockViewportRoutedDropPreview> {
        self.routed_drop_preview
            .as_ref()
            .filter(|preview| preview.matches(space, window_id))
            .cloned()
    }

    pub(crate) fn routed_drop_commit_for_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<DockViewportDropRouteCommit> {
        let session = session?;
        let hovered_window = self.last_hovered_window_for_drag_session(Some(session))?;
        let preview = self.routed_drop_preview.as_ref()?;
        if preview.window_id() != hovered_window {
            return None;
        }
        let commit = preview.commit();
        if commit.drag_session_id() != Some(session.id()) {
            return None;
        }
        Some(commit.clone())
    }

    fn routed_drop_target_hit_for_release(
        &self,
        request: &DockViewportDropRouteRequest,
    ) -> Option<DockViewportTargetHit> {
        let commit = self.routed_drop_commit_for_drag_session(request.drag_session())?;
        let target = commit.routed_preview_target_hit()?;
        let hit = self.adapter.resolve_viewport_target(
            request.release_position(),
            &DockViewportTargetContext::from_window_signals(
                Some(target.window_id()),
                None,
                Vec::new(),
            ),
        )?;
        if hit.space() == target.space() && hit.window_id() == target.window_id() {
            Some(target)
        } else {
            None
        }
    }

    pub(crate) fn update_routed_drop_preview(
        &mut self,
        resolution: &DockViewportResolvedDropRoute,
        payload_title: impl Into<String>,
    ) -> (bool, Vec<AnyWindowHandle>) {
        let payload_title = payload_title.into();
        let next = match resolution.route() {
            DockViewportDropRoute::KnownViewport { target } => {
                self.last_hovered_window = Some(DockViewportLastHoveredWindow::new(
                    target.window_id(),
                    resolution.drag_session_id(),
                ));
                self.routed_drop_preview_from_commit(resolution.commit(), payload_title)
            }
            DockViewportDropRoute::Local { .. }
            | DockViewportDropRoute::TearOff(_)
            | DockViewportDropRoute::Rejected(_) => {
                self.clear_last_hovered_window_for_drag_id(resolution.drag_session_id());
                None
            }
        };
        self.replace_routed_drop_preview(next)
    }

    pub(crate) fn clear_routed_drop_preview(&mut self) -> (bool, Vec<AnyWindowHandle>) {
        self.replace_routed_drop_preview(None)
    }

    fn routed_drop_preview_from_commit(
        &self,
        commit: &DockViewportDropRouteCommit,
        payload_title: String,
    ) -> Option<DockViewportRoutedDropPreview> {
        let (space, window_id, resolved) = commit.routed_preview_target()?;
        Some(DockViewportRoutedDropPreview::new(
            space.clone(),
            window_id,
            DockDropPreview::from_resolved_target(&resolved)?,
            commit.clone(),
            payload_title,
        ))
    }

    fn replace_routed_drop_preview(
        &mut self,
        next: Option<DockViewportRoutedDropPreview>,
    ) -> (bool, Vec<AnyWindowHandle>) {
        if self.routed_drop_preview == next {
            return (false, Vec::new());
        }

        let mut windows = Vec::new();
        if let Some(current) = self.routed_drop_preview.as_ref() {
            push_unique_window(&mut windows, self.adapter.window_for_space(current.space()));
        }
        if let Some(next) = next.as_ref() {
            push_unique_window(&mut windows, self.adapter.window_for_space(next.space()));
        }
        self.routed_drop_preview = next;
        (true, windows)
    }

    pub(crate) fn reusable_window_for_space(
        &mut self,
        space: &DockSpaceId,
        cx: &mut App,
    ) -> DockViewportReusableWindow {
        let Some(window) = self.adapter.window_for_space(space) else {
            return DockViewportReusableWindow::Missing;
        };
        if window
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
        {
            return DockViewportReusableWindow::Reused(window);
        }

        self.clear_last_hovered_window_if_matches(window.window_id());
        self.discard_owned_window(window.window_id());
        self.adapter.unregister_space(space);
        self.host_scenes.unregister_space(space);
        self.close_gate.sync_adapter(&self.adapter);
        DockViewportReusableWindow::Stale
    }

    pub(crate) fn register_opened_viewport(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
    ) -> Vec<AnyWindowHandle> {
        self.owned_windows.insert(window.window_id());
        let replaced = self.adapter.register_viewport_with_outcome(space, window);
        let mut replaced_windows = Vec::new();
        for removed in replaced.replaced() {
            self.clear_last_hovered_window_if_matches(removed.window.window_id());
            self.host_scenes.unregister_space(&removed.space);
            if removed.window != window
                && self.discard_owned_window(removed.window.window_id())
                && !replaced_windows.contains(&removed.window)
            {
                replaced_windows.push(removed.window);
            }
        }
        self.close_gate.sync_adapter(&self.adapter);
        replaced_windows
    }

    pub(crate) fn commit_payload_drop_route_with_outcome(
        &mut self,
        commit: DockViewportDropRouteCommit,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let result = self.commit_payload_drop_route_inner(commit, cx);
        self.status.record_drop_result(&result);
        result
    }

    pub(crate) fn record_drop_route_result(
        &mut self,
        result: &Result<DockViewportDropRouteOutcome, DockActionApplyError>,
    ) {
        self.status.record_drop_result(result);
    }

    pub(crate) fn record_tear_off_outcome(&mut self, outcome: &DockViewportTearOffOpenOutcome) {
        self.status.record_tear_off(outcome);
    }

    fn commit_payload_drop_route_inner(
        &mut self,
        commit: DockViewportDropRouteCommit,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let (source_space, source_tabs, payload, target_space) = match commit {
            DockViewportDropRouteCommit::Workspace(commit) => {
                self.validate_payload_drag_session(commit.drag_session())?;
                let (
                    source_space,
                    source_tabs,
                    payload,
                    route_space,
                    target_window_id,
                    host_position,
                    resolved_target,
                ) = commit.into_parts();
                let target_space = match resolved_target
                    .filter(|target| target.frame().is_current_in(&self.host_scenes))
                {
                    Some(target) => self.validate_cached_route_target(
                        &route_space,
                        target.into_target(),
                        &payload,
                        source_tabs,
                        cx,
                    )?,
                    None => self.resolve_route_target(
                        &route_space,
                        target_window_id,
                        host_position,
                        &payload,
                        source_tabs,
                        cx,
                    )?,
                };
                (source_space, source_tabs, payload, target_space)
            }
            DockViewportDropRouteCommit::TearOff(request) => {
                self.validate_payload_drag_session(request.drag_session())?;
                return Err(DockActionApplyError::TearOffViewportOpenFailed {
                    message:
                        "tear-off viewport commits must be opened through DockViewportRuntimeHandle"
                            .to_string(),
                });
            }
            DockViewportDropRouteCommit::Rejected(error) => return Err(error.into()),
        };

        let (target_space, target) = target_space;
        let focus_item = self.focus_item_for_payload(&payload, source_tabs, cx);
        let action = self.controller.update(cx, |controller, cx| {
            let outcome =
                controller.commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                    source_space: &source_space,
                    payload: payload.as_workspace_payload(source_tabs),
                    target_space: &target_space,
                    target,
                });
            if outcome
                .as_ref()
                .map(|outcome| outcome.changed())
                .unwrap_or(false)
            {
                cx.notify();
            }
            outcome
        })?;
        let activation = self.activate_viewport_for_space(&target_space, focus_item, cx);
        Ok(DockViewportDropRouteOutcome::Action(
            DockViewportDropActionOutcome::new(action, activation),
        ))
    }

    fn validate_cached_route_target(
        &self,
        target_space: &DockSpaceId,
        target: DockResolvedDropTarget,
        payload: &DockViewportDropPayload,
        source_tabs: DockNodeId,
        cx: &App,
    ) -> Result<(DockSpaceId, DockResolvedDropTarget), DockActionApplyError> {
        let controller = self.controller.read(cx);
        let workspace = controller.workspace();
        let policy = workspace.policy().clone();
        let payload_classes =
            workspace.payload_dock_classes_for_viewport_payload(payload, source_tabs);
        let target_validator = dock_target_validator(target_space, &payload_classes, &policy);
        match validate_resolved_drop_target(target, &policy, Some(&target_validator)) {
            DockDropResolution::Valid(target) => Ok((target_space.clone(), target)),
            DockDropResolution::Rejected(rejection) => {
                Err(DockActionApplyError::Policy(rejection.reason))
            }
        }
    }

    fn resolve_route_target(
        &self,
        target_space: &DockSpaceId,
        target_window_id: Option<WindowId>,
        host_position: Point<Pixels>,
        payload: &DockViewportDropPayload,
        source_tabs: DockNodeId,
        cx: &App,
    ) -> Result<(DockSpaceId, DockResolvedDropTarget), DockActionApplyError> {
        let controller = self.controller.read(cx);
        let workspace = controller.workspace();
        let policy = workspace.policy().clone();
        let payload_classes =
            workspace.payload_dock_classes_for_viewport_payload(payload, source_tabs);
        let target_validator = dock_target_validator(target_space, &payload_classes, &policy);
        let Some((_, resolution)) = self.host_scenes.resolve_frame_for_window(
            target_space,
            target_window_id,
            host_position,
            &policy,
            Some(&target_validator),
        ) else {
            return Err(DockActionApplyError::DropTargetUnavailable);
        };
        match resolution {
            crate::drop_target::DockDropResolution::Valid(target) => {
                Ok((target_space.clone(), target))
            }
            crate::drop_target::DockDropResolution::Rejected(rejection) => {
                Err(DockActionApplyError::Policy(rejection.reason))
            }
        }
    }

    pub(crate) fn prepare_tear_off_drop_route_commit(
        &mut self,
        request: DockViewportTearOffRequest,
        cx: &mut App,
    ) -> Result<DockViewportTearOffCommitPreparation, DockActionApplyError> {
        self.validate_payload_drag_session(request.drag_session())?;
        Ok(DockViewportTearOffCommitPreparation::Prepared(Box::new(
            self.prepare_tear_off_drop_route(request, cx)?,
        )))
    }

    pub(crate) fn prepare_tear_off_drop_route(
        &mut self,
        request: DockViewportTearOffRequest,
        cx: &App,
    ) -> Result<DockViewportPreparedTearOffDrop, DockActionApplyError> {
        {
            let graph = self.controller.read(cx).graph();
            match request.payload() {
                DockViewportDropPayload::Item(item) => {
                    if graph
                        .find_item_in_space(request.source_space(), item)
                        .is_none_or(|(tabs, _)| tabs != request.source_tabs())
                    {
                        return Err(DockActionApplyError::ItemNotInTabs {
                            tabs: request.source_tabs(),
                            item: item.clone(),
                        });
                    }
                }
                DockViewportDropPayload::Tabs => {
                    if graph
                        .root_for_node_in_space(request.source_space(), request.source_tabs())
                        .is_none()
                    {
                        return Err(tear_off_payload_mismatch(
                            request.source_space(),
                            request.source_tabs(),
                        ));
                    }
                    if !matches!(
                        graph.node(request.source_tabs()),
                        Some(DockNode::Tabs { items, .. }) if !items.is_empty()
                    ) {
                        return Err(tear_off_payload_mismatch(
                            request.source_space(),
                            request.source_tabs(),
                        ));
                    }
                }
                DockViewportDropPayload::Floating(floating) => {
                    if graph
                        .floating_containers(request.source_space())
                        .iter()
                        .all(|container| container.node != *floating)
                    {
                        return Err(DockGraphMutationError::FloatingContainerNotFound {
                            space: request.source_space().clone(),
                            floating: *floating,
                        }
                        .into());
                    }
                    if !matches!(
                        graph.node(request.source_tabs()),
                        Some(DockNode::Tabs { items, .. }) if !items.is_empty()
                    ) || graph
                        .root_for_node_in_space(request.source_space(), request.source_tabs())
                        != Some(*floating)
                    {
                        return Err(tear_off_payload_mismatch(
                            request.source_space(),
                            request.source_tabs(),
                        ));
                    }
                }
            }
        }

        let target_space = self.next_tear_off_space(&request, cx);
        let options = self.tear_off_window_options(&request);
        Ok(DockViewportPreparedTearOffDrop {
            request,
            target_space,
            options,
        })
    }

    pub(crate) fn next_tear_off_space(
        &mut self,
        request: &DockViewportTearOffRequest,
        cx: &App,
    ) -> DockSpaceId {
        loop {
            let tick = self.next_tear_off_tick();
            let space = DockSpaceId::new(format!(
                "{}:tear-off:{}:{}",
                request.source_space(),
                request.payload().label(),
                tick.as_u64()
            ));
            let graph_has_space = self
                .controller
                .read(cx)
                .graph()
                .spaces()
                .iter()
                .any(|known| known == &space);
            if !graph_has_space && self.adapter.window_for_space(&space).is_none() {
                return space;
            }
        }
    }

    pub(crate) fn tear_off_window_options(
        &self,
        request: &DockViewportTearOffRequest,
    ) -> WindowOptions {
        let window_bounds = request
            .suggested_window_bounds()
            .unwrap_or_else(|| default_tear_off_window_bounds(request.release_position()));

        WindowOptions {
            window_bounds: Some(window_bounds),
            ..Default::default()
        }
    }

    #[cfg(test)]
    pub(crate) fn last_host_scene_screen_position(
        &self,
        space: &DockSpaceId,
    ) -> Option<Point<Pixels>> {
        self.host_scenes.screen_position(space)
    }

    #[cfg(test)]
    pub(crate) fn resolve_host_scene_target(
        &self,
        space: &DockSpaceId,
        host_position: Point<Pixels>,
        cx: &App,
    ) -> Option<crate::drop_target::DockResolvedDropTarget> {
        let policy = self.controller.read(cx).workspace().policy().clone();
        self.host_scenes.resolve(space, host_position, &policy)
    }

    /// Resolves a rendered payload release into a runtime route without mutating the graph.
    #[cfg(test)]
    pub(crate) fn resolve_payload_drop_route(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &App,
    ) -> DockViewportDropRoute {
        self.resolve_payload_drop_route_with_commit(request, cx)
            .route()
            .clone()
    }

    /// Resolves a rendered payload release into route and commit facts from one snapshot.
    pub(crate) fn resolve_payload_drop_route_with_commit(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &App,
    ) -> DockViewportResolvedDropRoute {
        let controller = self.controller.read(cx);
        let workspace = controller.workspace();
        let policy = workspace.policy().to_owned();
        let mut route = self.adapter.resolve_payload_drop_route(request, &policy);
        let payload_classes = workspace
            .payload_dock_classes_for_viewport_payload(request.payload(), request.source_tabs());
        let mut resolved_target = match self.resolved_workspace_target_for_route(
            &route,
            request,
            &policy,
            &payload_classes,
        ) {
            DockViewportWorkspaceRouteTarget::Valid(target) => target,
            DockViewportWorkspaceRouteTarget::Rejected(error) => {
                if matches!(route, DockViewportDropRoute::KnownViewport { .. }) {
                    route = DockViewportDropRoute::Rejected(error);
                }
                None
            }
        };
        if !matches!(route, DockViewportDropRoute::KnownViewport { .. })
            && let Some(target) = self.routed_drop_target_hit_for_release(request)
        {
            route = DockViewportDropRoute::KnownViewport { target };
            resolved_target = match self.resolved_workspace_target_for_route(
                &route,
                request,
                &policy,
                &payload_classes,
            ) {
                DockViewportWorkspaceRouteTarget::Valid(target) => target,
                DockViewportWorkspaceRouteTarget::Rejected(error) => {
                    route = DockViewportDropRoute::Rejected(error);
                    None
                }
            };
        }
        self.status.record_route(request, &route);
        let commit = DockViewportDropRouteCommit::from_route_request_with_resolved_target(
            request,
            route.clone(),
            resolved_target,
        );
        DockViewportResolvedDropRoute::new(route, commit)
    }

    fn resolved_workspace_target_for_route(
        &self,
        route: &DockViewportDropRoute,
        request: &DockViewportDropRouteRequest,
        policy: &DockPolicy,
        payload_classes: &crate::workspace_move_validation::DockPayloadDockClasses,
    ) -> DockViewportWorkspaceRouteTarget {
        match route {
            DockViewportDropRoute::Local { host_position } => {
                let target_validator =
                    dock_target_validator(request.source_space(), payload_classes, policy);
                let resolved = self
                    .host_scenes
                    .resolve_frame_for_window(
                        request.source_space(),
                        None,
                        *host_position,
                        policy,
                        Some(&target_validator),
                    )
                    .map(|(frame, resolution)| cached_route_target(frame, resolution));
                DockViewportWorkspaceRouteTarget::Valid(resolved.and_then(Result::ok))
            }
            DockViewportDropRoute::KnownViewport { target } => {
                let target_validator =
                    dock_target_validator(target.space(), payload_classes, policy);
                let Some((frame, resolution)) = self.host_scenes.resolve_frame_for_window(
                    target.space(),
                    Some(target.window_id()),
                    target.host_position(),
                    policy,
                    Some(&target_validator),
                ) else {
                    return DockViewportWorkspaceRouteTarget::Valid(None);
                };
                match cached_route_target(frame, resolution) {
                    Ok(target) => DockViewportWorkspaceRouteTarget::Valid(Some(target)),
                    Err(error) => DockViewportWorkspaceRouteTarget::Rejected(error),
                }
            }
            DockViewportDropRoute::TearOff(_) | DockViewportDropRoute::Rejected(_) => {
                DockViewportWorkspaceRouteTarget::Valid(None)
            }
        }
    }

    pub(crate) fn begin_tear_off_request(
        &mut self,
        request: DockViewportTearOffRequest,
        target_space: impl Into<DockSpaceId>,
        cx: &App,
    ) -> DockViewportTearOffBeginOutcome {
        let focus_item = self.focus_item_for_request(&request, cx);
        let now = self.next_tear_off_tick();
        self.begin_tear_off_request_at(request, target_space, focus_item, now)
    }

    pub(crate) fn begin_tear_off_request_at(
        &mut self,
        request: DockViewportTearOffRequest,
        target_space: impl Into<DockSpaceId>,
        focus_item: Option<DockItemId>,
        now: DockViewportTearOffTick,
    ) -> DockViewportTearOffBeginOutcome {
        self.tear_off
            .begin(request, target_space.into(), focus_item, now)
    }

    pub(crate) fn cancel_tear_off_request(
        &mut self,
        key: &DockViewportTearOffKey,
        reason: DockViewportTearOffCancelReason,
    ) -> Option<DockViewportTearOffCancelled> {
        self.tear_off.cancel(key, reason)
    }

    #[cfg(test)]
    pub(crate) fn expire_tear_off_requests_at(
        &mut self,
        now: DockViewportTearOffTick,
    ) -> Vec<DockViewportTearOffCancelled> {
        self.tear_off.expire(now)
    }

    pub(crate) fn complete_tear_off_viewport(
        &mut self,
        key: &DockViewportTearOffKey,
        window: impl Into<AnyWindowHandle>,
        cx: &mut App,
    ) -> DockViewportTearOffCompletionOutcome {
        let now = self.next_tear_off_tick();
        self.complete_tear_off_viewport_at(key, window, now, cx)
    }

    pub(crate) fn complete_tear_off_viewport_at(
        &mut self,
        key: &DockViewportTearOffKey,
        window: impl Into<AnyWindowHandle>,
        now: DockViewportTearOffTick,
        cx: &mut App,
    ) -> DockViewportTearOffCompletionOutcome {
        let window = window.into();
        let payload = key.payload();
        let readiness = self.prepare_tear_off_completion(key, now, cx);
        let pending = match readiness {
            DockViewportTearOffCompletionPending::Pending(pending) => pending,
            DockViewportTearOffCompletionPending::Cancelled(cancelled) => {
                return DockViewportTearOffCompletionOutcome::Cancelled(cancelled);
            }
            DockViewportTearOffCompletionPending::Missing => {
                return DockViewportTearOffCompletionOutcome::MissingPending { payload };
            }
        };

        let registration = self
            .adapter
            .register_viewport_with_outcome(pending.target_space().clone(), window);
        self.owned_windows.insert(window.window_id());
        for removed in registration.replaced() {
            self.clear_last_hovered_window_if_matches(removed.window.window_id());
            self.discard_owned_window(removed.window.window_id());
            self.host_scenes.unregister_space(&removed.space);
        }
        self.close_gate.sync_adapter(&self.adapter);
        match self.commit_tear_off_move(&pending, cx) {
            Ok(action) => {
                let _ = registration
                    .window()
                    .update(cx, |_, window, _| window.activate_window());
                DockViewportTearOffCompletionOutcome::Completed(DockViewportTearOffCompleted::new(
                    pending,
                    registration,
                    action,
                ))
            }
            Err(error) => {
                self.discard_owned_window(window.window_id());
                self.adapter.unregister_space(pending.target_space());
                self.host_scenes.unregister_space(pending.target_space());
                self.close_gate.sync_adapter(&self.adapter);
                DockViewportTearOffCompletionOutcome::CommitFailed(
                    DockViewportTearOffCommitFailure::new(pending, registration, error),
                )
            }
        }
    }

    pub(crate) fn finish_tear_off_open(
        &mut self,
        pending: DockViewportTearOffPending,
        completion: DockViewportTearOffCompletionOutcome,
        cx: &App,
    ) -> DockViewportTearOffOpenOutcome {
        match completion {
            DockViewportTearOffCompletionOutcome::Completed(completed) => {
                DockViewportTearOffOpenOutcome::Completed(completed)
            }
            DockViewportTearOffCompletionOutcome::Cancelled(cancelled) => {
                self.discard_tear_off_target(pending.target_space());
                DockViewportTearOffOpenOutcome::Cancelled(cancelled)
            }
            DockViewportTearOffCompletionOutcome::MissingPending { .. } => {
                self.discard_tear_off_target(pending.target_space());
                let reason = match self.tear_off_source_status(&pending, cx) {
                    DockViewportTearOffSourceStatus::Ready => {
                        DockViewportTearOffCancelReason::Cancelled
                    }
                    DockViewportTearOffSourceStatus::Missing => {
                        DockViewportTearOffCancelReason::SourceMissing
                    }
                    DockViewportTearOffSourceStatus::Moved => {
                        DockViewportTearOffCancelReason::SourceMoved
                    }
                };
                DockViewportTearOffOpenOutcome::Cancelled(DockViewportTearOffCancelled::new(
                    pending, reason,
                ))
            }
            DockViewportTearOffCompletionOutcome::CommitFailed(failure) => {
                DockViewportTearOffOpenOutcome::CommitFailed(failure)
            }
        }
    }

    fn discard_tear_off_target(&mut self, target_space: &DockSpaceId) {
        if let Some(snapshot) = self.adapter.unregister_space(target_space) {
            self.discard_owned_window(snapshot.window.window_id());
        }
        self.host_scenes.unregister_space(target_space);
        self.close_gate.sync_adapter(&self.adapter);
    }

    fn activate_viewport_for_space(
        &mut self,
        target_space: &DockSpaceId,
        focus_item: Option<DockItemId>,
        cx: &mut App,
    ) -> Option<DockViewportActivationTarget> {
        match self.reusable_window_for_space(target_space, cx) {
            DockViewportReusableWindow::Reused(window) => Some(DockViewportActivationTarget::new(
                target_space.clone(),
                window,
                focus_item,
            )),
            DockViewportReusableWindow::Missing | DockViewportReusableWindow::Stale => None,
        }
    }

    fn focus_item_for_request(
        &self,
        request: &DockViewportTearOffRequest,
        cx: &App,
    ) -> Option<DockItemId> {
        self.focus_item_for_payload(request.payload(), request.source_tabs(), cx)
    }

    fn focus_item_for_payload(
        &self,
        payload: &DockViewportDropPayload,
        source_tabs: crate::DockNodeId,
        cx: &App,
    ) -> Option<DockItemId> {
        match payload {
            DockViewportDropPayload::Item(item) => Some(item.clone()),
            DockViewportDropPayload::Tabs | DockViewportDropPayload::Floating(_) => self
                .controller
                .read(cx)
                .graph()
                .active_item_in_tabs(source_tabs),
        }
    }

    /// Handles a GPUI window-closed notification by removing stale runtime mapping.
    ///
    /// Close policy is applied by [`Self::handle_window_should_close`] before GPUI accepts a close.
    /// Once a closed notification arrives, the platform window is already gone and docking must
    /// discard the runtime mapping even when the current policy is [`DockViewportClosePolicy::Prevent`].
    pub(crate) fn handle_window_closed(&mut self, window_id: WindowId) -> DockViewportCloseOutcome {
        self.clear_last_hovered_window_if_matches(window_id);
        self.discard_owned_window(window_id);
        let outcome = self.adapter.handle_window_closed(window_id);
        self.host_scenes.unregister_window(window_id);
        self.close_gate.sync_adapter(&self.adapter);
        self.status.record_close(&outcome);
        outcome
    }

    /// Handles a GPUI window-closed notification with access to graph mutation context.
    pub(crate) fn handle_window_closed_with_app(
        &mut self,
        window_id: WindowId,
        cx: &mut App,
    ) -> DockViewportCloseOutcome {
        let close_policy = self.close_policy();
        let outcome = self.handle_window_closed(window_id);
        let Some(source_space) = outcome.space().cloned() else {
            return outcome;
        };
        let DockViewportClosePolicy::MergeBack { target_space } = close_policy else {
            return outcome;
        };

        let outcome =
            outcome.with_status(self.merge_closed_space_back(&source_space, &target_space, cx));
        self.status.record_close(&outcome);
        outcome
    }

    pub(crate) fn activation_target_after_close(
        &mut self,
        outcome: &DockViewportCloseOutcome,
        cx: &mut App,
    ) -> Option<DockViewportActivationTarget> {
        if outcome.status() != DockViewportCloseStatus::MergedBack {
            return None;
        }
        let DockViewportClosePolicy::MergeBack { target_space } = self.close_policy() else {
            return None;
        };
        let focus_item = {
            let controller = self.controller.read(cx);
            let graph = controller.graph();
            graph
                .first_tabs_in_space(&target_space)
                .and_then(|tabs| graph.active_item_in_tabs(tabs))
        };
        self.activate_viewport_for_space(&target_space, focus_item, cx)
    }

    /// Handles a GPUI window should-close query by applying this runtime's close policy.
    pub(crate) fn handle_window_should_close(
        &mut self,
        window_id: WindowId,
    ) -> DockViewportShouldCloseOutcome {
        let outcome = self
            .adapter
            .should_close_viewport(window_id, self.close_policy());
        self.status.record_should_close(&outcome);
        outcome
    }

    pub(crate) fn handle_window_should_close_with_app(
        &mut self,
        window_id: WindowId,
        cx: &App,
    ) -> DockViewportShouldCloseOutcome {
        let mut outcome = self
            .adapter
            .should_close_viewport(window_id, self.close_policy());
        if matches!(outcome.status, DockViewportShouldCloseStatus::Allowed)
            && let Some(space) = outcome.space.as_ref()
        {
            let close_policy = self.close_policy();
            let allowed = {
                let controller = self.controller.read(cx);
                let workspace = controller.workspace();
                match &close_policy {
                    DockViewportClosePolicy::RetainLayout => {
                        workspace.validate_close_space(space).is_ok()
                    }
                    DockViewportClosePolicy::MergeBack { target_space } => workspace
                        .validate_merge_space_into(space, target_space)
                        .is_ok(),
                    DockViewportClosePolicy::Prevent => false,
                }
            };
            if !allowed {
                outcome.status = DockViewportShouldCloseStatus::Vetoed;
            }
        }
        self.status.record_should_close(&outcome);
        outcome
    }

    fn merge_closed_space_back(
        &self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        cx: &mut App,
    ) -> DockViewportCloseStatus {
        self.controller
            .update(cx, |controller, cx| {
                let outcome = controller.commit_merge_space_into(source_space, target_space);
                if outcome
                    .as_ref()
                    .map(|outcome| outcome.changed())
                    .unwrap_or(false)
                {
                    cx.notify();
                }
                outcome
            })
            .map(|outcome| {
                if outcome.changed() {
                    DockViewportCloseStatus::MergedBack
                } else {
                    DockViewportCloseStatus::Closed
                }
            })
            .unwrap_or(DockViewportCloseStatus::MergeBackFailed)
    }

    fn next_tear_off_tick(&mut self) -> DockViewportTearOffTick {
        let tick = self.tear_off_tick;
        self.tear_off_tick = self.tear_off_tick.saturating_add(1);
        tick
    }

    fn prepare_tear_off_completion(
        &mut self,
        key: &DockViewportTearOffKey,
        now: DockViewportTearOffTick,
        cx: &App,
    ) -> DockViewportTearOffCompletionPending {
        let Some(pending) = self.tear_off.pending(key).cloned() else {
            return DockViewportTearOffCompletionPending::Missing;
        };
        if pending.is_expired_at(now) {
            return DockViewportTearOffCompletionPending::Cancelled(
                self.tear_off
                    .cancel(key, DockViewportTearOffCancelReason::Expired)
                    .expect("pending payload should still be present"),
            );
        }

        match self.tear_off_source_status(&pending, cx) {
            DockViewportTearOffSourceStatus::Ready => self.tear_off.take_for_completion(key, now),
            DockViewportTearOffSourceStatus::Missing => {
                DockViewportTearOffCompletionPending::Cancelled(
                    self.tear_off
                        .cancel(key, DockViewportTearOffCancelReason::SourceMissing)
                        .expect("pending payload should still be present"),
                )
            }
            DockViewportTearOffSourceStatus::Moved => {
                DockViewportTearOffCompletionPending::Cancelled(
                    self.tear_off
                        .cancel(key, DockViewportTearOffCancelReason::SourceMoved)
                        .expect("pending payload should still be present"),
                )
            }
        }
    }

    fn tear_off_source_status(
        &self,
        pending: &DockViewportTearOffPending,
        cx: &App,
    ) -> DockViewportTearOffSourceStatus {
        let graph = self.controller.read(cx).graph();
        let request = pending.request();
        match request.payload() {
            DockViewportDropPayload::Item(item) => graph
                .find_item_in_space(request.source_space(), item)
                .map(|(tabs, _)| {
                    if tabs == request.source_tabs() {
                        DockViewportTearOffSourceStatus::Ready
                    } else {
                        DockViewportTearOffSourceStatus::Moved
                    }
                })
                .unwrap_or_else(|| {
                    if graph.contains_item(item) {
                        DockViewportTearOffSourceStatus::Moved
                    } else {
                        DockViewportTearOffSourceStatus::Missing
                    }
                }),
            DockViewportDropPayload::Tabs => {
                let source_tabs = request.source_tabs();
                let Some(DockNode::Tabs { items, .. }) = graph.node(source_tabs) else {
                    return DockViewportTearOffSourceStatus::Missing;
                };
                if graph
                    .root_for_node_in_space(request.source_space(), source_tabs)
                    .is_some()
                    && !items.is_empty()
                {
                    DockViewportTearOffSourceStatus::Ready
                } else {
                    DockViewportTearOffSourceStatus::Moved
                }
            }
            DockViewportDropPayload::Floating(floating) => {
                let source_tabs = request.source_tabs();
                let Some(DockNode::Tabs { items, .. }) = graph.node(source_tabs) else {
                    return DockViewportTearOffSourceStatus::Missing;
                };
                if graph
                    .floating_containers(request.source_space())
                    .iter()
                    .all(|container| container.node != *floating)
                {
                    return DockViewportTearOffSourceStatus::Missing;
                }
                if !items.is_empty()
                    && graph.root_for_node_in_space(request.source_space(), source_tabs)
                        == Some(*floating)
                {
                    DockViewportTearOffSourceStatus::Ready
                } else {
                    DockViewportTearOffSourceStatus::Moved
                }
            }
        }
    }

    fn commit_tear_off_move(
        &self,
        pending: &DockViewportTearOffPending,
        cx: &mut App,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.controller.update(cx, |controller, cx| {
            let request = pending.request();
            let outcome = match request.payload() {
                DockViewportDropPayload::Item(item) => controller.commit_item_to_empty_dock_space(
                    request.source_space(),
                    item,
                    pending.target_space(),
                ),
                DockViewportDropPayload::Tabs => controller.commit_tabs_to_empty_dock_space(
                    request.source_space(),
                    request.source_tabs(),
                    pending.target_space(),
                ),
                DockViewportDropPayload::Floating(floating) => controller
                    .commit_floating_to_empty_dock_space(
                        request.source_space(),
                        *floating,
                        pending.target_space(),
                    ),
            };
            if outcome
                .as_ref()
                .map(|outcome| outcome.changed())
                .unwrap_or(false)
            {
                cx.notify();
            }
            outcome
        })
    }

    /// Exports serializable placement snapshots from the adapter.
    pub(crate) fn export_placement(&self) -> DockViewportPlacementLayout {
        self.adapter.export_placement()
    }

    /// Applies saved placement snapshots to registered viewport windows.
    pub(crate) fn apply_placement(
        &mut self,
        placement: &DockViewportPlacementLayout,
    ) -> Result<DockViewportRestoreOutcome, DockViewportPlacementValidationError> {
        self.adapter.apply_placement(placement)
    }
}

fn default_tear_off_window_bounds(release_position: Point<Pixels>) -> WindowBounds {
    WindowBounds::Windowed(Bounds::new(
        release_position - DEFAULT_TEAR_OFF_CURSOR_OFFSET,
        DEFAULT_TEAR_OFF_WINDOW_SIZE,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DockViewportTearOffSourceStatus {
    Ready,
    Missing,
    Moved,
}

pub(crate) enum DockViewportReusableWindow {
    Missing,
    Reused(AnyWindowHandle),
    Stale,
}

fn tear_off_payload_mismatch(
    source_space: &DockSpaceId,
    source_tabs: crate::DockNodeId,
) -> DockActionApplyError {
    DockActionApplyError::DropPayloadMismatch {
        space: source_space.clone(),
        tabs: source_tabs,
    }
}
