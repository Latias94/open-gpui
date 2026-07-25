use crate::{
    DockDropGuideVisualState, DockHost, DockNode, DockNodeId, DockRoutePreviewVisualState,
    DockSpaceId, DockSplitterVisualState, DockSplitterVisualStyle, DockTargetPreviewVisualState,
    DockViewportHostGeometry, DockViewportRuntimeHandle, DropZone,
    accessibility_scene::DockAccessibilityScene,
    debug::DockDebugRegion,
    divider_hit_map::{DockDividerAffordanceState, DockDividerHitMap, DockDividerHitTarget},
    drag::DockDragPayload,
    drop_preview::{
        DockDropPreview, DockDropRoutePreview, DockPreviewDropBox, DockPreviewTabInsertionIndex,
    },
    drop_scene_fact, geometry,
    host_render_actions::DockRenderedPointerPosition,
    host_render_session::{DockHostRenderSession, selected_index},
    interaction::DockPayloadDropRelease,
    presentation_scene::DockPresentationScene,
    render_split::DockRenderSplitInput,
    transition_executor::{
        DockDividerSample, DockPaneClipSample, DockTransitionSample, DockVisualAffordanceSample,
    },
    transition_geometry::DockTransitionPlan,
    viewport_drop_scene::DockViewportHostSceneSnapshot,
    visual_affordance_scene::{
        DockPayloadTabPreviewLayout, DockPayloadTabPreviewPlacement, DockVisualAffordanceLayer,
        DockVisualAffordanceScene,
    },
};
use open_gpui::{
    AnyElement, App, BorderStyle, Bounds, Context, CursorStyle, DispatchPhase, DragMoveEvent,
    DropEvent, Entity, HitboxBehavior, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, PointerCaptureHandle,
    PrepaintPublicationId, Render, Rgba, SharedString, Styled, Window, WindowId, canvas, div,
    point, px, quad, rgba,
};
use open_gpui_motion::MotionTransition;
use std::{cell::RefCell, rc::Rc};

#[derive(Clone)]
pub(crate) struct DockViewportHostSceneCandidate {
    pub(crate) snapshot: DockViewportHostSceneSnapshot,
    pub(crate) presentation_scene: DockPresentationScene,
}

#[derive(Default)]
pub(crate) struct DockViewportHostSceneCandidateState {
    pending: Option<DockViewportHostSceneCandidate>,
    committed: Option<DockViewportHostSceneCandidate>,
    prepaint_ran: bool,
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

    fn candidate_for_commit(&mut self) -> Option<DockViewportHostSceneCandidate> {
        if !self.prepaint_ran {
            return self.committed.clone();
        }
        self.prepaint_ran = false;
        let candidate = self.pending.take();
        if candidate.is_none() {
            self.committed = None;
        }
        candidate
    }

    fn commit(&mut self, candidate: DockViewportHostSceneCandidate) {
        self.committed = Some(candidate);
    }

    fn discard(&mut self) {
        self.pending = None;
        self.committed = None;
        self.prepaint_ran = false;
    }
}

pub(crate) type DockViewportHostSceneCandidateSlot =
    Rc<RefCell<DockViewportHostSceneCandidateState>>;

