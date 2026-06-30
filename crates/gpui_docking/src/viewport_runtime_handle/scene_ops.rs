use super::*;

impl DockViewportRuntimeHandle {
    #[cfg(test)]
    pub(crate) fn begin_viewport_host_scene(
        &self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        host_bounds: Bounds<Pixels>,
        host_position: Point<Pixels>,
    ) -> bool {
        self.runtime.borrow_mut().begin_viewport_host_scene(
            space,
            window_id,
            window_facts,
            host_bounds,
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
            .unregister_host_for_space_with_pointer_sync(space, window_id);
        apply_pointer_synced_runtime_update(self, update, cx)
    }

    pub(crate) fn begin_viewport_host_scene_frame(
        &self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        host_bounds: Bounds<Pixels>,
        host_position: Point<Pixels>,
        drop_guide_style: crate::DockDropGuideStyle,
    ) -> Option<DockViewportHostSceneRegistration> {
        self.runtime.borrow_mut().begin_viewport_host_scene_frame(
            space,
            window_id,
            window_facts,
            host_bounds,
            host_position,
            drop_guide_style,
        )
    }

    pub(crate) fn prepare_rendered_viewport_host_scene_frame(
        &self,
        space: DockSpaceId,
        window: &mut Window,
        cx: &mut App,
        host_bounds: Bounds<Pixels>,
        host_position: Point<Pixels>,
        drop_guide_style: crate::DockDropGuideStyle,
        passthrough_pointer_input: bool,
    ) -> DockViewportRenderedHostScenePreparation {
        let window_id = window.window_handle().window_id();
        let backend_focus_changed = self.reconcile_backend_window_focus(cx);
        let viewport_frame_changed = self.reconcile_viewport_frame_except_window(window_id, cx);
        let pointer_sync_changed = sync_render_passthrough_pointer_input_for_runtime(
            &mut self.runtime.borrow_mut(),
            window,
            passthrough_pointer_input,
            cx.viewport_capabilities(),
        );
        let registration_update = self
            .register_rendered_host_viewport_with_cleanup(space.clone(), window.window_handle());
        let registration_changed = refresh_runtime_update(registration_update, cx);
        let registration = self.begin_viewport_host_scene_frame(
            space.clone(),
            window_id,
            DockViewportWindowFacts::from_window(window, cx),
            host_bounds,
            host_position,
            drop_guide_style,
        );
        let (host_scene_changed, frame) = registration
            .map(|registration| (registration.changed, Some(registration.frame)))
            .unwrap_or((false, None));
        DockViewportRenderedHostScenePreparation {
            changed: backend_focus_changed
                || viewport_frame_changed
                || pointer_sync_changed
                || registration_changed
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
