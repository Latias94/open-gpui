use super::*;

pub(crate) struct DockViewportLockedDropRoute {
    drag_session: DockRuntimeDragSession,
    kind: DockViewportLockedDropRouteKind,
}

#[must_use = "a prepared live-undock host drop must complete commit preflight"]
#[derive(Clone)]
pub(crate) struct DockViewportPreparedLiveUndockHostDrop {
    drag_session: DockRuntimeDragSession,
    locked: crate::viewport_runtime::DockViewportLockedWorkspaceDrop,
    target_window: AnyWindowHandle,
}

#[must_use = "a preflighted live-undock host drop must commit exactly once"]
#[derive(Clone)]
pub(crate) struct DockViewportPreflightedLiveUndockHostDrop {
    prepared: crate::viewport_runtime::DockViewportPreflightedLockedPayloadDrop,
}

#[must_use = "a committed live-undock host drop must settle its returned window effects"]
#[derive(Clone)]
pub(crate) struct DockViewportCommittedLiveUndockHostDrop {
    workspace: crate::workspace_drop_transaction::DockWorkspaceLockedPayloadDropCommitReceipt,
    outcome: DockViewportDropRouteOutcome,
    runtime_update: DockViewportRuntimeUpdate,
    window_effects: DockViewportCommittedWindowEffects,
    controller: Entity<DockController>,
}

impl DockViewportPreflightedLiveUndockHostDrop {
    pub(crate) fn committed_workspace(
        &self,
        cx: &App,
    ) -> Option<crate::workspace_drop_transaction::DockWorkspaceLockedPayloadDropCommitReceipt>
    {
        self.prepared.committed_workspace(cx)
    }
}

impl DockViewportCommittedLiveUndockHostDrop {
    pub(crate) fn outcome(&self) -> &DockViewportDropRouteOutcome {
        &self.outcome
    }

    pub(crate) fn window_effects_receipt(
        &self,
    ) -> Option<DockViewportCommittedWindowEffectsReceipt> {
        self.window_effects.receipt()
    }

    pub(crate) fn runtime_update(&self) -> &DockViewportRuntimeUpdate {
        &self.runtime_update
    }

    pub(crate) fn workspace_commit(
        &self,
    ) -> &crate::workspace_drop_transaction::DockWorkspaceLockedPayloadDropCommitReceipt {
        &self.workspace
    }

    pub(crate) fn controller(&self) -> &Entity<DockController> {
        &self.controller
    }
}

enum DockViewportLockedDropRouteKind {
    Workspace(crate::viewport_runtime::DockViewportLockedWorkspaceDrop),
    TearOff(DockViewportPreparedTearOffDrop),
}

impl DockViewportLockedDropRoute {
    pub(crate) fn drag_session(&self) -> &DockRuntimeDragSession {
        &self.drag_session
    }

    pub(crate) const fn is_workspace(&self) -> bool {
        matches!(self.kind, DockViewportLockedDropRouteKind::Workspace(_))
    }
}

impl DockViewportRuntimeHandle {
    pub(crate) fn prepare_live_undock_host_drop(
        &self,
        locked: DockViewportLockedDropRoute,
        target_window: AnyWindowHandle,
    ) -> Result<DockViewportPreparedLiveUndockHostDrop, DockActionApplyError> {
        let DockViewportLockedDropRoute { drag_session, kind } = locked;
        let DockViewportLockedDropRouteKind::Workspace(locked) = kind else {
            return Err(DockActionApplyError::DropTargetUnavailable);
        };
        Ok(DockViewportPreparedLiveUndockHostDrop {
            drag_session,
            locked,
            target_window,
        })
    }