fn clear_viewport_host_scene_publication(
    runtime: &DockViewportRuntimeHandle,
    entity: &Entity<DockHost>,
    space: &DockSpaceId,
    window_id: WindowId,
    window: &mut Window,
    app: &mut App,
) -> bool {
    let runtime_changed = runtime.discard_rendered_viewport_host_scene_frame(space, window_id);
    let host_changed = entity.update(app, |host, _| {
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
    passthrough_pointer_input: bool,
) {
    let discard_frame_slot = frame_slot.clone();
    let discard_runtime = runtime.clone();
    let discard_entity = entity.clone();
    let discard_space = space.clone();
    window.record_prepaint_window_transaction(
        publication,
        move |_, window, app| {
            let Some(candidate) = frame_slot.borrow_mut().candidate_for_commit() else {
                if clear_viewport_host_scene_publication(
                    &runtime, &entity, &space, window_id, window, app,
                ) {
                    window.refresh();
                }
                return;
            };
            let DockViewportHostSceneCandidate {
                snapshot,
                presentation_scene,
            } = candidate;
            let committed_candidate = DockViewportHostSceneCandidate {
                snapshot: snapshot.clone(),
                presentation_scene: presentation_scene.clone(),
            };
            let preparation = runtime.commit_rendered_viewport_host_scene_snapshot(
                snapshot,
                window,
                app,
                passthrough_pointer_input,
            );
            let Some(frame) = preparation.frame.clone() else {
                frame_slot.borrow_mut().discard();
                if preparation.changed
                    || clear_viewport_host_scene_publication(
                        &runtime, &entity, &space, window_id, window, app,
                    )
                {
                    window.refresh();
                }
                return;
            };
            let interaction_frame_changed = entity.update(app, |host, _| {
                host.set_last_presentation_scene(presentation_scene);
                host.publish_rendered_viewport_host_scene_frame_from_render(Some(frame), window)
            });
            frame_slot.borrow_mut().commit(committed_candidate);
            if preparation.changed || interaction_frame_changed {
                window.refresh();
            }
        },
        move |_, window: &mut Window, app: &mut App| {
            discard_frame_slot.borrow_mut().discard();
            if clear_viewport_host_scene_publication(
                &discard_runtime,
                &discard_entity,
                &discard_space,
                window_id,
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
        self.ensure_surface_activation_host_registration(window, cx);
        self.ensure_viewport_activation_subscription(window, cx);
        self.ensure_viewport_bounds_subscription(window, cx);
        self.ensure_viewport_release_subscription(window, cx);
        self.prepare_pending_focus_selection_from_render(window, cx);
        let raw_drag_pointer_capture = self.ensure_pointer_session(window);
        let window_binding = self.current_window_binding();
        let visual_style = self.resolve_visual_style(window, cx);
        #[cfg(test)]
        {
            self.record_resolved_visual_style_for_test(visual_style.clone());
        }
        let session = self.render_session_with_visual_style(visual_style, cx);
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
                window.on_pointer_cancel(move |_, phase, window, app| {
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
                        host.cancel_pointer_interactions_from_render(payload.as_ref(), window, cx)
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
                    let drag_session = this.active_payload_drag_session(payload);
                    let event_receiver_local_scene_proof =
                        this.interaction().viewport_host_scene_frame().cloned();
                    this.drop_payload_release_from_render(
                        DockPayloadDropRelease::hovered_host_with_positions(
                            payload.clone(),
                            drop_host_space.clone(),
                            layout_position,
                            event.pointer().window_event().position,
                            drag_session,
                        )
                        .with_event_receiver_local_scene_proof(event_receiver_local_scene_proof),
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

        host = host.child(self.render_viewport_host_scene_probe(
            &viewport_host_scene_frame,
            &session,
            session.drop_guide_metrics(),
            session.empty_central_requests_platform_pointer_passthrough(),
            cx,
        ));

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
        host = host.child(self.render_payload_drag_event_layer(cx));

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

        self.apply_pending_focus_from_render(&session, window, cx);

        host
    }
}

impl DockHost {
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

    // GPUI captures the source pointer for the lifetime of a drag, which suppresses
    // `on_mouse_up_out`. This preinstalled layer transports foreign hover events and owns the
    // terminal mouse-up for Dock payloads without weakening GPUI's window-local drag contract.
    fn render_payload_drag_event_layer(&self, cx: &mut Context<Self>) -> AnyElement {
        let entity = cx.entity();
        let window_binding = self.current_window_binding();

        canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |_, hitbox, window, _app| {
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
                        if !hitbox.contains_window_point(event.position) {
                            return;
                        }
                        let Ok(layout_position) = hitbox.window_to_layout_point(event.position)
                        else {
                            return;
                        };
                        let Some(payload) = app.active_drag_value::<DockDragPayload>().cloned()
                        else {
                            return;
                        };
                        let receiver_window_id = window.window_handle().window_id();
                        let handled = entity.update(app, |host, cx| {
                            if !host.accepts_window_callback(window_binding, receiver_window_id) {
                                return None;
                            }
                            if !host
                                .viewport_runtime()
                                .is_foreign_payload_drag_for_window(&payload, receiver_window_id)
                            {
                                return None;
                            }
                            Some(host.update_payload_drag_hover_from_rendered_host_scene(
                                &payload,
                                DockRenderedPointerPosition::new(layout_position, event.position),
                                window,
                                cx,
                            ))
                        });
                        let Some(changed) = handled else {
                            return;
                        };
                        if changed {
                            window.refresh();
                        }
                        app.stop_propagation();
                    }
                });

                window.on_mouse_event({
                    let entity = entity.clone();
                    let hitbox = hitbox.clone();
                    let window_binding = window_binding;
                    move |event: &MouseUpEvent, phase, window, app| {
                        if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                            return;
                        }
                        let Some(payload) = app.active_drag_value::<DockDragPayload>().cloned()
                        else {
                            return;
                        };
                        let receiver_window_id = window.window_handle().window_id();
                        let layout_position = hitbox
                            .contains_window_point(event.position)
                            .then(|| hitbox.window_to_layout_point(event.position).ok())
                            .flatten();
                        let handled = entity.update(app, |host, cx| {
                            if !host.accepts_window_callback(window_binding, receiver_window_id) {
                                return false;
                            }
                            if host.active_payload_drag_session(&payload).is_none() {
                                return false;
                            }
                            if let Some(layout_position) = layout_position {
                                host.drop_payload_release_from_rendered_host_scene(
                                    payload,
                                    DockRenderedPointerPosition::new(
                                        layout_position,
                                        event.position,
                                    ),
                                    window,
                                    cx,
                                );
                                return true;
                            }
                            if !host
                                .viewport_runtime()
                                .is_payload_drag_source_window(&payload, receiver_window_id)
                            {
                                return false;
                            }
                            host.drop_payload_release_outside_rendered_host_scene(
                                payload,
                                event.position,
                                window,
                                cx,
                            )
                        });
                        if !handled {
                            return;
                        }
                        app.stop_active_drag(window);
                        app.stop_propagation();
                        window.refresh();
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
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entity = cx.entity();
        let runtime = self.viewport_runtime().clone();
        let publication = self.viewport_scene_publication();
        let space = self.space().clone();
        let session = session.clone();
        let frame_slot = frame_slot.clone();
        canvas(
            move |bounds, window, app| {
                frame_slot.borrow_mut().begin_prepaint();
                let window_id = window.window_handle().window_id();
                record_viewport_host_scene_transaction(
                    window,
                    publication,
                    frame_slot.clone(),
                    runtime.clone(),
                    entity.clone(),
                    space.clone(),
                    window_id,
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
                let snapshot = DockViewportHostSceneSnapshot::new_with_facts(
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
                        snapshot,
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
                candidate.snapshot.push_fact(fact);
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
