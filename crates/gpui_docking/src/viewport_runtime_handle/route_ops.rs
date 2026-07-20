use super::*;

impl DockViewportRuntimeHandle {
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
        let result = match delivery.into_tear_off_request() {
            Ok(request) => self.commit_tear_off_drop_route(request, cx),
            Err(delivery) => {
                let mut runtime = self.runtime.borrow_mut();
                match live_window {
                    Some(window) => runtime
                        .deliver_drop_commit_delivery_from_live_window_with_outcome(
                            delivery, window, cx,
                        ),
                    None => runtime.deliver_drop_commit_delivery_with_outcome(delivery, cx),
                }
            }
        };
        self.clear_routed_drop_preview(cx);
        if let Ok(DockViewportDropRouteOutcome::Action(outcome)) = &result {
            apply_viewport_window_effects(outcome.window_effects(), cx);
        }
        result
    }

    fn commit_tear_off_drop_route(
        &self,
        request: DockViewportTearOffRequest,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let prepared = {
            let mut runtime = self.runtime.borrow_mut();
            runtime.prepare_tear_off_drop_delivery(request, cx)?
        };

        let result = self
            .open_prepared_tear_off_viewport(prepared, cx)
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
        self.runtime
            .borrow()
            .resolve_host_scene_target(space, host_position, cx)
    }

    #[cfg(test)]
    pub(crate) fn resolve_payload_drop_delivery<C: open_gpui::AppContext>(
        &self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportResolvedDropRoute {
        self.resolve_payload_drop_delivery_outcome(request, cx)
            .into_resolution()
    }

    pub(crate) fn resolve_payload_drop_delivery_outcome<C: open_gpui::AppContext>(
        &self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportResolvedDropRouteOutcome {
        let refresh = self
            .runtime
            .borrow_mut()
            .resolve_payload_drop_delivery_with_outcome(request, cx);
        refresh_viewport_window_effects(refresh.window_effects(), cx);
        refresh.outcome
    }

    pub(crate) fn resolve_payload_drop_delivery_for_request<C: open_gpui::AppContext>(
        &self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportResolvedDropRoute {
        self.resolve_payload_drop_delivery_for_request_outcome(request, cx)
            .into_resolution()
    }

    pub(crate) fn resolve_payload_drop_delivery_for_request_outcome<C: open_gpui::AppContext>(
        &self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportResolvedDropRouteOutcome {
        let refresh = self
            .runtime
            .borrow_mut()
            .resolve_payload_drop_delivery_for_request_with_outcome(request, cx);
        refresh_viewport_window_effects(refresh.window_effects(), cx);
        refresh.outcome
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
        refresh_runtime_update(update, cx)
    }

    pub(crate) fn clear_routed_drop_preview(&self, cx: &mut App) -> bool {
        let update = self.runtime.borrow_mut().clear_routed_drop_preview();
        refresh_runtime_update(update, cx)
    }

    pub(crate) fn vacate_empty_payload_drop_source_viewport(
        &self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        cx: &mut App,
    ) -> bool {
        let effects = self
            .runtime
            .borrow_mut()
            .vacate_empty_payload_drop_source_viewport_with_cleanup(source_space, target_space, cx);
        let changed = effects.has_effects();
        apply_viewport_window_effects(effects, cx);
        changed
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
        self.runtime
            .borrow_mut()
            .resolve_payload_drop_route_for_test(request, cx)
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
        let resolution = self.resolve_payload_drop_delivery_for_request(request, cx);
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