    pub(crate) fn preflight_live_undock_host_drop_commit(
        &self,
        prepared: DockViewportPreparedLiveUndockHostDrop,
        cx: &mut App,
    ) -> Result<DockViewportPreflightedLiveUndockHostDrop, DockActionApplyError> {
        let DockViewportPreparedLiveUndockHostDrop {
            drag_session,
            locked,
            target_window,
        } = prepared;
        let prepared =
            self.runtime
                .borrow()
                .prepare_atomic_locked_payload_drop(locked, target_window, None);
        let result = match prepared {
            Ok(prepared) => {
                let sampled = prepared.sample_atomic_locked_payload_drop(cx);
                self.runtime
                    .borrow()
                    .preflight_atomic_locked_payload_drop(sampled, cx)
            }
            Err(error) => Err(error),
        }
        .and_then(|prepared| {
            if prepared.graph_changed() {
                Ok(prepared)
            } else {
                Err(DockActionApplyError::DropTargetUnavailable)
            }
        });
        match result {
            Ok(prepared) => Ok(DockViewportPreflightedLiveUndockHostDrop { prepared }),
            Err(error) => {
                self.runtime
                    .borrow_mut()
                    .record_drop_route_result(&Err(error.clone()));
                let preview_update = self
                    .runtime
                    .borrow_mut()
                    .clear_routed_drop_preview_for_drag_session(Some(&drag_session));
                refresh_runtime_update(preview_update, cx);
                Err(error)
            }
        }
    }

    pub(crate) fn commit_preflighted_live_undock_host_drop(
        &self,
        preflighted: &DockViewportPreflightedLiveUndockHostDrop,
        cx: &mut App,
    ) -> DockViewportCommittedLiveUndockHostDrop {
        let workspace = preflighted.prepared.commit_workspace(cx);
        debug_assert!(
            workspace.outcome().changed(),
            "a preflighted live-undock host drop must change workspace topology",
        );
        let controller = preflighted.prepared.controller().clone();
        let committed = self
            .runtime
            .borrow_mut()
            .commit_preflighted_locked_payload_drop(
                self.identity,
                &preflighted.prepared,
                &workspace,
            );
        let (outcome, runtime_update, window_effects) = committed.into_parts();
        DockViewportCommittedLiveUndockHostDrop {
            workspace,
            outcome,
            runtime_update,
            window_effects,
            controller,
        }
    }

    pub(crate) fn publish_live_undock_host_drop_commit(
        &self,
        committed: &DockViewportCommittedLiveUndockHostDrop,
        cx: &mut App,
    ) {
        self.publish_surface_commit(committed.runtime_update(), cx);
    }

    pub(crate) fn notify_live_undock_host_drop_commit(
        &self,
        committed: &DockViewportCommittedLiveUndockHostDrop,
        cx: &mut App,
    ) {
        cx.update_entity(committed.controller(), |_, controller_cx| {
            controller_cx.notify();
        });
    }

    pub(crate) fn accept_live_undock_host_drop_window_effects(
        &self,
        committed: &DockViewportCommittedLiveUndockHostDrop,
        cx: &mut App,
    ) -> DockViewportCommittedWindowEffectsAcceptanceOutcome {
        let prepared = self
            .runtime
            .borrow()
            .prepare_locked_payload_drop_window_effects_acceptance(
                self.identity,
                committed.workspace_commit().commit_id(),
                &committed.window_effects,
            );
        match prepared {
            DockViewportCommittedWindowEffectsPreparation::Accepted(receipt) => {
                DockViewportCommittedWindowEffectsAcceptanceOutcome::Accepted(receipt)
            }
            DockViewportCommittedWindowEffectsPreparation::InProgress => {
                DockViewportCommittedWindowEffectsAcceptanceOutcome::InProgress
            }
            DockViewportCommittedWindowEffectsPreparation::Transfer(transfer) => {
                DockViewportCommittedWindowEffectsAcceptanceOutcome::Accepted(
                    transfer.accept(&self.runtime, cx),
                )
            }
            DockViewportCommittedWindowEffectsPreparation::Stale => {
                DockViewportCommittedWindowEffectsAcceptanceOutcome::Stale
            }
        }
    }

    pub(crate) fn retire_live_undock_host_drop_commit(
        &self,
        committed: &DockViewportCommittedLiveUndockHostDrop,
        acceptance: DockViewportCommittedWindowEffectsReceipt,
        cx: &mut App,
    ) -> bool {
        let commit_id = committed.workspace_commit().commit_id();
        if !self
            .runtime
            .borrow_mut()
            .begin_retire_locked_payload_drop_commit(
                self.identity,
                commit_id,
                &committed.window_effects,
                acceptance,
            )
        {
            return false;
        }
        committed.controller().update(cx, |controller, _| {
            controller
                .workspace_mut()
                .retire_locked_payload_drop_commit(committed.workspace_commit())
        });
        self.runtime
            .borrow_mut()
            .finish_retire_locked_payload_drop_commit(
                self.identity,
                commit_id,
                &committed.window_effects,
                acceptance,
            )
    }

