use crate::{
    DockAction, DockFloatingContainer, DockHost, DockItemId, DockNode, DockNodeId,
    DockPanelResolution, DropZone, SplitAxis,
    debug::DockDebugRegion,
    drag::{DockTabDragPayload, DockTabDragPreview},
    drop_target::{self, DockDropResolution},
    geometry, splitter,
};
use open_gpui::{
    AnyElement, AnyView, AppContext as _, Context, DragMoveEvent, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, black, canvas, div, px, relative, rgb, rgba, white,
};

enum PanelRenderResolution {
    Registered(AnyView),
    Missing { prefix: String, item: DockItemId },
}

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

        if let Some(root) =
            self.with_workspace(cx, |workspace| workspace.graph().root(self.space()))
        {
            host = host.child(self.render_node(root, cx));
        } else {
            host = host.child(self.render_empty_space(cx));
        }

        let floatings = self.with_workspace(cx, |workspace| {
            workspace.graph().floating_containers(self.space()).to_vec()
        });
        for floating in floatings {
            host = host.child(self.render_floating_container(floating, cx));
        }

        host
    }
}

impl DockHost {
    fn render_node(&mut self, node_id: DockNodeId, cx: &mut Context<Self>) -> AnyElement {
        let Some(node) =
            self.with_workspace(cx, |workspace| workspace.graph().node(node_id).cloned())
        else {
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
        let message =
            self.with_workspace(cx, |workspace| workspace.options().empty_message.clone());
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
        let child = self.with_workspace(cx, |workspace| {
            match workspace.graph().node(container.node).cloned() {
                Some(DockNode::Floating { child }) => Some(child),
                _ => None,
            }
        });
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
                                    if !host.with_workspace(cx, |workspace| {
                                        workspace.policy().allows_floating()
                                    }) {
                                        return;
                                    }
                                    host.start_floating_drag(
                                        space.clone(),
                                        floating,
                                        event.position,
                                        bounds,
                                    );
                                    let action = DockAction::RaiseFloating {
                                        space: space.clone(),
                                        floating,
                                    };
                                    if let Ok(outcome) = host.apply_action_from_host(&action, cx)
                                        && outcome.changed()
                                    {
                                        cx.notify();
                                    }
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
                                    if host.update_floating_drag(event.position, cx) {
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
                                host.finish_floating_drag();
                                cx.notify();
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
        let node = self.with_workspace(cx, |workspace| workspace.graph().node(node).cloned());
        match node {
            Some(DockNode::Tabs { items, active }) => {
                let Some(item) = items.get(active.min(items.len().saturating_sub(1))) else {
                    return "Floating".to_string();
                };
                self.with_workspace(cx, |workspace| match workspace.panels().resolve(item) {
                    DockPanelResolution::Registered(panel) => panel.title().to_string(),
                    DockPanelResolution::Missing { item } => item.to_string(),
                })
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
            let handle_size =
                self.with_workspace(cx, |workspace| workspace.options().splitter_handle_size);
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
        let handle_size =
            self.with_workspace(cx, |workspace| workspace.options().splitter_handle_size);

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
                                    if host.update_splitter_drag(position, cx) {
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
                    let mut intent = drop_target::resolve_tabs_drop(
                        node,
                        event.bounds,
                        event.event.position,
                        &this.with_workspace(cx, |workspace| *workspace.policy()),
                    )
                    .and_then(DockDropResolution::intent);
                    if let Some(existing) = this.tab_drop_intent()
                        && existing.target_tabs == node
                        && existing.insert_index.is_some()
                        && existing.preview_bounds.contains(&event.event.position)
                        && intent.as_ref().is_some_and(|intent| {
                            intent.target_tabs == node && intent.zone == DropZone::Center
                        })
                    {
                        intent = Some(existing);
                    }
                    if this.tab_drop_intent() != intent {
                        this.set_tab_drop_intent(intent);
                        cx.notify();
                    }
                },
            ))
            .on_drop(
                cx.listener(move |this, payload: &DockTabDragPayload, _window, cx| {
                    let Some(intent) = this
                        .tab_drop_intent()
                        .filter(|intent| intent.target_tabs == node)
                    else {
                        this.clear_tab_drop_intent();
                        cx.notify();
                        return;
                    };

                    let action = DockAction::MoveTab {
                        source_space: payload.source_space.clone(),
                        source_tabs: payload.source_tabs,
                        item: payload.item.clone(),
                        target_space: target_space.clone(),
                        target_tabs: intent.target_tabs,
                        zone: intent.zone,
                        insert_index: intent.insert_index,
                    };
                    let changed = this
                        .apply_action_from_host(&action, cx)
                        .map(|outcome| outcome.changed())
                        .unwrap_or(false);
                    this.clear_tab_drop_intent();
                    if changed {
                        cx.notify();
                    }
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
            let title =
                self.with_workspace(cx, |workspace| match workspace.panels().resolve(&item) {
                    DockPanelResolution::Registered(panel) => panel.title().to_string(),
                    DockPanelResolution::Missing { item } => item.to_string(),
                });
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
            let payload =
                DockTabDragPayload::new(self.space().clone(), node, item.clone(), title.clone());
            let target_index = index;
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
                    if let Ok(outcome) = this.apply_action_from_host(&action, cx)
                        && outcome.changed()
                    {
                        cx.notify();
                    }
                }))
                .on_drag_move(cx.listener(
                    move |this, event: &DragMoveEvent<DockTabDragPayload>, _, cx| {
                        let Some(resolution) = drop_target::resolve_tab_reorder_drop(
                            node,
                            target_index,
                            event.bounds,
                            event.event.position,
                            &this.with_workspace(cx, |workspace| *workspace.policy()),
                        ) else {
                            return;
                        };
                        let intent = resolution.intent();
                        if this.tab_drop_intent() != intent {
                            this.set_tab_drop_intent(intent);
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
        let intent = self
            .tab_drop_intent()
            .filter(|intent| intent.target_tabs == node)?;
        if intent.insert_index.is_some() {
            return None;
        }
        let bounds = intent.preview_bounds;
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
        let resolution = if let Some(panel) =
            self.with_workspace(cx, |workspace| workspace.panels().get(item).cloned())
        {
            PanelRenderResolution::Registered(panel.resolve_view(cx))
        } else {
            self.with_workspace(cx, |workspace| PanelRenderResolution::Missing {
                prefix: workspace.options().missing_panel_prefix.clone(),
                item: item.clone(),
            })
        };
        match resolution {
            PanelRenderResolution::Registered(panel_view) => {
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
            PanelRenderResolution::Missing { prefix, item } => {
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
