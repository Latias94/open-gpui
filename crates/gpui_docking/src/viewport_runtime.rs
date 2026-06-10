use crate::{
    DockActionApplyError, DockActionOutcome, DockController, DockItemId, DockNode, DockSpaceId,
    DockViewportActivationTarget, DockViewportAdapter, DockViewportCloseOutcome,
    DockViewportClosePolicy, DockViewportCloseStatus, DockViewportDropActionOutcome,
    DockViewportDropPayload, DockViewportDropRoute, DockViewportDropRouteCommit,
    DockViewportDropRouteOutcome, DockViewportDropRouteRequest, DockViewportOpenOutcome,
    DockViewportPlacementLayout, DockViewportPlacementValidationError, DockViewportRestoreOutcome,
    DockViewportRuntimeHandle, DockViewportRuntimeStatus, DockViewportShouldCloseOutcome,
    DockViewportTearOffBeginOutcome, DockViewportTearOffCancelReason, DockViewportTearOffCancelled,
    DockViewportTearOffCommitFailure, DockViewportTearOffCompleted,
    DockViewportTearOffCompletionOutcome, DockViewportTearOffCompletionPending,
    DockViewportTearOffKey, DockViewportTearOffMachine, DockViewportTearOffOpenOutcome,
    DockViewportTearOffPending, DockViewportTearOffRequest, DockViewportTearOffTick,
    drop_runtime::DockHostDropSceneFact,
    drop_target::DockResolvedDropTarget,
    viewport_close_gate::DockViewportCloseGate,
    viewport_drop_scene::{
        DockViewportHostSceneFrame, DockViewportHostSceneRegistration,
        DockViewportHostSceneRegistry, DockViewportHostSceneSnapshot,
    },
    workspace_transaction::DockWorkspacePayloadDropRequest,
};
use open_gpui::{
    AnyWindowHandle, App, Bounds, DisplayId, Entity, Pixels, Point, Result, WindowBounds, WindowId,
    WindowOptions, px, size,
};
use std::rc::Rc;

/// Internal owner for controller-backed platform viewport lifecycle.
///
/// The runtime keeps the shared [`DockController`] together with the low-level
/// [`DockViewportAdapter`] so the handle does not have to pass the controller into every open call
/// or duplicate close-callback cleanup logic. The adapter remains the place for window mappings,
/// coordinate snapshots, and placement import/export.
#[derive(Debug)]
pub(crate) struct DockViewportRuntime {
    controller: Entity<DockController>,
    adapter: DockViewportAdapter,
    close_gate: DockViewportCloseGate,
    host_scenes: DockViewportHostSceneRegistry,
    tear_off: DockViewportTearOffMachine,
    tear_off_tick: DockViewportTearOffTick,
    status: DockViewportRuntimeStatus,
}

pub(crate) struct DockViewportPreparedTearOffDrop {
    pub(crate) request: DockViewportTearOffRequest,
    pub(crate) target_space: DockSpaceId,
    pub(crate) options: WindowOptions,
}

