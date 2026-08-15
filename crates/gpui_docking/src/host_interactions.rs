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
    surface::DockSurfaceChangeCategory,
    workspace_drop_transaction::DockWorkspacePayloadDropRequest,
    workspace_move_validation::dock_target_validator,
};
use open_gpui::{AppContext as _, Bounds, Context, DragStartGeometry, Pixels, Point, Window};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

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
        drag_start: Option<&DragStartGeometry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> DockRuntimeDragSession {
        let runtime = self.viewport_runtime().clone();
        let source_window_handle: open_gpui::AnyWindowHandle = window.window_handle().into();
        let source_window = source_window_handle.window_id();
        let native_route_context =
            (drag_start.is_some() && cx.viewport_capabilities().window_hit_stack).then(|| {
                let work_context = self
                    .runtime_work_context(cx)
                    .expect("a rendered payload drag requires one admitted runtime work context");
                let source_binding = self
                    .current_window_binding()
                    .expect("a rendered payload drag requires one current source-window binding");
                assert!(
                    self.accepts_window_callback(Some(source_binding), source_window),
                    "a rendered payload drag must start from its current bound source window"
                );
                (work_context, source_binding)
            });
        let source_focus = native_route_context
            .as_ref()
            .and_then(|_| self.capture_payload_focus_snapshot(payload, window, cx));
        let session =
            runtime.begin_payload_drag_with_drag_visual_style(payload, drag_visual_style, cx);
        let mut route_receipt = None;
        let start = catch_unwind(AssertUnwindSafe(|| {
            self.interaction_mut()
                .bind_payload_drag_anchor_session(payload, &session);
            if let (Some(drag_start), Some((work_context, source_binding))) =
                (drag_start, native_route_context)
            {
                let receipt = crate::native_captured_drag::begin_native_captured_drag_route(
                    runtime.clone(),
                    work_context,
                    session.clone(),
                    payload.clone(),
                    source_window_handle,
                    cx.entity().downgrade(),
                    source_binding,
                    source_focus.clone(),
                    drag_start,
                    cx,
                );
                if !self.install_native_drag_transport_proxy(
                    receipt.transport_lease(),
                    payload.clone(),
                    drag_start.pointer_capture_handle(),
                    cx,
                ) {
                    crate::native_captured_drag::rollback_native_captured_drag_route_start(
                        &receipt, cx,
                    );
                    panic!("native captured-drag transport proxy must bind to its source host");
                }
                route_receipt = Some(receipt);
            }
            let deferred_runtime = runtime.clone();
            let deferred_payload = payload.clone();
            let deferred_session = session.clone();
            cx.defer(move |cx| {
                if deferred_runtime.active_payload_drag_session(&deferred_payload)
                    == Some(deferred_session)
                {
                    deferred_runtime.reconcile_viewport_frame_except_window(source_window, cx);
                }
            });
        }));
        if let Err(payload) = start {
            if let Some(receipt) = route_receipt.as_ref() {
                self.retire_native_drag_transport_proxy(receipt.transport_key(), cx);
                crate::native_captured_drag::rollback_native_captured_drag_route_start(receipt, cx);
            }
            self.interaction_mut()
                .clear_payload_drag_anchor_for_session(&session);
            runtime.abort_payload_drag_start(&session);
            resume_unwind(payload);
        }
        session
    }

    pub(crate) fn begin_tab_item_drag_interaction(
        &mut self,
        tabs: DockNodeId,
        item: DockItemId,
        payload: &DockDragPayload,
        drag_visual_style: crate::DockDragVisualStyle,
        drag_start: Option<&DragStartGeometry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> DockHostTabDragBegin {
        let outcome = self.select_tab_interaction(tabs, item, cx);
        let drag_session =
            self.begin_payload_drag_interaction(payload, drag_visual_style, drag_start, window, cx);
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
                window,
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
            let runtime_preview_cleared = self
                .viewport_runtime()
                .clear_routed_drop_preview_from_window(window, cx);
            return DockHostInteractionOutcome::Notify { changed: false }
                .merge(DockHostInteractionOutcome::from_session_changed(
                    runtime_preview_cleared,
                ))
                .merge(self.finish_floating_drag_interaction());
        };

        let runtime_preview_cleared = self
            .viewport_runtime()
            .clear_routed_drop_preview_from_window(window, cx);
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
        window: &Window,
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
        let runtime = self.viewport_runtime().clone();
        let prepared_source_vacate =
            runtime.prepare_empty_payload_drop_source_vacate(&source_space, &target_space);
        let Some(owner) = self.surface_owner_entity() else {
            return match self.mutate_controller_from_host_with(
                cx,
                &categories,
                |controller| {
                    let outcome = controller
                        .workspace_mut()
                        .commit_resolved_payload_drop(request)?;
                    let source_is_empty = outcome.changed()
                        && source_space != target_space
                        && controller
                            .graph()
                            .collect_items_in_space(&source_space)
                            .is_empty();
                    Ok((outcome, source_is_empty))
                },
                |(outcome, _)| outcome.changed(),
            ) {
                Ok((outcome, source_is_empty)) => {
                    let vacated_source_changed = runtime
                        .finalize_empty_payload_drop_source_vacate_from_window(
                            prepared_source_vacate.apply(source_is_empty),
                            window,
                            cx,
                        );
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
        match self.with_detached_surface_transaction(cx, |transaction, cx| {
            let (result, did_change, source_is_empty) =
                cx.update_entity(&controller, |controller, cx| {
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
                    let source_is_empty = did_change
                        && source_space != target_space
                        && controller
                            .graph()
                            .collect_items_in_space(&source_space)
                            .is_empty();
                    (result, did_change, source_is_empty)
                });
            if did_change {
                cx.update_entity(&owner, |owner, _| {
                    owner.record_changes(transaction, categories);
                });
            }
            match result {
                Ok(outcome) => {
                    let vacated_source_changed = runtime
                        .finalize_empty_payload_drop_source_vacate_with_transaction_from_window(
                            prepared_source_vacate.apply(source_is_empty),
                            Some(transaction),
                            window,
                            cx,
                        );
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
        }) {
            Ok(commit) => commit,
            Err(error) => DockHostResolvedDropCommit {
                outcome: DockHostInteractionOutcome::from_commit_result(
                    Err(error),
                    notify_on_unchanged,
                ),
                focus_item: None,
            },
        }
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
