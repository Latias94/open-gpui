use crate::{
    DockHost, DockItemId, DockNodeId, DockSpaceId, DockViewportHostGeometry, SplitAxis,
    divider_hit_map::{DockDividerHandleHitTarget, DockDividerHitTarget, DockDividerSurface},
    drag::{DockDragPayload, DockDragTearOffGeometry},
    drop_runtime::DockHostDropSceneFact,
    interaction::{DockPayloadDropRelease, DockRuntimeDragSession, SplitterDragAxis},
    presentation_scene::DockPresentationScene,
    viewport_drop_scene::DockViewportHostSceneFrame,
};
use open_gpui::{Bounds, Context, DragStartGeometry, Pixels, Point, PointerCancelReason, Window};
use open_gpui_ui_core::AccessibleAction;

const ACCESSIBILITY_SPLITTER_STEP_PX: f32 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockRenderedPointerPosition {
    pub(crate) layout: Point<Pixels>,
    pub(crate) window: Point<Pixels>,
}

impl DockRenderedPointerPosition {
    pub(crate) fn new(layout: Point<Pixels>, window: Point<Pixels>) -> Self {
        Self { layout, window }
    }
}

#[cfg(test)]
impl From<Point<Pixels>> for DockRenderedPointerPosition {
    fn from(position: Point<Pixels>) -> Self {
        Self::new(position, position)
    }
}

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

    #[cfg(test)]
    pub(crate) fn begin_payload_drag_from_render(
        &mut self,
        payload: &DockDragPayload,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> DockRuntimeDragSession {
        self.begin_payload_drag_interaction(
            payload,
            crate::DockVisualStyle::built_in().drag,
            None,
            window,
            cx,
        )
    }

    pub(crate) fn begin_payload_drag_from_render_with_drag_visual_style(
        &mut self,
        payload: &DockDragPayload,
        drag_visual_style: crate::DockDragVisualStyle,
        drag_start: &DragStartGeometry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> DockRuntimeDragSession {
        self.begin_payload_drag_interaction(
            payload,
            drag_visual_style,
            Some(drag_start),
            window,
            cx,
        )
    }

    #[cfg(test)]
    pub(crate) fn begin_tab_item_drag_from_render(
        &mut self,
        tabs: DockNodeId,
        item: DockItemId,
        payload: &DockDragPayload,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> DockRuntimeDragSession {
        let begin = self.begin_tab_item_drag_interaction(
            tabs,
            item,
            payload,
            crate::DockVisualStyle::built_in().drag,
            None,
            window,
            cx,
        );
        begin.outcome.finish(cx);
        begin.drag_session
    }

    pub(crate) fn begin_tab_item_drag_from_render_with_drag_visual_style(
        &mut self,
        tabs: DockNodeId,
        item: DockItemId,
        payload: &DockDragPayload,
        drag_visual_style: crate::DockDragVisualStyle,
        drag_start: &DragStartGeometry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> DockRuntimeDragSession {
        let begin = self.begin_tab_item_drag_interaction(
            tabs,
            item,
            payload,
            drag_visual_style,
            Some(drag_start),
            window,
            cx,
        );
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let changed = self
            .viewport_runtime()
            .finish_payload_drag_from_window(session, window, cx);
        let anchor_cleared = self.interaction_mut().clear_any_payload_drag_anchor();
        changed || anchor_cleared
    }

    pub(crate) fn cancel_payload_drag_from_render(
        &mut self,
        payload: &DockDragPayload,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let state_changed = self.cancel_payload_drag_state_from_render(
            payload,
            PointerCancelReason::CaptureRevoked,
            window,
            cx,
        );
        let active_drag_cleared = cx.stop_active_drag(window);
        state_changed || active_drag_cleared
    }

    fn cancel_payload_drag_state_from_render(
        &mut self,
        payload: &DockDragPayload,
        reason: PointerCancelReason,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let drag_session = self.active_payload_drag_session(payload);
        crate::native_captured_drag::cancel_native_captured_drag_route(
            self.viewport_runtime().identity(),
            drag_session.as_ref(),
            Some(payload),
            &cx.entity().downgrade(),
            self.current_window_binding(),
            reason,
            cx,
        );
        let session_changed = drag_session
            .as_ref()
            .is_some_and(|session| self.finish_payload_drag_session(session, window, cx));
        let anchor_cleared = self.interaction_mut().clear_any_payload_drag_anchor();
        let local_preview_cleared = self.clear_drop_preview_interaction();
        let routed_preview_cleared = self
            .viewport_runtime()
            .clear_routed_drop_preview_from_window(window, cx);
        session_changed || anchor_cleared || local_preview_cleared || routed_preview_cleared
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

    pub(crate) fn drop_payload_event_from_render(
        &mut self,
        payload: &DockDragPayload,
        target_space: DockSpaceId,
        position: DockRenderedPointerPosition,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let drag_session = self.active_payload_drag_session(payload);
        if crate::native_captured_drag::owns_native_captured_drag_source(
            self.viewport_runtime().identity(),
            drag_session.as_ref(),
            payload,
            window.window_handle().window_id(),
            &cx.entity().downgrade(),
            self.current_window_binding(),
            cx,
        ) {
            return false;
        }

        let event_receiver_local_scene_proof =
            self.interaction().viewport_host_scene_frame().cloned();
        self.drop_payload_release_from_render(
            DockPayloadDropRelease::hovered_host_with_positions(
                payload.clone(),
                target_space,
                position.layout,
                position.window,
                drag_session,
            )
            .with_event_receiver_local_scene_proof(event_receiver_local_scene_proof),
            window,
            cx,
        )
    }

    #[cfg(test)]
    pub(crate) fn drop_payload_release_from_rendered_host_scene(
        &mut self,
        payload: DockDragPayload,
        position: impl Into<DockRenderedPointerPosition>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.drop_payload_event_from_render(
            &payload,
            self.space().clone(),
            position.into(),
            window,
            cx,
        )
    }

    pub(crate) fn commit_payload_drop_release(
        &mut self,
        release: DockPayloadDropRelease,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let drag_session = release.drag_session().cloned();
        let outcome = self.commit_payload_drop_interaction(release, window, cx);
        let changed = outcome.finish_from_window(self, window, cx);
        let session_changed = drag_session
            .as_ref()
            .is_some_and(|session| self.finish_payload_drag_session(session, window, cx));
        changed || session_changed
    }

    pub(crate) fn update_local_drop_scene_fact_from_render(
        &mut self,
        payload: &DockDragPayload,
        fact: DockHostDropSceneFact,
        position: impl Into<DockRenderedPointerPosition>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.update_drop_scene_fact_interaction(payload, fact, position, window, cx)
            .finish(cx)
    }

    pub(crate) fn begin_host_drop_scene_from_render(
        &mut self,
        payload: &DockDragPayload,
        host_geometry: impl Into<DockViewportHostGeometry>,
        position: impl Into<DockRenderedPointerPosition>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let position = position.into();
        if !self.publish_viewport_host_scene_interaction(host_geometry, position.window, window, cx)
        {
            let local_preview_cleared = self.clear_drop_preview_interaction();
            let routed_preview_cleared = self
                .viewport_runtime()
                .clear_routed_drop_preview_from_window(window, cx);
            return crate::host_interaction_outcome::DockHostInteractionOutcome::from_session_changed(
                local_preview_cleared || routed_preview_cleared,
            )
            .finish(cx);
        }
        self.update_payload_drag_hover_state_from_render(payload, position, window, cx)
            .merge(self.ensure_host_drop_scene_interaction(payload, position.layout, cx))
            .finish(cx)
    }

    #[cfg(test)]
    pub(crate) fn update_payload_drag_hover_from_rendered_host_scene(
        &mut self,
        payload: &DockDragPayload,
        position: impl Into<DockRenderedPointerPosition>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let position = position.into();
        let local_preview_cleared =
            if self.interaction().drop_scene_position() == Some(position.layout) {
                false
            } else {
                self.clear_drop_preview_interaction()
            };
        let changed = self
            .update_payload_drag_hover_state_from_render(payload, position, window, cx)
            .merge(
                crate::host_interaction_outcome::DockHostInteractionOutcome::from_session_changed(
                    local_preview_cleared,
                ),
            )
            .finish(cx);
        changed
    }

    pub(crate) fn publish_rendered_viewport_host_scene_frame_from_render(
        &mut self,
        frame: Option<DockViewportHostSceneFrame>,
        window: &Window,
    ) -> bool {
        let window_id = window.window_handle().window_id();
        let frame = frame.filter(|frame| frame.matches_viewport(self.space(), window_id));
        self.interaction_mut().set_viewport_host_scene_frame(frame)
    }

    pub(crate) fn update_local_root_drop_scene_from_render(
        &mut self,
        payload: &DockDragPayload,
        root: DockNodeId,
        bounds: Bounds<Pixels>,
        position: impl Into<DockRenderedPointerPosition>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.update_root_drop_scene_interaction(payload, root, bounds, position, window, cx)
            .finish(cx)
    }

    pub(crate) fn update_local_empty_space_drop_scene_from_render(
        &mut self,
        payload: &DockDragPayload,
        position: impl Into<DockRenderedPointerPosition>,
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
        let outcome = self.begin_floating_drag_interaction(
            space,
            floating,
            start_position,
            initial_bounds,
            cx,
        );
        let active = self.interaction().floating_drag_active();
        outcome.finish(cx);
        active
    }

    pub(crate) fn update_floating_drag_from_render(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let bounds = self.interaction().floating_bounds_request(position)?.bounds;
        self.update_floating_drag_interaction(position, cx)
            .finish(cx);
        Some(bounds)
    }

    pub(crate) fn finish_floating_drag_from_render(&mut self, cx: &mut Context<Self>) -> bool {
        self.finish_floating_drag_interaction().finish(cx)
    }

    pub(crate) fn finish_raw_pointer_drag_from_render(&mut self, cx: &mut Context<Self>) -> bool {
        let splitter_changed = self.finish_splitter_drag_from_render(cx);
        let floating_changed = self.finish_floating_drag_from_render(cx);
        splitter_changed || floating_changed
    }

    pub(crate) fn cancel_pointer_interactions_from_render(
        &mut self,
        payload: Option<&DockDragPayload>,
        reason: PointerCancelReason,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let raw_drag_changed = self.finish_raw_pointer_drag_from_render(cx);
        let payload_changed = if let Some(payload) = payload {
            self.cancel_payload_drag_state_from_render(payload, reason, window, cx)
        } else {
            crate::native_captured_drag::cancel_native_captured_drag_route(
                self.viewport_runtime().identity(),
                None,
                None,
                &cx.entity().downgrade(),
                self.current_window_binding(),
                reason,
                cx,
            );
            let source_space = self.space().clone();
            let session_cleared = self
                .viewport_runtime()
                .finish_payload_drag_for_source_space_from_window(&source_space, window, cx);
            let anchor_cleared = self.interaction_mut().clear_any_payload_drag_anchor();
            let local_preview_cleared = self.clear_drop_preview_interaction();
            let routed_preview_cleared = self
                .viewport_runtime()
                .clear_routed_drop_preview_from_window(window, cx);
            session_cleared || anchor_cleared || local_preview_cleared || routed_preview_cleared
        };
        raw_drag_changed || payload_changed
    }

    pub(crate) fn begin_divider_drag_from_scene(
        &mut self,
        scene: &DockPresentationScene,
        target: &DockDividerHitTarget,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let outcome = match target {
            DockDividerHitTarget::Single(handle) => {
                self.begin_splitter_drag_axis_from_scene(scene, handle, position)
            }
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
            }
        };
        let active = self.interaction().splitter_drag_active();
        outcome.finish(cx);
        active
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
                before: splitter.before,
                after: splitter.after,
                bounds: splitter.bounds,
                extent: splitter.extent,
                surface: splitter
                    .floating
                    .map(DockDividerSurface::Floating)
                    .unwrap_or(DockDividerSurface::Root),
            },
            start_position,
        )
        .merge(self.update_splitter_drag_interaction(target_position, cx))
        .merge(self.finish_splitter_drag_interaction())
        .finish(cx)
    }

    fn update_payload_drag_hover_state_from_render(
        &mut self,
        payload: &DockDragPayload,
        position: DockRenderedPointerPosition,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> crate::host_interaction_outcome::DockHostInteractionOutcome {
        self.update_floating_drag_interaction(position.layout, cx)
            .merge(self.update_viewport_drop_route_preview_interaction(
                payload,
                position.window,
                window,
                cx,
            ))
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
