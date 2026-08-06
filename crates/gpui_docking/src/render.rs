use crate::{
    DockDropGuideVisualState, DockFloatingContainer, DockHost, DockNode, DockNodeId,
    DockRoutePreviewVisualState, DockSpaceId, DockSplitterVisualState, DockSplitterVisualStyle,
    DockTargetPreviewVisualState, DockViewportHostGeometry, DockViewportRuntimeHandle,
    DockViewportRuntimeWorkContext, DropZone,
    accessibility_scene::DockAccessibilityScene,
    debug::DockDebugRegion,
    divider_hit_map::{DockDividerAffordanceState, DockDividerHitMap, DockDividerHitTarget},
    drag::DockDragPayload,
    drop_preview::{
        DockDropPreview, DockDropRoutePreview, DockPreviewDropBox, DockPreviewTabInsertionIndex,
    },
    drop_scene_fact, geometry,
    host::{
        DockHostLiveDestinationPhase, DockHostLivePresentationKey, DockHostLivePresentationMode,
        DockHostLiveSourcePhase, DockHostLiveSourceRestorationPhase,
        DockHostRecoveryDestinationPhase, DockHostRecoveryPresentationMode,
        DockHostRecoverySourcePhase, DockHostWindowBinding,
    },
    host_render_actions::DockRenderedPointerPosition,
    host_render_session::{DockHostRenderSession, selected_index},
    presentation_scene::DockPresentationScene,
    render_split::DockRenderSplitInput,
    surface::{
        DockSurfaceOwner,
        live_undock::{
            DockLiveUndockFact, DockLiveUndockPayloadLeaseReceipt,
            DockLiveUndockPayloadMountReceipt, DockLiveUndockPayloadPresentationReceipt,
            DockLiveUndockPresentationFailure, DockLiveUndockRevealObservation,
            DockLiveUndockRevealOutcome, DockLiveUndockRevealReceipt,
            DockLiveUndockSourceProxyReceipt, DockLiveUndockSourceRestorationFailure,
            DockLiveUndockSourceRestorationReceipt,
        },
        live_undock_runtime::DockLiveUndockSourceFinishOutcome,
        payload_recovery::DockPayloadRecoveryEntry,
    },
    transition_executor::{
        DockDividerSample, DockPaneClipSample, DockTransitionSample, DockVisualAffordanceSample,
    },
    transition_geometry::DockTransitionPlan,
    viewport_drop_scene::{DockViewportHostSceneDraft, DockViewportHostSceneFrame},
    viewport_registry::DockViewportRegistrationKey,
    visual_affordance_scene::{
        DockPayloadTabPreviewLayout, DockPayloadTabPreviewPlacement, DockVisualAffordanceLayer,
        DockVisualAffordanceScene,
    },
};
use open_gpui::{
    AccessibleAction, AnyElement, AnyWindowHandle, App, AppContext as _, BorderStyle, Bounds,
    Context, CursorStyle, DispatchPhase, DragMoveEvent, DropEvent, Entity, HitboxBehavior,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement, Pixels, PointerCaptureHandle, PrepaintPublicationId, Render, Rgba,
    Role, SharedString, StatefulInteractiveElement, Styled, SubtreePresentation,
    SubtreePresentationExt, WeakEntity, Window, WindowId,
    WindowProvisionalRevealCancellationOutcome, WindowProvisionalRevealOutcome, canvas, div, point,
    px, quad, retained_visual, rgba, view_presentation_window,
};
use open_gpui_motion::MotionTransition;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

pub(crate) const LIVE_UNDOCK_REVEAL_OBSERVATION_DEADLINE: Duration = Duration::from_millis(500);

#[derive(Clone)]
pub(crate) struct DockViewportHostSceneCandidate {
    pub(crate) draft: DockViewportHostSceneDraft,
    host_binding: DockHostWindowBinding,
    expected_registration: Option<DockViewportRegistrationKey>,
    work_context: DockViewportRuntimeWorkContext,
    pub(crate) presentation_scene: DockPresentationScene,
}

#[derive(Default)]
pub(crate) struct DockViewportHostSceneCandidateState {
    pending: Option<DockViewportHostSceneCandidate>,
    committed: Option<DockViewportCommittedHostSceneCandidate>,
    prepaint_ran: bool,
}

#[derive(Clone)]
struct DockViewportCommittedHostSceneCandidate {
    candidate: DockViewportHostSceneCandidate,
    frame: DockViewportHostSceneFrame,
}

enum DockViewportHostSceneCandidateForCommit {
    Fresh(DockViewportHostSceneCandidate),
    CachedReplay(DockViewportCommittedHostSceneCandidate),
}

impl DockViewportHostSceneCandidateState {
    fn begin_prepaint(&mut self) {
        self.pending = None;
        self.prepaint_ran = true;
    }

    fn set_pending(&mut self, candidate: DockViewportHostSceneCandidate) {
        self.pending = Some(candidate);
    }

    fn pending_mut(&mut self) -> Option<&mut DockViewportHostSceneCandidate> {
        self.pending.as_mut()
    }

    fn candidate_for_commit(&mut self) -> Option<DockViewportHostSceneCandidateForCommit> {
        if !self.prepaint_ran {
            return self
                .committed
                .clone()
                .map(DockViewportHostSceneCandidateForCommit::CachedReplay);
        }
        self.prepaint_ran = false;
        self.pending
            .take()
            .map(DockViewportHostSceneCandidateForCommit::Fresh)
    }

    fn commit(
        &mut self,
        candidate: DockViewportHostSceneCandidate,
        frame: DockViewportHostSceneFrame,
    ) {
        self.committed = Some(DockViewportCommittedHostSceneCandidate { candidate, frame });
    }

    fn discard_current(&mut self, expected_frame: Option<&DockViewportHostSceneFrame>) {
        self.pending = None;
        self.prepaint_ran = false;
        self.retract_committed(expected_frame);
    }

    fn retract_committed(&mut self, expected_frame: Option<&DockViewportHostSceneFrame>) {
        let matches = match expected_frame {
            Some(expected_frame) => self
                .committed
                .as_ref()
                .is_some_and(|committed| &committed.frame == expected_frame),
            None => self.committed.is_none(),
        };
        if matches {
            self.committed = None;
        }
    }
}

pub(crate) type DockViewportHostSceneCandidateSlot =
    Rc<RefCell<DockViewportHostSceneCandidateState>>;

fn take_viewport_host_scene_candidate_for_commit(
    frame_slot: &DockViewportHostSceneCandidateSlot,
) -> Option<DockViewportHostSceneCandidateForCommit> {
    let mut candidate_slot = frame_slot.borrow_mut();
    candidate_slot.candidate_for_commit()
}

fn clear_viewport_host_scene_publication(
    runtime: &DockViewportRuntimeHandle,
    entity: &Entity<DockHost>,
    window_id: WindowId,
    host_binding: DockHostWindowBinding,
    expected_frame: Option<&DockViewportHostSceneFrame>,
    window: &mut Window,
    app: &mut App,
) -> bool {
    let Some(expected_frame) = expected_frame else {
        return false;
    };
    crate::native_captured_drag::clear_native_captured_host_scene(
        window_id,
        &entity.downgrade(),
        host_binding,
        Some(expected_frame),
        app,
    );
    let runtime_changed = runtime.discard_rendered_viewport_host_scene_frame_exact_from_window(
        expected_frame,
        window,
        app,
    );
    let host_changed = entity.update(app, |host, _| {
        if host.interaction().viewport_host_scene_frame() != Some(expected_frame) {
            return false;
        }
        let mut changed = host.clear_last_presentation_scene();
        changed |= host.publish_rendered_viewport_host_scene_frame_from_render(None, window);
        changed
    });
    runtime_changed || host_changed
}

fn record_viewport_host_scene_transaction(
    window: &mut Window,
    publication: PrepaintPublicationId,
    frame_slot: DockViewportHostSceneCandidateSlot,
    runtime: DockViewportRuntimeHandle,
    entity: Entity<DockHost>,
    space: DockSpaceId,
    window_id: WindowId,
    host_binding: DockHostWindowBinding,
    prior_published_frame: Option<DockViewportHostSceneFrame>,
    passthrough_pointer_input: bool,
) {
    let transaction_published_frame = Rc::new(RefCell::new(None));
    let discard_frame_slot = frame_slot.clone();
    let discard_runtime = runtime.clone();
    let discard_entity = entity.clone();
    let discard_prior_published_frame = prior_published_frame.clone();
    let discard_transaction_published_frame = transaction_published_frame.clone();
    window.record_prepaint_window_transaction(
        publication,
        move |_, window, app| {
            let expected_published_frame = transaction_published_frame
                .borrow()
                .clone()
                .or_else(|| prior_published_frame.clone());
            let candidate_for_commit = take_viewport_host_scene_candidate_for_commit(&frame_slot);
            let Some(candidate_for_commit) = candidate_for_commit else {
                frame_slot
                    .borrow_mut()
                    .discard_current(expected_published_frame.as_ref());
                if clear_viewport_host_scene_publication(
                    &runtime,
                    &entity,
                    window_id,
                    host_binding,
                    expected_published_frame.as_ref(),
                    window,
                    app,
                ) {
                    window.refresh();
                }
                return;
            };
            let (candidate, cached_frame) = match candidate_for_commit {
                DockViewportHostSceneCandidateForCommit::Fresh(candidate) => (candidate, None),
                DockViewportHostSceneCandidateForCommit::CachedReplay(committed) => {
                    (committed.candidate, Some(committed.frame))
                }
            };
            let DockViewportHostSceneCandidate {
                draft,
                host_binding: candidate_host_binding,
                expected_registration: candidate_registration,
                work_context: candidate_work_context,
                presentation_scene,
            } = candidate;
            let candidate_published_frame =
                cached_frame.as_ref().or(expected_published_frame.as_ref());
            if !entity.update(app, |host, cx| {
                host.accepts_viewport_scene_candidate(
                    candidate_host_binding,
                    candidate_registration.as_ref(),
                    candidate_work_context,
                    window_id,
                    cx,
                )
            }) {
                frame_slot
                    .borrow_mut()
                    .discard_current(candidate_published_frame);
                if clear_viewport_host_scene_publication(
                    &runtime,
                    &entity,
                    window_id,
                    candidate_host_binding,
                    candidate_published_frame,
                    window,
                    app,
                ) {
                    window.refresh();
                }
                return;
            }
            if let Some(cached_frame) = cached_frame {
                // Cached journals replay this callback without running prepaint. The committed
                // frame is already authoritative; finalizing its draft again would mint a new
                // scene generation for unchanged geometry.
                let frame_is_current = entity.update(app, |host, _| {
                    host.interaction().viewport_host_scene_frame() == Some(&cached_frame)
                });
                if !frame_is_current {
                    frame_slot.borrow_mut().discard_current(Some(&cached_frame));
                    if clear_viewport_host_scene_publication(
                        &runtime,
                        &entity,
                        window_id,
                        candidate_host_binding,
                        Some(&cached_frame),
                        window,
                        app,
                    ) {
                        window.refresh();
                    }
                    return;
                }

                *transaction_published_frame.borrow_mut() = Some(cached_frame.clone());
                if runtime.sync_rendered_viewport_pointer_input(
                    cached_frame.registration_key(),
                    passthrough_pointer_input,
                    window,
                ) {
                    window.refresh();
                }
                return;
            }
            let committed_draft = draft.clone();
            let Some(preparation) = runtime.prepare_rendered_viewport_host_scene_draft(
                draft,
                candidate_registration.as_ref(),
                candidate_work_context,
                window,
                app,
            ) else {
                frame_slot
                    .borrow_mut()
                    .discard_current(expected_published_frame.as_ref());
                if clear_viewport_host_scene_publication(
                    &runtime,
                    &entity,
                    window_id,
                    candidate_host_binding,
                    expected_published_frame.as_ref(),
                    window,
                    app,
                ) {
                    window.refresh();
                }
                return;
            };
            if !entity.update(app, |host, cx| {
                host.accepts_viewport_scene_candidate(
                    candidate_host_binding,
                    candidate_registration.as_ref(),
                    candidate_work_context,
                    window_id,
                    cx,
                )
            }) {
                let changed = preparation.changed();
                frame_slot
                    .borrow_mut()
                    .discard_current(expected_published_frame.as_ref());
                let clear_changed = clear_viewport_host_scene_publication(
                    &runtime,
                    &entity,
                    window_id,
                    candidate_host_binding,
                    expected_published_frame.as_ref(),
                    window,
                    app,
                );
                if changed || clear_changed {
                    window.refresh();
                }
                return;
            }

            let mut commit = runtime.finalize_rendered_viewport_host_scene_draft(preparation);
            let Some(frame) = commit.frame.clone() else {
                frame_slot
                    .borrow_mut()
                    .discard_current(expected_published_frame.as_ref());
                let clear_changed = clear_viewport_host_scene_publication(
                    &runtime,
                    &entity,
                    window_id,
                    candidate_host_binding,
                    expected_published_frame.as_ref(),
                    window,
                    app,
                );
                let commit_changed = runtime
                    .publish_rendered_viewport_host_scene_commit_from_window(commit, window, app);
                if clear_changed || commit_changed {
                    window.refresh();
                }
                return;
            };
            let frame_registration = frame.registration_key().clone();
            let committed_native_scene = committed_draft.clone();
            let committed_candidate = DockViewportHostSceneCandidate {
                draft: committed_draft,
                host_binding: candidate_host_binding,
                expected_registration: Some(frame_registration.clone()),
                work_context: candidate_work_context,
                presentation_scene: presentation_scene.clone(),
            };
            let (publication_accepted, interaction_frame_changed) =
                entity.update(app, |host, cx| {
                    if !host.adopt_viewport_scene_registration(
                        candidate_host_binding,
                        candidate_registration.as_ref(),
                        frame_registration.clone(),
                        candidate_work_context,
                        window_id,
                        cx,
                    ) {
                        return (false, false);
                    }
                    host.set_last_presentation_scene(presentation_scene.clone());
                    (
                        true,
                        host.publish_rendered_viewport_host_scene_frame_from_render(
                            Some(frame.clone()),
                            window,
                        ),
                    )
                });
            let rollback_changed = if publication_accepted {
                frame_slot
                    .borrow_mut()
                    .commit(committed_candidate, frame.clone());
                *transaction_published_frame.borrow_mut() = Some(frame.clone());
                crate::native_captured_drag::publish_native_captured_host_scene(
                    crate::native_captured_drag::native_captured_host_scene(
                        window_id,
                        entity.downgrade(),
                        candidate_host_binding,
                        &runtime,
                        candidate_work_context,
                        space.clone(),
                        frame.clone(),
                        committed_native_scene,
                        presentation_scene,
                    ),
                    app,
                );
                false
            } else {
                frame_slot
                    .borrow_mut()
                    .discard_current(expected_published_frame.as_ref());
                let mut changed =
                    runtime.rollback_rendered_viewport_host_scene_frame(&frame, &mut commit);
                changed |= clear_viewport_host_scene_publication(
                    &runtime,
                    &entity,
                    window_id,
                    candidate_host_binding,
                    expected_published_frame.as_ref(),
                    window,
                    app,
                );
                changed
            };
            let pointer_sync_changed = publication_accepted
                && runtime.sync_rendered_viewport_pointer_input(
                    &frame_registration,
                    passthrough_pointer_input,
                    window,
                );
            let commit_changed = runtime
                .publish_rendered_viewport_host_scene_commit_from_window(commit, window, app);
            if commit_changed
                || interaction_frame_changed
                || rollback_changed
                || pointer_sync_changed
            {
                window.refresh();
            }
        },
        move |_, window: &mut Window, app: &mut App| {
            let transaction_frame = discard_transaction_published_frame.borrow().clone();
            let expected_published_frame = transaction_frame
                .as_ref()
                .or(discard_prior_published_frame.as_ref());
            if transaction_frame.is_some() {
                discard_frame_slot
                    .borrow_mut()
                    .retract_committed(expected_published_frame);
            } else {
                discard_frame_slot
                    .borrow_mut()
                    .discard_current(expected_published_frame);
            }
            if clear_viewport_host_scene_publication(
                &discard_runtime,
                &discard_entity,
                window_id,
                host_binding,
                expected_published_frame,
                window,
                app,
            ) {
                window.refresh();
            }
        },
    );
}

const DROP_PREVIEW_TAB_HEIGHT: f32 = 26.0;
const DROP_PREVIEW_TAB_GAP: f32 = 6.0;
const DROP_PREVIEW_TAB_MIN_WIDTH: f32 = 72.0;
const DROP_PREVIEW_TAB_MAX_WIDTH: f32 = 180.0;
const DROP_PREVIEW_TAB_TEXT_PADDING: f32 = 22.0;
const DROP_PREVIEW_TAB_MIN_VISIBLE_WIDTH: f32 = 18.0;

