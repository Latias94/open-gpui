use crate::{
    DockHost, DockItemId, DockNodeId, DockSpaceId,
    drag::{DockDragPayload, DockDragTearOffGeometry},
    drop_runtime::DockHostDropSceneFact,
    interaction::{DockPayloadDropRelease, DockRuntimeDragSession},
};
use open_gpui::{Bounds, Context, Pixels, Point, Window};

impl DockHost {
    pub(crate) fn begin_payload_drag_from_render(
        &mut self,
        payload: &DockDragPayload,
        cx: &mut Context<Self>,
    ) -> DockRuntimeDragSession {
        self.begin_payload_drag_interaction(payload, cx)
    }

    pub(crate) fn begin_tab_item_drag_from_render(
        &mut self,
        tabs: DockNodeId,
        item: DockItemId,
        payload: &DockDragPayload,
        cx: &mut Context<Self>,
    ) -> DockRuntimeDragSession {
        let (outcome, drag_session) = self.begin_tab_item_drag_interaction(tabs, item, payload, cx);
        outcome.finish(cx);
        drag_session
    }

    pub(crate) fn update_payload_drag_tear_off_geometry_from_render(
        &mut self,
        payload: &DockDragPayload,
        geometry: DockDragTearOffGeometry,
    ) -> bool {
        let runtime = self.viewport_runtime();
        let Some(session) = runtime.active_payload_drag_session(payload) else {
            return false;
        };
        runtime.update_payload_drag_tear_off_geometry(&session, geometry)
    }

    pub(crate) fn active_payload_drag_session(
        &self,
        payload: &DockDragPayload,
    ) -> Option<DockRuntimeDragSession> {
        self.viewport_runtime().active_payload_drag_session(payload)
    }

    pub(crate) fn active_payload_drag_tear_off_geometry(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<DockDragTearOffGeometry> {
        self.viewport_runtime()
            .active_payload_drag_tear_off_geometry(session)
    }

    pub(crate) fn finish_payload_drag_session(
        &self,
        session: &DockRuntimeDragSession,
        cx: &mut Context<Self>,
    ) -> bool {
        self.viewport_runtime()
            .finish_payload_drag_with_app(session, cx)
    }

    pub(crate) fn select_tab_from_render(
        &mut self,
        tabs: DockNodeId,
        item: DockItemId,
        cx: &mut Context<Self>,
    ) -> bool {
        self.select_tab_interaction(tabs, item, cx).finish(cx)
    }

    pub(crate) fn close_item_from_render(
        &mut self,
        item: DockItemId,
        cx: &mut Context<Self>,
    ) -> bool {
        self.close_item_interaction(item, cx).finish(cx)
    }

    pub(crate) fn drop_payload_release_from_render(
        &mut self,
        release: DockPayloadDropRelease,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.commit_payload_drop_release(release, window, cx)
    }

    pub(crate) fn commit_payload_drop_release(
        &mut self,
        release: DockPayloadDropRelease,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let drag_session = release.drag_session().cloned();
        self.interaction_mut().cancel_outside_release_poll();
        let changed = self
            .commit_payload_drop_interaction(release, window, cx)
            .finish(cx);
        let session_changed = drag_session
            .as_ref()
            .is_some_and(|session| self.finish_payload_drag_session(session, cx));
        changed || session_changed
    }

    pub(crate) fn update_drop_scene_fact_from_render(
        &mut self,
        payload: &DockDragPayload,
        fact: DockHostDropSceneFact,
        position: Point<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.update_drop_scene_fact_interaction(payload, fact, position, window, cx)
            .merge(
                self.update_viewport_drop_route_preview_interaction(payload, position, window, cx),
            )
            .finish(cx)
    }

    pub(crate) fn begin_host_drop_scene_from_render(
        &mut self,
        payload: &DockDragPayload,
        host_bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.schedule_outside_release_poll_from_host(payload, window, cx);
        self.publish_viewport_host_scene_interaction(host_bounds, position, window, cx);
        self.update_floating_drag_interaction(position, cx)
            .merge(
                self.update_viewport_drop_route_preview_interaction(payload, position, window, cx),
            )
            .merge(self.ensure_host_drop_scene_interaction(payload, position, cx))
            .finish(cx)
    }

    pub(crate) fn update_root_drop_scene_from_render(
        &mut self,
        payload: &DockDragPayload,
        root: DockNodeId,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.update_root_drop_scene_interaction(payload, root, bounds, position, window, cx)
            .merge(
                self.update_viewport_drop_route_preview_interaction(payload, position, window, cx),
            )
            .finish(cx)
    }

    pub(crate) fn update_empty_space_drop_scene_from_render(
        &mut self,
        payload: &DockDragPayload,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        is_central: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.update_empty_space_drop_scene_interaction(
            payload, position, bounds, is_central, window, cx,
        )
        .merge(self.update_viewport_drop_route_preview_interaction(payload, position, window, cx))
        .finish(cx)
    }

    pub(crate) fn begin_floating_drag_from_render(
        &mut self,
        space: DockSpaceId,
        floating: DockNodeId,
        start_position: Point<Pixels>,
        initial_bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        self.begin_floating_drag_interaction(space, floating, start_position, initial_bounds, cx)
            .finish(cx)
    }

    pub(crate) fn update_floating_drag_from_render(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        self.update_floating_drag_interaction(position, cx)
            .finish(cx)
    }

    pub(crate) fn finish_floating_drag_from_render(&mut self, cx: &mut Context<Self>) -> bool {
        self.finish_floating_drag_interaction().finish(cx)
    }

    pub(crate) fn begin_splitter_drag_from_render(
        &mut self,
        split: DockNodeId,
        handle_index: usize,
        start_position: Pixels,
        split_extent: Pixels,
        initial_fractions: Vec<f32>,
        cx: &mut Context<Self>,
    ) -> bool {
        self.begin_splitter_drag_interaction(
            split,
            handle_index,
            start_position,
            split_extent,
            initial_fractions,
        )
        .finish(cx)
    }

    pub(crate) fn update_splitter_drag_from_render(
        &mut self,
        position: Pixels,
        cx: &mut Context<Self>,
    ) -> bool {
        self.update_splitter_drag_interaction(position, cx)
            .finish(cx)
    }

    pub(crate) fn finish_splitter_drag_from_render(&mut self, cx: &mut Context<Self>) -> bool {
        self.finish_splitter_drag_interaction().finish(cx)
    }
}
