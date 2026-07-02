use crate::{
    DockHost, DockItemId, DockNodeId,
    accessibility_scene::{DockAccessibilityScene, gpui_accessible_action_from_ui},
    debug::DockDebugRegion,
    drag::{DockDragPayload, DockDragTearOffGeometry},
    drop_scene_fact,
    host_render_session::{DockHostPanelRenderResolution, DockHostRenderSession},
    render::DockViewportHostSceneFrameSlot,
};
use open_gpui::{
    AnyElement, AppContext as _, Bounds, Context, DragMoveEvent, Empty, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Pixels, StatefulInteractiveElement, Styled, Window,
    black, div, px, rgb, white,
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
        _viewport_host_scene_frame: &DockViewportHostSceneFrameSlot,
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
        let stack_title = if items.len() == 1 {
            session.panel_title(&selected_item)
        } else {
            format!("{} tabs", items.len())
        };
        let mut stack_payload =
            DockDragPayload::new_tabs(session.space().clone(), node, stack_title);
        if let Some(preview_titles) = session.multi_preview_tab_titles_for_node(node) {
            stack_payload = stack_payload.with_preview_tabs(preview_titles);
        }
        let tab_count = items.len();
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
            .border_color(rgb(0xd8dde6))
            .bg(white())
            .capture_any_mouse_down(move |event, _window, cx| {
                if event.button != MouseButton::Left {
                    return;
                }
                anchor_entity.update(cx, |host, _| {
                    host.record_payload_drag_anchor_from_render(
                        anchor_space.clone(),
                        node,
                        event.position,
                    );
                });
            })
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DockDragPayload>, window, cx| {
                    let payload = event.drag(cx).clone();
                    if payload.source_space == source_space && payload.source_node == node {
                        let cursor_position = this
                            .payload_drag_anchor_position_from_render(&payload)
                            .unwrap_or(event.event.position);
                        let mut geometry = DockDragTearOffGeometry::from_source_bounds(
                            event.bounds,
                            cursor_position,
                        )
                        .with_preferred_size(event.bounds.size);
                        if let Some(display) = window.display(cx) {
                            geometry = geometry.with_display_work_area(display.visible_bounds());
                        }
                        this.update_payload_drag_tear_off_geometry_from_render(&payload, geometry);
                    }
                    if event.bounds.contains(&event.event.position)
                        && let Some(drop_root) = drop_root
                    {
                        let fact = drop_scene_fact::leaf(drop_root, node, event.bounds, is_central);
                        this.update_local_drop_scene_fact_from_render(
                            &payload,
                            fact,
                            event.event.position,
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
        let tab_bar_a11y = DockAccessibilityScene::tab_list_element_for_render(node, tab_count);
        let mut tab_bar = div()
            .id(tab_bar_a11y.id_str().to_string())
            .flex()
            .flex_row()
            .flex_none()
            .overflow_hidden()
            .bg(rgb(0xe7ebf0))
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DockDragPayload>, window, cx| {
                    if !event.bounds.contains(&event.event.position) {
                        return;
                    }
                    let payload = event.drag(cx).clone();
                    let position = event.event.position;
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
                            drop_scene_fact::tab_bar(node, tab_count, event.bounds, is_central)
                        });
                    this.update_local_drop_scene_fact_from_render(
                        &payload,
                        fact,
                        event.event.position,
                        window,
                        cx,
                    );
                },
            ))
            .on_drag(
                stack_payload,
                move |payload, position, source_bounds, window, cx| {
                    stack_drag_entity.update(cx, |host, cx| {
                        host.focus_host_for_drag_from_render(window, cx);
                        host.begin_payload_drag_from_render(payload, cx);
                        let cursor_position = host
                            .payload_drag_anchor_position_from_render(payload)
                            .unwrap_or(source_bounds.origin + position);
                        let source_bounds = host
                            .viewport_runtime()
                            .rendered_leaf_bounds_for_tabs(
                                host.space(),
                                Some(window.window_handle().window_id()),
                                node,
                            )
                            .unwrap_or(source_bounds);
                        host.update_payload_drag_tear_off_geometry_from_render(
                            payload,
                            DockDragTearOffGeometry::from_source_bounds(
                                source_bounds,
                                cursor_position,
                            )
                            .with_preferred_size(source_bounds.size),
                        );
                    });
                    cx.new(|_| Empty)
                },
            );
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
            let tab_a11y = DockAccessibilityScene::tab_element_for_render(
                node,
                item.clone(),
                title.clone(),
                index == selected,
                index,
            );
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
                .border_color(if index == selected {
                    rgb(0x4b5563)
                } else {
                    rgb(0xd8dde6)
                })
                .bg(if index == selected {
                    white()
                } else {
                    rgb(0xf0f3f7).into()
                })
                .cursor_pointer()
                .text_color(if index == selected {
                    black()
                } else {
                    rgb(0x657083).into()
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.select_tab_from_render(node, tab_item.clone(), cx);
                }))
                .on_a11y_action(
                    gpui_accessible_action_from_ui(AccessibleAction::Focus),
                    move |_, _, cx| {
                        focus_entity.update(cx, |host, cx| {
                            host.select_tab_from_render(node, focus_item.clone(), cx);
                        });
                    },
                )
                .on_drag_move(cx.listener(
                    move |this, event: &DragMoveEvent<DockDragPayload>, window, cx| {
                        if !event.bounds.contains(&event.event.position) {
                            return;
                        }
                        let payload = event.drag(cx).clone();
                        // The tabs leaf owns tear-off sizing; the tab label is only a drop target.
                        let fact = drop_scene_fact::tab_label(
                            node,
                            target_index,
                            event.bounds,
                            is_central,
                        );
                        this.update_local_drop_scene_fact_from_render(
                            &payload,
                            fact,
                            event.event.position,
                            window,
                            cx,
                        );
                    },
                ))
                .on_drag(
                    payload,
                    move |payload, position, source_bounds, window, cx| {
                        drag_entity.update(cx, |host, cx| {
                            host.focus_host_for_drag_from_render(window, cx);
                            host.begin_tab_item_drag_from_render(
                                node,
                                drag_item.clone(),
                                payload,
                                cx,
                            );
                            let cursor_position = host
                                .payload_drag_anchor_position_from_render(payload)
                                .unwrap_or(source_bounds.origin + position);
                            let source_bounds = host
                                .viewport_runtime()
                                .rendered_leaf_bounds_for_tabs(
                                    host.space(),
                                    Some(window.window_handle().window_id()),
                                    node,
                                )
                                .unwrap_or(source_bounds);
                            host.update_payload_drag_tear_off_geometry_from_render(
                                payload,
                                DockDragTearOffGeometry::from_source_bounds(
                                    source_bounds,
                                    cursor_position,
                                )
                                .with_preferred_size(source_bounds.size),
                            );
                        });
                        cx.new(|_| Empty)
                    },
                );
            tab = tab_a11y.apply_to(tab);
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
                    .border_color(rgb(0xcbd5e1))
                    .bg(rgb(0xf8fafc))
                    .text_color(rgb(0x475569))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.close_item_from_render(close_item.clone(), cx);
                        cx.stop_propagation();
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
                    .border_color(rgb(0xf59e0b))
                    .text_color(rgb(0x92400e))
                    .child(format!("{}: {}", prefix, missing))
                    .into_any_element()
            }
        }
    }
}
