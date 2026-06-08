use crate::{
    DockFloatingContainer, DockHost, DockItemId, DockNode, DockNodeId, SplitAxis,
    debug::DockDebugRegion,
    drag::{DockTabDragPayload, DockTabDragPreview},
    geometry,
    host_render_session::DockHostPanelRenderResolution,
    splitter,
};
use open_gpui::{
    AnyElement, AppContext as _, Context, DragMoveEvent, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, black, canvas, div, px, relative, rgb, rgba, white,
};

impl Render for DockHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.clear_debug_selectors();

        let selector = self.record_debug_selector(
            DockDebugRegion::Host,
            format!("{}:host", self.selector_prefix()),
        );

        let mut host = div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .bg(rgb(0xf7f8fa))
            .text_color(black());

        if let Some(root) = self.root_for_render(cx) {
            host = host.child(self.render_node(root, cx));
        } else {
            host = host.child(self.render_empty_space(cx));
        }

        let floatings = self.floating_containers_for_render(cx);
        for floating in floatings {
            host = host.child(self.render_floating_container(floating, cx));
        }

        host
    }
}

impl DockHost {
    fn render_node(&mut self, node_id: DockNodeId, cx: &mut Context<Self>) -> AnyElement {
        let Some(node) = self.node_for_render(node_id, cx) else {
            return self.render_missing_node(node_id);
        };

        match node {
            DockNode::Split {
                axis,
                children,
                fractions,
            } => self.render_split(node_id, axis, children, fractions, cx),
            DockNode::Tabs { items, active } => self.render_tabs(node_id, items, active, cx),
            DockNode::Floating { child } => self.render_floating_node(node_id, child, cx),
        }
    }

