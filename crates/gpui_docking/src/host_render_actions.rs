use crate::{
    DockHost, DockItemId, DockNodeId, DockSpaceId, SplitAxis,
    divider_hit_map::{DockDividerHandleHitTarget, DockDividerHitTarget},
    drag::{DockDragPayload, DockDragTearOffGeometry},
    drop_runtime::DockHostDropSceneFact,
    interaction::{DockPayloadDropRelease, DockRuntimeDragSession, SplitterDragAxis},
    presentation_scene::DockPresentationScene,
};
use open_gpui::{Bounds, Context, Pixels, Point, Window};
use open_gpui_ui_core::AccessibleAction;

const ACCESSIBILITY_SPLITTER_STEP_PX: f32 = 24.0;

impl DockHost {
    pub(crate) fn record_payload_drag_anchor_from_render(
        &mut self,
        source_space: DockSpaceId,
        source_node: DockNodeId,
        position: Point<Pixels>,
    ) {
        self.interaction_mut()
            .record_payload_drag_anchor(source_space, source_node, position);
    }

    pub(crate) fn payload_drag_anchor_position_from_render(
        &self,
        payload: &DockDragPayload,
    ) -> Option<Point<Pixels>> {
        self.interaction().payload_drag_anchor_position(payload)
    }

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
        let begin = self.begin_tab_item_drag_interaction(tabs, item, payload, cx);
        begin.outcome.finish(cx);
        begin.drag_session
    }

    pub(crate) fn focus_host_for_drag_from_render(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.host_focus_handle(), cx);
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
        &mut self,
        session: &DockRuntimeDragSession,
        cx: &mut Context<Self>,
    ) -> bool {
        let changed = self
            .viewport_runtime()
            .finish_payload_drag_with_app(session, cx);
        let anchor_cleared = self.interaction_mut().clear_any_payload_drag_anchor();
        changed || anchor_cleared
    }

    pub(crate) fn cancel_payload_drag_from_render(
        &mut self,
        payload: &DockDragPayload,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let drag_session = self.active_payload_drag_session(payload);
        let session_changed = drag_session
            .as_ref()
            .is_some_and(|session| self.finish_payload_drag_session(session, cx));
        let local_preview_cleared = self.clear_drop_preview_interaction();
        let routed_preview_cleared = self.viewport_runtime().clear_routed_drop_preview(cx);
        let active_drag_cleared = cx.stop_active_drag(window);
        session_changed || local_preview_cleared || routed_preview_cleared || active_drag_cleared
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

    pub(crate) fn update_local_drop_scene_fact_from_render(
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
        let _ = self.record_payload_drag_hovered_viewport_from_render(payload, window);
        self.schedule_outside_release_poll_from_host(payload, window, cx);
        self.publish_viewport_host_scene_interaction(host_bounds, position, window, cx);
        self.update_floating_drag_interaction(position, cx)
            .merge(
                self.update_viewport_drop_route_preview_interaction(payload, position, window, cx),
            )
            .merge(self.ensure_host_drop_scene_interaction(payload, position, cx))
            .finish(cx)
    }

    pub(crate) fn update_local_root_drop_scene_from_render(
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

    pub(crate) fn update_local_empty_space_drop_scene_from_render(
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

    pub(crate) fn begin_divider_drag_from_scene(
        &mut self,
        scene: &DockPresentationScene,
        target: &DockDividerHitTarget,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        match target {
            DockDividerHitTarget::Single(handle) => self
                .begin_splitter_drag_axis_from_scene(scene, handle, position)
                .finish(cx),
            DockDividerHitTarget::Corner(corner) => {
                let Some(horizontal) =
                    splitter_drag_axis_from_scene(scene, &corner.horizontal, position)
                else {
                    return false;
                };
                let Some(vertical) =
                    splitter_drag_axis_from_scene(scene, &corner.vertical, position)
                else {
                    return false;
                };
                self.begin_corner_splitter_drag_interaction(horizontal, vertical)
                    .finish(cx)
            }
        }
    }

    fn begin_splitter_drag_axis_from_scene(
        &mut self,
        scene: &DockPresentationScene,
        handle: &DockDividerHandleHitTarget,
        position: Point<Pixels>,
    ) -> crate::host_interaction_outcome::DockHostInteractionOutcome {
        let Some(axis) = splitter_drag_axis_from_scene(scene, handle, position) else {
            return crate::host_interaction_outcome::DockHostInteractionOutcome::Idle;
        };
        self.begin_splitter_drag_interaction(
            axis.axis,
            axis.split,
            axis.handle_index,
            axis.start_position,
            axis.split_extent,
            axis.initial_fractions,
        )
    }

    pub(crate) fn update_splitter_drag_from_render(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        self.update_splitter_drag_interaction(position, cx)
            .finish(cx)
    }

    pub(crate) fn finish_splitter_drag_from_render(&mut self, cx: &mut Context<Self>) -> bool {
        self.finish_splitter_drag_interaction().finish(cx)
    }

    pub(crate) fn resize_splitter_from_accessibility(
        &mut self,
        split: DockNodeId,
        axis: SplitAxis,
        handle_index: usize,
        action: AccessibleAction,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(scene) = self.last_presentation_scene().cloned() else {
            return false;
        };
        let Some(splitter) = scene.splitters.iter().find(|splitter| {
            splitter.split == split && splitter.axis == axis && splitter.index == handle_index
        }) else {
            return false;
        };
        let start_position = splitter.bounds.center();
        let delta = match action {
            AccessibleAction::Increment => open_gpui::px(ACCESSIBILITY_SPLITTER_STEP_PX),
            AccessibleAction::Decrement => open_gpui::px(-ACCESSIBILITY_SPLITTER_STEP_PX),
            _ => return false,
        };
        let target_position = match axis {
            SplitAxis::Horizontal => Point {
                x: start_position.x + delta,
                y: start_position.y,
            },
            SplitAxis::Vertical => Point {
                x: start_position.x,
                y: start_position.y + delta,
            },
        };

        self.begin_splitter_drag_axis_from_scene(
            &scene,
            &DockDividerHandleHitTarget {
                key: crate::divider_hit_map::DockDividerHandleKey {
                    split,
                    index: handle_index,
                    axis,
                },
                bounds: splitter.bounds,
                extent: splitter.extent,
            },
            start_position,
        )
        .merge(self.update_splitter_drag_interaction(target_position, cx))
        .merge(self.finish_splitter_drag_interaction())
        .finish(cx)
    }

    fn record_payload_drag_hovered_viewport_from_render(
        &self,
        payload: &DockDragPayload,
        window: &Window,
    ) -> bool {
        let Some(session) = self.active_payload_drag_session(payload) else {
            return false;
        };
        self.viewport_runtime()
            .record_payload_drag_hovered_viewport(
                &session,
                self.space().clone(),
                window.window_handle().window_id(),
            )
    }
}

fn splitter_drag_axis_from_scene(
    scene: &DockPresentationScene,
    handle: &DockDividerHandleHitTarget,
    position: Point<Pixels>,
) -> Option<SplitterDragAxis> {
    let splitter = scene.splitters.iter().find(|splitter| {
        splitter.split == handle.key.split
            && splitter.index == handle.key.index
            && splitter.axis == handle.key.axis
    })?;
    let start_position = match splitter.axis {
        SplitAxis::Horizontal => position.x,
        SplitAxis::Vertical => position.y,
    };
    Some(SplitterDragAxis::new(
        splitter.axis,
        splitter.split,
        splitter.index,
        start_position,
        splitter.extent,
        splitter.shares.clone(),
    ))
}