fn install_should_close_hook(
    window: AnyWindowHandle,
    cx: &mut App,
    should_close: Rc<dyn Fn(WindowId) -> bool>,
) -> Result<()> {
    let window_id = window.window_id();
    window.update(cx, move |_, window, cx| {
        window.on_window_should_close(cx, move |_, _| should_close(window_id));
    })
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

    /// Returns the close policy used by [`handle_window_should_close`](Self::handle_window_should_close).
    pub(crate) fn close_policy(&self) -> DockViewportClosePolicy {
        self.close_gate.close_policy()
    }

    /// Replaces the close policy used by [`handle_window_should_close`](Self::handle_window_should_close).
    pub(crate) fn set_close_policy(&mut self, close_policy: DockViewportClosePolicy) {
        self.close_gate.set_close_policy(close_policy);
    }

    /// Opens or reuses a controller-backed viewport window for a logical dock space.
    ///
    /// Runtime-opened windows install a GPUI should-close hook so
    /// [`DockViewportClosePolicy::Prevent`] can veto a platform close before
    /// [`Self::handle_window_closed`] performs post-close cleanup.
    pub(crate) fn open_viewport(
        &mut self,
        space: impl Into<DockSpaceId>,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<DockViewportOpenOutcome> {
        let close_gate = self.close_gate.clone();
        self.open_viewport_with_should_close(space, options, cx, move |window_id| {
            close_gate.should_allow_close(window_id)
        })
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
        display_id: Option<DisplayId>,
        window_bounds: WindowBounds,
        host_bounds: Bounds<Pixels>,
    ) -> bool {
        self.adapter
            .update_snapshot(space, display_id, window_bounds, host_bounds)
    }

    #[cfg(test)]
    pub(crate) fn begin_viewport_host_scene(
        &mut self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_bounds: WindowBounds,
        host_bounds: Bounds<Pixels>,
        host_position: Point<Pixels>,
    ) -> bool {
        self.begin_viewport_host_scene_frame(
            space,
            window_id,
            window_bounds,
            host_bounds,
            host_position,
        )
        .is_some_and(|registration| registration.changed)
    }

    pub(crate) fn begin_viewport_host_scene_frame(
        &mut self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_bounds: WindowBounds,
        host_bounds: Bounds<Pixels>,
        host_position: Point<Pixels>,
    ) -> Option<DockViewportHostSceneRegistration> {
        let space = space.into();
        let Some(window) = self.adapter.window_for_space(&space) else {
            return None;
        };
        if window.window_id() != window_id {
            return None;
        }
        let display_id = self
            .adapter
            .snapshot(&space)
            .and_then(|snapshot| snapshot.display_id);
        let changed = self.update_viewport_snapshot(&space, display_id, window_bounds, host_bounds);
        let mut registration = self
            .host_scenes
            .register(DockViewportHostSceneSnapshot::new(
                space,
                window_id,
                window_bounds,
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
    ) -> bool {
        self.host_scenes.push_frame_fact(frame, fact)
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

        self.adapter.unregister_space(space);
        self.host_scenes.unregister_space(space);
        self.close_gate.sync_adapter(&self.adapter);
        DockViewportReusableWindow::Stale
    }

    pub(crate) fn register_opened_viewport(&mut self, space: DockSpaceId, window: AnyWindowHandle) {
        let replaced = self.adapter.register_viewport_with_outcome(space, window);
        for removed in replaced.replaced {
            self.host_scenes.unregister_space(&removed.space);
        }
        self.close_gate.sync_adapter(&self.adapter);
    }

    pub(crate) fn discard_failed_opened_viewport(&mut self, window_id: WindowId) {
        self.adapter.handle_window_closed(window_id);
        self.host_scenes.unregister_window(window_id);
        self.close_gate.sync_adapter(&self.adapter);
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
                let (source_space, source_tabs, payload, route_space, host_position) =
                    commit.into_parts();
                let target_space = self.resolve_route_target(&route_space, host_position, cx)?;
                (source_space, source_tabs, payload, target_space)
            }
            DockViewportDropRouteCommit::TearOff(request) => {
                return self.commit_tear_off_drop_route(request, cx);
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
            DockViewportDropActionOutcome { action, activation },
        ))
    }

    fn resolve_route_target(
        &self,
        target_space: &DockSpaceId,
        host_position: Point<Pixels>,
        cx: &App,
    ) -> Result<(DockSpaceId, DockResolvedDropTarget), DockActionApplyError> {
        let policy = *self.controller.read(cx).workspace().policy();
        let Some(target) = self
            .host_scenes
            .resolve(target_space, host_position, &policy)
        else {
            return Err(DockActionApplyError::DropTargetUnavailable);
        };
        Ok((target_space.clone(), target))
    }

    pub(crate) fn commit_tear_off_drop_route(
        &mut self,
        request: DockViewportTearOffRequest,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        if let Some(outcome) = self.single_viewport_outside_release_noop(
            request.source_space(),
            request.source_tabs(),
            request.payload(),
            cx,
        ) {
            return Ok(outcome);
        }

        let prepared = self.prepare_tear_off_drop_route(request, cx)?;
        self.open_tear_off_viewport(
            prepared.request,
            prepared.target_space,
            prepared.options,
            cx,
        )
        .map(DockViewportDropRouteOutcome::TearOff)
        .map_err(|error| DockActionApplyError::TearOffViewportOpenFailed {
            message: error.to_string(),
        })
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
            }
        }

        let target_space = self.next_tear_off_space(&request);
        let options = self.tear_off_window_options(&request);
        Ok(DockViewportPreparedTearOffDrop {
            request,
            target_space,
            options,
        })
    }

    pub(crate) fn single_viewport_outside_release_noop(
        &mut self,
        source_space: &DockSpaceId,
        source_tabs: crate::DockNodeId,
        payload: &DockViewportDropPayload,
        cx: &mut App,
    ) -> Option<DockViewportDropRouteOutcome> {
        // A secondary viewport whose root payload already fills the whole window is the window.
        // Until GPUI exposes platform-window dragging here, outside release should not spawn a
        // replacement viewport and leave the source window empty.
        if !self.payload_covers_entire_secondary_viewport(source_space, source_tabs, payload, cx) {
            return None;
        }

        Some(DockViewportDropRouteOutcome::Action(
            DockViewportDropActionOutcome {
                action: DockActionOutcome::Unchanged,
                activation: None,
            },
        ))
    }

    fn payload_covers_entire_secondary_viewport(
        &self,
        source_space: &DockSpaceId,
        source_tabs: crate::DockNodeId,
        payload: &DockViewportDropPayload,
        cx: &App,
    ) -> bool {
        let controller = self.controller.read(cx);
        if source_space == controller.space()
            || self.adapter.window_for_space(source_space).is_none()
        {
            return false;
        }

        let graph = controller.graph();
        if graph.root(source_space) != Some(source_tabs) {
            return false;
        }

        let Some(DockNode::Tabs { items, .. }) = graph.node(source_tabs) else {
            return false;
        };
        let payload_covers_root = match payload {
            DockViewportDropPayload::Item(item) => items.len() == 1 && items.first() == Some(item),
            DockViewportDropPayload::Tabs => !items.is_empty(),
        };

        payload_covers_root && graph.collect_items_in_space(source_space).len() == items.len()
    }

    pub(crate) fn next_tear_off_space(
        &mut self,
        request: &DockViewportTearOffRequest,
    ) -> DockSpaceId {
        let tick = self.next_tear_off_tick();
        DockSpaceId::new(format!(
            "{}:tear-off:{}:{}",
            request.source_space(),
            request.payload().label(),
            tick.as_u64()
        ))
    }

    pub(crate) fn tear_off_window_options(
        &self,
        request: &DockViewportTearOffRequest,
    ) -> WindowOptions {
        let window_bounds = request.suggested_window_bounds().unwrap_or_else(|| {
            WindowBounds::Windowed(Bounds::new(
                request.release_position(),
                size(px(360.0), px(240.0)),
            ))
        });

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
        let policy = *self.controller.read(cx).workspace().policy();
        self.host_scenes.resolve(space, host_position, &policy)
    }

    /// Resolves a rendered payload release into a runtime route without mutating the graph.
    pub(crate) fn resolve_payload_drop_route(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &App,
    ) -> DockViewportDropRoute {
        let policy = self.controller.read(cx).workspace().policy().to_owned();
        let route = self.adapter.resolve_payload_drop_route(request, &policy);
        self.status.record_route(request, &route);
        route
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
        self.close_gate.sync_adapter(&self.adapter);
        match self.commit_tear_off_move(&pending, cx) {
            Ok(action) => {
                let _ = registration
                    .window
                    .update(cx, |_, window, _| window.activate_window());
                DockViewportTearOffCompletionOutcome::Completed(DockViewportTearOffCompleted {
                    pending,
                    registration,
                    action,
                })
            }
            Err(error) => {
                self.adapter.unregister_space(pending.target_space());
                self.host_scenes.unregister_space(pending.target_space());
                self.close_gate.sync_adapter(&self.adapter);
                DockViewportTearOffCompletionOutcome::CommitFailed(
                    DockViewportTearOffCommitFailure {
                        pending,
                        registration,
                        error,
                    },
                )
            }
        }
    }

    /// Opens a controller-backed viewport window and completes a tear-off transaction.
    ///
    /// The graph is not mutated until the destination viewport has opened and registered
    /// successfully. Duplicate requests for the same item are idempotent and do not open another
    /// window.
    pub(crate) fn open_tear_off_viewport(
        &mut self,
        request: DockViewportTearOffRequest,
        target_space: impl Into<DockSpaceId>,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<DockViewportTearOffOpenOutcome> {
        let key = request.key();
        let begin = self.begin_tear_off_request(request, target_space, cx);
        let pending = match begin {
            DockViewportTearOffBeginOutcome::Pending(pending) => pending,
            DockViewportTearOffBeginOutcome::Duplicate(pending) => {
                let outcome = DockViewportTearOffOpenOutcome::Duplicate(pending);
                self.status.record_tear_off(&outcome);
                return Ok(outcome);
            }
        };

        let opened = match self.open_viewport(pending.target_space().clone(), options, cx) {
            Ok(opened) => opened,
            Err(error) => {
                self.tear_off
                    .cancel(&key, DockViewportTearOffCancelReason::Cancelled);
                return Err(error);
            }
        };

        let completion = self.complete_tear_off_viewport(&key, opened.window, cx);
        let outcome = self.finish_tear_off_open(pending, completion, cx);
        self.status.record_tear_off(&outcome);
        Ok(outcome)
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
                DockViewportTearOffOpenOutcome::Cancelled(DockViewportTearOffCancelled {
                    pending,
                    reason,
                })
            }
            DockViewportTearOffCompletionOutcome::CommitFailed(failure) => {
                DockViewportTearOffOpenOutcome::CommitFailed(failure)
            }
        }
    }

    fn discard_tear_off_target(&mut self, target_space: &DockSpaceId) {
        self.adapter.unregister_space(target_space);
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
            DockViewportReusableWindow::Reused(window) => Some(DockViewportActivationTarget {
                space: target_space.clone(),
                window,
                focus_item,
            }),
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
            DockViewportDropPayload::Tabs => self
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
        let mut outcome = self.handle_window_closed(window_id);
        let Some(source_space) = outcome.space.clone() else {
            return outcome;
        };
        let DockViewportClosePolicy::MergeBack { target_space } = close_policy else {
            return outcome;
        };

        outcome.status = self.merge_closed_space_back(&source_space, &target_space, cx);
        self.status.record_close(&outcome);
        outcome
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

    pub(crate) fn open_viewport_with_should_close(
        &mut self,
        space: impl Into<DockSpaceId>,
        options: WindowOptions,
        cx: &mut App,
        should_close: impl Fn(WindowId) -> bool + 'static,
    ) -> Result<DockViewportOpenOutcome> {
        let should_close = Rc::new(should_close);
        let outcome = self
            .adapter
            .open_viewport(self.controller.clone(), space, options, cx);
        self.close_gate.sync_adapter(&self.adapter);
        let outcome = outcome?;
        if !matches!(outcome.status, crate::DockViewportOpenStatus::Reused) {
            self.host_scenes.unregister_space(&outcome.space);
        }
        if let Err(error) = install_should_close_hook(outcome.window, cx, should_close) {
            self.adapter
                .handle_window_closed(outcome.window.window_id());
            self.host_scenes
                .unregister_window(outcome.window.window_id());
            self.close_gate.sync_adapter(&self.adapter);
            return Err(error);
        }

        Ok(outcome)
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