    fn render_empty_space(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::EmptySpace,
            format!("{}:empty", self.selector_prefix()),
        );
        let message = self.empty_message_for_render(cx);
        div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .flex()
            .size_full()
            .items_center()
            .justify_center()
            .border_1()
            .border_color(rgb(0xd8dde6))
            .text_color(rgb(0x657083))
            .child(message)
            .into_any_element()
    }

    fn render_missing_node(&mut self, node: DockNodeId) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::MissingNode { node },
            format!("{}:missing-node:{}", self.selector_prefix(), node.as_u64()),
        );
        div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .flex()
            .size_full()
            .items_center()
            .justify_center()
            .border_1()
            .border_color(rgb(0xb42318))
            .text_color(rgb(0xb42318))
            .child(format!("Missing dock node: {}", node.as_u64()))
            .into_any_element()
    }

    fn render_floating_node(
        &mut self,
        node: DockNodeId,
        child: DockNodeId,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::Floating { node },
            format!("{}:floating:{}", self.selector_prefix(), node.as_u64()),
        );
        div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .child(self.render_node(child, cx))
            .into_any_element()
    }

    fn render_floating_container(
        &mut self,
        container: DockFloatingContainer,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::Floating {
                node: container.node,
            },
            format!(
                "{}:floating:{}",
                self.selector_prefix(),
                container.node.as_u64()
            ),
        );
        let child = match self.node_for_render(container.node, cx) {
            Some(DockNode::Floating { child }) => Some(child),
            _ => None,
        };
        let bounds = container.bounds;
        let content = child
            .map(|child| self.render_node(child, cx))
            .unwrap_or_else(|| self.render_missing_node(container.node));
        let title = child
            .map(|child| self.floating_title(child, cx))
            .unwrap_or_else(|| "Floating".to_string());

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
            .border_1()
            .border_color(rgb(0x4b5563))
            .bg(white())
            .shadow_md()
            .child(self.render_floating_handle(container, title, cx))
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
        title: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::FloatingHandle {
                node: container.node,
            },
            format!(
                "{}:floating:{}:handle",
                self.selector_prefix(),
                container.node.as_u64()
            ),
        );
        let space = self.space().clone();
        let floating = container.node;
        let bounds = container.bounds;
        let entity = cx.entity();

        div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .relative()
            .flex()
            .flex_none()
            .h(px(24.0))
            .items_center()
            .px_2()
            .bg(rgb(0xe7ebf0))
            .border_b_1()
            .border_color(rgb(0xd8dde6))
            .text_color(rgb(0x4b5563))
            .text_sm()
            .cursor_pointer()
            .child(title)
            .child(
                canvas(
                    |_, _, _| (),
                    move |handle_bounds, _, window, _| {
                        window.on_mouse_event({
                            let entity = entity.clone();
                            let space = space.clone();
                            move |event: &MouseDownEvent, _, _, app| {
                                if event.button != MouseButton::Left
                                    || !handle_bounds.contains(&event.position)
                                {
                                    return;
                                }

                                entity.update(app, |host, cx| {
                                    host.begin_floating_drag_from_render(
                                        space.clone(),
                                        floating,
                                        event.position,
                                        bounds,
                                        cx,
                                    );
                                });
                                app.stop_propagation();
                            }
                        });

                        window.on_mouse_event({
                            let entity = entity.clone();
                            move |event: &MouseMoveEvent, _, _, app| {
                                if event.pressed_button != Some(MouseButton::Left) {
                                    return;
                                }

                                entity.update(app, |host, cx| {
                                    host.update_floating_drag_from_render(event.position, cx);
                                });
                            }
                        });

                        window.on_mouse_event(move |event: &MouseUpEvent, _, _, app| {
                            if event.button != MouseButton::Left {
                                return;
                            }

                            entity.update(app, |host, cx| {
                                host.finish_floating_drag_from_render(cx);
                            });
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

    fn floating_title(&self, node: DockNodeId, cx: &Context<Self>) -> String {
        let node = self.node_for_render(node, cx);
        match node {
            Some(DockNode::Tabs { items, active }) => {
                let Some(item) = items.get(active.min(items.len().saturating_sub(1))) else {
                    return "Floating".to_string();
                };
                self.panel_title_for_render(item, cx)
            }
            Some(DockNode::Split { children, .. }) => children
                .first()
                .map(|child| self.floating_title(*child, cx))
                .unwrap_or_else(|| "Floating".to_string()),
            Some(DockNode::Floating { child }) => self.floating_title(child, cx),
            None => "Floating".to_string(),
        }
    }

    fn render_split(
        &mut self,
        node: DockNodeId,
        axis: SplitAxis,
        children: Vec<DockNodeId>,
        fractions: Vec<f32>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if children.is_empty() {
            return self.render_missing_node(node);
        }

        let selector = self.record_debug_selector(
            DockDebugRegion::Split { node },
            format!("{}:split:{}", self.selector_prefix(), node.as_u64()),
        );
        let shares = splitter::cleaned_shares(children.len(), &fractions);
        let mut split = div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .relative()
            .flex()
            .size_full()
            .overflow_hidden();

        split = match axis {
            SplitAxis::Horizontal => split.flex_row(),
            SplitAxis::Vertical => split.flex_col(),
        };

        for (index, child) in children.into_iter().enumerate() {
            let selector = self.record_debug_selector(
                DockDebugRegion::SplitChild { split: node, index },
                format!(
                    "{}:split:{}:child:{}",
                    self.selector_prefix(),
                    node.as_u64(),
                    index
                ),
            );
            let share = shares.get(index).copied().unwrap_or(1.0);
            split = split.child(
                div()
                    .id(selector.clone())
                    .debug_selector(move || selector)
                    .flex()
                    .flex_grow(share)
                    .flex_shrink_1()
                    .flex_basis(relative(0.0))
                    .overflow_hidden()
                    .child(self.render_node(child, cx)),
            );
        }

        if shares.len() >= 2 {
            let handle_size = self.splitter_handle_size_for_render(cx);
            let handle_offset = -handle_size / 2.0;
            let mut cursor = 0.0_f32;

            for (index, share) in shares
                .iter()
                .take(shares.len().saturating_sub(1))
                .enumerate()
            {
                cursor += *share;
                let selector = self.record_debug_selector(
                    DockDebugRegion::SplitterHandle { split: node, index },
                    format!(
                        "{}:split:{}:handle:{}",
                        self.selector_prefix(),
                        node.as_u64(),
                        index
                    ),
                );
                let mut handle = div()
                    .id(selector.clone())
                    .debug_selector(move || selector)
                    .absolute()
                    .bg(rgb(0xc8d0dc))
                    .hover(|this| this.bg(rgb(0x94a3b8)))
                    .cursor_pointer();

                handle = match axis {
                    SplitAxis::Horizontal => handle
                        .left(relative(cursor))
                        .top(px(0.0))
                        .ml(handle_offset)
                        .h_full()
                        .w(handle_size),
                    SplitAxis::Vertical => handle
                        .top(relative(cursor))
                        .left(px(0.0))
                        .mt(handle_offset)
                        .w_full()
                        .h(handle_size),
                };

                split = split.child(handle);
            }
        }

        split = split.child(self.render_splitter_event_layer(node, axis, shares, cx));

        split.into_any_element()
    }

    fn render_splitter_event_layer(
        &self,
        node: DockNodeId,
        axis: SplitAxis,
        shares: Vec<f32>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entity = cx.entity();
        let handle_size = self.splitter_handle_size_for_render(cx);

        div()
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .size_full()
            .child(
                canvas(
                    |_, _, _| (),
                    move |split_bounds, _, window, _| {
                        window.on_mouse_event({
                            let entity = entity.clone();
                            let shares = shares.clone();
                            move |event: &MouseDownEvent, _, _, app| {
                                if event.button != MouseButton::Left {
                                    return;
                                }

                                let handles = geometry::splitter_handle_bounds(
                                    axis,
                                    split_bounds,
                                    &shares,
                                    handle_size,
                                );
                                let Some(handle_index) = handles
                                    .iter()
                                    .position(|bounds| bounds.contains(&event.position))
                                else {
                                    return;
                                };

                                let split_extent = match axis {
                                    SplitAxis::Horizontal => split_bounds.size.width,
                                    SplitAxis::Vertical => split_bounds.size.height,
                                };
                                let start_position = match axis {
                                    SplitAxis::Horizontal => event.position.x,
                                    SplitAxis::Vertical => event.position.y,
                                };

                                entity.update(app, |host, cx| {
                                    host.begin_splitter_drag_from_render(
                                        node,
                                        handle_index,
                                        start_position,
                                        split_extent,
                                        shares.clone(),
                                        cx,
                                    );
                                });
                                app.stop_propagation();
                            }
                        });

                        window.on_mouse_event({
                            let entity = entity.clone();
                            move |event: &MouseMoveEvent, _, _, app| {
                                if event.pressed_button != Some(MouseButton::Left) {
                                    return;
                                }

                                let position = match axis {
                                    SplitAxis::Horizontal => event.position.x,
                                    SplitAxis::Vertical => event.position.y,
                                };
                                entity.update(app, |host, cx| {
                                    host.update_splitter_drag_from_render(position, cx);
                                });
                            }
                        });

                        window.on_mouse_event(move |event: &MouseUpEvent, _, _, app| {
                            if event.button != MouseButton::Left {
                                return;
                            }

                            entity.update(app, |host, cx| {
                                host.finish_splitter_drag_from_render(cx);
                            });
                        });
                    },
                )
                .size_full(),
            )
            .into_any_element()
    }

    fn render_tabs(
        &mut self,
        node: DockNodeId,
        items: Vec<DockItemId>,
        active: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if items.is_empty() {
            return self.render_missing_node(node);
        }

        let selector = self.record_debug_selector(
            DockDebugRegion::Tabs { node },
            format!("{}:tabs:{}", self.selector_prefix(), node.as_u64()),
        );
        let active = active.min(items.len().saturating_sub(1));
        let active_item = items[active].clone();
        let target_space = self.space().clone();

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
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DockTabDragPayload>, _, cx| {
                    if this.update_tabs_drop_intent(node, event.bounds, event.event.position, cx) {
                        cx.notify();
                    }
                },
            ))
            .on_drop(
                cx.listener(move |this, payload: &DockTabDragPayload, _window, cx| {
                    this.drop_tab_from_render(
                        payload.source_space.clone(),
                        payload.source_tabs,
                        payload.item.clone(),
                        target_space.clone(),
                        node,
                        cx,
                    );
                }),
            );

        let mut tab_bar = div()
            .id(format!(
                "{}:tabs:{}:bar",
                self.selector_prefix(),
                node.as_u64()
            ))
            .flex()
            .flex_row()
            .flex_none()
            .overflow_hidden()
            .bg(rgb(0xe7ebf0));

        for (index, item) in items.into_iter().enumerate() {
            let title = self.panel_title_for_render(&item, cx);
            let selector = self.record_debug_selector(
                DockDebugRegion::Tab {
                    tabs: node,
                    item: item.clone(),
                },
                format!(
                    "{}:tabs:{}:tab:{}",
                    self.selector_prefix(),
                    node.as_u64(),
                    item
                ),
            );
            let payload =
                DockTabDragPayload::new(self.space().clone(), node, item.clone(), title.clone());
            let target_index = index;
            let tab_item = item.clone();
            let tab = div()
                .id(selector.clone())
                .debug_selector(move || selector)
                .flex()
                .flex_none()
                .px_2()
                .py_1()
                .border_1()
                .border_color(if index == active {
                    rgb(0x4b5563)
                } else {
                    rgb(0xd8dde6)
                })
                .bg(if index == active {
                    white()
                } else {
                    rgb(0xf0f3f7).into()
                })
                .cursor_pointer()
                .text_color(if index == active {
                    black()
                } else {
                    rgb(0x657083).into()
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.select_tab_from_render(node, tab_item.clone(), cx);
                }))
                .on_drag_move(cx.listener(
                    move |this, event: &DragMoveEvent<DockTabDragPayload>, _, cx| {
                        if this.update_tab_reorder_drop_intent(
                            node,
                            target_index,
                            event.bounds,
                            event.event.position,
                            cx,
                        ) {
                            cx.notify();
                        }
                    },
                ))
                .on_drag(payload, |payload, _, _, cx| {
                    cx.new(|_| DockTabDragPreview::new(payload.title()))
                })
                .child(title);
            tab_bar = tab_bar.child(tab);
        }

        tabs = tabs.child(tab_bar);
        tabs = tabs.child(self.render_panel(&active_item, cx));
        if let Some(preview) = self.render_drop_preview(node) {
            tabs = tabs.child(preview);
        }
        tabs.into_any_element()
    }

    fn render_drop_preview(&mut self, node: DockNodeId) -> Option<AnyElement> {
        let bounds = self.tab_drop_preview_bounds(node)?;
        let selector = self.record_debug_selector(
            DockDebugRegion::DropPreview { tabs: node },
            format!(
                "{}:tabs:{}:drop-preview",
                self.selector_prefix(),
                node.as_u64()
            ),
        );

        Some(
            div()
                .id(selector.clone())
                .debug_selector(move || selector)
                .absolute()
                .left(bounds.origin.x)
                .top(bounds.origin.y)
                .w(bounds.size.width)
                .h(bounds.size.height)
                .border_1()
                .border_color(rgb(0x2563eb))
                .bg(rgba(0x60a5fa47))
                .into_any_element(),
        )
    }

    fn render_panel(&mut self, item: &DockItemId, cx: &mut Context<Self>) -> AnyElement {
        let resolution = self.panel_for_render(item, cx);
        match resolution {
            DockHostPanelRenderResolution::Registered(panel_view) => {
                let selector = self.record_debug_selector(
                    DockDebugRegion::Panel { item: item.clone() },
                    format!("{}:panel:{}", self.selector_prefix(), item),
                );
                div()
                    .id(selector.clone())
                    .debug_selector(move || selector)
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_hidden()
                    .child(panel_view)
                    .into_any_element()
            }
            DockHostPanelRenderResolution::Missing { prefix, item } => {
                let missing = item;
                let selector = self.record_debug_selector(
                    DockDebugRegion::MissingPanel {
                        item: missing.clone(),
                    },
                    format!("{}:panel:{}:missing", self.selector_prefix(), missing),
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
