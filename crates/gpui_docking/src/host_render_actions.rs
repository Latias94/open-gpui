use crate::{
    DockHost, DockItemId, DockNodeId, DockSpaceId,
    drag::DockDragPayload,
    drop_runtime::DockHostDropSceneFact,
    interaction::{DockPayloadDropRelease, DockRuntimeDragSession},
};
use open_gpui::{Bounds, Context, Pixels, Point, Window};

impl DockHost {
    pub(crate) fn begin_payload_drag_from_render(
        &mut self,
        payload: &DockDragPayload,
    ) -> Option<DockRuntimeDragSession> {
        Some(self.viewport_runtime()?.begin_payload_drag(payload))
    }

    pub(crate) fn active_payload_drag_session(
        &self,
        payload: &DockDragPayload,
    ) -> Option<DockRuntimeDragSession> {
        self.viewport_runtime()?
            .active_payload_drag_session(payload)
    }

    pub(crate) fn finish_payload_drag_session(&self, session: &DockRuntimeDragSession) -> bool {
        self.viewport_runtime()
            .is_some_and(|runtime| runtime.finish_payload_drag(session))
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

    #[cfg(test)]
    pub(crate) fn drop_payload_from_render(
        &mut self,
        payload: &DockDragPayload,
        host_space: DockSpaceId,
        release_position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.drop_payload_release_from_render(
            DockPayloadDropRelease::hovered_host(payload.clone(), host_space, release_position),
            window,
            cx,
        )
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
            .is_some_and(|session| self.finish_payload_drag_session(session));
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
            .merge(self.begin_host_drop_scene_interaction(payload, position, cx))
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
            .finish(cx)
    }

    pub(crate) fn update_empty_space_drop_scene_from_render(
        &mut self,
        payload: &DockDragPayload,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.update_empty_space_drop_scene_interaction(payload, position, bounds, window, cx)
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
