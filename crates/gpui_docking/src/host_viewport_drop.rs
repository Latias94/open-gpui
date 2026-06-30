use crate::viewport_registry::DockViewportWindowBoundsFrame;
use crate::{
    DockHost, DockViewportDropPayload, DockViewportDropReleasePoint, DockViewportDropRouteRequest,
    DockViewportPlatformSignals, DockViewportWindowFacts,
    drag::{DockDragPayload, DockDragTearOffGeometry},
    host_interaction_outcome::DockHostInteractionOutcome,
    interaction::{DockPayloadDropRelease, DockPayloadDropReleaseOrigin, DockRuntimeDragSession},
};
use open_gpui::{Bounds, Context, Pixels, Point, Window};
const DOCKING_DEBUG_PREFIX: &str = "[DEBUG-docking-native]";

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
        let event_receiver_local_scene_proof =
            self.interaction().viewport_host_scene_frame().cloned();
        let request = viewport_drop_route_request_from_host(
            payload,
            position,
            window,
            cx,
            DockPayloadDropReleaseOrigin::HoveredHost,
            drag_session,
            tear_off_geometry,
        )
        .with_event_receiver_local_scene_proof(event_receiver_local_scene_proof);
        let resolution_outcome = runtime.resolve_payload_drop_delivery_outcome(&request, cx);
        let route_resolution_changed = resolution_outcome.changed();
        let resolution = resolution_outcome.resolution();
        let routed_preview_changed = runtime.update_host_routed_drop_preview(
            resolution,
            payload,
            self.space().clone(),
            window.window_handle().window_id(),
            position,
            cx,
        );
        if route_resolution_changed || routed_preview_changed {
            log::debug!(
                "{DOCKING_DEBUG_PREFIX} hover preview route outcome space={} window_id={:?} origin={:?} route={:?} route_changed={} preview_changed={}",
                self.space().as_str(),
                window.window_handle().window_id(),
                DockPayloadDropReleaseOrigin::HoveredHost,
                resolution.route(),
                route_resolution_changed,
                routed_preview_changed
            );
        }
        DockHostInteractionOutcome::from_session_changed(
            route_resolution_changed || routed_preview_changed,
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
        log::debug!(
            "{DOCKING_DEBUG_PREFIX} commit routed payload release space={} window_id={:?} origin={:?} drag_session={:?} release_position=({}, {})",
            self.space().as_str(),
            window.window_handle().window_id(),
            release.origin(),
            release.drag_session().as_ref().map(|session| session.id()),
            release.release_position().x,
            release.release_position().y
        );
        let result = runtime.commit_payload_drop_from_screen(&release_request, cx);
        log::debug!(
            "{DOCKING_DEBUG_PREFIX} commit result space={} window_id={:?} success={}",
            self.space().as_str(),
            window.window_handle().window_id(),
            result.is_ok()
        );
        Some(DockHostInteractionOutcome::from_routed_drop_result(result))
    }

    fn viewport_drop_route_request_from_release(
        &self,
        release: &DockPayloadDropRelease,
        window: &Window,
        cx: &Context<Self>,
    ) -> DockViewportDropRouteRequest {
        let drag_session = release.drag_session().cloned();
        let tear_off_geometry = release.tear_off_geometry();
        viewport_drop_route_request_from_host(
            release.payload(),
            release.release_position(),
            window,
            cx,
            release.origin(),
            drag_session,
            tear_off_geometry,
        )
        .with_event_receiver_local_scene_proof(release.event_receiver_local_scene_proof().cloned())
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
    let suggested_window_bounds = suggested_window_bounds_for_host_release(
        window.window_bounds(),
        host_position,
        cx.viewport_capabilities().global_window_bounds,
        tear_off_geometry,
    );
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
        suggested_window_bounds,
        platform_signals,
        origin,
    )
    .with_drag_session(drag_session)
    .with_tear_off_geometry(tear_off_geometry)
}

fn suggested_window_bounds_for_host_release(
    source_window_bounds: open_gpui::WindowBounds,
    host_position: Point<Pixels>,
    has_global_window_bounds: bool,
    tear_off_geometry: Option<DockDragTearOffGeometry>,
) -> Option<open_gpui::WindowBounds> {
    if has_global_window_bounds {
        return None;
    }
    tear_off_geometry.map(|geometry| {
        crate::viewport_runtime::suggested_tear_off_window_bounds(
            source_window_bounds,
            host_position,
            geometry,
        )
    })
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
    use crate::host_test_support::floating_bounds;
    use open_gpui::{WindowBounds, point, px, size};

    #[test]
    fn host_release_suggests_tear_off_bounds_when_global_window_bounds_are_unavailable() {
        let geometry = DockDragTearOffGeometry::from_source_bounds(
            floating_bounds(20.0, 30.0, 480.0, 300.0),
            point(px(70.0), px(90.0)),
        )
        .with_preferred_size(size(px(360.0), px(240.0)));

        assert_eq!(
            suggested_window_bounds_for_host_release(
                WindowBounds::Windowed(floating_bounds(100.0, 200.0, 800.0, 600.0)),
                point(px(460.0), px(330.0)),
                false,
                Some(geometry),
            ),
            Some(WindowBounds::Windowed(floating_bounds(
                510.0, 470.0, 360.0, 240.0
            )))
        );
    }

    #[test]
    fn host_release_keeps_global_tear_off_placement_on_drag_geometry_path() {
        let geometry = DockDragTearOffGeometry::from_source_bounds(
            floating_bounds(20.0, 30.0, 480.0, 300.0),
            point(px(70.0), px(90.0)),
        );

        assert_eq!(
            suggested_window_bounds_for_host_release(
                WindowBounds::Windowed(floating_bounds(100.0, 200.0, 800.0, 600.0)),
                point(px(460.0), px(330.0)),
                true,
                Some(geometry),
            ),
            None,
            "global screen coordinates should continue to use exact drag geometry placement"
        );
    }

    #[test]
    fn host_release_suggests_undock_limited_bounds_for_large_geometry() {
        let geometry = DockDragTearOffGeometry::from_source_bounds(
            floating_bounds(0.0, 0.0, 1200.0, 900.0),
            point(px(600.0), px(450.0)),
        )
        .with_preferred_size(size(px(1200.0), px(900.0)))
        .with_display_work_area(floating_bounds(0.0, 0.0, 1000.0, 800.0));

        assert_eq!(
            suggested_window_bounds_for_host_release(
                WindowBounds::Windowed(floating_bounds(100.0, 200.0, 1200.0, 900.0)),
                point(px(1100.0), px(780.0)),
                false,
                Some(geometry),
            ),
            Some(WindowBounds::Windowed(floating_bounds(
                100.0, 80.0, 900.0, 720.0
            )))
        );
    }
}