    pub(crate) fn lock_payload_drop_from_screen(
        &self,
        request: &DockViewportDropRouteRequest,
        borrowed_source_window: AnyWindowHandle,
        cx: &mut App,
    ) -> Result<DockViewportLockedDropRoute, DockActionApplyError> {
        let drag_session = request
            .drag_session()
            .cloned()
            .ok_or(DockActionApplyError::DropDragSessionMissing)?;
        let resolution = self
            .resolve_payload_drop_delivery_for_request_outcome_excluding(
                request,
                Some(borrowed_source_window.window_id()),
                cx,
            )
            .into_resolution();
        let delivery = DockDropDelivery::from_resolution(resolution)?;
        let kind = match delivery.into_tear_off_request() {
            Ok(request) => {
                let controller = self.runtime.borrow().controller_entity();
                let graph_spaces =
                    cx.read_entity(&controller, |controller, _| controller.graph().spaces());
                let probe = self
                    .runtime
                    .borrow_mut()
                    .prepare_tear_off_drop_delivery(request, &graph_spaces)?;
                DockViewportLockedDropRouteKind::TearOff(probe.sample(cx)?)
            }
            Err(delivery) => {
                let controller = self.runtime.borrow().controller_entity();
                let workspace_facts = cx.read_entity(&controller, |controller, _| {
                    crate::DockViewportWorkspaceRouteFacts::capture_for_payload(
                        controller.workspace(),
                        delivery.payload(),
                        delivery.source_node(),
                    )
                });
                let commit = self
                    .runtime
                    .borrow()
                    .resolve_locked_workspace_drop_delivery(delivery, &workspace_facts)?;
                let drag_session = commit
                    .drag_session
                    .clone()
                    .ok_or(DockActionApplyError::DropDragSessionMissing)?;
                let resolved_target = commit.target.clone();
                let plan = cx.read_entity(&controller, |controller, _| {
                    controller.workspace().lock_resolved_payload_drop(
                        &commit.source_space,
                        commit.payload.as_workspace_payload(commit.source_node),
                        resolved_target,
                    )
                })?;
                DockViewportLockedDropRouteKind::Workspace(
                    crate::viewport_runtime::DockViewportLockedWorkspaceDrop::new(
                        plan,
                        drag_session,
                    ),
                )
            }
        };
        Ok(DockViewportLockedDropRoute { drag_session, kind })
    }

    pub(crate) fn commit_locked_payload_drop_from_screen(
        &self,
        locked: DockViewportLockedDropRoute,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let DockViewportLockedDropRoute { drag_session, kind } = locked;
        self.with_surface_transaction(cx, |surface_transaction, cx| {
            let result = match kind {
                DockViewportLockedDropRouteKind::Workspace(locked) => {
                    let prepared = self.runtime.borrow().prepare_locked_payload_drop(
                        locked,
                        None,
                        surface_transaction,
                    );
                    let result =
                        prepared
                            .and_then(|prepared| prepared.apply(cx))
                            .and_then(|applied| {
                                self.runtime.borrow_mut().finalize_payload_drop(applied)
                            });
                    if let Ok((_, update)) = &result {
                        self.publish_surface_commit(update, cx);
                    }
                    if let Err(error) = &result {
                        self.runtime
                            .borrow_mut()
                            .record_drop_route_result(&Err(error.clone()));
                    }
                    result.map(|(outcome, _)| outcome)
                }
                DockViewportLockedDropRouteKind::TearOff(prepared) => {
                    let result = self
                        .open_prepared_tear_off_viewport(prepared, None, cx)
                        .map(DockViewportDropRouteOutcome::tear_off)
                        .map_err(|error| DockActionApplyError::TearOffViewportOpenFailed {
                            message: error.to_string(),
                        });
                    self.runtime.borrow_mut().record_drop_route_result(&result);
                    result
                }
            };
            let preview_update = self
                .runtime
                .borrow_mut()
                .clear_routed_drop_preview_for_drag_session(Some(&drag_session));
            refresh_runtime_update(preview_update, cx);
            if let Ok(DockViewportDropRouteOutcome::Action(outcome)) = &result {
                apply_viewport_window_effects(&self.runtime, outcome.window_effects(), cx);
            }
            result
        })
    }

