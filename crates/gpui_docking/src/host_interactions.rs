#[cfg(test)]
use crate::interaction::{FloatingDrag, SplitterDrag};
use crate::{DockHost, DockNodeId, DockSpaceId, drop_target::DockDropIntent};
use open_gpui::{Bounds, Context, Pixels, Point};

impl DockHost {
    pub(crate) fn start_splitter_drag(
        &mut self,
        split: DockNodeId,
        handle_index: usize,
        start_position: Pixels,
        split_extent: Pixels,
        initial_fractions: Vec<f32>,
    ) {
        self.interaction.start_splitter_drag(
            split,
            handle_index,
            start_position,
            split_extent,
            initial_fractions,
        );
    }

    pub(crate) fn update_splitter_drag(
        &mut self,
        position: Pixels,
        cx: &mut Context<Self>,
    ) -> bool {
        let split_min_size =
            self.with_workspace(cx, |workspace| workspace.options().split_min_size);
        let Some(action) = self
            .interaction
            .resize_split_action(position, split_min_size)
        else {
            return false;
        };

        self.apply_action_from_host(&action, cx)
            .map(|outcome| outcome.changed())
            .unwrap_or(false)
    }

    pub(crate) fn finish_splitter_drag(&mut self) -> bool {
        self.interaction.finish_splitter_drag()
    }

    pub(crate) fn start_floating_drag(
        &mut self,
        space: DockSpaceId,
        floating: DockNodeId,
        start_position: Point<Pixels>,
        initial_bounds: Bounds<Pixels>,
    ) {
        self.interaction
            .start_floating_drag(space, floating, start_position, initial_bounds);
    }

    pub(crate) fn update_floating_drag(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(action) = self.interaction.set_floating_bounds_action(position) else {
            return false;
        };

        self.apply_action_from_host(&action, cx)
            .map(|outcome| outcome.changed())
            .unwrap_or(false)
    }

    pub(crate) fn finish_floating_drag(&mut self) -> bool {
        self.interaction.finish_floating_drag()
    }

    pub(crate) fn update_tabs_drop_intent(
        &mut self,
        target_tabs: DockNodeId,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        cx: &Context<Self>,
    ) -> bool {
        let policy = self.with_workspace(cx, |workspace| *workspace.policy());
        self.interaction
            .update_tabs_drop_intent(target_tabs, bounds, position, &policy)
    }

    pub(crate) fn update_tab_reorder_drop_intent(
        &mut self,
        target_tabs: DockNodeId,
        target_index: usize,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        cx: &Context<Self>,
    ) -> bool {
        let policy = self.with_workspace(cx, |workspace| *workspace.policy());
        self.interaction.update_tab_reorder_drop_intent(
            target_tabs,
            target_index,
            bounds,
            position,
            &policy,
        )
    }

    pub(crate) fn take_tab_drop_intent(
        &mut self,
        target_tabs: DockNodeId,
    ) -> Option<DockDropIntent> {
        self.interaction.take_tab_drop_intent(target_tabs)
    }

    pub(crate) fn tab_drop_preview_bounds(
        &self,
        target_tabs: DockNodeId,
    ) -> Option<Bounds<Pixels>> {
        self.interaction.tab_drop_preview_bounds(target_tabs)
    }

    #[cfg(test)]
    pub(crate) fn splitter_drag(&self) -> Option<&SplitterDrag> {
        self.interaction.splitter_drag()
    }

    #[cfg(test)]
    pub(crate) fn floating_drag(&self) -> Option<&FloatingDrag> {
        self.interaction.floating_drag()
    }
}
