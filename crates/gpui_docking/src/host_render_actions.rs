use crate::{DockHost, DockItemId, DockNodeId, DockSpaceId, drag::DockDragPayload};
use open_gpui::{Bounds, Context, Pixels, Point, Window};

impl DockHost {
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

    pub(crate) fn drop_payload_from_render(
        &mut self,
        payload: &DockDragPayload,
        target_space: DockSpaceId,
        release_position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.interaction_mut().finish_outside_release_poll();
        self.commit_payload_drop_interaction(payload, target_space, release_position, window, cx)
            .finish(cx)
    }

    pub(crate) fn update_tabs_drop_scene_from_render(
        &mut self,
        payload: &DockDragPayload,
        target_tabs: DockNodeId,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        is_central: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.update_tabs_drop_interaction(
            payload,
            target_tabs,
            bounds,
            position,
            is_central,
            window,
            cx,
        )
        .finish(cx)
    }

    pub(crate) fn update_tab_reorder_drop_scene_from_render(
        &mut self,
        payload: &DockDragPayload,
        target_tabs: DockNodeId,
        target_index: usize,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        is_central: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.update_tab_reorder_drop_interaction(
            payload,
            target_tabs,
            target_index,
            bounds,
            position,
            is_central,
            window,
            cx,
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
        self.schedule_outside_release_poll_from_host(window, cx);
        self.update_viewport_host_scene_from_window(host_bounds, position, window);
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

    pub(crate) fn update_floating_title_bar_drop_scene_from_render(
        &mut self,
        payload: &DockDragPayload,
        floating: DockNodeId,
        target_tabs: DockNodeId,
        title_bounds: Bounds<Pixels>,
        preview_bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.update_floating_title_bar_drop_scene_interaction(
            payload,
            floating,
            target_tabs,
            title_bounds,
            preview_bounds,
            position,
            window,
            cx,
        )
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