#[derive(Debug, Clone, PartialEq)]
struct DockDropPreviewPayloadTab {
    index: usize,
    title: String,
}

impl Render for DockHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.clear_debug_selectors();
        self.ensure_window_binding(window, cx);
        let visual_style = self.resolve_visual_style(window, cx);
        #[cfg(test)]
        {
            self.record_resolved_visual_style_for_test(visual_style.clone());
        }
        let mut session = self.render_session_with_visual_style(visual_style.clone(), cx);
        if matches!(
            session.kind(),
            crate::host_render_session::DockHostPresentationKind::PayloadRecoveryProjection
        ) {
            return self.render_payload_recovery_projection(&session, window, cx);
        }
        let runtime_work_context = self.runtime_work_context(cx);
        let runtime_publication_admitted = runtime_work_context.is_some();
        if runtime_publication_admitted {
            self.ensure_surface_activation_host_registration(
                runtime_work_context.expect("admitted runtime publication requires a context"),
                window,
                cx,
            );
            self.ensure_viewport_activation_subscription(window, cx);
            self.ensure_viewport_bounds_subscription(window, cx);
            self.ensure_viewport_release_subscription(window, cx);
        }
        if matches!(
            session.kind(),
            crate::host_render_session::DockHostPresentationKind::LivePayloadProjection
        ) {
            return self.render_live_payload_projection(&session, window, cx);
        }
        if session.is_provisional_shell() {
            let mut background = session.visual_style().host.background;
            background.a = background.a.max(1.0 / 255.0);
            return div()
                .size_full()
                .overflow_hidden()
                .bg(background)
                .into_any_element();
        }
        if runtime_publication_admitted
            && self.prepare_pending_focus_selection_from_render(window, cx)
        {
            session = self.render_session_with_visual_style(visual_style, cx);
        }
        let payload_recovery_entries = self.visible_payload_recovery_entries(cx);

        let raw_drag_pointer_capture = self.ensure_pointer_session(window);
        let window_binding = self.current_window_binding();
        self.sync_panel_focus_trackers(session.visible_panel_items(), window, cx);
        let drop_host_space = session.space().clone();
        let viewport_host_scene_frame =
            Rc::new(RefCell::new(DockViewportHostSceneCandidateState::default()));
        let transition_sample = self.sample_transition_for_render(Some(window));

        let selector = self.record_debug_selector(
            DockDebugRegion::Host,
            format!("{}:host", session.selector_prefix()),
        );
        let active_docking_payload = cx.active_drag_value::<DockDragPayload>().cloned();
        let active_docking_drag = active_docking_payload.is_some();
        let weak_host = cx.entity().downgrade();
        let pointer_session_payload = active_docking_payload.clone();
        let pointer_session_listener = canvas(
            |_, _, _| (),
            move |_, _, window, _app| {
                let weak_host = weak_host.clone();
                let frame_payload = pointer_session_payload.clone();
                let window_binding = window_binding;
                window.on_pointer_cancel(move |event, phase, window, app| {
                    if phase != DispatchPhase::Capture {
                        return;
                    }
                    let Some(host) = weak_host.upgrade() else {
                        return;
                    };
                    let payload = app
                        .active_drag_value::<DockDragPayload>()
                        .cloned()
                        .or_else(|| frame_payload.clone());
                    let changed = host.update(app, |host, cx| {
                        if !host.accepts_window_callback(
                            window_binding,
                            window.window_handle().window_id(),
                        ) {
                            return false;
                        }
                        host.cancel_pointer_interactions_from_render(
                            payload.as_ref(),
                            event.reason,
                            window,
                            cx,
                        )
                    });
                    if changed {
                        window.refresh();
                    }
                });
            },
        )
        .absolute()
        .size_full();

        let mut host = div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .text_color(session.visual_style().host.foreground)
            .child(pointer_session_listener)
            .on_drag_move(cx.listener({
                let window_binding = window_binding;
                move |this, event: &DragMoveEvent<DockDragPayload>, window, cx| {
                    if !this
                        .accepts_window_callback(window_binding, window.window_handle().window_id())
                    {
                        return;
                    }
                    let payload = event.drag().clone();
                    let drag_session = this.active_payload_drag_session(&payload);
                    if crate::native_captured_drag::owns_native_captured_drag_source(
                        this.viewport_runtime().identity(),
                        drag_session.as_ref(),
                        &payload,
                        window.window_handle().window_id(),
                        &cx.entity().downgrade(),
                        this.current_window_binding(),
                        cx,
                    ) {
                        return;
                    }
                    let Ok(layout_position) = event.target_layout_position() else {
                        return;
                    };
                    this.begin_host_drop_scene_from_render(
                        &payload,
                        DockViewportHostGeometry::from_hitbox(event.hitbox()),
                        DockRenderedPointerPosition::new(layout_position, event.window_position()),
                        window,
                        cx,
                    );
                }
            }))
            .on_drop(cx.listener({
                let window_binding = window_binding;
                move |this, event: &DropEvent<DockDragPayload>, window, cx| {
                    if !this
                        .accepts_window_callback(window_binding, window.window_handle().window_id())
                    {
                        return;
                    }
                    let payload = event.value();
                    let Ok(layout_position) = event.pointer().target_layout_position() else {
                        return;
                    };
                    this.drop_payload_event_from_render(
                        payload,
                        drop_host_space.clone(),
                        DockRenderedPointerPosition::new(
                            layout_position,
                            event.pointer().window_event().position,
                        ),
                        window,
                        cx,
                    );
                }
            }));

        if active_docking_drag {
            let host_focus = self.host_focus_handle();
            let focus_ring = session.visual_style().focus_ring.clone();
            host = host
                .track_focus(&host_focus)
                .focus_visible(move |style| style.shadow(focus_ring.clone()))
                .capture_key_down(cx.listener({
                    let window_binding = window_binding;
                    move |this, event: &KeyDownEvent, window, cx| {
                        if !this.accepts_window_callback(
                            window_binding,
                            window.window_handle().window_id(),
                        ) || event.keystroke.key != "escape"
                            || event.keystroke.modifiers.modified()
                        {
                            return;
                        }
                        let Some(payload) = cx.active_drag_value::<DockDragPayload>().cloned()
                        else {
                            return;
                        };
                        if this.cancel_payload_drag_from_render(&payload, window, cx) {
                            window.refresh();
                        }
                        cx.stop_propagation();
                    }
                }));
        }

        if session.empty_central_passthrough() {
            host = host.bg(rgba(0x00000000));
        } else {
            host = host.bg(session.visual_style().host.background);
        }

        if runtime_publication_admitted {
            host = host.child(self.render_viewport_host_scene_probe(
                &viewport_host_scene_frame,
                &session,
                session.drop_guide_metrics(),
                session.empty_central_requests_platform_pointer_passthrough(),
                runtime_work_context.expect("admitted runtime publication requires a context"),
                cx,
            ));
        }

        if let Some(root) = session.root() {
            host = host.child(self.render_root_node(
                root,
                &session,
                &viewport_host_scene_frame,
                window,
                cx,
            ));
        } else if session.empty_central_passthrough() {
            host = host.child(self.render_passthrough_empty_central_space(
                &session,
                &viewport_host_scene_frame,
                window,
                cx,
            ));
        } else {
            host = host.child(self.render_empty_space(
                &session,
                &viewport_host_scene_frame,
                window,
                cx,
            ));
        }

        for floating in session.floating_containers() {
            host = host.child(self.render_floating_container(
                *floating,
                &session,
                &viewport_host_scene_frame,
                raw_drag_pointer_capture,
                window,
                cx,
            ));
        }

        host = host.child(self.render_divider_event_layer(&session, raw_drag_pointer_capture, cx));

        if let Some(sample) = transition_sample.as_ref() {
            host = host.child(self.render_transition_sample_layer(
                &session,
                &viewport_host_scene_frame,
                sample,
                window,
                cx,
            ));
        }

        if let Some(preview) = self.render_host_drop_preview(&session, window, cx) {
            host = host.child(preview);
        }

        if runtime_publication_admitted {
            host = host.child(
                self.render_viewport_host_scene_routing_sentinel(&viewport_host_scene_frame),
            );
        }

        if let Some(semantics) = self.render_live_destination_semantics_marker(cx) {
            host = host.child(semantics);
        }

        if let Some(restoration) = self.render_live_source_restoration_layer(cx) {
            host = host.child(restoration);
        }

        if let Some(recovery_region) =
            self.render_payload_recovery_region(&payload_recovery_entries, &session, cx)
        {
            host = host.child(recovery_region);
        }

        if runtime_publication_admitted {
            self.apply_pending_focus_from_render(&session, window, cx);
        }
        self.apply_payload_recovery_entry_focus_from_render(&payload_recovery_entries, window, cx);
        self.apply_payload_recovery_restore_focus_from_render(&session, window, cx);

        let restoration_is_staging = matches!(
            self.live_presentation_state().map(|state| state.mode),
            Some(DockHostLivePresentationMode::SourceRestoration {
                phase: DockHostLiveSourceRestorationPhase::Staging,
                ..
            })
        );
        let rendered_host = if restoration_is_staging {
            host.with_subtree_presentation(SubtreePresentation::Inert)
                .into_any_element()
        } else {
            host.into_any_element()
        };
        self.wrap_transport_and_semantic_proxies(rendered_host, window)
    }
}

fn live_presentation_key_is_current(
    host: &WeakEntity<DockHost>,
    key: DockHostLivePresentationKey,
    cx: &App,
) -> bool {
    host.read_with(cx, |host, _| host.accepts_live_presentation_key(key))
        .unwrap_or(false)
}

fn live_source_is_releasing(
    host: &WeakEntity<DockHost>,
    key: DockHostLivePresentationKey,
    cx: &App,
) -> bool {
    host.read_with(cx, |host, _| {
        host.live_presentation_state().is_some_and(|state| {
            state.key == key
                && matches!(
                    state.mode,
                    DockHostLivePresentationMode::SourceProjection {
                        phase: DockHostLiveSourcePhase::Releasing,
                        ..
                    }
                )
        })
    })
    .unwrap_or(false)
}

fn live_destination_is_staging(
    host: &WeakEntity<DockHost>,
    key: DockHostLivePresentationKey,
    cx: &App,
) -> bool {
    host.read_with(cx, |host, _| {
        host.live_presentation_state().is_some_and(|state| {
            state.key == key
                && matches!(
                    state.mode,
                    DockHostLivePresentationMode::DestinationProjection {
                        phase: DockHostLiveDestinationPhase::Staging,
                        ..
                    }
                )
        })
    })
    .unwrap_or(false)
}

fn live_destination_is_exposed(
    host: &WeakEntity<DockHost>,
    key: DockHostLivePresentationKey,
    mount: DockLiveUndockPayloadMountReceipt,
    cx: &App,
) -> bool {
    host.read_with(cx, |host, _| {
        host.live_presentation_state().is_some_and(|state| {
            state.key == key
                && matches!(
                    state.mode,
                    DockHostLivePresentationMode::DestinationProjection {
                        phase: DockHostLiveDestinationPhase::Exposed(current),
                        ..
                    } if current == mount
                )
        })
    })
    .unwrap_or(false)
}

