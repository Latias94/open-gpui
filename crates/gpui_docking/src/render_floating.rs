use crate::{
    DockFloatingContainer, DockHost, DockNodeId,
    chrome_geometry::dock_floating_chrome_bounds,
    debug::DockDebugRegion,
    drag::{DockDragPayload, DockDragPayloadKind, DockDragTearOffGeometry},
    drag_visual::DockDragVisual,
    drop_scene_fact,
    host_render_actions::DockRenderedPointerPosition,
    host_render_session::{DockFloatingChromeTarget, DockHostRenderSession},
    render::DockViewportHostSceneCandidateSlot,
};
use open_gpui::{
    AnyElement, App, AppContext, Context, DispatchPhase, DragMoveEvent, HitboxBehavior,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, Pixels, PointerCaptureHandle, StatefulInteractiveElement, Styled, Window,
    canvas, div, px, rgba,
};

impl DockHost {
    pub(crate) fn render_floating_node(
        &mut self,
        node: DockNodeId,
        child: DockNodeId,
        session: &DockHostRenderSession,
        viewport_host_scene_frame: &DockViewportHostSceneCandidateSlot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::Floating { node },
            format!("{}:floating:{}", session.selector_prefix(), node.as_u64()),
        );
        div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .child(self.render_node(child, session, viewport_host_scene_frame, window, cx))
            .into_any_element()
    }

    pub(crate) fn render_floating_container(
        &mut self,
        container: DockFloatingContainer,
        session: &DockHostRenderSession,
        viewport_host_scene_frame: &DockViewportHostSceneCandidateSlot,
        pointer_capture: PointerCaptureHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::Floating {
                node: container.node,
            },
            format!(
                "{}:floating:{}",
                session.selector_prefix(),
                container.node.as_u64()
            ),
        );
        let child = session.floating_child(container.node);
        let bounds = container.bounds;
        let chrome_bounds = dock_floating_chrome_bounds(bounds);
        let content = child
            .map(|child| self.render_node(child, session, viewport_host_scene_frame, window, cx))
            .unwrap_or_else(|| self.render_missing_node(container.node, session));
        let title = child
            .map(|child| session.floating_title(child))
            .unwrap_or_else(|| "Floating".to_string());
        let floating_style = &session.visual_style().floating;

        div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .absolute()
            .left(bounds.origin.x)
            .top(bounds.origin.y)
            .w(bounds.size.width)
            .h(bounds.size.height)
            .flex()
            .flex_col()
            .overflow_hidden()
            .occlude()
            .border_1()
            .border_color(floating_style.border)
            .bg(floating_style.background)
            .shadow(floating_style.shadow.clone())
            .child(self.render_floating_handle(
                container,
                chrome_bounds.title_bar_bounds.size.height,
                title,
                session,
                viewport_host_scene_frame,
                pointer_capture,
                cx,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_hidden()
                    .child(content),
            )
            .into_any_element()
    }

    fn render_floating_handle(
        &mut self,
        container: DockFloatingContainer,
        handle_height: Pixels,
        title: String,
        session: &DockHostRenderSession,
        _viewport_host_scene_frame: &DockViewportHostSceneCandidateSlot,
        pointer_capture: PointerCaptureHandle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::FloatingHandle {
                node: container.node,
            },
            format!(
                "{}:floating:{}:handle",
                session.selector_prefix(),
                container.node.as_u64()
            ),
        );
        let space = session.space().clone();
        let floating = container.node;
        let bounds = container.bounds;
        let entity = cx.entity();
        let window_binding = self.current_window_binding();
        let chrome_target = session.floating_chrome_target(floating);
        let floating_style = &session.visual_style().floating;

        let drop_space = space.clone();
        let handle = div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .relative()
            .flex()
            .flex_none()
            .h(handle_height)
            .items_center()
            .px_2()
            .bg(floating_style.title_background)
            .border_b_1()
            .border_color(floating_style.title_border)
            .text_color(floating_style.title_text)
            .text_sm()
            .cursor_pointer()
            // Floating chrome occludes the host below it. It must therefore transport the
            // genuine local drop event while the render-owned preview remains the target fact.
            .on_drop(cx.listener(
                move |this, event: &open_gpui::DropEvent<DockDragPayload>, window, cx| {
                    if !this
                        .accepts_window_callback(window_binding, window.window_handle().window_id())
                    {
                        return;
                    }
                    let Ok(layout_position) = event.pointer().target_layout_position() else {
                        return;
                    };
                    this.drop_payload_event_from_render(
                        event.value(),
                        drop_space.clone(),
                        DockRenderedPointerPosition::new(
                            layout_position,
                            event.pointer().window_event().position,
                        ),
                        window,
                        cx,
                    );
                },
            ));

        if let Some(DockFloatingChromeTarget::SingleTabs(target_tabs)) = chrome_target {
            let mut payload = DockDragPayload::new_floating(space.clone(), floating, title.clone());
            if let Some(preview_titles) = session.multi_preview_tab_titles_for_node(floating) {
                payload = payload.with_preview_tabs(preview_titles);
            }
            let drag_entity = entity.clone();
            let drag_space = space.clone();
            let drag_visual_title = title.clone();
            let drag_visual_style = session.visual_style().drag.clone();
            let drag_surface_id = format!(
                "{}:floating:{}:drag-surface",
                session.selector_prefix(),
                floating.as_u64()
            );
            let drag_surface = div()
                .id(drag_surface_id)
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .size_full()
                .cursor_pointer()
                // Fully transparent empty surfaces do not reliably initiate GPUI drag hit-tests.
                .bg(rgba(0x00000001))
                .on_drag(payload, move |payload, geometry, window, cx| {
                    let Ok(start_layout_position) = geometry.target_layout_position() else {
                        defer_rejected_payload_drag_cleanup(payload, window, cx);
                        let drag_title = drag_visual_title.clone();
                        let drag_style = drag_visual_style.clone();
                        return cx.new(move |_| DockDragVisual::new(drag_title, drag_style));
                    };
                    let Ok(source_window_bounds) =
                        geometry.geometry().layout_to_window_bounds(bounds)
                    else {
                        defer_rejected_payload_drag_cleanup(payload, window, cx);
                        let drag_title = drag_visual_title.clone();
                        let drag_style = drag_visual_style.clone();
                        return cx.new(move |_| DockDragVisual::new(drag_title, drag_style));
                    };
                    let start_window_position = geometry.window_position();
                    let mut tear_off_geometry = DockDragTearOffGeometry::from_source_bounds(
                        source_window_bounds,
                        start_window_position,
                    )
                    .with_preferred_size(bounds.size);
                    if let Some(display) = window.display(cx) {
                        tear_off_geometry =
                            tear_off_geometry.with_display_work_area(display.visible_bounds());
                    }
                    let frozen_drag_visual_style = drag_entity.update(cx, |host, cx| {
                        if !host.accepts_window_callback(
                            window_binding,
                            window.window_handle().window_id(),
                        ) {
                            return Some(drag_visual_style.clone());
                        }
                        host.focus_host_for_drag_from_render(window, cx);
                        if !host.begin_floating_drag_from_render(
                            drag_space.clone(),
                            floating,
                            start_layout_position,
                            bounds,
                            cx,
                        ) {
                            return None;
                        }
                        let drag_session = host
                            .begin_payload_drag_from_render_with_drag_visual_style(
                                payload,
                                drag_visual_style.clone(),
                                geometry,
                                window,
                                cx,
                            );
                        let _ = host.update_payload_drag_tear_off_geometry_from_render(
                            payload,
                            tear_off_geometry,
                        );
                        Some(
                            host.viewport_runtime()
                                .active_payload_drag_visual_style(Some(&drag_session))
                                .expect("new drag session must retain its captured visual style"),
                        )
                    });
                    if frozen_drag_visual_style.is_none() {
                        defer_rejected_payload_drag_cleanup(payload, window, cx);
                    }
                    let drag_title = drag_visual_title.clone();
                    let drag_style =
                        frozen_drag_visual_style.unwrap_or_else(|| drag_visual_style.clone());
                    cx.new(move |_| DockDragVisual::new(drag_title, drag_style))
                })
                .on_drag_move(cx.listener(
                    move |this, event: &DragMoveEvent<DockDragPayload>, window, cx| {
                        if !this.accepts_window_callback(
                            window_binding,
                            window.window_handle().window_id(),
                        ) {
                            return;
                        }
                        let payload = event.drag().clone();
                        let Ok(layout_position) = event.target_layout_position() else {
                            return;
                        };
                        let mut floating_layout_bounds = bounds;
                        if payload.source_space == space
                            && matches!(
                                payload.kind,
                                DockDragPayloadKind::Floating { floating: payload_floating }
                                    if payload_floating == floating
                            )
                        {
                            let Some(updated_bounds) =
                                this.update_floating_drag_from_render(layout_position, cx)
                            else {
                                return;
                            };
                            floating_layout_bounds = updated_bounds;
                            let Ok(floating_window_bounds) = event
                                .geometry()
                                .layout_to_window_bounds(floating_layout_bounds)
                            else {
                                return;
                            };
                            let mut tear_off_geometry =
                                DockDragTearOffGeometry::from_source_bounds(
                                    floating_window_bounds,
                                    event.window_position(),
                                )
                                .with_preferred_size(floating_layout_bounds.size);
                            if let Some(display) = window.display(cx) {
                                tear_off_geometry = tear_off_geometry
                                    .with_display_work_area(display.visible_bounds());
                            }
                            this.update_payload_drag_tear_off_geometry_from_render(
                                &payload,
                                tear_off_geometry,
                            );
                        }
                        if !event.displayed_bounds().contains(&event.window_position()) {
                            return;
                        }
                        let fact = drop_scene_fact::floating_title_bar(
                            floating,
                            target_tabs,
                            event.layout_bounds(),
                            floating_layout_bounds,
                        );
                        this.update_local_drop_scene_fact_from_render(
                            &payload,
                            fact,
                            DockRenderedPointerPosition::new(
                                layout_position,
                                event.window_position(),
                            ),
                            window,
                            cx,
                        );
                    },
                ));

            return handle.child(title).child(drag_surface).into_any_element();
        }

        handle
            .child(title)
            .child(
                canvas(
                    |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
                    move |_, hitbox, window, _| {
                        window.on_mouse_event({
                            let entity = entity.clone();
                            let space = space.clone();
                            let hitbox = hitbox.clone();
                            let window_binding = window_binding;
                            move |event: &MouseDownEvent, phase, window, app| {
                                if phase != DispatchPhase::Bubble
                                    || event.button != MouseButton::Left
                                    || !hitbox.is_mouse_event_target(window)
                                    || !hitbox.contains_window_point(event.position)
                                {
                                    return;
                                }

                                let Ok(layout_position) =
                                    hitbox.window_to_layout_point(event.position)
                                else {
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
                                    host.begin_floating_drag_from_render(
                                        space.clone(),
                                        floating,
                                        layout_position,
                                        bounds,
                                        cx,
                                    )
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

                                let Ok(layout_position) =
                                    hitbox.window_to_layout_point(event.position)
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
                                    host.update_floating_drag_from_render(layout_position, cx);
                                });
                            }
                        });

                        window.on_mouse_event({
                            let window_binding = window_binding;
                            move |event: &MouseUpEvent, phase, window, app| {
                                if phase != DispatchPhase::Capture
                                    || event.button != MouseButton::Left
                                {
                                    return;
                                }

                                entity.update(app, |host, cx| {
                                    if !host.accepts_window_callback(
                                        window_binding,
                                        window.window_handle().window_id(),
                                    ) {
                                        return;
                                    }
                                    host.finish_floating_drag_from_render(cx);
                                });
                            }
                        });
                    },
                )
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .size_full(),
            )
            .into_any_element()
    }
}

fn defer_rejected_payload_drag_cleanup(payload: &DockDragPayload, window: &Window, cx: &mut App) {
    let rejected_payload = payload.clone();
    window.defer(cx, move |window, cx| {
        if cx.active_drag_value::<DockDragPayload>() == Some(&rejected_payload) {
            cx.stop_active_drag(window);
        }
    });
}
