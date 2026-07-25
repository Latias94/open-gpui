#[cfg(test)]
use crate::interaction::{FloatingDrag, SplitterDrag};
use crate::{
    DockEdgeDockSizing, DockHost, DockItemId, DockNodeId, DockSpaceId, DockViewportFocusCommand,
    DockViewportFocusRequest, DropZone, SplitAxis,
    drag::DockDragPayload,
    host_interaction_outcome::DockHostInteractionOutcome,
    interaction::{
        DockFloatingBoundsRequest, DockPayloadDropRelease, DockRuntimeDragSession,
        DockSplitterResizeRequest, SplitterDragAxis,
    },
    surface::{DockSurfaceChangeCategory, with_detached_root_transaction},
    workspace_drop_transaction::DockWorkspacePayloadDropRequest,
    workspace_move_validation::dock_target_validator,
};
use open_gpui::{AppContext as _, Bounds, Context, Pixels, Point, Window};

pub(crate) struct DockHostTabDragBegin {
    pub(crate) outcome: DockHostInteractionOutcome,
    pub(crate) drag_session: DockRuntimeDragSession,
}

struct DockHostResolvedDropCommit {
    outcome: DockHostInteractionOutcome,
    focus_item: Option<DockItemId>,
}

impl DockHost {
    pub(crate) fn clear_drop_preview_interaction(&mut self) -> bool {
        self.interaction_mut().clear_drop_resolution()
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

    pub(crate) fn begin_payload_drag_interaction(
        &mut self,
        payload: &DockDragPayload,
        drag_visual_style: crate::DockDragVisualStyle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> DockRuntimeDragSession {
        self.viewport_runtime()
            .begin_payload_drag_from_window_with_drag_visual_style(
                payload,
                drag_visual_style,
                window,
                cx,
            )
    }

    pub(crate) fn begin_tab_item_drag_interaction(
        &mut self,
        tabs: DockNodeId,
        item: DockItemId,
        payload: &DockDragPayload,
        drag_visual_style: crate::DockDragVisualStyle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> DockHostTabDragBegin {
        let outcome = self.select_tab_interaction(tabs, item, cx);
        let drag_session =
            self.begin_payload_drag_interaction(payload, drag_visual_style, window, cx);
        DockHostTabDragBegin {
            outcome,
            drag_session,
        }
    }

    pub(crate) fn begin_splitter_drag_interaction(
        &mut self,
        axis: SplitAxis,
        split: DockNodeId,
        handle_index: usize,
        start_position: Pixels,
        split_extent: Pixels,
        initial_fractions: Vec<f32>,
    ) -> DockHostInteractionOutcome {
        self.interaction_mut().start_splitter_drag(
            axis,
            split,
            handle_index,
            start_position,
            split_extent,
            initial_fractions,
        );
        DockHostInteractionOutcome::Notify { changed: false }
    }

    pub(crate) fn begin_corner_splitter_drag_interaction(
        &mut self,
        horizontal: SplitterDragAxis,
        vertical: SplitterDragAxis,
    ) -> DockHostInteractionOutcome {
        self.interaction_mut()
            .start_corner_splitter_drag(horizontal, vertical);
        DockHostInteractionOutcome::Notify { changed: false }
    }

    pub(crate) fn update_splitter_drag_interaction(
        &mut self,
        position: Point<Pixels>,
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
            DockHostInteractionOutcome::Notify { changed: false }
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
        let (policy, payload_classes, graph) = self.with_workspace(cx, |workspace| {
            (
                workspace.policy().clone(),
                workspace.payload_dock_classes_for_drag_payload(release.payload()),
                workspace.graph().clone(),
            )
        });
        let default_space = self.space().clone();
        let target_validator = dock_target_validator(&default_space, &payload_classes, &policy);
        let edge_plan_space = default_space.clone();
        let edge_plan_resolver =
            move |target_node: DockNodeId, zone: DropZone, sizing: DockEdgeDockSizing| {
                graph.edge_dock_plan_with_sizing(&edge_plan_space, target_node, zone, sizing)
            };
        let mut drop_preview_cleared = false;
        let local_delivery = self.interaction_mut().take_local_drop_delivery(
            &release,
            &policy,
            Some(&target_validator),
            Some(&edge_plan_resolver),
        );
        let outcome = if let Some(delivery) = local_delivery {
            let commit = self.commit_resolved_payload_drop_interaction(
                delivery.workspace_request(),
                cx,
                true,
            );
            self.with_panel_focus_after_local_drop(commit.outcome, commit.focus_item, cx)
        } else if let Some(outcome) =
            self.commit_runtime_routed_payload_drop_interaction(&release, window, cx)
        {
            drop_preview_cleared = self.clear_drop_preview_interaction();
            outcome
        } else {
            let runtime_preview_cleared = self.viewport_runtime().clear_routed_drop_preview(cx);
            return DockHostInteractionOutcome::Notify { changed: false }
                .merge(DockHostInteractionOutcome::from_session_changed(
                    runtime_preview_cleared,
                ))
                .merge(self.finish_floating_drag_interaction());
        };

        let runtime_preview_cleared = self.viewport_runtime().clear_routed_drop_preview(cx);
        outcome
            .merge(DockHostInteractionOutcome::from_session_changed(
                runtime_preview_cleared || drop_preview_cleared,
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
            self.mutate_controller_from_host(
                cx,
                &[DockSurfaceChangeCategory::Selection],
                |controller| controller.select_tab(tabs, item.clone()),
            ),
            notify_on_unchanged,
        );
        self.with_panel_focus(outcome, item.clone(), cx)
    }

    fn commit_close_item_interaction(
        &mut self,
        item: &DockItemId,
        cx: &mut Context<Self>,
        notify_on_unchanged: bool,
    ) -> DockHostInteractionOutcome {
        let space = self.space().clone();
        DockHostInteractionOutcome::from_commit_result(
            self.mutate_controller_from_host(
                cx,
                &[
                    DockSurfaceChangeCategory::Layout,
                    DockSurfaceChangeCategory::Selection,
                    DockSurfaceChangeCategory::PanelLifecycle,
                ],
                |controller| controller.close_item(space, item.clone()),
            ),
            notify_on_unchanged,
        )
    }

    fn commit_resolved_payload_drop_interaction(
        &mut self,
        request: DockWorkspacePayloadDropRequest<'_>,
        cx: &mut Context<Self>,
        notify_on_unchanged: bool,
    ) -> DockHostResolvedDropCommit {
        let source_space = request.source_space.clone();
        let target_space = request.target.target_space().clone();
        let categories = [
            DockSurfaceChangeCategory::Layout,
            DockSurfaceChangeCategory::Selection,
            DockSurfaceChangeCategory::PanelLifecycle,
        ];
        let Some(owner) = self.surface_owner_entity() else {
            return match self.mutate_controller_from_host_with(
                cx,
                &categories,
                |controller| {
                    controller
                        .workspace_mut()
                        .commit_resolved_payload_drop(request)
                },
                |outcome| outcome.changed(),
            ) {
                Ok(outcome) => {
                    let vacated_source_changed = if outcome.changed() {
                        self.viewport_runtime()
                            .vacate_empty_payload_drop_source_viewport(
                                &source_space,
                                &target_space,
                                cx,
                            )
                    } else {
                        false
                    };
                    DockHostResolvedDropCommit {
                        outcome: DockHostInteractionOutcome::from_commit_result(
                            Ok(outcome.action()),
                            notify_on_unchanged,
                        )
                        .merge(
                            DockHostInteractionOutcome::from_session_changed(
                                vacated_source_changed,
                            ),
                        ),
                        focus_item: outcome.focus_item().cloned(),
                    }
                }
                Err(error) => DockHostResolvedDropCommit {
                    outcome: DockHostInteractionOutcome::from_commit_result(
                        Err(error),
                        notify_on_unchanged,
                    ),
                    focus_item: None,
                },
            };
        };

        let controller = self.controller_entity();
        let runtime = self.viewport_runtime().clone();
        with_detached_root_transaction(&owner, cx, |transaction, cx| {
            let (result, did_change) = cx.update_entity(&controller, |controller, cx| {
                let result = controller
                    .workspace_mut()
                    .commit_resolved_payload_drop(request);
                let did_change = result
                    .as_ref()
                    .map(|outcome| outcome.changed())
                    .unwrap_or(false);
                if did_change {
                    cx.notify();
                }
                (result, did_change)
            });
            if did_change {
                cx.update_entity(&owner, |owner, _| {
                    owner.record_changes(transaction, categories);
                });
            }
            match result {
                Ok(outcome) => {
                    let vacated_source_changed = if did_change {
                        runtime.vacate_empty_payload_drop_source_viewport_with_transaction(
                            &source_space,
                            &target_space,
                            Some(transaction),
                            cx,
                        )
                    } else {
                        false
                    };
                    DockHostResolvedDropCommit {
                        outcome: DockHostInteractionOutcome::from_commit_result(
                            Ok(outcome.action()),
                            notify_on_unchanged,
                        )
                        .merge(
                            DockHostInteractionOutcome::from_session_changed(
                                vacated_source_changed,
                            ),
                        ),
                        focus_item: outcome.focus_item().cloned(),
                    }
                }
                Err(error) => DockHostResolvedDropCommit {
                    outcome: DockHostInteractionOutcome::from_commit_result(
                        Err(error),
                        notify_on_unchanged,
                    ),
                    focus_item: None,
                },
            }
        })
    }

    fn commit_resize_split_interaction(
        &mut self,
        request: DockSplitterResizeRequest,
        cx: &mut Context<Self>,
        notify_on_unchanged: bool,
    ) -> DockHostInteractionOutcome {
        DockHostInteractionOutcome::from_commit_result(
            self.mutate_controller_from_host(
                cx,
                &[DockSurfaceChangeCategory::Layout],
                |controller| controller.resize_splits(&request.updates),
            ),
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
            self.mutate_controller_from_host(
                cx,
                &[DockSurfaceChangeCategory::Layout],
                |controller| {
                    controller.set_floating_bounds(space, request.floating, request.bounds)
                },
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
        let space = space.clone();
        DockHostInteractionOutcome::from_commit_result(
            self.mutate_controller_from_host(
                cx,
                &[DockSurfaceChangeCategory::Layout],
                |controller| controller.raise_floating(space, floating),
            ),
            notify_on_unchanged,
        )
    }

    fn with_panel_focus(
        &mut self,
        outcome: DockHostInteractionOutcome,
        item: DockItemId,
        cx: &mut Context<Self>,
    ) -> DockHostInteractionOutcome {
        if matches!(outcome, DockHostInteractionOutcome::Rejected(_)) {
            return outcome;
        }
        if self.request_viewport_focus_command_in_context(
            DockViewportFocusCommand::viewport_activation(DockViewportFocusRequest::panel(item)),
            cx,
        ) {
            outcome.merge(DockHostInteractionOutcome::Notify { changed: false })
        } else {
            outcome
        }
    }

    fn with_panel_focus_after_local_drop(
        &mut self,
        outcome: DockHostInteractionOutcome,
        item: Option<DockItemId>,
        cx: &mut Context<DockHost>,
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
        self.with_panel_focus(outcome, item, cx)
    }
}
