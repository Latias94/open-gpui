use crate::{
    DockAction, DockActionOutcome, DockHost, DockItemId, DockNodeId, DockSpaceId,
    workspace_transaction::DockWorkspaceDropRequest,
};
use open_gpui::{Bounds, Context, Pixels, Point};

impl DockHost {
    pub(crate) fn select_tab_from_render(
        &mut self,
        tabs: DockNodeId,
        item: DockItemId,
        cx: &mut Context<Self>,
    ) -> bool {
        self.commit_action_from_render(DockAction::SelectTab { tabs, item }, cx)
    }

    pub(crate) fn drop_tab_from_render(
        &mut self,
        source_space: DockSpaceId,
        source_tabs: DockNodeId,
        item: DockItemId,
        target_space: DockSpaceId,
        target_tabs: DockNodeId,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(target) = self.take_tab_drop_target(target_tabs) else {
            cx.notify();
            return false;
        };

        self.commit_resolved_drop_from_render(
            DockWorkspaceDropRequest {
                source_space: &source_space,
                source_tabs,
                item: &item,
                target_space: &target_space,
                target,
            },
            cx,
        )
    }

    pub(crate) fn update_tabs_drop_intent_from_render(
        &mut self,
        target_tabs: DockNodeId,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let changed = self.update_tabs_drop_intent(target_tabs, bounds, position, cx);
        if changed {
            cx.notify();
        }
        changed
    }

    pub(crate) fn update_tab_reorder_drop_intent_from_render(
        &mut self,
        target_tabs: DockNodeId,
        target_index: usize,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let changed =
            self.update_tab_reorder_drop_intent(target_tabs, target_index, bounds, position, cx);
        if changed {
            cx.notify();
        }
        changed
    }

    pub(crate) fn begin_floating_drag_from_render(
        &mut self,
        space: DockSpaceId,
        floating: DockNodeId,
        start_position: Point<Pixels>,
        initial_bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.with_workspace(cx, |workspace| workspace.policy().allows_floating()) {
            return false;
        }

        let Some(outcome) = self.try_commit_action_from_render(
            DockAction::RaiseFloating {
                space: space.clone(),
                floating,
            },
            cx,
        ) else {
            return false;
        };

        self.start_floating_drag(space, floating, start_position, initial_bounds);
        if !outcome.changed() {
            cx.notify();
        }
        true
    }

    pub(crate) fn update_floating_drag_from_render(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let changed = self.update_floating_drag(position, cx);
        if changed {
            cx.notify();
        }
        changed
    }

    pub(crate) fn finish_floating_drag_from_render(&mut self, cx: &mut Context<Self>) -> bool {
        let changed = self.finish_floating_drag();
        if changed {
            cx.notify();
        }
        changed
    }

    pub(crate) fn begin_splitter_drag_from_render(
        &mut self,
        split: DockNodeId,
        handle_index: usize,
        start_position: Pixels,
        split_extent: Pixels,
        initial_fractions: Vec<f32>,
        cx: &mut Context<Self>,
    ) {
        self.start_splitter_drag(
            split,
            handle_index,
            start_position,
            split_extent,
            initial_fractions,
        );
        cx.notify();
    }

    pub(crate) fn update_splitter_drag_from_render(
        &mut self,
        position: Pixels,
        cx: &mut Context<Self>,
    ) -> bool {
        let changed = self.update_splitter_drag(position, cx);
        if changed {
            cx.notify();
        }
        changed
    }

    pub(crate) fn finish_splitter_drag_from_render(&mut self, cx: &mut Context<Self>) -> bool {
        let changed = self.finish_splitter_drag();
        if changed {
            cx.notify();
        }
        changed
    }

    fn commit_action_from_render(&mut self, action: DockAction, cx: &mut Context<Self>) -> bool {
        self.try_commit_action_from_render(action, cx)
            .map(|outcome| outcome.changed())
            .unwrap_or(false)
    }

    fn try_commit_action_from_render(
        &mut self,
        action: DockAction,
        cx: &mut Context<Self>,
    ) -> Option<DockActionOutcome> {
        let outcome = self.apply_action_from_host(&action, cx).ok()?;
        if outcome.changed() {
            cx.notify();
        }
        Some(outcome)
    }

    fn commit_resolved_drop_from_render(
        &mut self,
        request: DockWorkspaceDropRequest<'_>,
        cx: &mut Context<Self>,
    ) -> bool {
        self.try_commit_resolved_drop_from_render(request, cx)
            .map(|outcome| outcome.changed())
            .unwrap_or(false)
    }

    fn try_commit_resolved_drop_from_render(
        &mut self,
        request: DockWorkspaceDropRequest<'_>,
        cx: &mut Context<Self>,
    ) -> Option<DockActionOutcome> {
        let outcome = self.commit_resolved_drop_from_host(request, cx).ok()?;
        if outcome.changed() {
            cx.notify();
        }
        Some(outcome)
    }
}