fn live_destination_is_reveal_armed(
    host: &WeakEntity<DockHost>,
    key: DockHostLivePresentationKey,
    presentation: DockLiveUndockPayloadPresentationReceipt,
    cx: &App,
) -> bool {
    host.read_with(cx, |host, _| {
        host.live_presentation_state().is_some_and(|state| {
            state.key == key
                && matches!(
                    state.mode,
                    DockHostLivePresentationMode::DestinationProjection {
                        phase: DockHostLiveDestinationPhase::RevealArmed {
                            presentation: current,
                            ..
                        },
                        ..
                    } if current == presentation
                )
        })
    })
    .unwrap_or(false)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DockLiveRevealObserverAuthority {
    candidate_frame: DockLiveUndockPayloadPresentationReceipt,
    submitted_frame: Option<DockLiveUndockPayloadPresentationReceipt>,
}

fn live_destination_reveal_observer_authority(
    host: &WeakEntity<DockHost>,
    key: DockHostLivePresentationKey,
    presentation: DockLiveUndockPayloadPresentationReceipt,
    candidate_frame: DockLiveUndockPayloadPresentationReceipt,
    ticket: &open_gpui::WindowProvisionalRevealTicket,
    cx: &App,
) -> Option<DockLiveRevealObserverAuthority> {
    let expected_ticket = ticket.snapshot();
    host.read_with(cx, |host, _| {
        let state = host.live_presentation_state()?;
        if state.key != key {
            return None;
        }
        let DockHostLivePresentationMode::DestinationProjection {
            phase:
                DockHostLiveDestinationPhase::RevealObserving {
                    presentation: current,
                    candidate_frame: current_candidate_frame,
                    submitted_frame,
                    ticket: current_ticket,
                },
            ..
        } = state.mode
        else {
            return None;
        };
        let current_ticket = current_ticket.snapshot();
        (current == presentation
            && current_candidate_frame == candidate_frame
            && current_ticket.window_id() == expected_ticket.window_id()
            && current_ticket.session_generation() == expected_ticket.session_generation()
            && current_ticket.minimum_presentation_generation()
                == expected_ticket.minimum_presentation_generation())
        .then_some(DockLiveRevealObserverAuthority {
            candidate_frame,
            submitted_frame,
        })
    })
    .ok()
    .flatten()
}

fn submit_live_undock_fact(
    owner: &WeakEntity<DockSurfaceOwner>,
    fact: DockLiveUndockFact,
    cx: &mut App,
) {
    let Ok(runtime) = owner.read_with(cx, |owner, _| owner.live_undock_runtime()) else {
        return;
    };
    let _ = runtime.submit(fact, cx);
}

fn submit_live_presentation_failure(
    owner: &WeakEntity<DockSurfaceOwner>,
    host: &WeakEntity<DockHost>,
    key: DockHostLivePresentationKey,
    failure: DockLiveUndockPresentationFailure,
    cx: &mut App,
) {
    if !live_presentation_key_is_current(host, key, cx) {
        return;
    }
    submit_live_undock_fact(
        owner,
        DockLiveUndockFact::PresentationStageFailed {
            identity: key.identity(),
            failure,
        },
        cx,
    );
}

fn defer_live_source_restoration(
    owner: &WeakEntity<DockSurfaceOwner>,
    key: DockHostLivePresentationKey,
    lease: DockLiveUndockPayloadLeaseReceipt,
    failure: DockLiveUndockSourceRestorationFailure,
    cx: &mut App,
) {
    let Ok(runtime) = owner.read_with(cx, |owner, _| owner.live_undock_runtime()) else {
        return;
    };
    runtime.defer_source_restoration(key.identity(), lease.source(), lease, failure, cx);
}

fn observe_live_source_restoration(
    owner: WeakEntity<DockSurfaceOwner>,
    host: WeakEntity<DockHost>,
    key: DockHostLivePresentationKey,
    lease: DockLiveUndockPayloadLeaseReceipt,
    prepared: view_presentation_window::PreparedRehost,
    leases: view_presentation_window::LeaseBatch,
    phase: DockHostLiveSourceRestorationPhase,
    retained: Option<retained_visual::Ticket>,
    replay_succeeded: Option<bool>,
    accepted_frame: u64,
    window: &mut Window,
    cx: &mut App,
) {
    let current = host
        .read_with(cx, |host, _| {
            host.live_presentation_state().is_some_and(|state| {
                state.key == key
                    && matches!(
                        state.mode,
                        DockHostLivePresentationMode::SourceRestoration {
                            leases: ref current,
                            phase: current_phase,
                            ..
                        } if current_phase == phase
                            && current.window_id() == leases.window_id()
                            && current.leases() == leases.leases()
                    )
            })
        })
        .unwrap_or(false);
    if !current {
        return;
    }
    if retained.is_some() && replay_succeeded != Some(true) {
        defer_live_source_restoration(
            &owner,
            key,
            lease,
            DockLiveUndockSourceRestorationFailure::RetainedVisualReplayRejected,
            cx,
        );
        return;
    }

    match phase {
        DockHostLiveSourceRestorationPhase::Staging => {
            if prepared.accepted_source_restoration().is_none() {
                defer_live_source_restoration(
                    &owner,
                    key,
                    lease,
                    DockLiveUndockSourceRestorationFailure::PresentationTransitionRejected,
                    cx,
                );
                return;
            }
            let Ok(runtime) = owner.read_with(cx, |owner, _| owner.live_undock_runtime()) else {
                defer_live_source_restoration(
                    &owner,
                    key,
                    lease,
                    DockLiveUndockSourceRestorationFailure::ExecutionAuthorityUnavailable,
                    cx,
                );
                return;
            };
            match runtime.finish_source_restoration_presentation(
                key.identity(),
                key,
                lease,
                &prepared,
                &leases,
                cx,
            ) {
                DockLiveUndockSourceFinishOutcome::Finished => {}
                DockLiveUndockSourceFinishOutcome::AuthorityLossSubmitted => return,
                DockLiveUndockSourceFinishOutcome::Retry => {
                    defer_live_source_restoration(
                        &owner,
                        key,
                        lease,
                        DockLiveUndockSourceRestorationFailure::PresentationTransitionRejected,
                        cx,
                    );
                    return;
                }
            }
            #[cfg(test)]
            if runtime.take_replace_source_host_after_finish_for_test() {
                window.replace_root(cx, |_, _| open_gpui::Empty);
                return;
            }
            let advanced = host
                .update(cx, |host, cx| {
                    host.mark_live_source_restoration_visible_pending(key, &leases, cx)
                })
                .unwrap_or(false);
            if !advanced {
                defer_live_source_restoration(
                    &owner,
                    key,
                    lease,
                    DockLiveUndockSourceRestorationFailure::SourcePresentationMutationRejected,
                    cx,
                );
            }
        }
        DockHostLiveSourceRestorationPhase::AwaitingVisibleFrame => {
            let Some(stable) =
                view_presentation_window::stable_batch_presentation_receipt(cx, &leases)
                    .filter(|receipt| receipt.frame_generation() == accepted_frame)
            else {
                defer_live_source_restoration(
                    &owner,
                    key,
                    lease,
                    DockLiveUndockSourceRestorationFailure::StablePresentationUnavailable,
                    cx,
                );
                return;
            };
            let Some(receipt) =
                DockLiveUndockSourceRestorationReceipt::source_presented_after_release(
                    lease, &prepared, &leases, stable,
                )
            else {
                defer_live_source_restoration(
                    &owner,
                    key,
                    lease,
                    DockLiveUndockSourceRestorationFailure::RestorationReceiptUnavailable,
                    cx,
                );
                return;
            };
            let Ok(runtime) = owner.read_with(cx, |owner, _| owner.live_undock_runtime()) else {
                defer_live_source_restoration(
                    &owner,
                    key,
                    lease,
                    DockLiveUndockSourceRestorationFailure::ExecutionAuthorityUnavailable,
                    cx,
                );
                return;
            };
            if !runtime.stage_source_restoration_receipt(key.identity(), key, receipt)
                || !runtime.release_source_restoration_visual_in_frame(
                    key.identity(),
                    lease,
                    window,
                )
            {
                defer_live_source_restoration(
                    &owner,
                    key,
                    lease,
                    DockLiveUndockSourceRestorationFailure::SourcePresentationMutationRejected,
                    cx,
                );
                return;
            }
            runtime.finish_source_restoration_checkpoint(key.identity(), key, receipt, cx);
        }
    }
}

fn observe_live_source_proxy_commit(
    owner: WeakEntity<DockSurfaceOwner>,
    host: WeakEntity<DockHost>,
    key: DockHostLivePresentationKey,
    lease: DockLiveUndockPayloadLeaseReceipt,
    prepared: view_presentation_window::PreparedRehost,
    accepted_frame: u64,
    window: &mut Window,
    cx: &mut App,
) {
    if !live_source_is_releasing(&host, key, cx) {
        return;
    }
    let Some(gpui_receipt) = prepared
        .snapshot()
        .source_proxy_receipt()
        .filter(|receipt| receipt.frame_generation() == accepted_frame)
    else {
        let _ = window;
        submit_live_presentation_failure(
            &owner,
            &host,
            key,
            DockLiveUndockPresentationFailure::SourceProxyReplay { lease },
            cx,
        );
        return;
    };
    let Some(receipt) = DockLiveUndockSourceProxyReceipt::new(lease, gpui_receipt) else {
        submit_live_presentation_failure(
            &owner,
            &host,
            key,
            DockLiveUndockPresentationFailure::SourceProxyReplay { lease },
            cx,
        );
        return;
    };
    let advanced = host
        .update(cx, |host, cx| host.mark_live_source_frozen(key, cx))
        .unwrap_or(false);
    if advanced {
        submit_live_undock_fact(
            &owner,
            DockLiveUndockFact::SourceProxyCommitted {
                identity: key.identity(),
                receipt,
            },
            cx,
        );
    }
}

fn observe_live_destination_mount(
    owner: WeakEntity<DockSurfaceOwner>,
    host: WeakEntity<DockHost>,
    key: DockHostLivePresentationKey,
    proxy: DockLiveUndockSourceProxyReceipt,
    prepared: view_presentation_window::PreparedRehost,
    accepted_frame: u64,
    cx: &mut App,
) {
    if !live_destination_is_staging(&host, key, cx) {
        return;
    }
    let Some(mount) = prepared.destination_ready_for_exposure() else {
        submit_live_presentation_failure(
            &owner,
            &host,
            key,
            DockLiveUndockPresentationFailure::DestinationExposureFinish { proxy },
            cx,
        );
        return;
    };
    if mount.frame_generation() != accepted_frame {
        submit_live_presentation_failure(
            &owner,
            &host,
            key,
            DockLiveUndockPresentationFailure::DestinationExposureFinish { proxy },
            cx,
        );
        return;
    }
    let Ok(outcome) = view_presentation_window::expose_destination(cx, &prepared) else {
        submit_live_presentation_failure(
            &owner,
            &host,
            key,
            DockLiveUndockPresentationFailure::DestinationExposureFinish { proxy },
            cx,
        );
        return;
    };
    let view_presentation_window::DestinationExposureOutcome { batch, exposure } = outcome;
    let Some(receipt) = DockLiveUndockPayloadMountReceipt::new(proxy, exposure) else {
        submit_live_presentation_failure(
            &owner,
            &host,
            key,
            DockLiveUndockPresentationFailure::DestinationExposureFinish { proxy },
            cx,
        );
        return;
    };
    let advanced = host
        .update(cx, |host, cx| {
            host.expose_live_destination_projection(key, batch, receipt, cx)
        })
        .unwrap_or(false);
    if advanced {
        submit_live_undock_fact(
            &owner,
            DockLiveUndockFact::PayloadMounted {
                identity: key.identity(),
                receipt,
            },
            cx,
        );
    }
}

fn observe_live_destination_presentation(
    owner: WeakEntity<DockSurfaceOwner>,
    host: WeakEntity<DockHost>,
    key: DockHostLivePresentationKey,
    mount: DockLiveUndockPayloadMountReceipt,
    leases: view_presentation_window::LeaseBatch,
    accepted_frame: u64,
    window: &mut Window,
    cx: &mut App,
) {
    if !live_destination_is_exposed(&host, key, mount, cx) {
        return;
    }
    let Some(gpui_receipt) = view_presentation_window::presented_batch_receipt(cx, &leases) else {
        let _ = window;
        submit_live_presentation_failure(
            &owner,
            &host,
            key,
            DockLiveUndockPresentationFailure::PayloadPresentationObservation { mount },
            cx,
        );
        return;
    };
    let Some(receipt) = DockLiveUndockPayloadPresentationReceipt::new(mount, gpui_receipt) else {
        submit_live_presentation_failure(
            &owner,
            &host,
            key,
            DockLiveUndockPresentationFailure::PayloadPresentationObservation { mount },
            cx,
        );
        return;
    };
    if receipt.frame_generation() != accepted_frame {
        submit_live_presentation_failure(
            &owner,
            &host,
            key,
            DockLiveUndockPresentationFailure::PayloadPresentationObservation { mount },
            cx,
        );
        return;
    }
    let advanced = host
        .update(cx, |host, cx| {
            host.mark_live_destination_presented(key, receipt, cx)
        })
        .unwrap_or(false);
    if advanced {
        submit_live_undock_fact(
            &owner,
            DockLiveUndockFact::PayloadPresented {
                identity: key.identity(),
                receipt,
            },
            cx,
        );
    }
}

fn dock_live_reveal_outcome(
    outcome: WindowProvisionalRevealOutcome,
) -> Option<DockLiveUndockRevealOutcome> {
    match outcome {
        WindowProvisionalRevealOutcome::Pending | WindowProvisionalRevealOutcome::Revealed => None,
        WindowProvisionalRevealOutcome::Rejected => Some(DockLiveUndockRevealOutcome::Rejected),
        WindowProvisionalRevealOutcome::NativeObservationMissing => {
            Some(DockLiveUndockRevealOutcome::NativeObservationMissing)
        }
        WindowProvisionalRevealOutcome::Cancelled => Some(DockLiveUndockRevealOutcome::Stale),
        WindowProvisionalRevealOutcome::Stale => Some(DockLiveUndockRevealOutcome::Stale),
        WindowProvisionalRevealOutcome::WindowTerminal => {
            Some(DockLiveUndockRevealOutcome::WindowTerminal)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockLiveRevealSnapshotClassification {
    Pending,
    Revealed,
    Failed(DockLiveUndockRevealOutcome),
}

fn classify_live_reveal_snapshot(
    outcome: WindowProvisionalRevealOutcome,
    presentation_generation: Option<u64>,
    submitted_frame_generation: Option<u64>,
) -> DockLiveRevealSnapshotClassification {
    let exact_submission = presentation_generation
        .zip(submitted_frame_generation)
        .is_some_and(|(presentation, submitted)| presentation == submitted);
    match outcome {
        WindowProvisionalRevealOutcome::Pending
            if presentation_generation.is_none() && submitted_frame_generation.is_none() =>
        {
            DockLiveRevealSnapshotClassification::Pending
        }
        WindowProvisionalRevealOutcome::Pending if exact_submission => {
            DockLiveRevealSnapshotClassification::Pending
        }
        WindowProvisionalRevealOutcome::Revealed if exact_submission => {
            DockLiveRevealSnapshotClassification::Revealed
        }
        WindowProvisionalRevealOutcome::Pending | WindowProvisionalRevealOutcome::Revealed => {
            DockLiveRevealSnapshotClassification::Failed(DockLiveUndockRevealOutcome::Stale)
        }
        terminal => DockLiveRevealSnapshotClassification::Failed(
            dock_live_reveal_outcome(terminal)
                .expect("a non-pending reveal outcome must map to a Dock terminal outcome"),
        ),
    }
}

fn current_live_destination_reveal_frame(
    preflight: DockLiveUndockPayloadPresentationReceipt,
    leases: &view_presentation_window::LeaseBatch,
    cx: &App,
) -> Option<DockLiveUndockPayloadPresentationReceipt> {
    view_presentation_window::presented_batch_receipt(cx, leases).and_then(|receipt| {
        DockLiveUndockPayloadPresentationReceipt::new(preflight.mount(), receipt)
    })
}

fn bind_live_destination_reveal_submission(
    host: &WeakEntity<DockHost>,
    key: DockHostLivePresentationKey,
    preflight: DockLiveUndockPayloadPresentationReceipt,
    submitted_frame: DockLiveUndockPayloadPresentationReceipt,
    authority: &mut DockLiveRevealObserverAuthority,
    cx: &mut App,
) -> bool {
    if authority.submitted_frame == Some(submitted_frame) {
        return true;
    }
    let bound = host
        .update(cx, |host, cx| {
            host.bind_live_destination_reveal_submission(key, preflight, submitted_frame, cx)
        })
        .unwrap_or(false);
    if bound {
        authority.submitted_frame = Some(submitted_frame);
    }
    bound
}

#[allow(clippy::too_many_arguments)]
fn expire_live_destination_reveal_observation(
    owner: WeakEntity<DockSurfaceOwner>,
    host: WeakEntity<DockHost>,
    window: AnyWindowHandle,
    key: DockHostLivePresentationKey,
    preflight: DockLiveUndockPayloadPresentationReceipt,
    candidate_frame: DockLiveUndockPayloadPresentationReceipt,
    leases: view_presentation_window::LeaseBatch,
    ticket: open_gpui::WindowProvisionalRevealTicket,
    cx: &mut App,
) {
    let Some(mut authority) = live_destination_reveal_observer_authority(
        &host,
        key,
        preflight,
        candidate_frame,
        &ticket,
        cx,
    ) else {
        return;
    };
    let cancellation = match window.update(cx, |_, window, cx| {
        window.cancel_provisional_presentation(&ticket, cx)
    }) {
        Ok(Ok(cancellation)) => cancellation,
        Ok(Err(error)) => {
            log::error!(
                "failed to cancel exact live-undock reveal ticket at its deadline: {error}"
            );
            let snapshot = ticket.snapshot();
            let outcome = dock_live_reveal_outcome(snapshot.outcome())
                .unwrap_or(DockLiveUndockRevealOutcome::Stale);
            let observation = DockLiveUndockRevealObservation::failed(
                authority
                    .submitted_frame
                    .unwrap_or(authority.candidate_frame),
                outcome,
            );
            submit_live_destination_reveal_observation(
                &owner,
                &host,
                key,
                preflight,
                authority.submitted_frame,
                observation,
                cx,
            );
            return;
        }
        Err(error) => {
            log::debug!(
                "live-undock reveal deadline observed a logically unavailable window: {error}"
            );
            let observation = DockLiveUndockRevealObservation::failed(
                authority
                    .submitted_frame
                    .unwrap_or(authority.candidate_frame),
                DockLiveUndockRevealOutcome::WindowTerminal,
            );
            submit_live_destination_reveal_observation(
                &owner,
                &host,
                key,
                preflight,
                authority.submitted_frame,
                observation,
                cx,
            );
            return;
        }
    };
    let deadline_won = matches!(
        cancellation,
        WindowProvisionalRevealCancellationOutcome::Cancelled(_)
    );
    let snapshot = match cancellation {
        WindowProvisionalRevealCancellationOutcome::Cancelled(snapshot)
        | WindowProvisionalRevealCancellationOutcome::AlreadySettled(snapshot) => snapshot,
    };
    if authority.submitted_frame.is_none()
        && let Some(presentation_generation) = snapshot.presentation_generation()
        && let Some(submitted_frame) = current_live_destination_reveal_frame(preflight, &leases, cx)
        && submitted_frame.frame_generation() == presentation_generation
        && !bind_live_destination_reveal_submission(
            &host,
            key,
            preflight,
            submitted_frame,
            &mut authority,
            cx,
        )
    {
        return;
    }

    let classification = if deadline_won {
        DockLiveRevealSnapshotClassification::Failed(
            DockLiveUndockRevealOutcome::ObservationDeadlineExpired,
        )
    } else {
        classify_live_reveal_snapshot(
            snapshot.outcome(),
            snapshot.presentation_generation(),
            authority
                .submitted_frame
                .map(DockLiveUndockPayloadPresentationReceipt::frame_generation),
        )
    };
    let observation = match classification {
        DockLiveRevealSnapshotClassification::Revealed => {
            let Some(submitted_frame) = authority.submitted_frame else {
                return;
            };
            let Some(receipt) =
                DockLiveUndockRevealReceipt::new(preflight, submitted_frame, snapshot)
            else {
                submit_live_presentation_failure(
                    &owner,
                    &host,
                    key,
                    DockLiveUndockPresentationFailure::ExactRevealTicket {
                        presentation: preflight,
                    },
                    cx,
                );
                return;
            };
            DockLiveUndockRevealObservation::Visible(receipt)
        }
        DockLiveRevealSnapshotClassification::Failed(outcome) => {
            DockLiveUndockRevealObservation::failed(
                authority
                    .submitted_frame
                    .unwrap_or(authority.candidate_frame),
                outcome,
            )
        }
        DockLiveRevealSnapshotClassification::Pending => return,
    };
    submit_live_destination_reveal_observation(
        &owner,
        &host,
        key,
        preflight,
        authority.submitted_frame,
        observation,
        cx,
    );
}

fn capture_live_destination_reveal_frame(
    owner: WeakEntity<DockSurfaceOwner>,
    host: WeakEntity<DockHost>,
    key: DockHostLivePresentationKey,
    preflight: DockLiveUndockPayloadPresentationReceipt,
    leases: view_presentation_window::LeaseBatch,
    accepted_frame: u64,
    window: &mut Window,
    cx: &mut App,
) {
    if !live_destination_is_reveal_armed(&host, key, preflight, cx) {
        return;
    }
    let Some(gpui_receipt) = view_presentation_window::presented_batch_receipt(cx, &leases) else {
        submit_live_presentation_failure(
            &owner,
            &host,
            key,
            DockLiveUndockPresentationFailure::ExactRevealTicket {
                presentation: preflight,
            },
            cx,
        );
        return;
    };
    let Some(candidate_frame) =
        DockLiveUndockPayloadPresentationReceipt::new(preflight.mount(), gpui_receipt)
    else {
        submit_live_presentation_failure(
            &owner,
            &host,
            key,
            DockLiveUndockPresentationFailure::ExactRevealTicket {
                presentation: preflight,
            },
            cx,
        );
        return;
    };
    if candidate_frame.frame_generation() != accepted_frame
        || candidate_frame.frame_generation() <= preflight.frame_generation()
    {
        submit_live_presentation_failure(
            &owner,
            &host,
            key,
            DockLiveUndockPresentationFailure::ExactRevealTicket {
                presentation: preflight,
            },
            cx,
        );
        return;
    }

    let Some(ticket) = host
        .update(cx, |host, cx| {
            host.begin_live_destination_reveal_observation(key, preflight, candidate_frame, cx)
        })
        .ok()
        .flatten()
    else {
        return;
    };

    let deadline_owner = owner.clone();
    let deadline_host = host.clone();
    let deadline_window = window.window_handle();
    let deadline_leases = leases.clone();
    let deadline_ticket = ticket.clone();
    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(LIVE_UNDOCK_REVEAL_OBSERVATION_DEADLINE)
            .await;
        cx.update(|cx| {
            expire_live_destination_reveal_observation(
                deadline_owner,
                deadline_host,
                deadline_window,
                key,
                preflight,
                candidate_frame,
                deadline_leases,
                deadline_ticket,
                cx,
            );
        });
    })
    .detach();

    window.on_next_frame(move |window, cx| {
        observe_live_destination_reveal_native(
            owner,
            host,
            key,
            preflight,
            candidate_frame,
            leases,
            ticket,
            window,
            cx,
        );
    });
    window.refresh();
}

#[allow(clippy::too_many_arguments)]
fn observe_live_destination_reveal_native(
    owner: WeakEntity<DockSurfaceOwner>,
    host: WeakEntity<DockHost>,
    key: DockHostLivePresentationKey,
    preflight: DockLiveUndockPayloadPresentationReceipt,
    candidate_frame: DockLiveUndockPayloadPresentationReceipt,
    leases: view_presentation_window::LeaseBatch,
    ticket: open_gpui::WindowProvisionalRevealTicket,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(mut authority) = live_destination_reveal_observer_authority(
        &host,
        key,
        preflight,
        candidate_frame,
        &ticket,
        cx,
    ) else {
        return;
    };
    let mut snapshot = ticket.snapshot();
    let mut wait_for_submitted_frame = false;

    if authority.submitted_frame.is_none()
        && let Some(presentation_generation) = snapshot.presentation_generation()
    {
        match current_live_destination_reveal_frame(preflight, &leases, cx) {
            Some(submitted_frame)
                if submitted_frame.frame_generation() == presentation_generation =>
            {
                if !bind_live_destination_reveal_submission(
                    &host,
                    key,
                    preflight,
                    submitted_frame,
                    &mut authority,
                    cx,
                ) {
                    return;
                }
            }
            Some(current_frame)
                if current_frame.frame_generation() < presentation_generation
                    && snapshot.outcome() == WindowProvisionalRevealOutcome::Pending =>
            {
                wait_for_submitted_frame = true;
            }
            None if snapshot.outcome() == WindowProvisionalRevealOutcome::Pending => {
                wait_for_submitted_frame = true;
            }
            None => {}
            Some(_) => {}
        }
    }

    if wait_for_submitted_frame {
        let next_owner = owner.clone();
        let next_host = host.clone();
        let next_ticket = ticket.clone();
        let next_leases = leases.clone();
        window.on_next_frame(move |window, cx| {
            observe_live_destination_reveal_native(
                next_owner,
                next_host,
                key,
                preflight,
                candidate_frame,
                next_leases,
                next_ticket,
                window,
                cx,
            );
        });
        window.refresh();
        return;
    }

    let mut classification = classify_live_reveal_snapshot(
        snapshot.outcome(),
        snapshot.presentation_generation(),
        authority
            .submitted_frame
            .map(DockLiveUndockPayloadPresentationReceipt::frame_generation),
    );
    if matches!(
        classification,
        DockLiveRevealSnapshotClassification::Failed(DockLiveUndockRevealOutcome::Stale)
    ) && matches!(
        snapshot.outcome(),
        WindowProvisionalRevealOutcome::Pending | WindowProvisionalRevealOutcome::Revealed
    ) {
        match window.cancel_provisional_presentation(&ticket, cx) {
            Ok(WindowProvisionalRevealCancellationOutcome::Cancelled(settled))
            | Ok(WindowProvisionalRevealCancellationOutcome::AlreadySettled(settled)) => {
                snapshot = settled;
                classification = classify_live_reveal_snapshot(
                    snapshot.outcome(),
                    snapshot.presentation_generation(),
                    authority
                        .submitted_frame
                        .map(DockLiveUndockPayloadPresentationReceipt::frame_generation),
                );
            }
            Err(error) => {
                log::error!("failed to cancel a stale exact live-undock reveal ticket: {error}");
            }
        }
    }

    let observation = match classification {
        DockLiveRevealSnapshotClassification::Pending => {
            let next_owner = owner.clone();
            let next_host = host.clone();
            let next_ticket = ticket.clone();
            let next_leases = leases.clone();
            window.on_next_frame(move |window, cx| {
                observe_live_destination_reveal_native(
                    next_owner,
                    next_host,
                    key,
                    preflight,
                    candidate_frame,
                    next_leases,
                    next_ticket,
                    window,
                    cx,
                );
            });
            window.refresh();
            return;
        }
        DockLiveRevealSnapshotClassification::Revealed => {
            let Some(submitted_frame) = authority.submitted_frame else {
                unreachable!("a revealed snapshot must retain its exact submitted frame");
            };
            let Some(receipt) =
                DockLiveUndockRevealReceipt::new(preflight, submitted_frame, snapshot)
            else {
                submit_live_presentation_failure(
                    &owner,
                    &host,
                    key,
                    DockLiveUndockPresentationFailure::ExactRevealTicket {
                        presentation: preflight,
                    },
                    cx,
                );
                return;
            };
            DockLiveUndockRevealObservation::Visible(receipt)
        }
        DockLiveRevealSnapshotClassification::Failed(outcome) => {
            DockLiveUndockRevealObservation::failed(
                authority
                    .submitted_frame
                    .unwrap_or(authority.candidate_frame),
                outcome,
            )
        }
    };
    submit_live_destination_reveal_observation(
        &owner,
        &host,
        key,
        preflight,
        authority.submitted_frame,
        observation,
        cx,
    );
}

fn submit_live_destination_reveal_observation(
    owner: &WeakEntity<DockSurfaceOwner>,
    host: &WeakEntity<DockHost>,
    key: DockHostLivePresentationKey,
    preflight: DockLiveUndockPayloadPresentationReceipt,
    submitted_frame: Option<DockLiveUndockPayloadPresentationReceipt>,
    observation: DockLiveUndockRevealObservation,
    cx: &mut App,
) {
    if host
        .update(cx, |host, cx| {
            host.settle_live_destination_reveal(key, preflight, submitted_frame, cx)
        })
        .unwrap_or(false)
    {
        submit_live_undock_fact(
            owner,
            DockLiveUndockFact::RevealObserved {
                identity: key.identity(),
                observation,
            },
            cx,
        );
    }
}

impl DockHost {
    fn render_payload_recovery_region(
        &mut self,
        entries: &[DockPayloadRecoveryEntry],
        session: &DockHostRenderSession,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if entries.is_empty() {
            return None;
        }

        let window_binding = self.current_window_binding();
        let mut region = div()
            .id("dock-lost-viewport-recovery-region")
            .absolute()
            .top(px(12.0))
            .right(px(12.0))
            .w(px(280.0))
            .flex()
            .flex_col()
            .gap_2();
        for entry in entries {
            let payload_name = self
                .with_workspace(cx, |workspace| {
                    entry
                        .items()
                        .iter()
                        .map(|item| {
                            workspace
                                .panels()
                                .catalog()
                                .descriptor(item)
                                .map(|descriptor| descriptor.title().to_string())
                                .unwrap_or_else(|| item.to_string())
                        })
                        .collect::<Vec<_>>()
                })
                .join(", ");
            let accessible_name: SharedString =
                format!("Lost viewport recovery for {payload_name}").into();
            let action = entry.action();
            let click_action = action;
            let accessibility_action = action;
            let accessibility_host = cx.entity();
            let entry_focus = entry.focus_handle().clone();
            let focus_ring = session.visual_style().focus_ring.clone();
            let background = session.visual_style().tabs.frame_background;
            let hover_background = session.visual_style().tabs.hovered.background;
            let border = session.visual_style().tabs.frame_border;
            let foreground = session.visual_style().host.foreground;
            let muted = session.visual_style().host.empty_text;
            let accent = session.visual_style().tabs.selected.text;
            let card = div()
                .id(format!(
                    "dock-lost-viewport-recovery:{}",
                    entry.generation()
                ))
                .role(Role::Group)
                .aria_label(accessible_name)
                .aria_actions([AccessibleAction::Click])
                .focusable()
                .track_focus(&entry_focus)
                .flex()
                .flex_col()
                .gap_1()
                .p_3()
                .rounded_sm()
                .border_1()
                .border_color(border)
                .bg(background)
                .text_color(foreground)
                .cursor_pointer()
                .occlude()
                .hover(move |style| style.bg(hover_background))
                .focus_visible(move |style| style.shadow(focus_ring.clone()))
                .on_click(cx.listener(move |host, _, window, cx| {
                    if !host
                        .accepts_window_callback(window_binding, window.window_handle().window_id())
                    {
                        return;
                    }
                    let _ = host.restore_payload_recovery_from_render(click_action, window, cx);
                }))
                .on_a11y_action(AccessibleAction::Click, move |_, window, app| {
                    accessibility_host.update(app, |host, cx| {
                        if !host.accepts_window_callback(
                            window_binding,
                            window.window_handle().window_id(),
                        ) {
                            return;
                        }
                        let _ = host.restore_payload_recovery_from_render(
                            accessibility_action,
                            window,
                            cx,
                        );
                    });
                })
                .child(div().text_sm().text_color(muted).child("Lost viewport"))
                .child(div().child(payload_name))
                .child(div().text_sm().text_color(accent).child("Restore"));
            region = region.child(card);
        }
        Some(region.into_any_element())
    }

    fn render_live_destination_semantics_marker(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let semantics = self.live_destination_semantics()?;
        let owner = self.surface_owner_entity()?.downgrade();
        let host = cx.entity().downgrade();
        Some(
            canvas(
                move |_, window, _| {
                    let owner = owner.clone();
                    let host = host.clone();
                    let semantics = semantics.clone();
                    window.record_prepaint_focus_stable_commit(move |frame, window, cx| {
                        let Some(owner) = owner.upgrade() else {
                            return;
                        };
                        let Some(host) = host.upgrade() else {
                            return;
                        };
                        let runtime =
                            cx.read_entity(&owner, |owner, _| owner.live_undock_runtime());
                        runtime.accept_destination_semantics_frame(
                            &host, &semantics, frame, window, cx,
                        );
                    });
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full()
            .into_any_element(),
        )
    }

    fn render_live_source_restoration_layer(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let state = self.live_presentation_state()?;
        let DockHostLivePresentationMode::SourceRestoration {
            lease,
            prepared,
            leases,
            retained,
            phase,
        } = state.mode
        else {
            return None;
        };
        let owner = self.surface_owner_entity()?.downgrade();
        let host = cx.entity().downgrade();
        let retained_ticket = retained.as_ref().map(|(ticket, _)| *ticket);
        let retained_bounds = retained.as_ref().map(|(_, carrier)| carrier.bounds);
        let replay_succeeded = Rc::new(Cell::new(None));
        let observe_replay_succeeded = replay_succeeded.clone();
        let paint_replay_succeeded = replay_succeeded;
        let observe_owner = owner.clone();
        let observe_host = host.clone();
        let observe_prepared = prepared.clone();
        let observe_leases = leases.clone();
        let observer = canvas(
            move |_, window, _| {
                let owner = observe_owner.clone();
                let host = observe_host.clone();
                let prepared = observe_prepared.clone();
                let leases = observe_leases.clone();
                let replay_succeeded = observe_replay_succeeded.clone();
                window.record_prepaint_focus_stable_commit(move |frame, window, cx| {
                    observe_live_source_restoration(
                        owner.clone(),
                        host.clone(),
                        state.key,
                        lease,
                        prepared.clone(),
                        leases.clone(),
                        phase,
                        retained_ticket,
                        replay_succeeded.take(),
                        frame,
                        window,
                        cx,
                    );
                });
            },
            move |_, _, window, _| {
                paint_replay_succeeded.set(Some(
                    retained_ticket
                        .map(|retained| retained_visual::replay(window, &retained).is_ok())
                        .unwrap_or(true),
                ));
            },
        )
        .absolute()
        .size_full();

        if let Some(bounds) = retained_bounds {
            Some(
                div()
                    .id(format!(
                        "dock-live-source-restoration:{}:{}",
                        state.key.identity().opening().generation(),
                        state.key.identity().drag_generation().get()
                    ))
                    .relative()
                    .absolute()
                    .left(bounds.origin.x)
                    .top(bounds.origin.y)
                    .w(bounds.size.width)
                    .h(bounds.size.height)
                    .occlude()
                    .child(observer)
                    .into_any_element(),
            )
        } else {
            Some(observer.into_any_element())
        }
    }

    fn render_live_source_semantic_proxy(&self, window: &Window) -> Option<AnyElement> {
        let proxy = self.live_source_semantic_proxy()?;
        if matches!(
            self.live_presentation_state().map(|state| state.mode),
            Some(DockHostLivePresentationMode::SourceRestoration {
                phase: DockHostLiveSourceRestorationPhase::AwaitingVisibleFrame,
                ..
            })
        ) {
            return None;
        }

        let key = proxy.key();
        let bounds = proxy.carrier().bounds;
        let mut element = div()
            .id(format!(
                "dock-live-source-semantic-proxy:{}:{}:{}:{}:{}",
                key.identity().opening().generation(),
                key.identity().drag_generation().get(),
                key.rehost_generation(),
                key.binding().generation(),
                key.epoch(),
            ))
            .absolute()
            .left(bounds.origin.x)
            .top(bounds.origin.y)
            .w(bounds.size.width)
            .h(bounds.size.height)
            .role(Role::Group)
            .aria_label(proxy.accessible_name().clone())
            .aria_actions([]);
        if let Some(source_focus) = proxy
            .source_focus()
            .filter(|focus| focus.claim_revision() == window.focus_claim_revision())
        {
            element = element.track_accessibility_focus(source_focus.focus_handle());
        }
        Some(element.into_any_element())
    }

    fn render_native_drag_transport_proxy(&self) -> Option<AnyElement> {
        let proxy = self.native_drag_transport_proxy()?;
        // This hitbox transports capture identity only, so its geometry stays host-local.
        Some(
            div()
                .absolute()
                .size(px(1.0))
                .track_pointer_capture(&proxy.pointer_capture())
                .into_any_element(),
        )
    }

    fn wrap_transport_and_semantic_proxies(
        &self,
        visual: AnyElement,
        window: &Window,
    ) -> AnyElement {
        let semantic_proxy = self.render_live_source_semantic_proxy(window);
        let transport_proxy = self.render_native_drag_transport_proxy();
        if semantic_proxy.is_none() && transport_proxy.is_none() {
            return visual;
        }

        let mut root = div().relative().size_full().child(visual);
        if let Some(transport_proxy) = transport_proxy {
            root = root.child(transport_proxy);
        }
        if let Some(semantic_proxy) = semantic_proxy {
            root = root.child(semantic_proxy);
        }
        root.into_any_element()
    }

    fn render_payload_recovery_projection(
        &mut self,
        session: &DockHostRenderSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let viewport_host_scene_frame =
            Rc::new(RefCell::new(DockViewportHostSceneCandidateState::default()));
        let selector = self.record_debug_selector(
            DockDebugRegion::Host,
            format!("{}:payload-recovery", session.selector_prefix()),
        );
        let mut root = div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .text_color(session.visual_style().host.foreground)
            .bg(session.visual_style().host.background);
        let Some(state) = self.payload_recovery_presentation_state() else {
            return root
                .with_subtree_presentation(SubtreePresentation::Inert)
                .into_any_element();
        };
        let destination_awaits_source_release = matches!(
            &state.mode,
            DockHostRecoveryPresentationMode::DestinationProjection {
                phase: DockHostRecoveryDestinationPhase::AwaitingSourceRelease,
                ..
            }
        );
        if !destination_awaits_source_release {
            if let Some(node) = session.root() {
                root = root.child(self.render_node(
                    node,
                    session,
                    &viewport_host_scene_frame,
                    window,
                    cx,
                ));
            }
            for floating in session.floating_containers() {
                root = root.child(self.render_payload_recovery_floating_container(
                    *floating,
                    session,
                    &viewport_host_scene_frame,
                    window,
                    cx,
                ));
            }
        }
        let Some(owner) = self.surface_owner_entity().map(|owner| owner.downgrade()) else {
            return root
                .with_subtree_presentation(SubtreePresentation::Inert)
                .into_any_element();
        };
        let host = cx.entity().downgrade();
        let key = state.key;

        match state.mode {
            DockHostRecoveryPresentationMode::SourceProjection { prepared, phase } => {
                if phase == DockHostRecoverySourcePhase::Releasing {
                    let proxy_color = rgba(0x00000001);
                    let barrier = view_presentation_window::source_release_barrier(
                        prepared.clone(),
                        move |attempt| {
                            canvas(
                                |_, _, _| (),
                                move |bounds, _, window, _| {
                                    window.paint_quad(quad(
                                        bounds,
                                        px(0.0),
                                        proxy_color,
                                        px(0.0),
                                        proxy_color,
                                        BorderStyle::Solid,
                                    ));
                                    let _ = view_presentation_window::source_proxy_replay_succeeded(
                                        &attempt, window,
                                    );
                                },
                            )
                            .absolute()
                            .size_full()
                        },
                    );
                    let observer_owner = owner.clone();
                    let observer_host = host.clone();
                    let observer_prepared = prepared;
                    let observer = canvas(
                        move |_, window, _| {
                            let owner = observer_owner.clone();
                            let host = observer_host.clone();
                            let prepared = observer_prepared.clone();
                            window.record_prepaint_focus_stable_commit(
                                move |accepted_frame, _, cx| {
                                    crate::surface::payload_recovery_executor::payload_recovery_source_proxy_committed(
                                        owner.clone(),
                                        host.clone(),
                                        key,
                                        prepared.clone(),
                                        accepted_frame,
                                        cx,
                                    );
                                },
                            );
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full();
                    root = root.child(barrier).child(observer);
                }
            }
            DockHostRecoveryPresentationMode::DestinationProjection {
                prepared,
                leases,
                resolved_roots,
                phase,
            } => {
                if let Some(hidden_roots) =
                    self.render_payload_recovery_hidden_roots(&resolved_roots, session, window, cx)
                {
                    root = root.child(hidden_roots);
                }
                let observer_owner = owner.clone();
                let observer_host = host.clone();
                let observer_prepared = prepared;
                let observer_leases = leases;
                let observer = canvas(
                    move |_, window, _| {
                        let owner = observer_owner.clone();
                        let host = observer_host.clone();
                        let prepared = observer_prepared.clone();
                        let leases = observer_leases.clone();
                        window.record_prepaint_focus_stable_commit(
                            move |accepted_frame, _, cx| match phase {
                                DockHostRecoveryDestinationPhase::AwaitingSourceRelease => {}
                                DockHostRecoveryDestinationPhase::Staging => {
                                    crate::surface::payload_recovery_executor::payload_recovery_destination_mounted(
                                        owner.clone(),
                                        host.clone(),
                                        key,
                                        prepared.clone(),
                                        leases.clone(),
                                        accepted_frame,
                                        cx,
                                    );
                                }
                                DockHostRecoveryDestinationPhase::Exposed(_) => {
                                    crate::surface::payload_recovery_executor::payload_recovery_destination_presented(
                                        owner.clone(),
                                        host.clone(),
                                        key,
                                        prepared.clone(),
                                        leases.clone(),
                                        accepted_frame,
                                        cx,
                                    );
                                }
                            },
                        );
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full();
                root = root.child(observer);
            }
            DockHostRecoveryPresentationMode::SourceRestoration {
                prepared,
                resolved_roots,
                ..
            } => {
                if let Some(hidden_roots) =
                    self.render_payload_recovery_hidden_roots(&resolved_roots, session, window, cx)
                {
                    root = root.child(hidden_roots);
                }
                let observer_owner = owner;
                let observer_host = host;
                let observer = canvas(
                    move |_, window, _| {
                        let owner = observer_owner.clone();
                        let host = observer_host.clone();
                        let prepared = prepared.clone();
                        window.record_prepaint_focus_stable_commit(move |_, _, cx| {
                            crate::surface::payload_recovery_executor::payload_recovery_source_restoration_frame_committed(
                                owner.clone(),
                                host.clone(),
                                key,
                                prepared.clone(),
                                cx,
                            );
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full();
                root = root.child(observer);
            }
        }

        root.with_subtree_presentation(SubtreePresentation::Inert)
            .into_any_element()
    }

    fn render_payload_recovery_hidden_roots(
        &mut self,
        resolved_roots: &[open_gpui::AnyView],
        session: &DockHostRenderSession,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let visible = session.resolved_visible_panel_entity_ids();
        let hidden_roots = resolved_roots
            .iter()
            .filter(|root| !visible.contains(&root.entity_id()))
            .cloned()
            .collect::<Vec<_>>();
        if hidden_roots.is_empty() {
            return None;
        }
        let mut hidden_lane = div().absolute().size_full();
        for root in hidden_roots {
            hidden_lane = hidden_lane.child(self.present_panel_view(root, window, cx));
        }
        Some(
            hidden_lane
                .with_subtree_presentation(SubtreePresentation::Hidden)
                .into_any_element(),
        )
    }

    fn render_payload_recovery_floating_container(
        &mut self,
        container: DockFloatingContainer,
        session: &DockHostRenderSession,
        viewport_host_scene_frame: &DockViewportHostSceneCandidateSlot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let content = session
            .floating_child(container.node)
            .map(|child| self.render_node(child, session, viewport_host_scene_frame, window, cx))
            .unwrap_or_else(|| self.render_missing_node(container.node, session));
        let floating_style = &session.visual_style().floating;
        div()
            .absolute()
            .left(container.bounds.origin.x)
            .top(container.bounds.origin.y)
            .w(container.bounds.size.width)
            .h(container.bounds.size.height)
            .flex()
            .flex_col()
            .overflow_hidden()
            .border_1()
            .border_color(floating_style.border)
            .bg(floating_style.background)
            .child(content)
            .into_any_element()
    }

    fn render_live_destination_geometry_probe(
        &self,
        work_context: DockViewportRuntimeWorkContext,
        _cx: &mut Context<Self>,
    ) -> AnyElement {
        let entity = _cx.entity();
        let binding = self
            .current_window_binding()
            .expect("a live destination projection must retain its window binding");
        canvas(
            move |bounds, window, app| {
                let window_id = window.window_handle().window_id();
                let window_facts = crate::DockViewportWindowFacts::from_window(window, app);
                let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                let host_geometry = DockViewportHostGeometry::from_hitbox(&hitbox);
                window.record_prepaint_focus_stable_commit({
                    let entity = entity.clone();
                    move |_, _, app| {
                        entity.update(app, |host, cx| {
                            host.commit_live_destination_geometry_from_accepted_frame(
                                binding,
                                work_context,
                                window_facts.current_bounds,
                                host_geometry.clone(),
                                window_id,
                                cx,
                            );
                        });
                    }
                });
            },
            |_, _, _, _| (),
        )
        .absolute()
        .size_full()
        .into_any_element()
    }

    fn render_live_payload_projection(
        &mut self,
        session: &DockHostRenderSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        debug_assert!(
            session.floating_containers().is_empty(),
            "a detached payload projection must not retain source floating wrappers"
        );
        let viewport_host_scene_frame =
            Rc::new(RefCell::new(DockViewportHostSceneCandidateState::default()));
        let selector = self.record_debug_selector(
            DockDebugRegion::Host,
            format!("{}:live-payload", session.selector_prefix()),
        );
        let mut root = div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .text_color(session.visual_style().host.foreground)
            .bg(session.visual_style().host.background);
        if let Some(node) = session.root() {
            root =
                root.child(self.render_node(node, session, &viewport_host_scene_frame, window, cx));
        }

        let Some(state) = self.live_presentation_state() else {
            return root
                .with_subtree_presentation(SubtreePresentation::Inert)
                .into_any_element();
        };
        let Some(owner) = self.surface_owner_entity().map(|owner| owner.downgrade()) else {
            return root
                .with_subtree_presentation(SubtreePresentation::Inert)
                .into_any_element();
        };
        let host = cx.entity().downgrade();
        match state.mode {
            DockHostLivePresentationMode::SourceProjection {
                lease,
                prepared,
                retained,
                carrier,
                phase,
            } => {
                let (proxy, observer) = match phase {
                    DockHostLiveSourcePhase::Releasing => {
                        let observe_owner = owner.clone();
                        let observe_host = host.clone();
                        let observe_prepared = prepared.clone();
                        let barrier =
                            view_presentation_window::retained_visual_source_release_barrier(
                                prepared,
                                &retained,
                                move |attempt| {
                                    canvas(
                                        |_, _, _| (),
                                        move |_, _, window, cx| {
                                            let replay = retained_visual::replay(window, &retained);
                                            let _ = replay.is_ok_and(|receipt| {
                                                view_presentation_window::retained_visual_source_proxy_replay_succeeded(
                                                    &attempt,
                                                    receipt,
                                                    window,
                                                )
                                                .is_ok()
                                            });
                                            let _ = cx;
                                        },
                                    )
                                    .size_full()
                                },
                            );
                        let observer = canvas(
                            move |_, window, _| {
                                window.record_prepaint_focus_stable_commit(
                                    move |frame, window, cx| {
                                        observe_live_source_proxy_commit(
                                            observe_owner.clone(),
                                            observe_host.clone(),
                                            state.key,
                                            lease,
                                            observe_prepared.clone(),
                                            frame,
                                            window,
                                            cx,
                                        );
                                    },
                                );
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full()
                        .into_any_element();
                        (Some(barrier.into_any_element()), Some(observer))
                    }
                    DockHostLiveSourcePhase::Frozen => {
                        let paint_owner = owner.clone();
                        let paint_host = host.clone();
                        let replay_succeeded = Rc::new(Cell::new(None));
                        let observe_replay_succeeded = replay_succeeded.clone();
                        let paint_replay_succeeded = replay_succeeded;
                        (
                            Some(
                                canvas(
                                    move |_, window, _| {
                                        let paint_owner = paint_owner.clone();
                                        let paint_host = paint_host.clone();
                                        let replay_succeeded = observe_replay_succeeded.clone();
                                        window.record_prepaint_focus_stable_commit(
                                            move |_, _, cx| {
                                                if replay_succeeded.take() != Some(true) {
                                                    submit_live_presentation_failure(
                                                        &paint_owner,
                                                        &paint_host,
                                                        state.key,
                                                        DockLiveUndockPresentationFailure::SourceProxyReplay {
                                                            lease,
                                                        },
                                                        cx,
                                                    );
                                                }
                                            },
                                        );
                                    },
                                    move |_, _, window, _| {
                                        paint_replay_succeeded
                                            .set(Some(retained_visual::replay(window, &retained).is_ok()));
                                    },
                                )
                                .size_full()
                                .into_any_element(),
                            ),
                            None,
                        )
                    }
                    DockHostLiveSourcePhase::Retired => (None, None),
                };
                if let Some(proxy) = proxy {
                    let mut overlay = div()
                        .id(format!(
                            "dock-live-source-proxy:{}:{}",
                            state.key.identity().opening().generation(),
                            state.key.identity().drag_generation().get()
                        ))
                        .relative()
                        .absolute()
                        .left(carrier.bounds.origin.x)
                        .top(carrier.bounds.origin.y)
                        .w(carrier.bounds.size.width)
                        .h(carrier.bounds.size.height)
                        .occlude()
                        .child(proxy);
                    if let Some(observer) = observer {
                        overlay = overlay.child(observer);
                    }
                    root = root.child(overlay);
                }
            }
            DockHostLivePresentationMode::DestinationProjection {
                proxy,
                prepared,
                leases,
                phase,
                ..
            } => {
                if let Some(work_context) = self.live_destination_runtime_work_context(cx) {
                    root =
                        root.child(self.render_live_destination_geometry_probe(work_context, cx));
                }
                let observer = match phase {
                    DockHostLiveDestinationPhase::Staging => {
                        let next_owner = owner.clone();
                        let next_host = host.clone();
                        Some(
                            canvas(
                                move |_, window, _| {
                                    let next_owner = next_owner.clone();
                                    let next_host = next_host.clone();
                                    let prepared = prepared.clone();
                                    window.record_prepaint_focus_stable_commit(
                                        move |frame, _, cx| {
                                            observe_live_destination_mount(
                                                next_owner.clone(),
                                                next_host.clone(),
                                                state.key,
                                                proxy,
                                                prepared.clone(),
                                                frame,
                                                cx,
                                            );
                                        },
                                    );
                                },
                                |_, _, _, _| {},
                            )
                            .absolute()
                            .size_full()
                            .into_any_element(),
                        )
                    }
                    DockHostLiveDestinationPhase::Exposed(mount) => {
                        let next_owner = owner.clone();
                        let next_host = host.clone();
                        Some(
                            canvas(
                                move |_, window, _| {
                                    let next_owner = next_owner.clone();
                                    let next_host = next_host.clone();
                                    let leases = leases.clone();
                                    window.record_prepaint_focus_stable_commit(
                                        move |frame, window, cx| {
                                            observe_live_destination_presentation(
                                                next_owner.clone(),
                                                next_host.clone(),
                                                state.key,
                                                mount,
                                                leases.clone(),
                                                frame,
                                                window,
                                                cx,
                                            );
                                        },
                                    );
                                },
                                |_, _, _, _| {},
                            )
                            .absolute()
                            .size_full()
                            .into_any_element(),
                        )
                    }
                    DockHostLiveDestinationPhase::RevealArmed { presentation, .. } => {
                        let next_owner = owner.clone();
                        let next_host = host.clone();
                        Some(
                            canvas(
                                move |_, window, _| {
                                    let next_owner = next_owner.clone();
                                    let next_host = next_host.clone();
                                    let leases = leases.clone();
                                    window.record_prepaint_focus_stable_commit(
                                        move |frame, window, cx| {
                                            capture_live_destination_reveal_frame(
                                                next_owner.clone(),
                                                next_host.clone(),
                                                state.key,
                                                presentation,
                                                leases.clone(),
                                                frame,
                                                window,
                                                cx,
                                            );
                                        },
                                    );
                                },
                                |_, _, _, _| {},
                            )
                            .absolute()
                            .size_full()
                            .into_any_element(),
                        )
                    }
                    DockHostLiveDestinationPhase::Presented(_)
                    | DockHostLiveDestinationPhase::RevealObserving { .. }
                    | DockHostLiveDestinationPhase::RevealSettled => None,
                };
                if let Some(observer) = observer {
                    root = root.child(observer);
                }
            }
            DockHostLivePresentationMode::SourceRestoration { .. } => {
                debug_assert!(
                    false,
                    "source restoration must render through its workspace presentation session"
                );
            }
        }
        let visual = root
            .with_subtree_presentation(SubtreePresentation::Inert)
            .into_any_element();
        self.wrap_transport_and_semantic_proxies(visual, window)
    }

    fn drop_preview_payload_tab_layout(
        &self,
        session: &DockHostRenderSession,
        preview_bounds: Bounds<Pixels>,
        affordance_scene: &DockVisualAffordanceScene,
        window: &Window,
    ) -> Option<DockPayloadTabPreviewLayout> {
        let insertion = affordance_scene.tab_insertion()?;
        let target_tabs = insertion.target_node?;
        let DockNode::Tabs { items, .. } = session.node(target_tabs)?.clone() else {
            return None;
        };
        let payload_tabs = affordance_payload_tabs(affordance_scene);
        if payload_tabs.is_empty() {
            return None;
        }
        let tab_height = px(f32::from(preview_bounds.size.height)
            .min(DROP_PREVIEW_TAB_HEIGHT)
            .max(0.0));
        if tab_height <= px(0.0) {
            return None;
        }

        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let tab_gap = px(DROP_PREVIEW_TAB_GAP);
        let insert_index = insertion
            .tab_insertion
            .as_ref()
            .map(|insertion| match insertion.index {
                DockPreviewTabInsertionIndex::At(index) => index,
                DockPreviewTabInsertionIndex::Append => items.len(),
            })
            .unwrap_or(items.len())
            .min(items.len());
        let slot_insertion_x = insertion
            .tab_insertion
            .as_ref()
            .and_then(|insertion| insertion.slot_bounds)
            .map(|bounds| bounds.center().x);
        let mut tab_left = self
            .viewport_runtime()
            .rendered_tab_bar_bounds_for_tabs(self.space(), None, target_tabs)
            .map(|tab_bar_bounds| tab_bar_bounds.origin.x)
            .unwrap_or(preview_bounds.origin.x);

        let existing_tab_widths = items
            .iter()
            .map(|item| {
                let title = session.panel_title(item);
                let title_line = window.text_system().shape_line(
                    SharedString::from(title.clone()),
                    font_size,
                    &[text_style.to_run(title.len())],
                    None,
                );
                preview_tab_width(title_line.width())
            })
            .collect::<Vec<_>>();
        tab_left = slot_insertion_x.unwrap_or_else(|| {
            stable_tab_preview_insert_left(tab_left, insert_index, &existing_tab_widths)
        });

        let mut tab_widths = Vec::with_capacity(payload_tabs.len());
        for payload_tab in &payload_tabs {
            let payload_title = payload_tab.title.as_str();
            let payload_line = window.text_system().shape_line(
                SharedString::from(payload_title.to_string()),
                font_size,
                &[text_style.to_run(payload_title.len())],
                None,
            );
            tab_widths.push(f32::from(preview_tab_width(payload_line.width())));
        }

        let tab_strip_left = f32::from(preview_bounds.origin.x);
        let tab_strip_right = f32::from(preview_bounds.origin.x + preview_bounds.size.width);
        let tab_gap = f32::from(tab_gap);
        let requested_left = f32::from(tab_left).max(tab_strip_left);
        let mut visible_count = tab_widths.len();
        while visible_count > 0 {
            let total_gap = tab_gap * visible_count.saturating_sub(1) as f32;
            if tab_strip_right - tab_strip_left
                >= (DROP_PREVIEW_TAB_MIN_VISIBLE_WIDTH * visible_count as f32) + total_gap
            {
                break;
            }
            visible_count -= 1;
        }
        if visible_count == 0 {
            return None;
        }
        tab_widths.truncate(visible_count);
        let total_gap = tab_gap * visible_count.saturating_sub(1) as f32;
        let available_width =
            (tab_strip_right - requested_left).max(tab_strip_right - tab_strip_left);
        let max_total_tab_width =
            (available_width - total_gap).max(DROP_PREVIEW_TAB_MIN_VISIBLE_WIDTH);
        let requested_total_tab_width: f32 = tab_widths.iter().sum();
        if requested_total_tab_width > max_total_tab_width {
            let compressed_width = (max_total_tab_width / visible_count as f32)
                .max(DROP_PREVIEW_TAB_MIN_VISIBLE_WIDTH);
            tab_widths.fill(compressed_width);
        }
        let tab_strip_width = tab_widths.iter().sum::<f32>() + total_gap;
        let mut tab_left =
            requested_left.min((tab_strip_right - tab_strip_width).max(tab_strip_left));
        let mut tab_bounds = Vec::with_capacity(payload_tabs.len());
        for (payload_tab, tab_width) in payload_tabs.iter().zip(tab_widths) {
            tab_bounds.push(DockPayloadTabPreviewPlacement {
                payload_index: payload_tab.index,
                bounds: Bounds::new(
                    point(px(tab_left), preview_bounds.origin.y),
                    open_gpui::size(px(tab_width), tab_height),
                ),
            });
            tab_left += tab_width + tab_gap;
        }

        let first_tab_bounds = tab_bounds.first()?.bounds;
        let insertion_width = px(3.0);
        let insertion_x = slot_insertion_x
            .unwrap_or_else(|| stable_tab_preview_insertion_x(first_tab_bounds.origin.x));
        let insertion_bounds = Bounds::new(
            point(
                insertion_x - insertion_width / 2.0,
                first_tab_bounds.origin.y,
            ),
            open_gpui::size(insertion_width, first_tab_bounds.size.height),
        );

        let body_origin_y = first_tab_bounds.origin.y + first_tab_bounds.size.height;
        let body_height =
            (preview_bounds.origin.y + preview_bounds.size.height - body_origin_y).max(px(0.0));
        let body_bounds = Bounds::new(
            point(preview_bounds.origin.x, body_origin_y),
            open_gpui::size(preview_bounds.size.width, body_height),
        );

        Some(DockPayloadTabPreviewLayout {
            body_bounds,
            insertion_bounds,
            payload_tabs: tab_bounds,
        })
    }

    pub(crate) fn render_node(
        &mut self,
        node_id: DockNodeId,
        session: &DockHostRenderSession,
        viewport_host_scene_frame: &DockViewportHostSceneCandidateSlot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(node) = session.node(node_id).cloned() else {
            return self.render_missing_node(node_id, session);
        };

        match node {
            DockNode::Split {
                axis,
                children,
                fractions,
            } => self.render_split(
                DockRenderSplitInput::new(node_id, axis, children, fractions),
                session,
                viewport_host_scene_frame,
                window,
                cx,
            ),
            DockNode::Tabs { items, selected } => {
                let Some(selected) = selected_index(&items, &selected) else {
                    return self.render_missing_node(node_id, session);
                };
                self.render_tabs(
                    node_id,
                    items,
                    selected,
                    session,
                    viewport_host_scene_frame,
                    window,
                    cx,
                )
            }
            DockNode::Floating { child } => self.render_floating_node(
                node_id,
                child,
                session,
                viewport_host_scene_frame,
                window,
                cx,
            ),
        }
    }

    fn render_root_node(
        &mut self,
        root: DockNodeId,
        session: &DockHostRenderSession,
        viewport_host_scene_frame: &DockViewportHostSceneCandidateSlot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rendered_root = self
            .zoom_state()
            .target(session.space())
            .filter(|target| session.node(*target).is_some())
            .unwrap_or(root);
        let root_child = self.render_node(
            rendered_root,
            session,
            viewport_host_scene_frame,
            window,
            cx,
        );
        let mut root_container = div()
            .relative()
            .flex()
            .size_full()
            .overflow_hidden()
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DockDragPayload>, window, cx| {
                    let payload = event.drag().clone();
                    let Ok(layout_position) = event.target_layout_position() else {
                        return;
                    };
                    this.update_local_root_drop_scene_from_render(
                        &payload,
                        root,
                        event.layout_bounds(),
                        DockRenderedPointerPosition::new(layout_position, event.window_position()),
                        window,
                        cx,
                    );
                },
            ));
        root_container = root_container.child(root_child);
        root_container.into_any_element()
    }

    fn render_divider_event_layer(
        &self,
        session: &DockHostRenderSession,
        pointer_capture: PointerCaptureHandle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entity = cx.entity();
        let window_binding = self.current_window_binding();
        let session = session.clone();
        let prepaint_entity = entity.clone();
        let prepaint_session = session.clone();

        canvas(
            move |bounds, window, app| {
                let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                window
                    .bind_pointer_capture(&pointer_capture, hitbox.id)
                    .expect("DockHost pointer capture must bind to its interaction layer");
                let scene = prepaint_entity.update(app, |host, _| {
                    host.resolved_render_presentation_scene(
                        &prepaint_session,
                        hitbox.layout_bounds(),
                    )
                });
                let hit_map = DockDividerHitMap::from_scene(&scene);
                (hitbox, scene, hit_map)
            },
            move |_, (hitbox, scene, hit_map), window, app| {
                let hover_position = (hitbox.is_active() && hitbox.is_hovered(window))
                    .then(|| hitbox.window_to_layout_point(window.mouse_position()).ok())
                    .flatten();
                let corner_dragging = entity.read(app).interaction().corner_splitter_drag_active();
                let corner_affordances =
                    hit_map.corner_affordances(hover_position, corner_dragging, true);

                if let Some(target) = hover_position.and_then(|position| hit_map.hit(position)) {
                    window.set_window_cursor_style(cursor_for_divider_target(target));
                }
                for affordance in &corner_affordances {
                    window.paint_quad(quad(
                        affordance.corner.bounds,
                        px(3.0),
                        background_for_divider_affordance_state(
                            affordance.state,
                            &session.visual_style().splitters,
                        ),
                        px(1.0),
                        session.visual_style().splitters.corner_border,
                        BorderStyle::Solid,
                    ));
                }

                window.on_mouse_event({
                    let entity = entity.clone();
                    let scene = scene.clone();
                    let hit_map = hit_map.clone();
                    let hitbox = hitbox.clone();
                    let window_binding = window_binding;
                    move |event: &MouseDownEvent, phase, window, app| {
                        if phase != DispatchPhase::Bubble || event.button != MouseButton::Left {
                            return;
                        }
                        if !hitbox.is_active()
                            || !hitbox.is_mouse_event_target(window)
                            || !hitbox.contains_window_point(event.position)
                        {
                            return;
                        }
                        let Ok(layout_position) = hitbox.window_to_layout_point(event.position)
                        else {
                            return;
                        };
                        let Some(target) = hit_map.hit(layout_position).cloned() else {
                            return;
                        };
                        let captured = window
                            .capture_pointer(&pointer_capture, MouseButton::Left)
                            .is_ok();
                        let began = entity.update(app, |host, cx| {
                            if !host.accepts_window_callback(
                                window_binding,
                                window.window_handle().window_id(),
                            ) {
                                return false;
                            }
                            host.begin_divider_drag_from_scene(&scene, &target, layout_position, cx)
                        });
                        if !began {
                            if captured {
                                let _ = window.release_pointer(&pointer_capture);
                            }
                            return;
                        }
                        app.stop_propagation();
                    }
                });

                window.on_mouse_event({
                    let entity = entity.clone();
                    let hitbox = hitbox.clone();
                    let window_binding = window_binding;
                    move |event: &MouseMoveEvent, phase, window, app| {
                        if phase != DispatchPhase::Capture
                            || event.pressed_button != Some(MouseButton::Left)
                        {
                            return;
                        }
                        let Ok(layout_position) = hitbox.window_to_layout_point(event.position)
                        else {
                            return;
                        };
                        entity.update(app, |host, cx| {
                            if !host.accepts_window_callback(
                                window_binding,
                                window.window_handle().window_id(),
                            ) {
                                return;
                            }
                            host.update_splitter_drag_from_render(layout_position, cx);
                        });
                    }
                });

                window.on_mouse_event({
                    let window_binding = window_binding;
                    move |event: &MouseUpEvent, phase, window, app| {
                        if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                            return;
                        }
                        entity.update(app, |host, cx| {
                            if !host.accepts_window_callback(
                                window_binding,
                                window.window_handle().window_id(),
                            ) {
                                return;
                            }
                            host.finish_splitter_drag_from_render(cx);
                        });
                    }
                });
            },
        )
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .size_full()
        .into_any_element()
    }

    fn resolved_render_presentation_scene(
        &mut self,
        session: &crate::host_render_session::DockHostPresentationSession,
        bounds: Bounds<Pixels>,
    ) -> DockPresentationScene {
        let base = DockPresentationScene::from_presentation_session(session, bounds);
        let space = session.space().clone();
        self.zoom_state_mut().clear_missing_target(&space, &base);
        self.zoom_state()
            .resolve(&base, session.motion_preference())
            .map(|zoom| zoom.scene)
            .unwrap_or(base)
    }

    #[cfg(test)]
    pub(crate) fn divider_event_scene_for_test(
        &mut self,
        bounds: Bounds<Pixels>,
        cx: &Context<Self>,
    ) -> DockPresentationScene {
        let session = self.presentation_session(cx);
        self.resolved_render_presentation_scene(&session, bounds)
    }

    fn render_transition_sample_layer(
        &mut self,
        session: &DockHostRenderSession,
        viewport_host_scene_frame: &DockViewportHostSceneCandidateSlot,
        sample: &crate::transition_executor::DockTransitionSample,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::TransitionLayer,
            format!("{}:transition-layer", session.selector_prefix()),
        );
        let mut layer = div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .size_full()
            .overflow_hidden();

        for clip in &sample.pane_clips {
            layer = layer.child(self.render_transition_pane_occlusion(session, clip));
        }
        for clip in &sample.pane_clips {
            layer = layer.child(self.render_transition_pane_clip(
                session,
                viewport_host_scene_frame,
                clip,
                window,
                cx,
            ));
        }
        for divider in &sample.dividers {
            layer = layer.child(self.render_transition_divider(session, divider));
        }
        for (index, affordance) in sample.visual_affordances.iter().enumerate() {
            layer =
                layer.child(self.render_transition_visual_affordance(session, index, affordance));
        }

        layer.into_any_element()
    }

    fn render_transition_pane_occlusion(
        &mut self,
        session: &DockHostRenderSession,
        clip: &DockPaneClipSample,
    ) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::TransitionPaneOcclusion { node: clip.node },
            format!(
                "{}:transition:pane-occlusion:{}",
                session.selector_prefix(),
                clip.node.as_u64()
            ),
        );
        let background = if session.empty_central_passthrough() {
            rgba(0x00000000)
        } else {
            session.visual_style().host.transition_occlusion
        };
        div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .absolute()
            .left(clip.occlusion_bounds.origin.x)
            .top(clip.occlusion_bounds.origin.y)
            .w(clip.occlusion_bounds.size.width)
            .h(clip.occlusion_bounds.size.height)
            .bg(background)
            .into_any_element()
    }

    fn render_transition_pane_clip(
        &mut self,
        session: &DockHostRenderSession,
        viewport_host_scene_frame: &DockViewportHostSceneCandidateSlot,
        clip: &DockPaneClipSample,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::TransitionPaneClip { node: clip.node },
            format!(
                "{}:transition:pane-clip:{}",
                session.selector_prefix(),
                clip.node.as_u64()
            ),
        );
        let content_offset = point(
            clip.content_bounds.origin.x - clip.visible_bounds.origin.x,
            clip.content_bounds.origin.y - clip.visible_bounds.origin.y,
        );
        let content_selector = self.record_debug_selector(
            DockDebugRegion::TransitionPaneContent { node: clip.node },
            format!(
                "{}:transition:pane-content:{}",
                session.selector_prefix(),
                clip.node.as_u64()
            ),
        );
        let content = self.with_debug_selector_recording_suppressed(|host| {
            host.render_node(clip.node, session, viewport_host_scene_frame, window, cx)
        });
        div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .absolute()
            .left(clip.visible_bounds.origin.x)
            .top(clip.visible_bounds.origin.y)
            .w(clip.visible_bounds.size.width)
            .h(clip.visible_bounds.size.height)
            .overflow_hidden()
            .child(
                div()
                    .id(content_selector.clone())
                    .debug_selector(move || content_selector)
                    .absolute()
                    .left(content_offset.x)
                    .top(content_offset.y)
                    .w(clip.content_bounds.size.width)
                    .h(clip.content_bounds.size.height)
                    .child(content),
            )
            .into_any_element()
    }

    fn render_transition_divider(
        &mut self,
        session: &DockHostRenderSession,
        divider: &DockDividerSample,
    ) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::TransitionDivider {
                split: divider.split,
                index: divider.index,
            },
            format!(
                "{}:transition:divider:{}:{}",
                session.selector_prefix(),
                divider.split.as_u64(),
                divider.index
            ),
        );
        div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .absolute()
            .left(divider.bounds.origin.x)
            .top(divider.bounds.origin.y)
            .w(divider.bounds.size.width)
            .h(divider.bounds.size.height)
            .rounded_sm()
            .bg(session.visual_style().previews.transition_divider)
            .into_any_element()
    }

    fn render_transition_visual_affordance(
        &mut self,
        session: &DockHostRenderSession,
        index: usize,
        affordance: &DockVisualAffordanceSample,
    ) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::TransitionVisualAffordance { index },
            format!(
                "{}:transition:visual-affordance:{index}",
                session.selector_prefix()
            ),
        );
        div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .absolute()
            .left(affordance.bounds.origin.x)
            .top(affordance.bounds.origin.y)
            .w(affordance.bounds.size.width)
            .h(affordance.bounds.size.height)
            .rounded_sm()
            .border_1()
            .border_color(session.visual_style().previews.transition_affordance_border)
            .bg(session
                .visual_style()
                .previews
                .transition_affordance_background)
            .into_any_element()
    }

    fn render_empty_space(
        &mut self,
        session: &DockHostRenderSession,
        _viewport_host_scene_frame: &DockViewportHostSceneCandidateSlot,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::EmptySpace,
            format!("{}:empty", session.selector_prefix()),
        );
        let mut empty = div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .relative()
            .flex()
            .size_full()
            .items_center()
            .justify_center()
            .border_1()
            .border_color(session.visual_style().host.empty_border)
            .text_color(session.visual_style().host.empty_text)
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DockDragPayload>, window, cx| {
                    let payload = event.drag().clone();
                    let Ok(layout_position) = event.target_layout_position() else {
                        return;
                    };
                    this.update_local_empty_space_drop_scene_from_render(
                        &payload,
                        DockRenderedPointerPosition::new(layout_position, event.window_position()),
                        event.layout_bounds(),
                        false,
                        window,
                        cx,
                    );
                },
            ));
        empty = empty.child(session.empty_message().to_string());
        empty.into_any_element()
    }

    fn render_passthrough_empty_central_space(
        &mut self,
        session: &DockHostRenderSession,
        _viewport_host_scene_frame: &DockViewportHostSceneCandidateSlot,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::EmptySpace,
            format!("{}:empty-central", session.selector_prefix()),
        );
        let empty = div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .relative()
            .flex()
            .size_full()
            .bg(rgba(0x00000000))
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DockDragPayload>, window, cx| {
                    let payload = event.drag().clone();
                    let Ok(layout_position) = event.target_layout_position() else {
                        return;
                    };
                    this.update_local_empty_space_drop_scene_from_render(
                        &payload,
                        DockRenderedPointerPosition::new(layout_position, event.window_position()),
                        event.layout_bounds(),
                        true,
                        window,
                        cx,
                    );
                },
            ));
        empty.into_any_element()
    }

    pub(crate) fn render_missing_node(
        &mut self,
        node: DockNodeId,
        session: &DockHostRenderSession,
    ) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::MissingNode { node },
            format!(
                "{}:missing-node:{}",
                session.selector_prefix(),
                node.as_u64()
            ),
        );
        div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .flex()
            .size_full()
            .items_center()
            .justify_center()
            .border_1()
            .border_color(session.visual_style().host.missing_border)
            .text_color(session.visual_style().host.missing_text)
            .child(format!("Missing dock node: {}", node.as_u64()))
            .into_any_element()
    }

    fn render_host_drop_preview(
        &mut self,
        session: &DockHostRenderSession,
        window: &mut Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let active_payload = cx.active_drag_value::<DockDragPayload>().cloned();
        let routed_preview = self
            .viewport_runtime()
            .routed_drop_preview_for(self.space(), window.window_handle().window_id());
        let local_preview = self.interaction().drop_preview();
        let route_preview = self
            .viewport_runtime()
            .routed_drop_route_preview_for(self.space(), window.window_handle().window_id());
        if let Some(mut preview) = local_preview {
            if let Some(payload) = active_payload.as_ref() {
                preview.populate_payload_tabs(payload);
            }
            return Some(self.render_target_drop_preview(session, preview, window));
        }

        if let Some(routed_preview) = routed_preview {
            return Some(self.render_target_drop_preview(session, routed_preview.preview, window));
        }

        if let Some(preview) = route_preview {
            return Some(self.render_route_drop_preview(session, preview, window));
        }

        if self.clear_visual_affordance_transition_for_render() {
            self.clear_visual_affordance_debug_summary(window.window_handle().window_id());
        }
        None
    }

    fn render_target_drop_preview(
        &mut self,
        session: &DockHostRenderSession,
        preview: DockDropPreview,
        window: &Window,
    ) -> AnyElement {
        let scene = &preview.scene;
        let mut affordance_scene = DockVisualAffordanceScene::from_preview(scene);
        let bounds = scene
            .payload_tabs
            .as_ref()
            .and_then(|payload_tabs| payload_tabs.target_tabs)
            .and_then(|tabs| {
                self.viewport_runtime()
                    .rendered_leaf_bounds_for_tabs(self.space(), None, tabs)
            })
            .unwrap_or(scene.body.future_bounds);
        let payload_tab_layout = if affordance_scene.has_payload_tab_preview() {
            self.drop_preview_payload_tab_layout(session, bounds, &affordance_scene, window)
        } else {
            None
        };
        if let Some(layout) = payload_tab_layout.as_ref() {
            affordance_scene.apply_payload_tab_layout(layout);
        }
        let visual_affordance_sample = self.sync_visual_affordance_transition_for_render(
            session,
            &affordance_scene,
            bounds,
            window,
        );
        let affordance_opacity = visual_affordance_sample
            .as_ref()
            .map(|sample| preview_transition_opacity(sample.progress))
            .unwrap_or(1.0);
        let selector = self.record_debug_selector(
            DockDebugRegion::DropPreview,
            format!("{}:drop-preview", session.selector_prefix()),
        );
        let palette = session
            .visual_style()
            .previews
            .target(target_preview_visual_state(&scene.decision));
        let mut element = div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .absolute()
            .left(bounds.origin.x)
            .top(bounds.origin.y)
            .w(bounds.size.width)
            .h(bounds.size.height)
            .overflow_hidden()
            .opacity(affordance_opacity);

        if affordance_scene.has_payload_tab_preview() && payload_tab_layout.is_some() {
            let body_layer = affordance_scene.target_body();
            let insertion_layer = affordance_scene.tab_insertion();
            let Some(body_layer) = body_layer else {
                return element.into_any_element();
            };
            let Some(insertion_layer) = insertion_layer else {
                return element.into_any_element();
            };
            let body_selector = self.record_debug_selector(
                DockDebugRegion::DropPreviewBody,
                format!("{}:drop-preview:body", session.selector_prefix()),
            );
            let mut body = div()
                .id(body_selector.clone())
                .debug_selector(move || body_selector)
                .absolute()
                .left(body_layer.bounds.origin.x - bounds.origin.x)
                .top(body_layer.bounds.origin.y - bounds.origin.y)
                .w(body_layer.bounds.size.width)
                .h(body_layer.bounds.size.height)
                .border_1()
                .border_color(palette.border)
                .bg(palette.body_background);
            if body_layer.bounds.size.height > px(0.0) {
                body = body.rounded_b_sm().border_t_0();
            }
            element = element.child(body);
            let insertion_selector = self.record_debug_selector(
                DockDebugRegion::DropTabInsertionPreview,
                format!("{}:drop-preview:tab-insertion", session.selector_prefix()),
            );
            element = element.child(
                div()
                    .id(insertion_selector.clone())
                    .debug_selector(move || insertion_selector)
                    .absolute()
                    .left(insertion_layer.bounds.origin.x - bounds.origin.x)
                    .top(insertion_layer.bounds.origin.y - bounds.origin.y)
                    .w(insertion_layer.bounds.size.width)
                    .h(insertion_layer.bounds.size.height)
                    .rounded_sm()
                    .bg(palette.border),
            );
            for placement in affordance_payload_tabs(&affordance_scene) {
                let placement_bounds = affordance_scene
                    .payload_tabs()
                    .find(|layer| layer.payload_index == Some(placement.index))
                    .map(|layer| layer.bounds)
                    .unwrap_or(insertion_layer.bounds);
                let tab_selector = self.record_debug_selector(
                    DockDebugRegion::DropPayloadTabPreview {
                        index: placement.index,
                    },
                    format!(
                        "{}:drop-preview:payload-tab:{}",
                        session.selector_prefix(),
                        placement.index
                    ),
                );
                element = element.child(
                    div()
                        .id(tab_selector.clone())
                        .debug_selector(move || tab_selector)
                        .absolute()
                        .left(placement_bounds.origin.x - bounds.origin.x)
                        .top(placement_bounds.origin.y - bounds.origin.y)
                        .flex()
                        .items_center()
                        .justify_start()
                        .h(placement_bounds.size.height)
                        .w(placement_bounds.size.width)
                        .px_2()
                        .border_1()
                        .border_color(palette.border)
                        .bg(palette.tab_background)
                        .text_color(palette.tab_text)
                        .text_sm()
                        .shadow(session.visual_style().previews.payload_tab_shadow.clone())
                        .truncate()
                        .rounded_t_sm()
                        .rounded_br_sm()
                        .border_b_0()
                        .child(placement.title),
                );
            }
        } else if scene.body.body_bounds.size.width > px(0.0)
            && scene.body.body_bounds.size.height > px(0.0)
        {
            let body_selector = self.record_debug_selector(
                DockDebugRegion::DropPreviewBody,
                format!("{}:drop-preview:body", session.selector_prefix()),
            );
            let body_bounds = localize_bounds(scene.body.body_bounds, bounds.origin);
            element = element.child(
                div()
                    .id(body_selector.clone())
                    .debug_selector(move || body_selector)
                    .absolute()
                    .left(body_bounds.origin.x)
                    .top(body_bounds.origin.y)
                    .w(body_bounds.size.width)
                    .h(body_bounds.size.height)
                    .border_1()
                    .border_color(palette.border)
                    .bg(palette.body_background),
            );
        }

        for drop_box in affordance_scene.guide_drop_boxes() {
            element = element.child(self.render_scene_drop_guide(session, bounds, drop_box));
        }

        for accessible in
            DockAccessibilityScene::visual_affordance_elements_for_render(&affordance_scene)
        {
            let local_bounds = localize_bounds(accessible.bounds, bounds.origin);
            let marker = div()
                .id(accessible.id_str().to_string())
                .absolute()
                .left(local_bounds.origin.x)
                .top(local_bounds.origin.y)
                .w(local_bounds.size.width)
                .h(local_bounds.size.height)
                .bg(rgba(0x00000000));
            element = element.child(accessible.apply_to(marker));
        }

        element.into_any_element()
    }

    fn render_route_drop_preview(
        &mut self,
        session: &DockHostRenderSession,
        preview: DockDropRoutePreview,
        window: &Window,
    ) -> AnyElement {
        let affordance_scene = DockVisualAffordanceScene::from_route_preview(&preview);
        let bounds = affordance_scene
            .layers
            .first()
            .map(|layer| layer.bounds)
            .unwrap_or(preview.bounds);
        let visual_affordance_sample = self.sync_visual_affordance_transition_for_render(
            session,
            &affordance_scene,
            bounds,
            window,
        );
        let affordance_opacity = visual_affordance_sample
            .as_ref()
            .map(|sample| preview_transition_opacity(sample.progress))
            .unwrap_or(1.0);
        let selector = self.record_debug_selector(
            DockDebugRegion::DropRoutePreview { kind: preview.kind },
            format!("{}:drop-route-preview", session.selector_prefix()),
        );
        let palette = session
            .visual_style()
            .previews
            .route(route_preview_visual_state(&preview));

        div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .absolute()
            .left(bounds.origin.x)
            .top(bounds.origin.y)
            .w(bounds.size.width)
            .h(bounds.size.height)
            .border_1()
            .border_color(palette.border)
            .bg(palette.background)
            .opacity(affordance_opacity)
            .into_any_element()
    }

    fn sync_visual_affordance_transition_for_render(
        &mut self,
        session: &crate::host_render_session::DockHostPresentationSession,
        affordance_scene: &DockVisualAffordanceScene,
        fallback_bounds: Bounds<Pixels>,
        window: &Window,
    ) -> Option<DockTransitionSample> {
        if self.last_visual_affordance_scene() != Some(affordance_scene) {
            let final_scene = self.last_presentation_scene().cloned().unwrap_or_else(|| {
                DockPresentationScene::from_presentation_session(session, fallback_bounds)
            });
            let plan = DockTransitionPlan::from_visual_affordance_scene(
                &final_scene,
                affordance_scene,
                session.motion_preference(),
            );
            self.set_last_visual_affordance_scene(affordance_scene.clone());
            self.execute_visual_affordance_transition_plan(
                plan,
                MotionTransition::visual_affordance(session.motion_preference()),
            );
        }

        let sample = self.sample_visual_affordance_transition_for_render(Some(window));
        self.publish_visual_affordance_debug_summary(window.window_handle().window_id());
        sample
    }

    #[cfg(test)]
    pub(crate) fn sync_visual_affordance_transition_for_test(
        &mut self,
        session: &crate::host_render_session::DockHostPresentationSession,
        affordance_scene: &DockVisualAffordanceScene,
        fallback_bounds: Bounds<Pixels>,
        window: &Window,
    ) -> Option<DockTransitionSample> {
        self.sync_visual_affordance_transition_for_render(
            session,
            affordance_scene,
            fallback_bounds,
            window,
        )
    }

    fn render_scene_drop_guide(
        &mut self,
        session: &DockHostRenderSession,
        container_bounds: Bounds<Pixels>,
        drop_box: DockPreviewDropBox,
    ) -> AnyElement {
        let node = drop_box.debug_node;
        let zone = drop_box.zone;
        let selector_suffix = drop_box_selector_suffix(drop_box);
        let selector = self.record_debug_selector(
            DockDebugRegion::DropGuide { node, zone },
            format!("{}:drop-guide:{selector_suffix}", session.selector_prefix()),
        );
        let local_bounds = localize_bounds(drop_box.draw_bounds, container_bounds.origin);
        let palette = session
            .visual_style()
            .previews
            .guide(drop_guide_visual_state(drop_box.kind, drop_box.active));
        let cue = guide_directional_cue(zone, local_bounds.size, palette.cue);
        let inset = guide_inset_outline(local_bounds.size, palette.inset);

        let mut guide = div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .absolute()
            .left(local_bounds.origin.x)
            .top(local_bounds.origin.y)
            .w(local_bounds.size.width)
            .h(local_bounds.size.height)
            .flex()
            .items_center()
            .justify_center()
            .border_1()
            .border_color(palette.border)
            .rounded_sm()
            .bg(palette.background);
        if let Some(inset) = inset {
            guide = guide.child(inset);
        }
        if let Some(cue) = cue {
            guide = guide.child(cue);
        }

        guide.into_any_element()
    }

    /// Captures viewport geometry during prepaint and publishes it after a valid paint.
    pub(crate) fn render_viewport_host_scene_probe(
        &self,
        frame_slot: &DockViewportHostSceneCandidateSlot,
        session: &DockHostRenderSession,
        drop_guide_metrics: geometry::DockDropGuideMetrics,
        passthrough_pointer_input: bool,
        work_context: DockViewportRuntimeWorkContext,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entity = cx.entity();
        let runtime = self.viewport_runtime().clone();
        let publication = self.viewport_scene_publication();
        let space = self.space().clone();
        let session = session.clone();
        let frame_slot = frame_slot.clone();
        let host_binding = self
            .current_window_binding()
            .expect("a rendered DockHost must have a window binding");
        let expected_registration = self.current_viewport_registration();
        canvas(
            move |bounds, window, app| {
                frame_slot.borrow_mut().begin_prepaint();
                let window_id = window.window_handle().window_id();
                let prior_published_frame = entity.update(app, |host, _| {
                    host.interaction().viewport_host_scene_frame().cloned()
                });
                record_viewport_host_scene_transaction(
                    window,
                    publication,
                    frame_slot.clone(),
                    runtime.clone(),
                    entity.clone(),
                    space.clone(),
                    window_id,
                    host_binding,
                    prior_published_frame,
                    passthrough_pointer_input,
                );
                let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                if !hitbox.is_active() {
                    return;
                }
                let scene = entity.update(app, |host, _| {
                    host.resolved_render_presentation_scene(&session, hitbox.layout_bounds())
                });
                let mouse_position = window.mouse_position();
                let Ok(host_position) = hitbox.window_to_local_point(mouse_position) else {
                    return;
                };
                let window_facts = crate::DockViewportWindowFacts::from_window(window, app);
                let draft = DockViewportHostSceneDraft::new_with_facts(
                    space.clone(),
                    window.window_handle().window_id(),
                    window_facts.current_bounds,
                    DockViewportHostGeometry::from_hitbox(&hitbox),
                    host_position,
                    drop_guide_metrics,
                    drop_scene_fact::presentation_scene_drop_facts(&scene, &session),
                );
                frame_slot
                    .borrow_mut()
                    .set_pending(DockViewportHostSceneCandidate {
                        draft,
                        host_binding,
                        expected_registration: expected_registration.clone(),
                        work_context,
                        presentation_scene: scene,
                    });
            },
            |_, _, _, _| (),
        )
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .size_full()
        .into_any_element()
    }

    /// Publishes the host's routing hitbox after every Dock-owned descendant.
    ///
    /// Floating containers intentionally occlude content below them. A host hitbox inserted before
    /// those descendants would therefore disappear from the committed target set over its own
    /// floating chrome. This eventless sentinel remains behind later application overlays while
    /// keeping all descendants of this DockHost inside the same native routing surface.
    fn render_viewport_host_scene_routing_sentinel(
        &self,
        frame_slot: &DockViewportHostSceneCandidateSlot,
    ) -> AnyElement {
        let frame_slot = frame_slot.clone();
        canvas(
            move |bounds, window, _| {
                let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                if !hitbox.is_active() {
                    return;
                }
                let mut frame_slot = frame_slot.borrow_mut();
                let Some(candidate) = frame_slot.pending_mut() else {
                    return;
                };
                candidate.draft.host_geometry = DockViewportHostGeometry::from_hitbox(&hitbox);
            },
            |_, _, _, _| (),
        )
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .size_full()
        .into_any_element()
    }

    /// Publishes render-measured tab-label bounds whose size depends on text shaping.
    pub(crate) fn render_tab_label_drop_scene_fact_probe(
        &self,
        frame_slot: &DockViewportHostSceneCandidateSlot,
        tabs: DockNodeId,
        target_index: usize,
        is_central: bool,
        _cx: &mut Context<Self>,
    ) -> AnyElement {
        let frame_slot = frame_slot.clone();
        canvas(
            move |bounds, _, _| {
                let mut candidate_slot = frame_slot.borrow_mut();
                let Some(candidate) = candidate_slot.pending_mut() else {
                    return;
                };
                let fact = drop_scene_fact::tab_label(tabs, target_index, bounds, is_central);
                candidate.draft.push_fact(fact);
            },
            |_, _, _, _| (),
        )
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .size_full()
        .into_any_element()
    }
}

