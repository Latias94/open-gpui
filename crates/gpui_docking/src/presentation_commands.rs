use crate::{
    DockHost, DockItemId, DockNode, DockNodeId, DockViewportFocusCommand, DockViewportFocusRequest,
    presentation_scene::DockPresentationScene,
    spatial_navigation::{self, DockSpatialDirection},
    transition_executor::{DockTransitionExecutionState, DockTransitionSample},
    transition_geometry::{DockMotionPreference, DockTransitionPlan},
};
use open_gpui::{Context, Window};
use open_gpui_ui_core::MotionSpec;

impl DockHost {
    /// Presents one pane as a zoomed full-host pane without mutating the dock graph.
    pub fn zoom_pane(&mut self, target: DockNodeId, cx: &mut Context<Self>) -> bool {
        if let Some(previous) = self.last_presentation_scene().cloned() {
            return self.zoom_pane_with_scene(
                target,
                previous,
                MotionSpec::layout(DockMotionPreference::Animated),
                None,
                cx,
            );
        }

        if self.zoom_state().target(self.space()) == Some(target) {
            return false;
        }

        let space = self.space().clone();
        self.zoom_state_mut().zoom(space, target);
        cx.notify();
        true
    }

    pub(crate) fn zoom_pane_with_scene(
        &mut self,
        target: DockNodeId,
        previous: DockPresentationScene,
        spec: MotionSpec,
        window: Option<&Window>,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.zoom_state().target(self.space()) == Some(target) {
            return false;
        }

        let space = self.space().clone();
        self.zoom_state_mut().zoom(space, target);
        let Some(zoom_scene) = self.zoom_state().resolve(&previous, spec.preference()) else {
            cx.notify();
            return true;
        };
        let plan = DockTransitionPlan::from_zoom_scene(&previous, &zoom_scene, spec.preference());
        self.set_last_presentation_scene(zoom_scene.scene.clone());
        self.execute_transition_plan(plan, spec, window, cx);
        true
    }

    /// Clears the presentation-only zoom state for this host.
    pub fn unzoom(&mut self, cx: &mut Context<Self>) -> bool {
        if let Some(previous) = self.last_presentation_scene().cloned() {
            let final_scene = DockPresentationScene::from_render_session(
                &self.render_session(cx),
                previous.bounds,
            );
            return self.unzoom_with_scene(
                previous,
                final_scene,
                MotionSpec::layout(DockMotionPreference::Animated),
                None,
                cx,
            );
        }

        let space = self.space().clone();
        if self.zoom_state_mut().unzoom(&space).is_none() {
            return false;
        }

        cx.notify();
        true
    }

    pub(crate) fn unzoom_with_scene(
        &mut self,
        previous: DockPresentationScene,
        final_scene: DockPresentationScene,
        spec: MotionSpec,
        window: Option<&Window>,
        cx: &mut Context<Self>,
    ) -> bool {
        let space = self.space().clone();
        if self.zoom_state_mut().unzoom(&space).is_none() {
            return false;
        }

        let plan = DockTransitionPlan::between(&previous, &final_scene, spec.preference());
        self.set_last_presentation_scene(final_scene.clone());
        self.execute_transition_plan(plan, spec, window, cx);
        true
    }

    /// Toggles presentation zoom for one pane.
    pub fn toggle_zoom_pane(&mut self, target: DockNodeId, cx: &mut Context<Self>) -> bool {
        if self.zoom_state().target(self.space()) == Some(target) {
            self.unzoom(cx)
        } else {
            self.zoom_pane(target, cx)
        }
    }

    /// Requests focus for the selected item inside one tabs pane.
    pub fn focus_pane(&mut self, target: DockNodeId, cx: &mut Context<Self>) -> bool {
        let scene = self.last_presentation_scene().cloned();
        let Some((item, changed)) = self.request_focus_pane_command(target, cx) else {
            return false;
        };
        if changed && let Some(scene) = scene {
            self.execute_focus_ring_transition(
                target,
                &item,
                scene,
                MotionSpec::immediate(),
                None,
                cx,
            );
        }
        changed
    }

    /// Requests focus for the nearest pane in a direction from the current tabs pane.
    pub fn focus_neighbor_pane(
        &mut self,
        current_tabs: DockNodeId,
        direction: DockSpatialDirection,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(scene) = self.last_presentation_scene().cloned() else {
            return false;
        };
        let Some(target) = spatial_navigation::resolve_neighbor(&scene, current_tabs, direction)
        else {
            return false;
        };
        self.focus_pane_with_scene(target.tabs, scene, MotionSpec::immediate(), None, cx)
    }

