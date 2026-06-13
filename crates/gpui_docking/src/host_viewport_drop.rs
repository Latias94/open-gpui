use crate::{
    DockDropDelivery, DockHost, DockViewportDropPayload, DockViewportDropRouteRequest,
    DockViewportPlatformSignals, DockViewportWindowFacts,
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
        let Some(runtime) = self.viewport_runtime().cloned() else {
            self.interaction_mut().set_viewport_host_scene_frame(None);
            return;
        };
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
        let Some(runtime) = self.viewport_runtime().cloned() else {
            return DockHostInteractionOutcome::from_session_changed(
                self.interaction_mut().clear_drop_route_preview(),
            );
        };

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
        let route = resolution.route().clone();
        let delivery = resolution.delivery().clone();
        let routed_preview_changed =
            runtime.update_routed_drop_preview(&resolution, payload.title(), cx);
        DockHostInteractionOutcome::from_session_changed(
            self.interaction_mut()
                .update_drop_route_preview(&route, position, delivery)
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
        let runtime = self.viewport_runtime()?.clone();
        if release.origin() == DockPayloadDropReleaseOrigin::HoveredHost
            && let Some(delivery) = delivery
        {
            let result = if delivery.accepts_drag_payload(release.payload()) {
                runtime.deliver_payload_drop_with_outcome(delivery, cx)
            } else {
                Err(delivery.payload_mismatch_error())
            };
            return Some(DockHostInteractionOutcome::from_routed_drop_result(result));
        }

        let drag_session = release.drag_session().cloned();
        let tear_off_geometry = self.active_payload_drag_tear_off_geometry(drag_session.as_ref());
        let request = viewport_drop_route_request_from_host(
            release.payload(),
            release.release_position(),
            window,
            cx,
            release.origin(),
            drag_session,
            tear_off_geometry,
        );
        let result = runtime.commit_payload_drop_from_screen(&request, cx);
        Some(DockHostInteractionOutcome::from_routed_drop_result(result))
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
            DockViewportPlatformSignals::from_hovered_window(window, cx)
        }
        DockPayloadDropReleaseOrigin::SourceOnly => DockViewportPlatformSignals::from_app(cx),
    };
    DockViewportDropRouteRequest::from_platform_signals(
        payload.source_space.clone(),
        payload.source_node,
        DockViewportDropPayload::from_drag_payload(payload),
        window_screen_position(window, host_position),
        None,
        platform_signals,
    )
    .with_drag_session(drag_session)
    .with_tear_off_geometry(tear_off_geometry)
}

fn window_screen_position(window: &Window, position: Point<Pixels>) -> Point<Pixels> {
    let window_bounds = window.bounds();
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
