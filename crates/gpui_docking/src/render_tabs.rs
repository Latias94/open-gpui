use crate::{
    DockHost, DockItemId, DockNodeId, DockTabVisualState,
    accessibility_scene::{DockAccessibilityScene, gpui_accessible_action_from_ui},
    debug::DockDebugRegion,
    drag::{DockDragPayload, DockDragTearOffGeometry},
    drag_visual::DockDragVisual,
    drop_scene_fact,
    host_render_actions::DockRenderedPointerPosition,
    host_render_session::{DockHostPanelRenderResolution, DockHostRenderSession},
    render::DockViewportHostSceneCandidateSlot,
};
use open_gpui::{
    AnyElement, AppContext as _, Bounds, Context, DragMoveEvent, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Pixels, StatefulInteractiveElement, Styled, Window, div, px,
};
use open_gpui_ui_core::AccessibleAction;

#[derive(Debug, Clone, Copy, PartialEq)]
struct RenderedTabHitTarget {
    index: usize,
    bounds: Bounds<Pixels>,
}

impl DockHost {
    pub(crate) fn render_tabs(
        &mut self,
        node: DockNodeId,
        items: Vec<DockItemId>,
        selected: usize,
        session: &DockHostRenderSession,
        viewport_host_scene_frame: &DockViewportHostSceneCandidateSlot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if items.is_empty() {
            return self.render_missing_node(node, session);
        }

        let selector = self.record_debug_selector(
            DockDebugRegion::Tabs { node },
            format!("{}:tabs:{}", session.selector_prefix(), node.as_u64()),
        );
        let selected = selected.min(items.len().saturating_sub(1));
        let selected_item = items[selected].clone();
        let is_central = session.is_central_tabs(node);
        let drop_root = session.drop_root_for_tabs(node);
        let source_space = session.space().clone();
        let entity = cx.entity();
        let window_binding = self.current_window_binding();
        let tabs_style = &session.visual_style().tabs;
        let stack_title = if items.len() == 1 {
            session.panel_title(&selected_item)
        } else {
            format!("{} tabs", items.len())
        };
        let stack_drag_title = stack_title.clone();
        let mut stack_payload =
            DockDragPayload::new_tabs(session.space().clone(), node, stack_title);
        if let Some(preview_titles) = session.multi_preview_tab_titles_for_node(node) {
            stack_payload = stack_payload.with_preview_tabs(preview_titles);
        }
        let tab_count = items.len();
        let stack_drag_visual_style = session.visual_style().drag.clone();
        let anchor_entity = entity.clone();
        let anchor_space = session.space().clone();

        let mut tabs = div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .border_1()
            .border_color(tabs_style.frame_border)
            .bg(tabs_style.frame_background)
            .capture_any_mouse_down(move |event, _window, cx| {
                if event.window_event().button != MouseButton::Left {
                    return;
                }
                anchor_entity.update(cx, |host, _| {
                    if !host.accepts_bound_window(window_binding) {
                        return;
                    }
                    host.record_payload_drag_anchor_from_render(
                        anchor_space.clone(),
                        node,
                        event.window_event().position,
                    );
                });
            })
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DockDragPayload>, window, cx| {
                    if !this
                        .accepts_window_callback(window_binding, window.window_handle().window_id())
                    {
                        return;
                    }
                    let payload = event.drag().clone();
                    if payload.source_space == source_space && payload.source_node == node {
                        let cursor_position = this
                            .payload_drag_anchor_position_from_render(&payload)
                            .unwrap_or_else(|| event.window_position());
                        let source_bounds = this
                            .viewport_runtime()
                            .rendered_leaf_displayed_bounds_for_tabs(
                                &source_space,
                                Some(window.window_handle().window_id()),
                                node,
                            )
                            .unwrap_or_else(|| event.displayed_bounds());
                        let mut geometry = DockDragTearOffGeometry::from_source_bounds(
                            source_bounds,
                            cursor_position,
                        )
                        .with_preferred_size(source_bounds.size);
                        if let Some(display) = window.display(cx) {
                            geometry = geometry.with_display_work_area(display.visible_bounds());
                        }
                        this.update_payload_drag_tear_off_geometry_from_render(&payload, geometry);
                    }
                    if let Ok(layout_position) = event.target_layout_position()
                        && let Some(drop_root) = drop_root
                    {
                        let fact = drop_scene_fact::leaf(
                            drop_root,
                            node,
                            event.layout_bounds(),
                            is_central,
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
                    }
                },
            ));
        let stack_drag_entity = entity.clone();
        let mut tab_hit_targets = Vec::with_capacity(items.len());
        for index in 0..items.len() {
            if let Some(bounds) = self.viewport_runtime().rendered_tab_label_bounds_for_tabs(
                self.space(),
                Some(window.window_handle().window_id()),
                node,
                index,
            ) {
                tab_hit_targets.push(RenderedTabHitTarget { index, bounds });
            }
        }
        let tab_hit_targets_for_bar = tab_hit_targets.clone();
        let tab_bar_selector = self.record_debug_selector(
            DockDebugRegion::TabBar { node },
            format!("{}:tabs:{}:bar", session.selector_prefix(), node.as_u64()),
        );
        let tab_bar_a11y = DockAccessibilityScene::tab_list_element_for_render(node, tab_count);
        let mut tab_bar = div()
            .id(tab_bar_a11y.id_str().to_string())
            .debug_selector(move || tab_bar_selector)
            .flex()
            .flex_row()
            .flex_none()
            .overflow_hidden()
            .bg(tabs_style.strip_background)
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DockDragPayload>, window, cx| {
                    if !this
                        .accepts_window_callback(window_binding, window.window_handle().window_id())
                    {
                        return;
                    }
                    let Ok(position) = event.target_layout_position() else {
                        return;
                    };
                    let payload = event.drag().clone();
                    let window_id = window.window_handle().window_id();
                    let fact = tab_hit_targets_for_bar
                        .iter()
                        .copied()
                        .chain((0..tab_count).filter_map(|index| {
                            this.viewport_runtime()
                                .rendered_tab_label_bounds_for_tabs(
                                    this.space(),
                                    Some(window_id),
                                    node,
                                    index,
                                )
                                .map(|bounds| RenderedTabHitTarget { index, bounds })
                        }))
                        .find(|target| target.bounds.contains(&position))
                        .map(|target| {
                            drop_scene_fact::tab_label(
                                node,
                                target.index,
                                target.bounds,
                                is_central,
                            )
                        })
                        .unwrap_or_else(|| {
                            drop_scene_fact::tab_bar(
                                node,
                                tab_count,
                                event.layout_bounds(),
                                is_central,
                            )
                        });
                    this.update_local_drop_scene_fact_from_render(
                        &payload,
                        fact,
                        DockRenderedPointerPosition::new(position, event.window_position()),
                        window,
                        cx,
                    );
                },
            ))
            .on_drag(stack_payload, move |payload, geometry, window, cx| {
                let source_drag_visual_style = stack_drag_visual_style.clone();
                let frozen_drag_visual_style = stack_drag_entity.update(cx, |host, cx| {
                    if !host
                        .accepts_window_callback(window_binding, window.window_handle().window_id())
                    {
                        return source_drag_visual_style.clone();
                    }
                    host.focus_host_for_drag_from_render(window, cx);
                    let drag_session = host.begin_payload_drag_from_render_with_drag_visual_style(
                        payload,
                        source_drag_visual_style.clone(),
                        window,
                        cx,
                    );
                    let source_bounds = geometry.displayed_bounds();
                    let cursor_position = host
                        .payload_drag_anchor_position_from_render(payload)
                        .unwrap_or_else(|| geometry.window_position());
                    let source_bounds = host
                        .viewport_runtime()
                        .rendered_leaf_displayed_bounds_for_tabs(
                            host.space(),
                            Some(window.window_handle().window_id()),
                            node,
                        )
                        .unwrap_or(source_bounds);
                    let _ = host.update_payload_drag_tear_off_geometry_from_render(
                        payload,
                        DockDragTearOffGeometry::from_source_bounds(source_bounds, cursor_position)
                            .with_preferred_size(source_bounds.size),
                    );
                    host.viewport_runtime()
                        .active_payload_drag_visual_style(Some(&drag_session))
                        .expect("new drag session must retain its captured visual style")
                });
                let drag_title = stack_drag_title.clone();
                cx.new(move |_| DockDragVisual::new(drag_title, frozen_drag_visual_style))
            });
        tab_bar = tab_bar_a11y.apply_to(tab_bar);

        for (index, item) in items.into_iter().enumerate() {
            let title = session.panel_title(&item);
            let selector = self.record_debug_selector(
                DockDebugRegion::Tab {
                    tabs: node,
                    item: item.clone(),
                },
                format!(
                    "{}:tabs:{}:tab:{}",
                    session.selector_prefix(),
                    node.as_u64(),
                    item
                ),
            );
            let payload = DockDragPayload::new_item(
                session.space().clone(),
                node,
                item.clone(),
                title.clone(),
            );
            let drag_entity = entity.clone();
            let focus_entity = entity.clone();
            let target_index = index;
            let tab_item = item.clone();
            let focus_item = item.clone();
            let drag_item = item.clone();
            let drag_visual_title = title.clone();
            let drag_visual_style = session.visual_style().drag.clone();
            let tab_a11y = DockAccessibilityScene::tab_element_for_render(
                node,
                item.clone(),
                title.clone(),
                index == selected,
                index,
            );
            let tab_palette = tabs_style.tab(if index == selected {
                DockTabVisualState::Selected
            } else {
                DockTabVisualState::Idle
            });
            let tab_hover_palette = tabs_style.tab(if index == selected {
                DockTabVisualState::SelectedHovered
            } else {
                DockTabVisualState::Hovered
            });
            let mut tab = div()
                .id(tab_a11y.id_str().to_string())
                .debug_selector(move || selector)
                .relative()
                .flex()
                .flex_row()
                .flex_none()
                .items_center()
                .gap_1()
                .px_2()
                .py_1()
                .border_1()
                .border_color(tab_palette.border)
                .bg(tab_palette.background)
                .cursor_pointer()
                .text_color(tab_palette.text)
                .hover(move |style| {
                    style
                        .border_color(tab_hover_palette.border)
                        .bg(tab_hover_palette.background)
                        .text_color(tab_hover_palette.text)
                })
                .on_click(cx.listener({
                    let window_binding = window_binding;
                    move |this, _, _, cx| {
                        if !this.accepts_bound_window(window_binding) {
                            return;
                        }
                        this.select_tab_from_render(node, tab_item.clone(), cx);
                    }
                }))
                .on_a11y_action(gpui_accessible_action_from_ui(AccessibleAction::Focus), {
                    let window_binding = window_binding;
                    move |_, _, cx| {
                        focus_entity.update(cx, |host, cx| {
                            if !host.accepts_bound_window(window_binding) {
                                return;
                            }
                            host.select_tab_from_render(node, focus_item.clone(), cx);
                        });
                    }
                })
                .on_drag_move(cx.listener(
                    move |this, event: &DragMoveEvent<DockDragPayload>, window, cx| {
                        if !this.accepts_window_callback(
                            window_binding,
                            window.window_handle().window_id(),
                        ) {
                            return;
                        }
                        let Ok(layout_position) = event.target_layout_position() else {
                            return;
                        };
                        let payload = event.drag().clone();
                        // The tabs leaf owns tear-off sizing; the tab label is only a drop target.
                        let fact = drop_scene_fact::tab_label(
                            node,
                            target_index,
                            event.layout_bounds(),
                            is_central,
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
                ))
                .on_drag(payload, move |payload, geometry, window, cx| {
                    let frozen_drag_visual_style = drag_entity.update(cx, |host, cx| {
                        if !host.accepts_window_callback(
                            window_binding,
                            window.window_handle().window_id(),
                        ) {
                            return drag_visual_style.clone();
                        }
                        host.focus_host_for_drag_from_render(window, cx);
                        let drag_session = host
                            .begin_tab_item_drag_from_render_with_drag_visual_style(
                                node,
                                drag_item.clone(),
                                payload,
                                drag_visual_style.clone(),
                                window,
                                cx,
                            );
                        let source_bounds = geometry.displayed_bounds();
                        let cursor_position = host
                            .payload_drag_anchor_position_from_render(payload)
                            .unwrap_or_else(|| geometry.window_position());
                        let source_bounds = host
                            .viewport_runtime()
                            .rendered_leaf_displayed_bounds_for_tabs(
                                host.space(),
                                Some(window.window_handle().window_id()),
                                node,
                            )
                            .unwrap_or(source_bounds);
                        let _ = host.update_payload_drag_tear_off_geometry_from_render(
                            payload,
                            DockDragTearOffGeometry::from_source_bounds(
                                source_bounds,
                                cursor_position,
                            )
                            .with_preferred_size(source_bounds.size),
                        );
                        host.viewport_runtime()
                            .active_payload_drag_visual_style(Some(&drag_session))
                            .expect("new drag session must retain its captured visual style")
                    });
                    let drag_title = drag_visual_title.clone();
                    cx.new(move |_| DockDragVisual::new(drag_title, frozen_drag_visual_style))
                });
            tab = tab_a11y.apply_to(tab);
            // Tab labels are a deliberate render-measured exception: final hit bounds depend on
            // intrinsic title and close-button layout, not the presentation scene's equal slots.
            tab = tab.child(self.render_tab_label_drop_scene_fact_probe(
                viewport_host_scene_frame,
                node,
                target_index,
                is_central,
                cx,
            ));
            tab = tab.child(title.clone());
            if session.panel_is_closable(&item) {
                let close_selector = self.record_debug_selector(
                    DockDebugRegion::TabClose {
                        tabs: node,
                        item: item.clone(),
                    },
                    format!(
                        "{}:tabs:{}:tab:{}:close",
                        session.selector_prefix(),
                        node.as_u64(),
                        item
                    ),
                );
                let close_item = item.clone();
                let close_palette = tabs_style.close_idle;
                let close_hover_palette = tabs_style.close_hovered;
                let close = div()
                    .id(format!("{}:a11y-close", close_selector))
                    .debug_selector(move || close_selector)
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .w(px(16.0))
                    .h(px(16.0))
                    .border_1()
                    .border_color(close_palette.border)
                    .bg(close_palette.background)
                    .text_color(close_palette.text)
                    .hover(move |style| {
                        style
                            .border_color(close_hover_palette.border)
                            .bg(close_hover_palette.background)
                            .text_color(close_hover_palette.text)
                    })
                    .cursor_pointer()
                    .on_click(cx.listener({
                        let window_binding = window_binding;
                        move |this, _, _, cx| {
                            if !this.accepts_bound_window(window_binding) {
                                return;
                            }
                            this.close_item_from_render(close_item.clone(), cx);
                            cx.stop_propagation();
                        }
                    }))
                    .child("x");
                tab = tab.child(close);
            }
            tab_bar = tab_bar.child(tab);
        }

        tabs = tabs
            .child(tab_bar)
            .child(self.render_panel(&selected_item, session, window, cx));
        tabs.into_any_element()
    }

    fn render_panel(
        &mut self,
        item: &DockItemId,
        session: &DockHostRenderSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let resolution = session.panel_for_render(item, cx);
        match resolution {
            DockHostPanelRenderResolution::Registered(panel_view) => {
                let focus_handle = self.ensure_panel_focus_tracker(item, window, cx);
                let selector = self.record_debug_selector(
                    DockDebugRegion::Panel { item: item.clone() },
                    format!("{}:panel:{}", session.selector_prefix(), item),
                );
                let panel_a11y = DockAccessibilityScene::tab_panel_element_for_render(
                    item.clone(),
                    session.panel_title(item),
                );
                let panel = div()
                    .id(panel_a11y.id_str().to_string())
                    .debug_selector(move || selector)
                    .track_focus(&focus_handle)
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_hidden()
                    .child(panel_view);
                panel_a11y.apply_to(panel).into_any_element()
            }
            DockHostPanelRenderResolution::Missing { prefix, item } => {
                let missing = item;
                let selector = self.record_debug_selector(
                    DockDebugRegion::MissingPanel {
                        item: missing.clone(),
                    },
                    format!("{}:panel:{}:missing", session.selector_prefix(), missing),
                );
                div()
                    .id(selector.clone())
                    .debug_selector(move || selector)
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .border_1()
                    .border_color(session.visual_style().host.missing_border)
                    .text_color(session.visual_style().host.missing_text)
                    .child(format!("{}: {}", prefix, missing))
                    .into_any_element()
            }
        }
    }
}
