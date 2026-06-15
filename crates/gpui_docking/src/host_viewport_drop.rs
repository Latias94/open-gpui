use crate::{
    DockActionApplyError, DockDropDelivery, DockHost, DockViewportDropPayload,
    DockViewportDropRouteRequest, DockViewportPlatformSignals, DockViewportPointerCoordinateSpace,
    DockViewportWindowFacts,
    drag::{DockDragPayload, DockDragTearOffGeometry},
    host_interaction_outcome::DockHostInteractionOutcome,
    interaction::{DockPayloadDropRelease, DockPayloadDropReleaseOrigin, DockRuntimeDragSession},
};
use open_gpui::{Bounds, Context, Pixels, Point, Window, point};

impl DockHost {
    pub(crate) fn publish_viewport_host_scene_interaction(
        &mut self,
        host_bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        window: &Window,
        cx: &Context<Self>,
    ) {
        let runtime = self.viewport_runtime().clone();
        let space = self.space().clone();
        let window_id = window.window_handle().window_id();
        if runtime.window_id_for_space(&space) != Some(window_id) {
            self.interaction_mut().set_viewport_host_scene_frame(None);
            return;
        }

        let registration = runtime.begin_viewport_host_scene_frame(
            space,
            window_id,
            window_facts(window, cx),
            host_bounds,
            host_local_point(host_bounds, position),
        );
        self.interaction_mut()
            .set_viewport_host_scene_frame(registration.map(|registration| registration.frame));
    }

