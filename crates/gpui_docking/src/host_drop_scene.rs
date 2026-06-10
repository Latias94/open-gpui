use crate::{
    DockHost, DockNodeId,
    drag::DockDragPayload,
    drop_runtime::{DockHostDropScene, DockHostDropSceneFact},
    drop_scene_fact,
    host_interaction_outcome::DockHostInteractionOutcome,
};
use open_gpui::{Bounds, Context, Pixels, Point, Window};

impl DockHost {
    pub(crate) fn begin_host_drop_scene_interaction(
        &mut self,
        payload: &DockDragPayload,
        position: Point<Pixels>,
        cx: &Context<Self>,
    ) -> DockHostInteractionOutcome {
        let policy = self.with_workspace(cx, |workspace| *workspace.policy());
        DockHostInteractionOutcome::from_session_changed(self.interaction_mut().begin_drop_scene(
            DockHostDropScene::new(position).excluding_tabs(payload.excluded_tabs_for_drop_scene()),
            &policy,
        ))
    }

    pub(crate) fn update_leaf_drop_scene_interaction(
        &mut self,
        payload: &DockDragPayload,
        root: DockNodeId,
        target_tabs: DockNodeId,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        is_central: bool,
        window: &Window,
        cx: &Context<Self>,
    ) -> DockHostInteractionOutcome {
        let policy = self.with_workspace(cx, |workspace| *workspace.policy());
        let fact = drop_scene_fact::leaf(root, target_tabs, bounds, is_central);
        DockHostInteractionOutcome::from_session_changed(self.push_drop_scene_fact_interaction(
            position,
            payload.excluded_tabs_for_drop_scene(),
            fact,
            window,
            &policy,
        ))
    }

    pub(crate) fn update_tab_label_drop_scene_interaction(
        &mut self,
        payload: &DockDragPayload,
        target_tabs: DockNodeId,
        target_index: usize,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        is_central: bool,
        window: &Window,
        cx: &Context<Self>,
    ) -> DockHostInteractionOutcome {
        let policy = self.with_workspace(cx, |workspace| *workspace.policy());
        let fact = drop_scene_fact::tab_label(target_tabs, target_index, bounds, is_central);
        DockHostInteractionOutcome::from_session_changed(self.push_drop_scene_fact_interaction(
            position,
            payload.excluded_tabs_for_drop_scene(),
            fact,
            window,
            &policy,
        ))
    }

    pub(crate) fn update_root_drop_scene_interaction(
        &mut self,
        payload: &DockDragPayload,
        root: DockNodeId,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        window: &Window,
        cx: &Context<Self>,
    ) -> DockHostInteractionOutcome {
        let policy = self.with_workspace(cx, |workspace| *workspace.policy());
        let fact = drop_scene_fact::root(root, bounds);
        DockHostInteractionOutcome::from_session_changed(self.push_drop_scene_fact_interaction(
            position,
            payload.excluded_tabs_for_drop_scene(),
            fact,
            window,
            &policy,
        ))
    }

    pub(crate) fn update_empty_space_drop_scene_interaction(
        &mut self,
        payload: &DockDragPayload,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        window: &Window,
        cx: &Context<Self>,
    ) -> DockHostInteractionOutcome {
        let policy = self.with_workspace(cx, |workspace| *workspace.policy());
        let space = self.space().clone();
        let fact = drop_scene_fact::empty_space(space, bounds);
        DockHostInteractionOutcome::from_session_changed(self.push_drop_scene_fact_interaction(
            position,
            payload.excluded_tabs_for_drop_scene(),
            fact,
            window,
            &policy,
        ))
    }

    pub(crate) fn update_floating_title_bar_drop_scene_interaction(
        &mut self,
        payload: &DockDragPayload,
        floating: DockNodeId,
        target_tabs: DockNodeId,
        title_bounds: Bounds<Pixels>,
        preview_bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        window: &Window,
        cx: &Context<Self>,
    ) -> DockHostInteractionOutcome {
        let policy = self.with_workspace(cx, |workspace| *workspace.policy());
        let fact = drop_scene_fact::floating_title_bar(
            floating,
            target_tabs,
            title_bounds,
            preview_bounds,
        );
        DockHostInteractionOutcome::from_session_changed(self.push_drop_scene_fact_interaction(
            position,
            payload.excluded_tabs_for_drop_scene(),
            fact,
            window,
            &policy,
        ))
    }

    fn push_drop_scene_fact_interaction(
        &mut self,
        position: Point<Pixels>,
        excluded_tabs: Option<DockNodeId>,
        fact: DockHostDropSceneFact,
        window: &Window,
        policy: &crate::DockPolicy,
    ) -> bool {
        let viewport_runtime = self.viewport_runtime().cloned();
        let frame = self.interaction().viewport_host_scene_frame().cloned();
        if let (Some(runtime), Some(frame)) = (viewport_runtime, frame) {
            if frame.matches_viewport(self.space(), window.window_handle().window_id()) {
                runtime.push_viewport_host_scene_frame_fact(&frame, fact.clone());
            }
        }
        self.interaction_mut()
            .push_drop_scene_fact(position, excluded_tabs, fact, policy)
    }
}
