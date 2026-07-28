use super::*;

impl DockViewportRuntimeHandle {
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
        let controller = self.runtime.borrow().controller_entity();
        let graph_spaces = cx.read_entity(&controller, |controller, _| controller.graph().spaces());
        let probe = {
            let mut runtime = self.runtime.borrow_mut();
            runtime.prepare_tear_off_drop_delivery(request, &graph_spaces)?
        };
        let prepared = probe.sample(cx)?;

        let result = self
            .open_prepared_tear_off_viewport(prepared, excluded_window, cx)
            .map(DockViewportDropRouteOutcome::tear_off)
            .map_err(|error| DockActionApplyError::TearOffViewportOpenFailed {
                message: error.to_string(),
            });
        self.runtime.borrow_mut().record_drop_route_result(&result);
        result
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

    #[cfg(test)]
    pub(crate) fn resolve_and_update_routed_drop_preview(
        &self,
        request: &DockViewportDropRouteRequest,
        payload: &DockDragPayload,
        cx: &mut App,
    ) -> (DockViewportResolvedDropRoute, bool) {
        self.resolve_and_update_routed_drop_preview_inner(request, payload, None, cx)
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

    #[cfg(test)]
    pub(crate) fn clear_routed_drop_preview(&self, cx: &mut App) -> bool {
        self.clear_routed_drop_preview_excluding(None, cx)
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
    pub(crate) fn last_hovered_viewport_identity_for_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<crate::DockViewportIdentity> {
        self.runtime
            .borrow()
            .last_hovered_viewport_identity_for_drag_session(session)
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
    #[cfg(test)]
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