    pub(crate) fn update_viewport_drop_route_preview_interaction(
        &mut self,
        payload: &DockDragPayload,
        position: Point<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> DockHostInteractionOutcome {
        let runtime = self.viewport_runtime().clone();

        let drag_session = self.active_payload_drag_session(payload);
        let tear_off_geometry = self.active_payload_drag_tear_off_geometry(drag_session.as_ref());
        let request = viewport_drop_route_request_from_host(
            payload,
            position,
            window,
            cx,
            DockPayloadDropReleaseOrigin::HoveredHost,
            drag_session,
            tear_off_geometry,
        );
        let resolution = runtime.resolve_payload_drop_delivery(&request, cx);
        let routed_preview_changed =
            runtime.update_routed_drop_preview(&resolution, payload.title(), cx);
        DockHostInteractionOutcome::from_session_changed(
            self.interaction_mut()
                .update_drop_route_preview(&resolution, position)
                || routed_preview_changed,
        )
    }

    pub(crate) fn commit_runtime_routed_payload_drop_interaction(
        &mut self,
        delivery: Option<DockDropDelivery>,
        release: &DockPayloadDropRelease,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<DockHostInteractionOutcome> {
        let runtime = self.viewport_runtime().clone();
        let release_request = self.viewport_drop_route_request_from_release(release, window, cx);

        if release.origin() == DockPayloadDropReleaseOrigin::HoveredHost {
            let result = self.commit_hovered_host_routed_payload_drop(
                delivery,
                &release_request,
                release,
                window,
                cx,
            );
            runtime.record_drop_route_result(&result);
            return Some(DockHostInteractionOutcome::from_routed_drop_result(result));
        }

        let result = runtime.commit_payload_drop_from_screen(&release_request, cx);
        Some(DockHostInteractionOutcome::from_routed_drop_result(result))
    }

    fn commit_hovered_host_routed_payload_drop(
        &self,
        delivery: Option<DockDropDelivery>,
        release_request: &DockViewportDropRouteRequest,
        release: &DockPayloadDropRelease,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Result<crate::DockViewportDropRouteOutcome, DockActionApplyError> {
        let Some(delivery) = delivery else {
            return Err(DockActionApplyError::DropTargetUnavailable);
        };

        if !delivery.release_authority_for_cached_preview(
            release.origin(),
            release.host_space(),
            window.window_handle().window_id(),
            release.payload(),
        )? {
            return Err(DockActionApplyError::DropTargetUnavailable);
        }

        let runtime = self.viewport_runtime().clone();
        let fresh = runtime.resolve_payload_drop_delivery(release_request, cx);
        if fresh.delivery() != Some(&delivery) {
            return Err(DockActionApplyError::DropTargetUnavailable);
        }

        runtime.validate_payload_drop_delivery(&delivery, cx)?;
        runtime.deliver_payload_drop_with_outcome(delivery, cx)
    }

    fn viewport_drop_route_request_from_release(
        &self,
        release: &DockPayloadDropRelease,
        window: &Window,
        cx: &Context<Self>,
    ) -> DockViewportDropRouteRequest {
        let drag_session = release.drag_session().cloned();
        let tear_off_geometry = self.active_payload_drag_tear_off_geometry(drag_session.as_ref());
        viewport_drop_route_request_from_host(
            release.payload(),
            release.release_position(),
            window,
            cx,
            release.origin(),
            drag_session,
            tear_off_geometry,
        )
    }
}

fn viewport_drop_route_request_from_host(
    payload: &DockDragPayload,
    host_position: Point<Pixels>,
    window: &Window,
    cx: &Context<DockHost>,
    origin: DockPayloadDropReleaseOrigin,
    drag_session: Option<DockRuntimeDragSession>,
    tear_off_geometry: Option<DockDragTearOffGeometry>,
) -> DockViewportDropRouteRequest {
    let platform_signals = match origin {
        DockPayloadDropReleaseOrigin::HoveredHost => {
            DockViewportPlatformSignals::from_event_receiver_window(window, cx)
        }
        DockPayloadDropReleaseOrigin::SourceOnly => DockViewportPlatformSignals::from_app(cx),
    };
    let release_position =
        route_release_position(window.bounds(), host_position, &platform_signals);
    let coordinate_space = route_coordinate_space(origin, &platform_signals);
    DockViewportDropRouteRequest::from_platform_signals_with_coordinate_space(
        payload.source_space.clone(),
        payload.source_node,
        DockViewportDropPayload::from_drag_payload(payload),
        release_position,
        None,
        platform_signals,
        coordinate_space,
    )
    .with_drag_session(drag_session)
    .with_tear_off_geometry(tear_off_geometry)
}

fn route_coordinate_space(
    origin: DockPayloadDropReleaseOrigin,
    platform_signals: &DockViewportPlatformSignals,
) -> DockViewportPointerCoordinateSpace {
    if platform_signals.has_global_window_bounds() {
        return DockViewportPointerCoordinateSpace::GlobalScreen;
    }

    match origin {
        DockPayloadDropReleaseOrigin::HoveredHost => {
            DockViewportPointerCoordinateSpace::for_hovered_host(false)
        }
        DockPayloadDropReleaseOrigin::SourceOnly => {
            DockViewportPointerCoordinateSpace::for_source_only(false)
        }
    }
}

fn route_release_position(
    window_bounds: Bounds<Pixels>,
    position: Point<Pixels>,
    platform_signals: &DockViewportPlatformSignals,
) -> Point<Pixels> {
    if platform_signals.has_global_window_bounds() {
        return window_screen_position(window_bounds, position);
    }

    position
}

fn window_screen_position(window_bounds: Bounds<Pixels>, position: Point<Pixels>) -> Point<Pixels> {
    point(
        window_bounds.origin.x + position.x,
        window_bounds.origin.y + position.y,
    )
}

fn window_facts(window: &Window, cx: &Context<DockHost>) -> DockViewportWindowFacts {
    DockViewportWindowFacts::new(
        window.display(cx).map(|display| display.id()),
        window.window_bounds(),
        window.bounds(),
    )
}

fn host_local_point(host_bounds: Bounds<Pixels>, position: Point<Pixels>) -> Point<Pixels> {
    Point::new(
        position.x - host_bounds.origin.x,
        position.y - host_bounds.origin.y,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DockViewportTargetContext;
    use open_gpui::{Bounds, point, px, size};

    #[test]
    fn route_release_position_uses_screen_coordinates_when_bounds_are_global() {
        let window_bounds = Bounds::new(point(px(400.0), px(300.0)), size(px(320.0), px(240.0)));
        let window_position = point(px(30.0), px(50.0));
        let signals =
            DockViewportPlatformSignals::from_target_context(DockViewportTargetContext::new());

        assert_eq!(
            route_release_position(window_bounds, window_position, &signals),
            point(px(430.0), px(350.0))
        );
    }

    #[test]
    fn route_release_position_keeps_window_local_coordinates_without_global_bounds() {
        let window_bounds = Bounds::new(point(px(400.0), px(300.0)), size(px(320.0), px(240.0)));
        let window_position = point(px(30.0), px(50.0));
        let signals =
            DockViewportPlatformSignals::from_target_context(DockViewportTargetContext::new())
                .with_global_window_bounds(false);

        assert_eq!(
            route_release_position(window_bounds, window_position, &signals),
            window_position
        );
    }

    #[test]
    fn route_coordinate_space_is_selected_from_platform_bounds_and_release_origin() {
        let global =
            DockViewportPlatformSignals::from_target_context(DockViewportTargetContext::new())
                .with_global_window_bounds(true);
        assert_eq!(
            route_coordinate_space(DockPayloadDropReleaseOrigin::SourceOnly, &global),
            DockViewportPointerCoordinateSpace::GlobalScreen,
            "global bounds make release positions screen-space regardless of release origin"
        );

        let local =
            DockViewportPlatformSignals::from_target_context(DockViewportTargetContext::new())
                .with_global_window_bounds(false);
        assert_eq!(
            route_coordinate_space(DockPayloadDropReleaseOrigin::HoveredHost, &local),
            DockViewportPointerCoordinateSpace::ReceiverLocal
        );
        assert_eq!(
            route_coordinate_space(DockPayloadDropReleaseOrigin::SourceOnly, &local),
            DockViewportPointerCoordinateSpace::SourceLocalOnly
        );
    }
}