    fn request_focus_pane_command(
        &mut self,
        target: DockNodeId,
        cx: &mut Context<Self>,
    ) -> Option<(DockItemId, bool)> {
        let Some(item) = self.selected_item_for_tabs(target, cx) else {
            return None;
        };
        self.viewport_runtime()
            .record_panel_focus(self.space().clone(), item.clone());
        let changed =
            self.request_viewport_focus_command(DockViewportFocusCommand::viewport_activation(
                DockViewportFocusRequest::panel(item.clone()),
            ));
        if changed {
            cx.notify();
        }
        Some((item, changed))
    }

    pub(crate) fn focus_pane_with_scene(
        &mut self,
        target: DockNodeId,
        scene: DockPresentationScene,
        spec: MotionSpec,
        window: Option<&Window>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((item, changed)) = self.request_focus_pane_command(target, cx) else {
            return false;
        };
        if !changed {
            return false;
        }
        self.execute_focus_ring_transition(target, &item, scene, spec, window, cx);
        true
    }

    fn execute_focus_ring_transition(
        &mut self,
        target: DockNodeId,
        item: &DockItemId,
        scene: DockPresentationScene,
        spec: MotionSpec,
        window: Option<&Window>,
        cx: &mut Context<Self>,
    ) {
        if let Some(focus) = scene
            .focus_regions
            .iter()
            .find(|focus| focus.tabs == target && &focus.item == item)
        {
            let plan = DockTransitionPlan::from_focus_region(&scene, focus, spec.preference());
            self.execute_transition_plan(plan, spec, window, cx);
        }
    }

    /// Executes a transition plan through the docking adapter-owned motion executor.
    /// Executes a docking transition plan through the adapter-owned motion executor.
    pub fn execute_transition_plan(
        &mut self,
        plan: DockTransitionPlan,
        spec: MotionSpec,
        window: Option<&Window>,
        cx: &mut Context<Self>,
    ) -> DockTransitionExecutionState {
        let state = self
            .transition_executor_mut()
            .execute(plan, spec, window)
            .state;
        cx.notify();
        state
    }

    pub(crate) fn sample_transition_for_render(
        &mut self,
        window: Option<&Window>,
    ) -> Option<DockTransitionSample> {
        self.transition_executor_mut().sample(window)
    }

    pub(crate) fn execute_visual_affordance_transition_plan(
        &mut self,
        plan: DockTransitionPlan,
        spec: MotionSpec,
        window: Option<&Window>,
    ) -> DockTransitionExecutionState {
        self.visual_affordance_transition_executor_mut()
            .execute(plan, spec, window)
            .state
    }

    pub(crate) fn sample_visual_affordance_transition_for_render(
        &mut self,
        window: Option<&Window>,
    ) -> Option<DockTransitionSample> {
        self.visual_affordance_transition_executor_mut()
            .sample(window)
    }

    pub(crate) fn clear_visual_affordance_transition_for_render(&mut self) -> bool {
        let scene_cleared = self.clear_last_visual_affordance_scene();
        let execution_cleared = self
            .visual_affordance_transition_executor_mut()
            .clear()
            .is_some();
        scene_cleared || execution_cleared
    }

    #[cfg(test)]
    pub(crate) fn clear_transition_execution_for_test(
        &mut self,
    ) -> Option<crate::transition_executor::DockTransitionExecution> {
        self.transition_executor_mut().clear()
    }

    #[cfg(test)]
    pub(crate) fn sample_transition_for_test(
        &mut self,
        now: std::time::Duration,
    ) -> Option<DockTransitionSample> {
        self.transition_executor_mut().sample_for_test(now)
    }

    #[cfg(test)]
    pub(crate) fn sample_visual_affordance_transition_for_test(
        &mut self,
        now: std::time::Duration,
    ) -> Option<DockTransitionSample> {
        self.visual_affordance_transition_executor_mut()
            .sample_for_test(now)
    }

    #[cfg(test)]
    pub(crate) fn zoom_target_for_test(&self) -> Option<DockNodeId> {
        self.zoom_state().target(self.space())
    }

    fn selected_item_for_tabs(&self, tabs: DockNodeId, cx: &Context<Self>) -> Option<DockItemId> {
        self.with_workspace(cx, |workspace| {
            let DockNode::Tabs { items, selected } = workspace.graph().node(tabs)? else {
                return None;
            };
            selected.clone().or_else(|| items.first().cloned())
        })
    }
}