fn affordance_payload_tabs(
    affordance_scene: &DockVisualAffordanceScene,
) -> Vec<DockDropPreviewPayloadTab> {
    let mut tabs = affordance_scene
        .payload_tabs()
        .filter_map(payload_tab_from_affordance_layer)
        .collect::<Vec<_>>();
    tabs.sort_by_key(|tab| tab.index);
    tabs
}

fn payload_tab_from_affordance_layer(
    layer: &DockVisualAffordanceLayer,
) -> Option<DockDropPreviewPayloadTab> {
    Some(DockDropPreviewPayloadTab {
        index: layer.payload_index?,
        title: layer.payload_title.clone().unwrap_or_default(),
    })
}

fn preview_transition_opacity(progress: f32) -> f32 {
    0.68 + (0.32 * progress.clamp(0.0, 1.0))
}

fn stable_tab_preview_insert_left(
    tab_strip_start: Pixels,
    insert_index: usize,
    existing_tab_widths: &[Pixels],
) -> Pixels {
    existing_tab_widths
        .iter()
        .take(insert_index)
        .fold(tab_strip_start, |left, width| {
            left + *width + px(DROP_PREVIEW_TAB_GAP)
        })
}

fn stable_tab_preview_insertion_x(payload_tab_left: Pixels) -> Pixels {
    payload_tab_left
}

