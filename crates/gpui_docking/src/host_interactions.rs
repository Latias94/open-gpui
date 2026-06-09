#[cfg(test)]
use crate::interaction::{FloatingDrag, SplitterDrag};
use crate::{
    DockActionApplyError, DockActionOutcome, DockHost, DockItemId, DockNodeId, DockSpaceId,
    drop_runtime::{DockHostDropScene, DockHostDropSceneFact},
    drop_target::{
        DockEmptySpaceDropTarget, DockFloatingTitleBarDropTarget, DockLeafDropTarget,
        DockRootDropTarget, DockTabLabelDropTarget,
    },
    interaction::{DockFloatingBoundsRequest, DockSplitterResizeRequest},
    workspace_transaction::DockWorkspaceDropRequest,
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
        self.commit_select_tab_interaction(tabs, &item, cx, false)
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
        let Some(request) = self
            .interaction()
            .resize_split_request(position, split_min_size)
        else {
            return DockHostInteractionOutcome::Idle;
        };

        self.commit_resize_split_interaction(request, cx, false)
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

        let outcome = self.commit_raise_floating_interaction(&space, floating, cx, false);
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
        let Some(request) = self.interaction().floating_bounds_request(position) else {
            return DockHostInteractionOutcome::Idle;
        };

        self.commit_set_floating_bounds_interaction(request, cx, false)
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
            self.interaction_mut().push_drop_scene_fact(
                position,
                DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                    root: target_tabs,
                    target_tabs,
                    bounds,
                    is_central,
                }),
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
            self.interaction_mut().push_drop_scene_fact(
                position,
                DockHostDropSceneFact::TabLabel(DockTabLabelDropTarget {
                    target_tabs,
                    target_index,
                    bounds,
                    is_central,
                }),
                &policy,
            ),
        )
    }

    pub(crate) fn begin_host_drop_scene_interaction(
        &mut self,
        position: Point<Pixels>,
        cx: &Context<Self>,
    ) -> DockHostInteractionOutcome {
        let policy = self.with_workspace(cx, |workspace| *workspace.policy());
        DockHostInteractionOutcome::from_session_changed(
            self.interaction_mut()
                .begin_drop_scene(DockHostDropScene::new(position), &policy),
        )
    }

    pub(crate) fn update_root_drop_scene_interaction(
        &mut self,
        root: DockNodeId,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        cx: &Context<Self>,
    ) -> DockHostInteractionOutcome {
        let policy = self.with_workspace(cx, |workspace| *workspace.policy());
        DockHostInteractionOutcome::from_session_changed(
            self.interaction_mut().push_drop_scene_fact(
                position,
                DockHostDropSceneFact::Root(DockRootDropTarget { root, bounds }),
                &policy,
            ),
        )
    }

    pub(crate) fn update_empty_space_drop_scene_interaction(
        &mut self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        cx: &Context<Self>,
    ) -> DockHostInteractionOutcome {
        let policy = self.with_workspace(cx, |workspace| *workspace.policy());
        let space = self.space().clone();
        DockHostInteractionOutcome::from_session_changed(
            self.interaction_mut().push_drop_scene_fact(
                position,
                DockHostDropSceneFact::EmptySpace(DockEmptySpaceDropTarget { space, bounds }),
                &policy,
            ),
        )
    }

    pub(crate) fn update_floating_title_bar_drop_scene_interaction(
        &mut self,
        floating: DockNodeId,
        target_tabs: DockNodeId,
        title_bounds: Bounds<Pixels>,
        preview_bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        cx: &Context<Self>,
    ) -> DockHostInteractionOutcome {
        let policy = self.with_workspace(cx, |workspace| *workspace.policy());
        DockHostInteractionOutcome::from_session_changed(
            self.interaction_mut().push_drop_scene_fact(
                position,
                DockHostDropSceneFact::FloatingTitleBar(DockFloatingTitleBarDropTarget {
                    floating,
                    target_tabs,
                    title_bounds,
                    preview_bounds,
                }),
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
        cx: &mut Context<Self>,
    ) -> DockHostInteractionOutcome {
        let Some(target) = self.interaction_mut().take_resolved_drop_target() else {
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

    fn commit_select_tab_interaction(
        &mut self,
        tabs: DockNodeId,
        item: &DockItemId,
        cx: &mut Context<Self>,
        notify_on_unchanged: bool,
    ) -> DockHostInteractionOutcome {
        DockHostInteractionOutcome::from_commit_result(
            self.commit_select_tab_from_host(tabs, item, cx),
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

    fn commit_resize_split_interaction(
        &mut self,
        request: DockSplitterResizeRequest,
        cx: &mut Context<Self>,
        notify_on_unchanged: bool,
    ) -> DockHostInteractionOutcome {
        DockHostInteractionOutcome::from_commit_result(
            self.commit_resize_split_from_host(request.split, &request.fractions, cx),
            notify_on_unchanged,
        )
    }

    fn commit_set_floating_bounds_interaction(
        &mut self,
        request: DockFloatingBoundsRequest,
        cx: &mut Context<Self>,
        notify_on_unchanged: bool,
    ) -> DockHostInteractionOutcome {
        DockHostInteractionOutcome::from_commit_result(
            self.commit_set_floating_bounds_from_host(
                &request.space,
                request.floating,
                request.bounds,
                cx,
            ),
            notify_on_unchanged,
        )
    }

    fn commit_raise_floating_interaction(
        &mut self,
        space: &DockSpaceId,
        floating: DockNodeId,
        cx: &mut Context<Self>,
        notify_on_unchanged: bool,
    ) -> DockHostInteractionOutcome {
        DockHostInteractionOutcome::from_commit_result(
            self.commit_raise_floating_from_host(space, floating, cx),
            notify_on_unchanged,
        )
    }
}
