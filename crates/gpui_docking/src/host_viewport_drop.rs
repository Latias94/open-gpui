use crate::viewport_registry::DockViewportWindowBoundsFrame;
use crate::{
    drag::{DockDragPayload, DockDragTearOffGeometry},
    host_interaction_outcome::DockHostInteractionOutcome,
    interaction::{DockPayloadDropRelease, DockPayloadDropReleaseOrigin, DockRuntimeDragSession},
    DockHost, DockViewportDropPayload, DockViewportDropReleasePoint, DockViewportDropRouteRequest,
    DockViewportPlatformSignals, DockViewportWindowFacts,
};
use open_gpui::{Bounds, Context, Pixels, Point, Window};

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
        let drop_guide_style =
            self.with_workspace(cx, |workspace| workspace.options().drop_guide_style);
        let window_id = window.window_handle().window_id();
        if runtime.window_id_for_space(&space) != Some(window_id) {
            self.interaction_mut().set_viewport_host_scene_frame(None);
            return;
        }

        let registration = runtime.begin_viewport_host_scene_frame(
            space,
            window_id,
            DockViewportWindowFacts::from_window(window, cx),
            host_bounds,
            host_local_point(host_bounds, position),
            drop_guide_style,
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
        release: &DockPayloadDropRelease,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<DockHostInteractionOutcome> {
        let runtime = self.viewport_runtime().clone();
        let release_request = self.viewport_drop_route_request_from_release(release, window, cx);
        let result = runtime.commit_payload_drop_from_screen(&release_request, cx);
        Some(DockHostInteractionOutcome::from_routed_drop_result(result))
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
    DockViewportDropRouteRequest::from_host_release(
        payload.source_space.clone(),
        payload.source_node,
        DockViewportDropPayload::from_drag_payload(payload),
        DockViewportDropReleasePoint::host_local_with_bounds_frame(
            host_position,
            if cx.viewport_capabilities().global_window_bounds {
                DockViewportWindowBoundsFrame::GlobalScreen(window.bounds())
            } else {
                DockViewportWindowBoundsFrame::WindowLocal(window.bounds())
            },
        ),
        None,
        platform_signals,
        origin,
    )
    .with_drag_session(drag_session)
    .with_tear_off_geometry(tear_off_geometry)
}

fn host_local_point(host_bounds: Bounds<Pixels>, position: Point<Pixels>) -> Point<Pixels> {
    Point::new(
        position.x - host_bounds.origin.x,
        position.y - host_bounds.origin.y,
    )
}