fn cursor_for_divider_target(target: &DockDividerHitTarget) -> CursorStyle {
    match target {
        DockDividerHitTarget::Single(handle) => match handle.key.axis {
            crate::SplitAxis::Horizontal => CursorStyle::ResizeColumn,
            crate::SplitAxis::Vertical => CursorStyle::ResizeRow,
        },
        DockDividerHitTarget::Corner(_) => CursorStyle::ResizeUpRightDownLeft,
    }
}

fn background_for_divider_affordance_state(
    state: DockDividerAffordanceState,
    style: &DockSplitterVisualStyle,
) -> Rgba {
    style.color(match state {
        DockDividerAffordanceState::Idle => DockSplitterVisualState::Idle,
        DockDividerAffordanceState::Hover => DockSplitterVisualState::Hovered,
        DockDividerAffordanceState::Active => DockSplitterVisualState::Active,
        DockDividerAffordanceState::Disabled => DockSplitterVisualState::Disabled,
    })
}

fn guide_directional_cue(
    zone: DropZone,
    box_size: open_gpui::Size<Pixels>,
    cue: Rgba,
) -> Option<AnyElement> {
    match zone {
        DropZone::Center => Some(
            div()
                .w((box_size.width * 0.48).max(px(10.0)))
                .h(px(2.0))
                .bg(cue)
                .into_any_element(),
        ),
        DropZone::Left | DropZone::Right => Some(
            div()
                .w(px(2.0))
                .h((box_size.height * 0.62).max(px(10.0)))
                .bg(cue)
                .into_any_element(),
        ),
        DropZone::Top | DropZone::Bottom => Some(
            div()
                .w((box_size.width * 0.62).max(px(10.0)))
                .h(px(2.0))
                .bg(cue)
                .into_any_element(),
        ),
    }
}

