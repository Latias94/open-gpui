use super::*;
use crate::DockViewportHostGeometry;

impl DockViewportRuntimeHandle {
    #[cfg(test)]
    pub(crate) fn begin_viewport_host_scene(
        &self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        host_geometry: impl Into<DockViewportHostGeometry>,
        host_position: Point<Pixels>,
    ) -> bool {
        self.runtime.borrow_mut().begin_viewport_host_scene(
            space,
            window_id,
            window_facts,
            host_geometry,
            host_position,
        )
    }

    pub(crate) fn unregister_host_for_space_with_app(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
        cx: &mut App,
    ) -> bool {
        let update = self
            .runtime
            .borrow_mut()
            .unregister_host_for_space_with_cleanup(space, window_id);
        apply_runtime_update(self, update, cx)
    }

    pub(crate) fn begin_viewport_host_scene_frame(
        &self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        host_geometry: impl Into<DockViewportHostGeometry>,
        host_position: Point<Pixels>,
        drop_guide_metrics: crate::DockDropGuideMetrics,
    ) -> Option<DockViewportHostSceneRegistration> {
        self.begin_viewport_host_scene_frame_with_facts(
            space,
            window_id,
            window_facts,
            host_geometry,
            host_position,
            drop_guide_metrics,
            Vec::new(),
        )
    }

    pub(crate) fn begin_viewport_host_scene_frame_with_facts(
        &self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        host_geometry: impl Into<DockViewportHostGeometry>,
        host_position: Point<Pixels>,
        drop_guide_metrics: crate::DockDropGuideMetrics,
        initial_facts: Vec<DockHostDropSceneFact>,
    ) -> Option<DockViewportHostSceneRegistration> {
        self.runtime
            .borrow_mut()
            .begin_viewport_host_scene_frame_with_facts(
                space,
                window_id,
                window_facts,
                host_geometry,
                host_position,
                drop_guide_metrics,
                initial_facts,
            )
    }

    pub(crate) fn commit_rendered_viewport_host_scene_snapshot(
        &self,
        snapshot: DockViewportHostSceneSnapshot,
        window: &mut Window,
        cx: &mut App,
        passthrough_pointer_input: bool,
    ) -> DockViewportRenderedHostScenePreparation {
        let space = snapshot.space.clone();
        let window_id = window.window_handle().window_id();
        let backend_focus_changed = self.reconcile_backend_window_focus(cx);
        let viewport_frame_changed = self.reconcile_viewport_frame_except_window(window_id, cx);
        let accepts_pointer_input = window.platform_facts().accepts_pointer_input;
        let pending_pointer_input = self
            .pending_platform_mutation_request(window_id, WindowMutationDomain::PointerInput)
            .and_then(|request| match request {
                WindowMutationRequest::PointerInput(accepts_pointer_input) => {
                    Some(accepts_pointer_input)
                }
                WindowMutationRequest::Placement(_)
                | WindowMutationRequest::FocusOnAppearing(_)
                | WindowMutationRequest::FocusOnClick(_)
                | WindowMutationRequest::Alpha(_)
                | WindowMutationRequest::Topmost(_)
                | WindowMutationRequest::TaskbarVisibility(_) => None,
            });
        let pointer_input_resolution = {
            let mut runtime = self.runtime.borrow_mut();
            resolve_render_passthrough_pointer_input_request(
                &mut runtime,
                window_id,
                accepts_pointer_input,
                pending_pointer_input,
                passthrough_pointer_input,
            )
        };
        let retry_blocked = pointer_input_resolution.target.is_some_and(|target| {
            self.platform_mutation_retry_is_blocked(
                window_id,
                WindowMutationRequest::PointerInput(target),
                window.platform_facts(),
            )
        });
        let pointer_input_request = pointer_input_resolution.request.filter(|_| !retry_blocked);
        let pointer_sync = pointer_input_request.and_then(|accepts_pointer_input| {
            (pending_pointer_input != Some(accepts_pointer_input))
                .then(|| sync_pointer_input_window(window, accepts_pointer_input))
        });
        let pointer_sync_changed = pointer_sync.as_ref().is_some_and(|result| {
            result.record().dispatches.iter().any(|dispatch| {
                matches!(
                    dispatch,
                    crate::DockViewportPlatformSyncDispatch::Queued { .. }
                )
            })
        });
        if let Some(pointer_sync) = pointer_sync {
            self.record_platform_dispatch_result(pointer_sync, window.platform_facts());
        }
        let registration_update = self
            .register_rendered_host_viewport_with_cleanup(space.clone(), window.window_handle());
        let registration_changed =
            refresh_runtime_update_with_commit(self, registration_update, cx);
        let (registration, route_preview_update) = {
            let mut runtime = self.runtime.borrow_mut();
            let registration = runtime.commit_viewport_host_scene_snapshot(
                snapshot,
                DockViewportWindowFacts::from_window(window, cx),
            );
            let route_preview_update = runtime.clear_preview_for_unready_window_route(window_id);
            (registration, route_preview_update)
        };
        let route_preview_changed = refresh_runtime_update(route_preview_update, cx);
        let (host_scene_changed, frame) = registration
            .map(|registration| (registration.changed, Some(registration.frame)))
            .unwrap_or((false, None));
        DockViewportRenderedHostScenePreparation {
            changed: backend_focus_changed
                || viewport_frame_changed
                || pointer_sync_changed
                || registration_changed
                || route_preview_changed
                || host_scene_changed,
            frame,
        }
    }

