#[cfg(test)]
use crate::interaction::{FloatingDrag, SplitterDrag};
use crate::{
    DockHost, DockItemId, DockNodeId, DockSpaceId,
    drag::{DockDragPayload, DockDragPayloadKind},
    host_interaction_outcome::DockHostInteractionOutcome,
    interaction::{DockFloatingBoundsRequest, DockPayloadDropRelease, DockSplitterResizeRequest},
    workspace_transaction::{DockWorkspaceDropPayload, DockWorkspacePayloadDropRequest},
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
        let route_preview_cleared = self.interaction_mut().clear_drop_route_preview();
        let mut drop_preview_cleared = false;
        let outcome = if let Some(outcome) =
            self.commit_runtime_routed_payload_drop_interaction(&release, window, cx)
        {
            drop_preview_cleared = self.clear_drop_preview_interaction();
            outcome
        } else {
            let target = self.interaction_mut().take_resolved_drop_target();
            let Some(target) = target else {
                return DockHostInteractionOutcome::Notify
                    .merge(DockHostInteractionOutcome::from_session_changed(
                        route_preview_cleared,
                    ))
                    .merge(self.finish_floating_drag_interaction());
            };

            let focus_item = self.focus_item_for_drag_payload(payload, cx);
            let outcome = self.commit_resolved_payload_drop_interaction(
                DockWorkspacePayloadDropRequest {
                    source_space: &payload.source_space,
                    payload: workspace_payload(payload),
                    target_space: release.host_space(),
                    target,
                },
                cx,
                true,
            );
            self.with_panel_focus_after_local_drop(outcome, focus_item, cx)
        };

        outcome
            .merge(DockHostInteractionOutcome::from_session_changed(
                route_preview_cleared || drop_preview_cleared,
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
            self.commit_select_tab_from_host(tabs, item, cx),
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
        DockHostInteractionOutcome::from_commit_result(
            self.commit_close_item_from_host(item, cx),
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
            self.commit_resolved_payload_drop_from_host(request, cx),
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

    fn focus_item_for_drag_payload(
        &self,
        payload: &DockDragPayload,
        cx: &Context<Self>,
    ) -> Option<DockItemId> {
        match &payload.kind {
            DockDragPayloadKind::Item { item } => Some(item.clone()),
            DockDragPayloadKind::Tabs => self.with_workspace(cx, |workspace| {
                workspace.graph().active_item_in_tabs(payload.source_tabs)
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

fn workspace_payload(payload: &DockDragPayload) -> DockWorkspaceDropPayload<'_> {
    match &payload.kind {
        DockDragPayloadKind::Item { item } => DockWorkspaceDropPayload::Item {
            source_tabs: payload.source_tabs,
            item,
        },
        DockDragPayloadKind::Tabs => DockWorkspaceDropPayload::Tabs {
            source_tabs: payload.source_tabs,
        },
    }
}
