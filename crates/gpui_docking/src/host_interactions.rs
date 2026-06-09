#[cfg(test)]
use crate::interaction::{FloatingDrag, SplitterDrag};
use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockHost, DockItemId, DockNodeId,
    DockSpaceId, workspace_transaction::DockWorkspaceDropRequest,
};
use open_gpui::{Bounds, Context, Pixels, Point};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DockHostInteractionOutcome {
    Idle,
    Changed,
    Notify,
    Rejected(DockActionApplyError),
}

impl DockHostInteractionOutcome {
    pub(crate) fn changed(&self) -> bool {
        matches!(self, Self::Changed)
    }

    pub(crate) fn finish(self, cx: &mut Context<DockHost>) -> bool {
        let changed = self.changed();
        if matches!(self, Self::Changed | Self::Notify | Self::Rejected(_)) {
            cx.notify();
        }
        changed
    }

    fn from_session_changed(changed: bool) -> Self {
        if changed { Self::Notify } else { Self::Idle }
    }

    fn from_commit_result(
        result: Result<DockActionOutcome, DockActionApplyError>,
        notify_on_unchanged: bool,
    ) -> Self {
        match result {
            Ok(DockActionOutcome::Changed) => Self::Changed,
            Ok(DockActionOutcome::Unchanged) if notify_on_unchanged => Self::Notify,
            Ok(DockActionOutcome::Unchanged) => Self::Idle,
            Err(error) => Self::Rejected(error),
        }
    }
}

impl DockHost {
    pub(crate) fn select_tab_interaction(
        &mut self,
        tabs: DockNodeId,
        item: DockItemId,
        cx: &mut Context<Self>,
    ) -> DockHostInteractionOutcome {
        self.commit_action_interaction(DockAction::SelectTab { tabs, item }, cx, false)
    }

    pub(crate) fn begin_splitter_drag_interaction(
        &mut self,
        split: DockNodeId,
        handle_index: usize,
        start_position: Pixels,
        split_extent: Pixels,
        initial_fractions: Vec<f32>,
    ) -> DockHostInteractionOutcome {
        self.interaction_mut().start_splitter_drag(
            split,
            handle_index,
            start_position,
            split_extent,
            initial_fractions,
        );
        DockHostInteractionOutcome::Notify
    }

    pub(crate) fn update_splitter_drag_interaction(
        &mut self,
        position: Pixels,
        cx: &mut Context<Self>,
    ) -> DockHostInteractionOutcome {
        let split_min_size =
            self.with_workspace(cx, |workspace| workspace.options().split_min_size);
        let Some(action) = self
            .interaction()
            .resize_split_action(position, split_min_size)
        else {
            return DockHostInteractionOutcome::Idle;
        };

        self.commit_action_interaction(action, cx, false)
    }

    pub(crate) fn finish_splitter_drag_interaction(&mut self) -> DockHostInteractionOutcome {
        DockHostInteractionOutcome::from_session_changed(
            self.interaction_mut().finish_splitter_drag(),
        )
    }

    pub(crate) fn begin_floating_drag_interaction(
        &mut self,
        space: DockSpaceId,
        floating: DockNodeId,
        start_position: Point<Pixels>,
        initial_bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) -> DockHostInteractionOutcome {
        if let Err(error) =
            self.with_workspace(cx, |workspace| workspace.policy().validate_floating())
        {
            return DockHostInteractionOutcome::Rejected(error.into());
        }

        let outcome = self.commit_action_interaction(
            DockAction::RaiseFloating {
                space: space.clone(),
                floating,
            },
            cx,
            false,
        );
        if matches!(outcome, DockHostInteractionOutcome::Rejected(_)) {
            return outcome;
        }

        self.interaction_mut()
            .start_floating_drag(space, floating, start_position, initial_bounds);
        if outcome.changed() {
            outcome
        } else {
            DockHostInteractionOutcome::Notify
        }
    }

    pub(crate) fn update_floating_drag_interaction(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> DockHostInteractionOutcome {
        let Some(action) = self.interaction().set_floating_bounds_action(position) else {
            return DockHostInteractionOutcome::Idle;
        };

        self.commit_action_interaction(action, cx, false)
    }

    pub(crate) fn finish_floating_drag_interaction(&mut self) -> DockHostInteractionOutcome {
        DockHostInteractionOutcome::from_session_changed(
            self.interaction_mut().finish_floating_drag(),
        )
    }

    pub(crate) fn update_tabs_drop_interaction(
        &mut self,
        target_tabs: DockNodeId,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        is_central: bool,
        cx: &Context<Self>,
    ) -> DockHostInteractionOutcome {
        let policy = self.with_workspace(cx, |workspace| *workspace.policy());
        DockHostInteractionOutcome::from_session_changed(
            self.interaction_mut().update_tabs_drop_intent(
                target_tabs,
                bounds,
                position,
                is_central,
                &policy,
            ),
        )
    }

    pub(crate) fn update_tab_reorder_drop_interaction(
        &mut self,
        target_tabs: DockNodeId,
        target_index: usize,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        is_central: bool,
        cx: &Context<Self>,
    ) -> DockHostInteractionOutcome {
        let policy = self.with_workspace(cx, |workspace| *workspace.policy());
        DockHostInteractionOutcome::from_session_changed(
            self.interaction_mut().update_tab_reorder_drop_intent(
                target_tabs,
                target_index,
                bounds,
                position,
                is_central,
                &policy,
            ),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn commit_tab_drop_interaction(
        &mut self,
        source_space: DockSpaceId,
        source_tabs: DockNodeId,
        item: DockItemId,
        target_space: DockSpaceId,
        target_tabs: DockNodeId,
        cx: &mut Context<Self>,
    ) -> DockHostInteractionOutcome {
        let Some(target) = self.interaction_mut().take_tab_drop_target(target_tabs) else {
            return DockHostInteractionOutcome::Notify;
        };

        self.commit_resolved_drop_interaction(
            DockWorkspaceDropRequest {
                source_space: &source_space,
                source_tabs,
                item: &item,
                target_space: &target_space,
                target,
            },
            cx,
            true,
        )
    }

    pub(crate) fn tab_drop_preview_bounds(
        &self,
        target_tabs: DockNodeId,
    ) -> Option<Bounds<Pixels>> {
        self.interaction().tab_drop_preview_bounds(target_tabs)
    }

    #[cfg(test)]
    pub(crate) fn splitter_drag(&self) -> Option<&SplitterDrag> {
        self.interaction().splitter_drag()
    }

    #[cfg(test)]
    pub(crate) fn floating_drag(&self) -> Option<&FloatingDrag> {
        self.interaction().floating_drag()
    }

    fn commit_action_interaction(
        &mut self,
        action: DockAction,
        cx: &mut Context<Self>,
        notify_on_unchanged: bool,
    ) -> DockHostInteractionOutcome {
        DockHostInteractionOutcome::from_commit_result(
            self.apply_action_from_host(&action, cx),
            notify_on_unchanged,
        )
    }

    fn commit_resolved_drop_interaction(
        &mut self,
        request: DockWorkspaceDropRequest<'_>,
        cx: &mut Context<Self>,
        notify_on_unchanged: bool,
    ) -> DockHostInteractionOutcome {
        DockHostInteractionOutcome::from_commit_result(
            self.commit_resolved_drop_from_host(request, cx),
            notify_on_unchanged,
        )
    }
}