    pub(crate) fn record_locked_payload_drop_failure(
        &self,
        drag_session: &DockRuntimeDragSession,
        error: DockActionApplyError,
        cx: &mut App,
    ) {
        self.runtime
            .borrow_mut()
            .record_drop_route_result(&Err(error));
        let preview_update = self
            .runtime
            .borrow_mut()
            .clear_routed_drop_preview_for_drag_session(Some(drag_session));
        refresh_runtime_update(preview_update, cx);
    }

    #[cfg(test)]
    pub(crate) fn deliver_drop_commit_delivery(
        &self,
        delivery: DockDropDelivery,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        self.deliver_drop_commit_delivery_with_live_window(delivery, None, cx)
    }

    fn deliver_drop_commit_delivery_with_live_window(
        &self,
        delivery: DockDropDelivery,
        live_window: Option<AnyWindowHandle>,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let excluded_window = live_window.as_ref().map(|window| window.window_id());
        self.with_surface_transaction(cx, |surface_transaction, cx| {
            let drag_session = delivery.drag_session().cloned();
            let result = match delivery.into_tear_off_request() {
                Ok(request) => self.commit_tear_off_drop_route(request, excluded_window, cx),
                Err(delivery) => {
                    let controller = self.runtime.borrow().controller_entity();
                    let workspace_facts = cx.read_entity(&controller, |controller, _| {
                        crate::DockViewportWorkspaceRouteFacts::capture_for_payload(
                            controller.workspace(),
                            delivery.payload(),
                            delivery.source_node(),
                        )
                    });
                    let prepared = {
                        self.runtime.borrow().prepare_payload_drop(
                            delivery,
                            live_window,
                            surface_transaction,
                            &workspace_facts,
                        )
                    };
                    let result =
                        prepared
                            .and_then(|prepared| prepared.apply(cx))
                            .and_then(|applied| {
                                self.runtime.borrow_mut().finalize_payload_drop(applied)
                            });
                    if let Ok((_, update)) = &result {
                        self.publish_surface_commit(update, cx);
                    }
                    if let Err(error) = &result {
                        self.runtime
                            .borrow_mut()
                            .record_drop_route_result(&Err(error.clone()));
                    }
                    result.map(|(outcome, _)| outcome)
                }
            };
            let preview_update = self
                .runtime
                .borrow_mut()
                .clear_routed_drop_preview_for_drag_session(drag_session.as_ref());
            refresh_runtime_update_excluding(preview_update, excluded_window, cx);
            if let Ok(DockViewportDropRouteOutcome::Action(outcome)) = &result {
                apply_viewport_window_effects_excluding(
                    &self.runtime,
                    outcome.window_effects(),
                    excluded_window,
                    cx,
                );
            }
            result
        })
    }

    fn commit_tear_off_drop_route(
        &self,
        request: DockViewportTearOffRequest,
        excluded_window: Option<WindowId>,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let result = (|| {
            let controller = self.runtime.borrow().controller_entity();
            let graph_spaces =
                cx.read_entity(&controller, |controller, _| controller.graph().spaces());
            let probe = {
                let mut runtime = self.runtime.borrow_mut();
                runtime.prepare_tear_off_drop_delivery(request, &graph_spaces)?
            };
            let prepared = probe.sample(cx)?;

            self.open_prepared_tear_off_viewport(prepared, excluded_window, cx)
                .map(DockViewportDropRouteOutcome::tear_off)
                .map_err(|error| DockActionApplyError::TearOffViewportOpenFailed {
                    message: error.to_string(),
                })
        })();
        self.runtime.borrow_mut().record_drop_route_result(&result);
        result
    }

    #[cfg(test)]
    pub(crate) fn commit_tear_off_drop_route_for_test(
        &self,
        request: DockViewportTearOffRequest,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        self.commit_tear_off_drop_route(request, None, cx)
    }

    #[cfg(test)]
    pub(crate) fn last_host_scene_screen_position(
        &self,
        space: &DockSpaceId,
    ) -> Option<Point<Pixels>> {
        self.runtime.borrow().last_host_scene_screen_position(space)
    }

    #[cfg(test)]
    pub(crate) fn resolve_host_scene_target(
        &self,
        space: &DockSpaceId,
        host_position: Point<Pixels>,
        cx: &App,
    ) -> Option<crate::drop_target::DockResolvedDropTarget> {
        let controller = self.runtime.borrow().controller_entity();
        let policy = cx.read_entity(&controller, |controller, _| {
            controller.workspace().policy().clone()
        });
        self.runtime
            .borrow()
            .resolve_host_scene_target(space, host_position, &policy)
    }

