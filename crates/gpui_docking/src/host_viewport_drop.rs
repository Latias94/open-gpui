use crate::{
    DockHost, DockViewportDropPayload, DockViewportDropRouteRequest, DockViewportPlatformSignals,
    drag::{DockDragPayload, DockDragPayloadKind},
    host_interaction_outcome::DockHostInteractionOutcome,
};
use open_gpui::{Bounds, Context, Pixels, Point, Window, point};

impl DockHost {
    pub(crate) fn publish_viewport_host_scene_interaction(
        &self,
        host_bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        window: &Window,
    ) {
        let Some(runtime) = self.viewport_runtime().cloned() else {
            return;
        };
        let window_id = window.window_handle().window_id();
        if runtime.window_id_for_space(self.space()) != Some(window_id) {
            return;
        }

        runtime.begin_viewport_host_scene(
            self.space().clone(),
            window_id,
            window.window_bounds(),
            host_bounds,
            host_local_point(host_bounds, position),
        );
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

        let request = viewport_drop_route_request_from_host(payload, position, window, cx);
        let route = runtime.resolve_payload_drop_route(&request, cx);
        DockHostInteractionOutcome::from_session_changed(
            self.interaction_mut()
                .update_drop_route_preview(&route, position),
        )
    }

    pub(crate) fn commit_runtime_routed_payload_drop_interaction(
        &mut self,
        payload: &DockDragPayload,
        release_position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<DockHostInteractionOutcome> {
        let runtime = self.viewport_runtime()?.clone();
        let request = viewport_drop_route_request_from_host(payload, release_position, window, cx);
        let result = runtime.commit_payload_drop_from_screen(&request, cx);
        Some(DockHostInteractionOutcome::from_routed_drop_result(result))
    }
}

fn viewport_drop_route_request_from_host(
    payload: &DockDragPayload,
    host_position: Point<Pixels>,
    window: &Window,
    cx: &Context<DockHost>,
) -> DockViewportDropRouteRequest {
    DockViewportDropRouteRequest::new(
        payload.source_space.clone(),
        payload.source_tabs,
        viewport_payload(payload),
        window_screen_position(window, host_position),
        None,
        DockViewportPlatformSignals::from_window(window, cx).target_context(),
    )
}

fn window_screen_position(window: &Window, position: Point<Pixels>) -> Point<Pixels> {
    let window_bounds = window.window_bounds().get_bounds();
    point(
        window_bounds.origin.x + position.x,
        window_bounds.origin.y + position.y,
    )
}

fn host_local_point(host_bounds: Bounds<Pixels>, position: Point<Pixels>) -> Point<Pixels> {
    Point::new(
        position.x - host_bounds.origin.x,
        position.y - host_bounds.origin.y,
    )
}

fn viewport_payload(payload: &DockDragPayload) -> DockViewportDropPayload {
    match &payload.kind {
        DockDragPayloadKind::Item { item } => DockViewportDropPayload::Item(item.clone()),
        DockDragPayloadKind::Tabs => DockViewportDropPayload::Tabs,
    }
}
