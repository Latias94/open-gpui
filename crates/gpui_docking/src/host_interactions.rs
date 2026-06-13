#[cfg(test)]
use crate::interaction::{FloatingDrag, SplitterDrag};
use crate::{
    DockHost, DockItemId, DockNodeId, DockSpaceId,
    drag::{DockDragPayload, DockDragPayloadKind},
    host_interaction_outcome::DockHostInteractionOutcome,
    interaction::{DockFloatingBoundsRequest, DockPayloadDropRelease, DockSplitterResizeRequest},
    workspace_transaction::DockWorkspacePayloadDropRequest,
};
use open_gpui::{Bounds, Context, Pixels, Point, Window};

impl DockHost {
    pub(crate) fn clear_drop_preview_interaction(&mut self) -> bool {
        let route_preview_cleared = self.interaction_mut().clear_drop_route_preview();
        let resolved_target_cleared = self.interaction_mut().take_resolved_drop_target().is_some();
        route_preview_cleared || resolved_target_cleared
    }

    pub(crate) fn select_tab_interaction(
        &mut self,
        tabs: DockNodeId,
        item: DockItemId,
        cx: &mut Context<Self>,
    ) -> DockHostInteractionOutcome {
        self.commit_select_tab_interaction(tabs, &item, cx, false)
    }

    pub(crate) fn close_item_interaction(
        &mut self,
        item: DockItemId,
        cx: &mut Context<Self>,
    ) -> DockHostInteractionOutcome {
        self.commit_close_item_interaction(&item, cx, false)
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

    pub(crate) fn commit_payload_drop_interaction(
        &mut self,
        release: DockPayloadDropRelease,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> DockHostInteractionOutcome {
        let payload = release.payload();
        let delivery = self.interaction_mut().take_drop_delivery();
        let route_preview_cleared = self.interaction_mut().clear_drop_route_preview();
        let runtime_preview_cleared = self
            .viewport_runtime()
            .cloned()
            .is_some_and(|runtime| runtime.clear_routed_drop_preview(cx));
        let mut drop_preview_cleared = false;
        let target = self.interaction_mut().take_resolved_drop_target();
        let outcome = if let Some(target) = target {
            let focus_item = self.focus_item_for_drag_payload(payload, cx);
            let outcome = self.commit_resolved_payload_drop_interaction(
                DockWorkspacePayloadDropRequest {
                    source_space: &payload.source_space,
                    payload: payload.as_workspace_payload(),
                    target_space: release.host_space(),
                    target,
                },
                cx,
                true,
            );
            self.with_panel_focus_after_local_drop(outcome, focus_item, cx)
        } else if let Some(outcome) =
            self.commit_runtime_routed_payload_drop_interaction(delivery, &release, window, cx)
        {
            drop_preview_cleared = self.clear_drop_preview_interaction();
            outcome
        } else {
            return DockHostInteractionOutcome::Notify
                .merge(DockHostInteractionOutcome::from_session_changed(
                    route_preview_cleared,
                ))
                .merge(self.finish_floating_drag_interaction());
        };

        outcome
            .merge(DockHostInteractionOutcome::from_session_changed(
                route_preview_cleared || runtime_preview_cleared || drop_preview_cleared,
            ))
            .merge(self.finish_floating_drag_interaction())
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
        let outcome = DockHostInteractionOutcome::from_commit_result(
            self.mutate_controller_from_host(cx, |controller| {
                controller.select_tab(tabs, item.clone())
            }),
            notify_on_unchanged,
        );
        self.with_panel_focus(outcome, item.clone())
    }

    fn commit_close_item_interaction(
        &mut self,
        item: &DockItemId,
        cx: &mut Context<Self>,
        notify_on_unchanged: bool,
    ) -> DockHostInteractionOutcome {
        let space = self.space().clone();
        DockHostInteractionOutcome::from_commit_result(
            self.mutate_controller_from_host(cx, |controller| {
                controller.close_item(space, item.clone())
            }),
            notify_on_unchanged,
        )
    }

    fn commit_resolved_payload_drop_interaction(
        &mut self,
        request: DockWorkspacePayloadDropRequest<'_>,
        cx: &mut Context<Self>,
        notify_on_unchanged: bool,
    ) -> DockHostInteractionOutcome {
        DockHostInteractionOutcome::from_commit_result(
            self.mutate_controller_from_host(cx, |controller| {
                controller
                    .workspace_mut()
                    .commit_resolved_payload_drop(request)
            }),
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
            self.mutate_controller_from_host(cx, |controller| {
                controller.resize_split(request.split, &request.fractions)
            }),
            notify_on_unchanged,
        )
    }

    fn commit_set_floating_bounds_interaction(
        &mut self,
        request: DockFloatingBoundsRequest,
        cx: &mut Context<Self>,
        notify_on_unchanged: bool,
    ) -> DockHostInteractionOutcome {
        let space = request.space;
        DockHostInteractionOutcome::from_commit_result(
            self.mutate_controller_from_host(cx, |controller| {
                controller.set_floating_bounds(space, request.floating, request.bounds)
            }),
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
        let space = space.clone();
        DockHostInteractionOutcome::from_commit_result(
            self.mutate_controller_from_host(cx, |controller| {
                controller.raise_floating(space, floating)
            }),
            notify_on_unchanged,
        )
    }

    fn focus_item_for_drag_payload(
        &self,
        payload: &DockDragPayload,
        cx: &Context<Self>,
    ) -> Option<DockItemId> {
        match &payload.kind {
            DockDragPayloadKind::Item { item } => Some(item.clone()),
            DockDragPayloadKind::Tabs => self.with_workspace(cx, |workspace| {
                workspace.graph().active_item_in_tabs(payload.source_node)
            }),
            DockDragPayloadKind::Floating { floating } => self.with_workspace(cx, |workspace| {
                workspace.graph().active_item_in_subtree(*floating)
            }),
        }
    }

    fn with_panel_focus(
        &mut self,
        outcome: DockHostInteractionOutcome,
        item: DockItemId,
    ) -> DockHostInteractionOutcome {
        if matches!(outcome, DockHostInteractionOutcome::Rejected(_)) {
            return outcome;
        }
        if self.request_panel_focus(item) {
            outcome.merge(DockHostInteractionOutcome::Notify)
        } else {
            outcome
        }
    }

    fn with_panel_focus_after_local_drop(
        &mut self,
        outcome: DockHostInteractionOutcome,
        item: Option<DockItemId>,
        cx: &Context<DockHost>,
    ) -> DockHostInteractionOutcome {
        let Some(item) = item else {
            return outcome;
        };
        if matches!(outcome, DockHostInteractionOutcome::Rejected(_)) {
            return outcome;
        }
        if !self.with_workspace(cx, |workspace| {
            workspace
                .graph()
                .find_item_in_space(self.space(), &item)
                .is_some()
        }) {
            return outcome;
        }
        self.with_panel_focus(outcome, item)
    }
}