fn guide_inset_outline(box_size: open_gpui::Size<Pixels>, color: Rgba) -> Option<AnyElement> {
    if box_size.width <= px(10.0) || box_size.height <= px(10.0) {
        return None;
    }
    Some(
        div()
            .absolute()
            .left(px(3.0))
            .top(px(3.0))
            .w((box_size.width - px(6.0)).max(px(1.0)))
            .h((box_size.height - px(6.0)).max(px(1.0)))
            .border_1()
            .border_color(color)
            .rounded_sm()
            .into_any_element(),
    )
}

fn localize_bounds(bounds: Bounds<Pixels>, origin: open_gpui::Point<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        point(bounds.origin.x - origin.x, bounds.origin.y - origin.y),
        bounds.size,
    )
}

fn drop_box_selector_suffix(drop_box: DockPreviewDropBox) -> String {
    let layer = match drop_box.layer {
        crate::drop_preview::DockPreviewLayerKind::Inner => "inner",
        crate::drop_preview::DockPreviewLayerKind::Outer => "outer",
    };
    match drop_box.debug_node {
        Some(node) => format!("{layer}:{}:{:?}", node.as_u64(), drop_box.zone),
        None => format!("{layer}:{:?}", drop_box.zone),
    }
}

fn target_preview_visual_state(
    decision: &crate::drop_preview::DockPreviewDecision,
) -> DockTargetPreviewVisualState {
    if decision.is_allowed() {
        DockTargetPreviewVisualState::Accepted
    } else {
        DockTargetPreviewVisualState::Rejected
    }
}