    #[cfg(test)]
    pub(crate) fn begin_tear_off_request_for_test(
        &self,
        request: DockViewportTearOffRequest,
        target_space: impl Into<DockSpaceId>,
        cx: &App,
    ) -> crate::viewport_tear_off::DockViewportTearOffBeginOutcome {
        let controller = self.runtime.borrow().controller_entity();
        let focus_item = cx.read_entity(&controller, |controller, _| {
            controller
                .workspace()
                .activation_focus_item_for_viewport_payload(
                    request.payload(),
                    request.source_node(),
                    request
                        .drag_session()
                        .and_then(DockRuntimeDragSession::focus_item),
                )
        });
        self.runtime.borrow_mut().begin_tear_off_request_with_focus(
            request,
            target_space,
            focus_item,
        )
    }

    #[cfg(test)]
    pub(crate) fn resolve_payload_drop_delivery<C: open_gpui::AppContext>(
        &self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportResolvedDropRoute {
        self.resolve_payload_drop_delivery_for_request(request, cx)
    }

    #[cfg(test)]
    pub(crate) fn resolve_payload_drop_delivery_for_request<C: open_gpui::AppContext>(
        &self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportResolvedDropRoute {
        self.resolve_payload_drop_delivery_for_request_outcome(request, cx)
            .into_resolution()
    }

    #[cfg(test)]
    pub(crate) fn resolve_payload_drop_delivery_for_request_outcome<C: open_gpui::AppContext>(
        &self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportResolvedDropRouteOutcome {
        self.resolve_payload_drop_delivery_for_request_outcome_excluding(
            request,
            request.frame_sampling_exclusion_window(),
            cx,
        )
    }

    fn resolve_payload_drop_delivery_for_request_outcome_excluding<C: open_gpui::AppContext>(
        &self,
        request: &DockViewportDropRouteRequest,
        excluded_window: Option<WindowId>,
        cx: &mut C,
    ) -> DockViewportResolvedDropRouteOutcome {
        let frame_changed = self.reconcile_viewport_frame_skipping(excluded_window, cx);
        let prepared = self
            .runtime
            .borrow()
            .prepare_payload_drop_route_resolution(request, frame_changed);
        let sampled = prepared.sample(cx);
        let refresh = self
            .runtime
            .borrow_mut()
            .finalize_payload_drop_route_resolution(sampled);
        self.settle_backend_focus_cancellations(cx);
        refresh_viewport_window_effects_excluding(refresh.window_effects(), excluded_window, cx);
        refresh.outcome
    }

    pub(crate) fn resolve_and_update_host_routed_drop_preview<C: open_gpui::AppContext>(
        &self,
        request: &DockViewportDropRouteRequest,
        payload: &DockDragPayload,
        host_space: DockSpaceId,
        host_window_id: WindowId,
        host_position: Point<Pixels>,
        cx: &mut C,
    ) -> bool {
        self.resolve_and_update_routed_drop_preview_inner(
            request,
            payload,
            Some((host_space, host_window_id, host_position)),
            cx,
        )
        .1
    }

    pub(crate) fn resolve_and_update_routed_drop_preview(
        &self,
        request: &DockViewportDropRouteRequest,
        payload: &DockDragPayload,
        cx: &mut App,
    ) -> (DockViewportResolvedDropRoute, bool) {
        self.resolve_and_update_routed_drop_preview_inner(request, payload, None, cx)
    }

    pub(crate) fn resolve_and_project_captured_native_foreign_surface_preview(
        &self,
        request: &DockViewportDropRouteRequest,
        owner: &crate::DockViewportRoutedPreviewOwner,
        cx: &mut App,
    ) -> bool {
        let Some((source_runtime, _, _, _)) = owner.captured_native_parts() else {
            return false;
        };
        if source_runtime == self.identity() || !owner.is_current() {
            self.clear_routed_drop_preview_for_owner(owner, cx);
            return false;
        }

        self.reconcile_viewport_frame_skipping(request.frame_sampling_exclusion_window(), cx);
        if !owner.is_current() {
            self.clear_routed_drop_preview_for_owner(owner, cx);
            return false;
        }
        let (current, update) = self
            .runtime
            .borrow_mut()
            .update_captured_native_foreign_surface_preview(request, owner);
        refresh_runtime_update(update, cx);
        current
    }

    pub(crate) fn project_captured_native_source_foreign_surface_preview(
        &self,
        request: &DockViewportDropRouteRequest,
        owner: &crate::DockViewportRoutedPreviewOwner,
        payload: &DockDragPayload,
        source_window: WindowId,
        source_frame: &DockViewportHostSceneFrame,
        host_position: Point<Pixels>,
        cx: &mut App,
    ) -> bool {
        let Some((source_runtime, _, _, _)) = owner.captured_native_parts() else {
            return false;
        };
        if source_runtime != self.identity() || !owner.is_current() {
            self.clear_routed_drop_preview_for_owner(owner, cx);
            return false;
        }
        let (current, update) = self
            .runtime
            .borrow_mut()
            .update_captured_native_source_foreign_surface_preview(
                request,
                owner,
                payload,
                source_window,
                source_frame,
                host_position,
            );
        refresh_runtime_update(update, cx);
        current
    }

    pub(crate) fn record_captured_native_source_foreign_surface_feedback(
        &self,
        request: &DockViewportDropRouteRequest,
        owner: &crate::DockViewportRoutedPreviewOwner,
        payload: &DockDragPayload,
    ) -> bool {
        let Some((source_runtime, _, _, _)) = owner.captured_native_parts() else {
            return false;
        };
        source_runtime == self.identity()
            && self
                .runtime
                .borrow_mut()
                .record_captured_native_source_foreign_surface_feedback(request, owner, payload)
    }

    pub(crate) fn record_captured_native_foreign_surface_terminal(
        &self,
        request: &DockViewportDropRouteRequest,
        owner: &crate::DockViewportRoutedPreviewOwner,
        payload: &DockDragPayload,
    ) -> bool {
        let Some((source_runtime, _, _, _)) = owner.captured_native_parts() else {
            return false;
        };
        source_runtime == self.identity()
            && self
                .runtime
                .borrow_mut()
                .record_captured_native_foreign_surface_terminal(request, owner, payload)
    }

    pub(crate) fn record_captured_native_unavailable_terminal(
        &self,
        request: &DockViewportDropRouteRequest,
        owner: &crate::DockViewportRoutedPreviewOwner,
        payload: &DockDragPayload,
    ) -> bool {
        let Some((source_runtime, _, _, _)) = owner.captured_native_parts() else {
            return false;
        };
        source_runtime == self.identity()
            && self
                .runtime
                .borrow_mut()
                .record_captured_native_unavailable_terminal(request, owner, payload)
    }

    fn resolve_and_update_routed_drop_preview_inner<C: open_gpui::AppContext>(
        &self,
        request: &DockViewportDropRouteRequest,
        payload: &DockDragPayload,
        host: Option<(DockSpaceId, WindowId, Point<Pixels>)>,
        cx: &mut C,
    ) -> (DockViewportResolvedDropRoute, bool) {
        let excluded_window = request.frame_sampling_exclusion_window();
        let preview_excluded_window = host.as_ref().map(|(_, window_id, _)| *window_id);
        let frame_changed = self.reconcile_viewport_frame_skipping(excluded_window, cx);
        let prepared = self
            .runtime
            .borrow()
            .prepare_payload_drop_route_resolution(request, frame_changed);
        let sampled = prepared.sample(cx);
        let (resolution, route_changed, route_effects, preview_update) = {
            let mut runtime = self.runtime.borrow_mut();
            let refresh = runtime.finalize_payload_drop_route_resolution(sampled);
            let resolution = refresh.outcome.resolution().clone();
            let route_changed = refresh.outcome.changed();
            let preview_update = match host {
                Some((host_space, host_window_id, host_position)) => runtime
                    .update_host_routed_drop_preview(
                        &resolution,
                        payload,
                        host_space,
                        host_window_id,
                        host_position,
                    ),
                None => runtime.update_routed_drop_preview(&resolution, payload),
            };
            (
                resolution,
                route_changed,
                refresh.window_effects(),
                preview_update,
            )
        };
        self.settle_backend_focus_cancellations(cx);
        refresh_viewport_window_effects_excluding(route_effects, excluded_window, cx);
        let preview_changed =
            refresh_runtime_update_excluding(preview_update, preview_excluded_window, cx);
        (resolution, route_changed || preview_changed)
    }

    #[cfg(test)]
    pub(crate) fn update_routed_drop_preview(
        &self,
        resolution: &DockViewportResolvedDropRoute,
        payload: &DockDragPayload,
        cx: &mut App,
    ) -> bool {
        let update = self
            .runtime
            .borrow_mut()
            .update_routed_drop_preview(resolution, payload);
        refresh_runtime_update(update, cx)
    }

    #[cfg(test)]
    pub(crate) fn update_host_routed_drop_preview(
        &self,
        resolution: &DockViewportResolvedDropRoute,
        payload: &DockDragPayload,
        host_space: DockSpaceId,
        host_window_id: WindowId,
        host_position: Point<Pixels>,
        cx: &mut App,
    ) -> bool {
        let update = self.runtime.borrow_mut().update_host_routed_drop_preview(
            resolution,
            payload,
            host_space,
            host_window_id,
            host_position,
        );
        refresh_runtime_update_excluding(update, Some(host_window_id), cx)
    }

    pub(crate) fn clear_routed_drop_preview(&self, cx: &mut App) -> bool {
        self.clear_routed_drop_preview_excluding(None, cx)
    }

    pub(crate) fn clear_routed_drop_preview_for_owner(
        &self,
        owner: &crate::DockViewportRoutedPreviewOwner,
        cx: &mut App,
    ) -> bool {
        let update = self
            .runtime
            .borrow_mut()
            .clear_routed_drop_preview_for_owner(owner);
        refresh_runtime_update(update, cx)
    }

    pub(crate) fn clear_routed_drop_preview_for_target_scene_frame(
        &self,
        frame: &DockViewportHostSceneFrame,
        cx: &mut App,
    ) -> bool {
        let update = self
            .runtime
            .borrow_mut()
            .clear_routed_drop_preview_for_target_scene_frame(frame);
        refresh_runtime_update(update, cx)
    }

    pub(crate) fn clear_routed_drop_preview_from_window(
        &self,
        window: &Window,
        cx: &mut App,
    ) -> bool {
        self.clear_routed_drop_preview_excluding(Some(window.window_handle().window_id()), cx)
    }

    fn clear_routed_drop_preview_excluding(
        &self,
        excluded_window: Option<WindowId>,
        cx: &mut App,
    ) -> bool {
        let update = self.runtime.borrow_mut().clear_routed_drop_preview();
        refresh_runtime_update_excluding(update, excluded_window, cx)
    }

    pub(crate) fn prepare_empty_payload_drop_source_vacate(
        &self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
    ) -> crate::viewport_runtime::DockViewportPreparedSourceVacate {
        self.runtime
            .borrow()
            .prepare_empty_payload_drop_source_vacate(source_space, target_space)
    }

    #[cfg(test)]
    pub(crate) fn finalize_empty_payload_drop_source_vacate(
        &self,
        applied: crate::viewport_runtime::DockViewportAppliedSourceVacate,
        cx: &mut App,
    ) -> bool {
        self.finalize_empty_payload_drop_source_vacate_with_transaction_excluding(
            applied,
            self.active_surface_transaction.get(),
            None,
            cx,
        )
    }

    pub(crate) fn finalize_empty_payload_drop_source_vacate_from_window(
        &self,
        applied: crate::viewport_runtime::DockViewportAppliedSourceVacate,
        window: &Window,
        cx: &mut App,
    ) -> bool {
        self.finalize_empty_payload_drop_source_vacate_with_transaction_excluding(
            applied,
            self.active_surface_transaction.get(),
            Some(window.window_handle().window_id()),
            cx,
        )
    }

    pub(crate) fn finalize_empty_payload_drop_source_vacate_with_transaction_from_window(
        &self,
        applied: crate::viewport_runtime::DockViewportAppliedSourceVacate,
        surface_transaction: Option<DockSurfaceTransactionId>,
        window: &Window,
        cx: &mut App,
    ) -> bool {
        self.finalize_empty_payload_drop_source_vacate_with_transaction_excluding(
            applied,
            surface_transaction,
            Some(window.window_handle().window_id()),
            cx,
        )
    }

    fn finalize_empty_payload_drop_source_vacate_with_transaction_excluding(
        &self,
        applied: crate::viewport_runtime::DockViewportAppliedSourceVacate,
        surface_transaction: Option<DockSurfaceTransactionId>,
        excluded_window: Option<WindowId>,
        cx: &mut App,
    ) -> bool {
        let work_context = self
            .runtime
            .borrow()
            .current_work_context(surface_transaction);
        let (effects, topology_changed) = self
            .runtime
            .borrow_mut()
            .finalize_empty_payload_drop_source_vacate(applied);
        let mut update = DockViewportRuntimeUpdate::default();
        if let Some(work_context) = work_context {
            update.mark_viewport_topology(topology_changed, work_context);
        }
        self.publish_surface_commit(&update, cx);
        apply_viewport_window_effects_excluding(&self.runtime, effects, excluded_window, cx);
        topology_changed
    }

    pub(crate) fn routed_drop_preview_for(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<DockViewportRoutedDropPreview> {
        self.runtime
            .borrow()
            .routed_drop_preview_for(space, window_id)
    }

    pub(crate) fn routed_drop_route_preview_for(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<crate::drop_preview::DockDropRoutePreview> {
        self.runtime
            .borrow()
            .routed_drop_route_preview_for(space, window_id)
    }

    #[cfg(test)]
    pub(crate) fn has_routed_drop_preview_for_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> bool {
        self.runtime
            .borrow()
            .has_routed_drop_preview_for_drag_session(session)
    }

    #[cfg(test)]
    pub(crate) fn last_routed_viewport_identity_for_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<crate::DockViewportIdentity> {
        self.runtime
            .borrow()
            .last_routed_viewport_identity_for_drag_session(session)
    }

    #[cfg(test)]
    pub(crate) fn resolve_payload_drop_route_for_test(
        &self,
        request: &DockViewportDropRouteRequest,
        cx: &mut App,
    ) -> DockViewportDropRoute {
        self.reconcile_viewport_frame(cx);
        let controller = self.runtime.borrow().controller_entity();
        let policy = cx.read_entity(&controller, |controller, _| {
            controller.workspace().policy().clone()
        });
        self.runtime
            .borrow()
            .resolve_payload_drop_route_for_test(request, &policy)
    }

    /// Resolves and commits a rendered payload release from a screen-space point.
    pub(crate) fn commit_payload_drop_from_screen(
        &self,
        request: &DockViewportDropRouteRequest,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        self.commit_payload_drop_from_screen_with_live_window(request, None, cx)
    }

    pub(crate) fn commit_payload_drop_from_window(
        &self,
        request: &DockViewportDropRouteRequest,
        window: &Window,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        self.commit_payload_drop_from_screen_with_live_window(
            request,
            Some(window.window_handle()),
            cx,
        )
    }

    fn commit_payload_drop_from_screen_with_live_window(
        &self,
        request: &DockViewportDropRouteRequest,
        live_window: Option<AnyWindowHandle>,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let live_window_id = live_window.as_ref().map(|window| window.window_id());
        let request_exclusion = request.frame_sampling_exclusion_window();
        debug_assert!(
            live_window_id.is_none()
                || request_exclusion.is_none()
                || live_window_id == request_exclusion,
            "a from-window drop request must not exclude a different current window"
        );
        let resolution = self
            .resolve_payload_drop_delivery_for_request_outcome_excluding(
                request,
                live_window_id.or(request_exclusion),
                cx,
            )
            .into_resolution();
        let delivery = match DockDropDelivery::from_resolution(resolution) {
            Ok(delivery) => delivery,
            Err(error) => {
                let result = Err(error);
                self.runtime.borrow_mut().record_drop_route_result(&result);
                return result;
            }
        };
        self.deliver_drop_commit_delivery_with_live_window(delivery, live_window, cx)
    }

    /// Resolves and commits a rendered payload release from platform signal snapshots in tests.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn commit_payload_drop_from_screen_with_platform_signals(
        &self,
        source_space: impl Into<DockSpaceId>,
        source_tabs: DockNodeId,
        payload: DockViewportDropPayload,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        platform_signals: DockViewportPlatformSignals,
        release_origin: DockPayloadDropReleaseOrigin,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source_space,
            source_tabs,
            payload,
            release_position,
            suggested_window_bounds,
            platform_signals,
            release_origin,
        );
        self.commit_payload_drop_from_screen(&request, cx)
    }
}
