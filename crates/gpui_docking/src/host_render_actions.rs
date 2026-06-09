use crate::{DockHost, DockItemId, DockNodeId, DockSpaceId, drag::DockDragPayload};
use open_gpui::{Bounds, Context, MouseButton, Pixels, Point, Window};
use std::time::Duration;

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

    pub(crate) fn update_tabs_drop_target_from_render(
        &mut self,
        payload: &DockDragPayload,
        target_tabs: DockNodeId,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        is_central: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        self.update_tabs_drop_interaction(payload, target_tabs, bounds, position, is_central, cx)
            .finish(cx)
    }

    pub(crate) fn update_tab_reorder_drop_target_from_render(
        &mut self,
        payload: &DockDragPayload,
        target_tabs: DockNodeId,
        target_index: usize,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        is_central: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        self.update_tab_reorder_drop_interaction(
            payload,
            target_tabs,
            target_index,
            bounds,
            position,
            is_central,
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
        self.schedule_outside_release_poll_from_render(window, cx);
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
        cx: &mut Context<Self>,
    ) -> bool {
        self.update_root_drop_scene_interaction(payload, root, bounds, position, cx)
            .finish(cx)
    }

    pub(crate) fn update_empty_space_drop_scene_from_render(
        &mut self,
        payload: &DockDragPayload,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        self.update_empty_space_drop_scene_interaction(payload, position, bounds, cx)
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
        cx: &mut Context<Self>,
    ) -> bool {
        self.update_floating_title_bar_drop_scene_interaction(
            payload,
            floating,
            target_tabs,
            title_bounds,
            preview_bounds,
            position,
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

    fn schedule_outside_release_poll_from_render(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.viewport_runtime().is_none()
            || cx.mouse_button_is_pressed(MouseButton::Left).is_none()
            || !self.interaction_mut().begin_outside_release_poll()
        {
            return false;
        }

        cx.spawn_in(window, async move |host, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let should_continue = host
                    .update_in(cx, |host, window, cx| {
                        host.poll_outside_release_from_render(window, cx)
                    })
                    .unwrap_or(false);
                if !should_continue {
                    break;
                }
            }
        })
        .detach();
        true
    }

    fn poll_outside_release_from_render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.interaction().outside_release_poll_running() {
            return false;
        }

        let Some(payload) = cx.active_drag_value::<DockDragPayload>().cloned() else {
            self.interaction_mut().finish_outside_release_poll();
            return false;
        };

        match cx.mouse_button_is_pressed(MouseButton::Left) {
            Some(true) => true,
            Some(false) => {
                self.interaction_mut().finish_outside_release_poll();
                let target_space = self.space().clone();
                let release_position = window.mouse_position();
                let changed = self.drop_payload_from_render(
                    &payload,
                    target_space,
                    release_position,
                    window,
                    cx,
                );
                cx.stop_active_drag(window);
                changed
            }
            None => {
                self.interaction_mut().finish_outside_release_poll();
                false
            }
        }
    }
}
