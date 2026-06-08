use crate::{
    DockHost, DockItemId, DockNode, DockNodeId,
    debug::DockDebugRegion,
    drag::{DockTabDragPayload, DockTabDragPreview},
    host_render_session::{DockHostPanelRenderResolution, DockHostRenderSession},
};
use open_gpui::{
    AnyElement, AppContext as _, Context, DragMoveEvent, InteractiveElement, IntoElement,
    ParentElement, Render, StatefulInteractiveElement, Styled, Window, black, div, rgb, rgba,
    white,
};

impl Render for DockHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.clear_debug_selectors();
        let session = self.render_session(cx);

        let selector = self.record_debug_selector(
            DockDebugRegion::Host,
            format!("{}:host", session.selector_prefix()),
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

        if let Some(root) = session.root() {
            host = host.child(self.render_node(root, &session, cx));
        } else {
            host = host.child(self.render_empty_space(&session));
        }

        for floating in session.floating_containers() {
            host = host.child(self.render_floating_container(*floating, &session, cx));
        }

        host
    }
}

impl DockHost {
    pub(crate) fn render_node(
        &mut self,
        node_id: DockNodeId,
        session: &DockHostRenderSession,
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
            } => self.render_split(node_id, axis, children, fractions, session, cx),
            DockNode::Tabs { items, active } => {
                self.render_tabs(node_id, items, active, session, cx)
            }
            DockNode::Floating { child } => self.render_floating_node(node_id, child, session, cx),
        }
    }

    fn render_empty_space(&mut self, session: &DockHostRenderSession) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::EmptySpace,
            format!("{}:empty", session.selector_prefix()),
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
            .child(session.empty_message().to_string())
            .into_any_element()
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
            .border_color(rgb(0xb42318))
            .text_color(rgb(0xb42318))
            .child(format!("Missing dock node: {}", node.as_u64()))
            .into_any_element()
    }

    fn render_tabs(
        &mut self,
        node: DockNodeId,
        items: Vec<DockItemId>,
        active: usize,
        session: &DockHostRenderSession,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if items.is_empty() {
            return self.render_missing_node(node, session);
        }

        let selector = self.record_debug_selector(
            DockDebugRegion::Tabs { node },
            format!("{}:tabs:{}", session.selector_prefix(), node.as_u64()),
        );
        let active = active.min(items.len().saturating_sub(1));
        let active_item = items[active].clone();
        let target_space = session.space().clone();

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
                session.selector_prefix(),
                node.as_u64()
            ))
            .flex()
            .flex_row()
            .flex_none()
            .overflow_hidden()
            .bg(rgb(0xe7ebf0));

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
            let payload =
                DockTabDragPayload::new(session.space().clone(), node, item.clone(), title.clone());
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
        tabs = tabs.child(self.render_panel(&active_item, session, cx));
        if let Some(preview) = self.render_drop_preview(node, session) {
            tabs = tabs.child(preview);
        }
        tabs.into_any_element()
    }

    fn render_drop_preview(
        &mut self,
        node: DockNodeId,
        session: &DockHostRenderSession,
    ) -> Option<AnyElement> {
        let bounds = self.tab_drop_preview_bounds(node)?;
        let selector = self.record_debug_selector(
            DockDebugRegion::DropPreview { tabs: node },
            format!(
                "{}:tabs:{}:drop-preview",
                session.selector_prefix(),
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

    fn render_panel(
        &mut self,
        item: &DockItemId,
        session: &DockHostRenderSession,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let resolution = session.panel_for_render(item, cx);
        match resolution {
            DockHostPanelRenderResolution::Registered(panel_view) => {
                let selector = self.record_debug_selector(
                    DockDebugRegion::Panel { item: item.clone() },
                    format!("{}:panel:{}", session.selector_prefix(), item),
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