    pub(crate) fn register_rendered_host_viewport_with_cleanup(
        &self,
        space: DockSpaceId,
        window: AnyWindowHandle,
    ) -> DockViewportRuntimeUpdate {
        self.runtime
            .borrow_mut()
            .register_rendered_host_viewport_with_cleanup(space, window)
    }

    pub(crate) fn discard_rendered_viewport_host_scene_frame(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> bool {
        self.runtime
            .borrow_mut()
            .discard_viewport_host_scene_frame(space, window_id)
    }

    pub(crate) fn reconcile_viewport_frame<C: open_gpui::AppContext>(&self, cx: &mut C) -> bool {
        let update = self.runtime.borrow_mut().reconcile_viewport_frame(cx);
        refresh_runtime_update(update, cx)
    }

    pub(crate) fn reconcile_viewport_frame_except_window<C: open_gpui::AppContext>(
        &self,
        skip_window_id: WindowId,
        cx: &mut C,
    ) -> bool {
        let update = self
            .runtime
            .borrow_mut()
            .reconcile_viewport_frame_except_window(Some(skip_window_id), cx);
        refresh_runtime_update(update, cx)
    }

    #[cfg(test)]
    pub(crate) fn push_viewport_host_scene_fact(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
        fact: DockHostDropSceneFact,
    ) -> bool {
        self.runtime
            .borrow_mut()
            .push_viewport_host_scene_fact(space, window_id, fact)
    }

    pub(crate) fn push_viewport_host_scene_frame_fact(
        &self,
        frame: &DockViewportHostSceneFrame,
        fact: DockHostDropSceneFact,
    ) -> Option<DockViewportHostSceneFrame> {
        self.runtime
            .borrow_mut()
            .push_viewport_host_scene_frame_fact(frame, fact)
    }

    pub(crate) fn rendered_leaf_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
    ) -> Option<Bounds<Pixels>> {
        self.runtime
            .borrow()
            .rendered_leaf_bounds_for_tabs(space, window_id, tabs)
    }

    pub(crate) fn rendered_leaf_displayed_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
    ) -> Option<Bounds<Pixels>> {
        self.runtime
            .borrow()
            .rendered_leaf_displayed_bounds_for_tabs(space, window_id, tabs)
    }

    pub(crate) fn rendered_tab_bar_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
    ) -> Option<Bounds<Pixels>> {
        self.runtime
            .borrow()
            .rendered_tab_bar_bounds_for_tabs(space, window_id, tabs)
    }

    pub(crate) fn rendered_tab_label_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
        target_index: usize,
    ) -> Option<Bounds<Pixels>> {
        self.runtime.borrow().rendered_tab_label_bounds_for_tabs(
            space,
            window_id,
            tabs,
            target_index,
        )
    }

    pub(crate) fn window_id_for_space(&self, space: &DockSpaceId) -> Option<WindowId> {
        self.runtime
            .borrow()
            .adapter()
            .window_for_space(space)
            .map(|window| window.window_id())
    }
}
