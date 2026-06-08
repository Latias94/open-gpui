use crate::{DockAction, DockHost, DockItemId, DockNodeId, DockSpaceId};
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
        let Some(intent) = self.take_tab_drop_intent(target_tabs) else {
            cx.notify();
            return false;
        };

        self.commit_action_from_render(
            DockAction::MoveTab {
                source_space,
                source_tabs,
                item,
                target_space,
                target_tabs: intent.target_tabs,
                zone: intent.zone,
                insert_index: intent.insert_index,
            },
            cx,
        )
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

        self.start_floating_drag(space.clone(), floating, start_position, initial_bounds);
        let changed =
            self.commit_action_from_render(DockAction::RaiseFloating { space, floating }, cx);
        if !changed {
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
        let Ok(outcome) = self.apply_action_from_host(&action, cx) else {
            return false;
        };
        if outcome.changed() {
            cx.notify();
            true
        } else {
            false
        }
    }
}