fn drop_guide_visual_state(
    kind: geometry::DockDropBoxKind,
    active: bool,
) -> DockDropGuideVisualState {
    match (kind.is_center(), active) {
        (true, true) => DockDropGuideVisualState::CenterActive,
        (true, false) => DockDropGuideVisualState::CenterIdle,
        (false, true) => DockDropGuideVisualState::EdgeActive,
        (false, false) => DockDropGuideVisualState::EdgeIdle,
    }
}

fn route_preview_visual_state(preview: &DockDropRoutePreview) -> DockRoutePreviewVisualState {
    if preview.rejected {
        return DockRoutePreviewVisualState::Rejected;
    }

    match preview.kind {
        crate::drop_preview::DockDropRoutePreviewKind::KnownViewport => {
            DockRoutePreviewVisualState::KnownViewport
        }
        crate::drop_preview::DockDropRoutePreviewKind::TearOff => {
            DockRoutePreviewVisualState::TearOff
        }
        crate::drop_preview::DockDropRoutePreviewKind::Rejected => {
            DockRoutePreviewVisualState::Rejected
        }
    }
}

fn preview_tab_width(text_width: Pixels) -> Pixels {
    (text_width + px(DROP_PREVIEW_TAB_TEXT_PADDING))
        .max(px(DROP_PREVIEW_TAB_MIN_WIDTH))
        .min(px(DROP_PREVIEW_TAB_MAX_WIDTH))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drop_preview::DockDropRoutePreviewKind;
    use open_gpui::{point, size};

    #[test]
    fn missing_frame_candidate_releases_slot_before_cleanup_reentry() {
        let frame_slot = Rc::new(RefCell::new(DockViewportHostSceneCandidateState::default()));
        frame_slot.borrow_mut().begin_prepaint();

        let candidate = take_viewport_host_scene_candidate_for_commit(&frame_slot);
        let Some(_) = candidate else {
            frame_slot
                .try_borrow_mut()
                .expect("cleanup reentry must not retain the candidate-slot borrow")
                .discard_current(None);
            return;
        };

        panic!("a prepaint without a pending candidate must take the cleanup branch");
    }

    #[test]
    fn reveal_generation_drift_settles_without_overwriting_stronger_terminal_outcomes() {
        assert_eq!(
            classify_live_reveal_snapshot(
                WindowProvisionalRevealOutcome::Pending,
                Some(41),
                Some(42),
            ),
            DockLiveRevealSnapshotClassification::Failed(DockLiveUndockRevealOutcome::Stale)
        );
        assert_eq!(
            classify_live_reveal_snapshot(
                WindowProvisionalRevealOutcome::Revealed,
                Some(41),
                Some(42),
            ),
            DockLiveRevealSnapshotClassification::Failed(DockLiveUndockRevealOutcome::Stale)
        );
        assert_eq!(
            classify_live_reveal_snapshot(
                WindowProvisionalRevealOutcome::Rejected,
                Some(41),
                Some(42),
            ),
            DockLiveRevealSnapshotClassification::Failed(DockLiveUndockRevealOutcome::Rejected)
        );
        assert_eq!(
            classify_live_reveal_snapshot(
                WindowProvisionalRevealOutcome::WindowTerminal,
                Some(41),
                Some(42),
            ),
            DockLiveRevealSnapshotClassification::Failed(
                DockLiveUndockRevealOutcome::WindowTerminal
            )
        );
        assert_eq!(
            classify_live_reveal_snapshot(WindowProvisionalRevealOutcome::Pending, None, None),
            DockLiveRevealSnapshotClassification::Pending
        );
        assert_eq!(
            classify_live_reveal_snapshot(WindowProvisionalRevealOutcome::Pending, Some(42), None,),
            DockLiveRevealSnapshotClassification::Failed(DockLiveUndockRevealOutcome::Stale)
        );
        assert_eq!(
            classify_live_reveal_snapshot(
                WindowProvisionalRevealOutcome::Revealed,
                Some(42),
                Some(42),
            ),
            DockLiveRevealSnapshotClassification::Revealed
        );
    }

    fn preview(rejected: bool, payload_tab: bool) -> DockDropPreview {
        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(80.0)));
        let target_tabs = None;
        let insert_index = None;
        let decision = if rejected {
            crate::drop_preview::DockPreviewDecision::rejected(None)
        } else {
            crate::drop_preview::DockPreviewDecision::allowed()
        };
        let payload_tabs = payload_tab.then(|| crate::drop_preview::DockPreviewPayloadTabs {
            target_tabs,
            insert_index,
            insertion: None,
            tabs: vec![crate::drop_preview::DockPreviewPayloadTab {
                title: "Panel".to_string(),
            }],
        });
        DockDropPreview {
            scene: crate::drop_preview::DockPreviewScene {
                decision,
                layers: Vec::new(),
                body: crate::drop_preview::DockPreviewBody {
                    future_bounds: bounds,
                    body_bounds: bounds,
                },
                payload_tabs,
            },
        }
    }

    fn route_preview(kind: DockDropRoutePreviewKind, rejected: bool) -> DockDropRoutePreview {
        DockDropRoutePreview {
            kind,
            bounds: Bounds::new(point(px(0.0), px(0.0)), size(px(56.0), px(40.0))),
            rejected,
        }
    }

    #[test]
    fn payload_tab_render_inputs_come_from_visual_affordance_layers() {
        let mut preview = preview(false, true);
        preview.scene.payload_tabs.as_mut().unwrap().insertion =
            Some(crate::drop_preview::DockPreviewTabInsertion {
                target_tabs: None,
                index: crate::drop_preview::DockPreviewTabInsertionIndex::Append,
                slot_bounds: Some(Bounds::new(
                    point(px(0.0), px(0.0)),
                    size(px(3.0), px(26.0)),
                )),
                clipping_bounds: Bounds::new(point(px(0.0), px(0.0)), size(px(80.0), px(26.0))),
            });
        preview.scene.payload_tabs.as_mut().unwrap().tabs = vec![
            crate::drop_preview::DockPreviewPayloadTab {
                title: "Diff".to_string(),
            },
            crate::drop_preview::DockPreviewPayloadTab {
                title: "Preview".to_string(),
            },
        ];
        let affordance_scene = DockVisualAffordanceScene::from_preview(&preview.scene);

        assert!(affordance_scene.has_payload_tab_preview());
        assert_eq!(
            affordance_payload_tabs(&affordance_scene),
            vec![
                DockDropPreviewPayloadTab {
                    index: 0,
                    title: "Diff".to_string(),
                },
                DockDropPreviewPayloadTab {
                    index: 1,
                    title: "Preview".to_string(),
                },
            ]
        );
    }

    #[test]
    fn active_center_guides_have_stronger_palette_than_inactive_edge_guides() {
        let style = crate::DockVisualStyle::built_in();
        let active_center = style.previews.guide(drop_guide_visual_state(
            geometry::DockDropBoxKind::Center,
            true,
        ));
        let inactive_edge = style.previews.guide(drop_guide_visual_state(
            geometry::DockDropBoxKind::InnerEdge(DropZone::Left),
            false,
        ));

        assert_ne!(active_center.border, inactive_edge.border);
        assert_ne!(active_center.background, inactive_edge.background);
        assert!(active_center.background.a > inactive_edge.background.a);
    }

    #[test]
    fn rejected_drop_preview_uses_rejected_palette() {
        let style = crate::DockVisualStyle::built_in();
        let accepted = style.previews.target(target_preview_visual_state(
            &preview(false, false).scene.decision,
        ));
        let rejected = style.previews.target(target_preview_visual_state(
            &preview(true, false).scene.decision,
        ));

        assert_ne!(accepted, rejected);
        assert_eq!(rejected, style.previews.rejected_target);
    }

    #[test]
    fn payload_tab_preview_uses_stronger_selected_tab_palette() {
        let style = crate::DockVisualStyle::built_in();
        let palette = style.previews.target(target_preview_visual_state(
            &preview(false, true).scene.decision,
        ));

        assert!(palette.tab_background.a > palette.body_background.a);
        assert_eq!(
            palette.tab_text,
            crate::DockVisualPalette::built_in().accent_foreground
        );
    }

    #[test]
    fn route_preview_kinds_keep_distinct_palettes() {
        let style = crate::DockVisualStyle::built_in();
        let known_preview = route_preview(DockDropRoutePreviewKind::KnownViewport, false);
        let tear_off_preview = route_preview(DockDropRoutePreviewKind::TearOff, false);
        let rejected_preview = route_preview(DockDropRoutePreviewKind::Rejected, true);
        let known = style
            .previews
            .route(route_preview_visual_state(&known_preview));
        let tear_off = style
            .previews
            .route(route_preview_visual_state(&tear_off_preview));
        let rejected = style
            .previews
            .route(route_preview_visual_state(&rejected_preview));

        assert_ne!(known, tear_off);
        assert_ne!(known, rejected);
        assert_ne!(tear_off, rejected);
    }

    #[test]
    fn divider_affordance_states_have_distinct_feedback_colors() {
        let style = crate::DockVisualStyle::built_in();
        let states = [
            DockDividerAffordanceState::Idle,
            DockDividerAffordanceState::Hover,
            DockDividerAffordanceState::Active,
            DockDividerAffordanceState::Disabled,
        ];

        for (index, state) in states.iter().enumerate() {
            for other in states.iter().skip(index + 1) {
                assert_ne!(
                    background_for_divider_affordance_state(*state, &style.splitters),
                    background_for_divider_affordance_state(*other, &style.splitters),
                    "{state:?} and {other:?} should be visually distinguishable"
                );
            }
        }
    }

    #[test]
    fn preview_tab_width_stays_within_bounds() {
        assert_eq!(preview_tab_width(px(8.0)), px(DROP_PREVIEW_TAB_MIN_WIDTH));
        assert_eq!(preview_tab_width(px(240.0)), px(DROP_PREVIEW_TAB_MAX_WIDTH));
        assert_eq!(
            preview_tab_width(px(90.0)),
            px(90.0 + DROP_PREVIEW_TAB_TEXT_PADDING)
        );
    }

    #[test]
    fn stable_tab_preview_insert_left_uses_deterministic_tab_widths() {
        let tab_strip_start = px(8.0);
        let widths = [px(72.0), px(90.0), px(120.0)];

        assert_eq!(
            stable_tab_preview_insert_left(tab_strip_start, 0, &widths),
            px(8.0)
        );
        assert_eq!(
            stable_tab_preview_insert_left(tab_strip_start, 1, &widths),
            px(86.0)
        );
        assert_eq!(
            stable_tab_preview_insert_left(tab_strip_start, 2, &widths),
            px(182.0)
        );
    }
}
