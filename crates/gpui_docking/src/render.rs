use crate::{
    DockAction, DockHost, DockItemId, DockNode, DockNodeId, DockPanelResolution, SplitAxis,
    debug::DockDebugRegion, splitter,
};
use open_gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Render, StatefulInteractiveElement, Styled,
    Window, black, canvas, div, px, relative, rgb, white,
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
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .bg(rgb(0xf7f8fa))
            .text_color(black());

        if let Some(root) = self.graph().root(self.space()) {
            host = host.child(self.render_node(root, cx));
        } else {
            host = host.child(self.render_empty_space());
        }

        host
    }
}

impl DockHost {
    fn render_node(&mut self, node_id: DockNodeId, cx: &mut Context<Self>) -> AnyElement {
        let Some(node) = self.graph().node(node_id).cloned() else {
            return self.render_missing_node(node_id);
        };

        match node {
            DockNode::Split {
                axis,
                children,
                fractions,
            } => self.render_split(node_id, axis, children, fractions, cx),
            DockNode::Tabs { items, active } => self.render_tabs(node_id, items, active, cx),
            DockNode::Floating { .. } => self.render_deferred_floating(node_id),
        }
    }

    fn render_empty_space(&mut self) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::EmptySpace,
            format!("{}:empty", self.selector_prefix()),
        );
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
            .child(self.options().empty_message.clone())
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

    fn render_deferred_floating(&mut self, node: DockNodeId) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::DeferredFloating { node },
            format!(
                "{}:floating:{}:deferred",
                self.selector_prefix(),
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
            .border_color(rgb(0xd8dde6))
            .text_color(rgb(0x657083))
            .child(self.options().deferred_floating_message.clone())
            .into_any_element()
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
            let handle_size = self.options().splitter_handle_size;
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
        let handle_size = self.options().splitter_handle_size;

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

                                let handles = splitter::handle_bounds(
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
                                    host.start_splitter_drag(
                                        node,
                                        handle_index,
                                        start_position,
                                        split_extent,
                                        shares.clone(),
                                    );
                                    cx.notify();
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
                                    if host.update_splitter_drag(position) {
                                        cx.notify();
                                    }
                                });
                            }
                        });

                        window.on_mouse_event(move |event: &MouseUpEvent, _, _, app| {
                            if event.button != MouseButton::Left {
                                return;
                            }

                            entity.update(app, |host, cx| {
                                host.finish_splitter_drag();
                                cx.notify();
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

        let mut tabs = div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .border_1()
            .border_color(rgb(0xd8dde6))
            .bg(white());

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
            let title = match self.panels().resolve(&item) {
                DockPanelResolution::Registered(panel) => panel.title().to_string(),
                DockPanelResolution::Missing { item } => item.to_string(),
            };
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
            let action = DockAction::SelectTab {
                tabs: node,
                item: item.clone(),
            };
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
                    if let Ok(outcome) = this.apply_action(&action)
                        && outcome.changed()
                    {
                        cx.notify();
                    }
                }))
                .child(title);
            tab_bar = tab_bar.child(tab);
        }

        tabs = tabs.child(tab_bar);
        tabs.child(self.render_panel(&active_item))
            .into_any_element()
    }

    fn render_panel(&mut self, item: &DockItemId) -> AnyElement {
        match self.panels().resolve(item) {
            DockPanelResolution::Registered(panel) => {
                let panel_view = panel.view().clone();
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
            DockPanelResolution::Missing { item } => {
                let missing = item.clone();
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
                    .child(format!(
                        "{}: {}",
                        self.options().missing_panel_prefix,
                        missing
                    ))
                    .into_any_element()
            }
        }
    }
}
