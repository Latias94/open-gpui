use crate::{
    DockHost, DockItemId, DockNodeId,
    debug::DockDebugRegion,
    drag::{DockTabDragPayload, DockTabDragPreview},
    host_render_session::{DockHostPanelRenderResolution, DockHostRenderSession},
};
use open_gpui::{
    AnyElement, AppContext as _, Context, DragMoveEvent, InteractiveElement, IntoElement,
    ParentElement, StatefulInteractiveElement, Styled, black, div, rgb, rgba, white,
};

impl DockHost {
    pub(crate) fn render_tabs(
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
        let is_central = session.is_central_tabs(node);

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
                    this.update_tabs_drop_target_from_render(
                        node,
                        event.bounds,
                        event.event.position,
                        is_central,
                        cx,
                    );
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
                        this.update_tab_reorder_drop_target_from_render(
                            node,
                            target_index,
                            event.bounds,
                            event.event.position,
                            is_central,
                            cx,
                        );
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
