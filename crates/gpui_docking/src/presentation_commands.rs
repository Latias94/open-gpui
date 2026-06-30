use crate::{
    DockHost, DockItemId, DockNode, DockNodeId, DockViewportFocusCommand, DockViewportFocusRequest,
    transition_executor::DockTransitionExecutionState, transition_geometry::DockTransitionPlan,
};
use open_gpui::{Context, Window};
use open_gpui_ui_core::MotionSpec;

impl DockHost {
    /// Presents one pane as a zoomed full-host pane without mutating the dock graph.
    pub fn zoom_pane(&mut self, target: DockNodeId, cx: &mut Context<Self>) -> bool {
        if self.zoom_state().target(self.space()) == Some(target) {
            return false;
        }

        let space = self.space().clone();
        self.zoom_state_mut().zoom(space, target);
        cx.notify();
        true
    }

    /// Clears the presentation-only zoom state for this host.
    pub fn unzoom(&mut self, cx: &mut Context<Self>) -> bool {
        let space = self.space().clone();
        if self.zoom_state_mut().unzoom(&space).is_none() {
            return false;
        }

        cx.notify();
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
        let Some(item) = self.selected_item_for_tabs(target, cx) else {
            return false;
        };
        self.viewport_runtime()
            .record_panel_focus(self.space().clone(), item.clone());
        let changed = self.request_viewport_focus_command(
            DockViewportFocusCommand::viewport_activation(DockViewportFocusRequest::panel(item)),
        );
        if changed {
            cx.notify();
        }
        changed
    }

    /// Executes a transition plan through the docking adapter-owned motion executor.
    /// Executes a docking transition plan through the adapter-owned motion executor.
    pub fn execute_transition_plan(
        &mut self,
        plan: DockTransitionPlan,
        spec: MotionSpec,
        window: Option<&Window>,
    ) -> DockTransitionExecutionState {
        self.transition_executor_mut()
            .execute(plan, spec, window)
            .state
    }

    #[cfg(test)]
    pub(crate) fn clear_transition_execution_for_test(
        &mut self,
    ) -> Option<crate::transition_executor::DockTransitionExecution> {
        self.transition_executor_mut().clear()
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
