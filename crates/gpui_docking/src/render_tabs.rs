use crate::{
    DockHost, DockItemId, DockNodeId,
    debug::DockDebugRegion,
    drag::{DockDragPayload, DockDragPreview},
    drop_runtime::DockHostDropSceneFact,
    drop_target::{DockLeafDropTarget, DockTabLabelDropTarget},
    host_render_session::{DockHostPanelRenderResolution, DockHostRenderSession},
};
use open_gpui::{
    AnyElement, AppContext as _, Context, DragMoveEvent, InteractiveElement, IntoElement,
    ParentElement, StatefulInteractiveElement, Styled, black, div, px, rgb, white,
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
        let is_central = session.is_central_tabs(node);
        let stack_title = if items.len() == 1 {
            session.panel_title(&active_item)
        } else {
            format!("{} tabs", items.len())
        };
        let stack_payload = DockDragPayload::new_tabs(session.space().clone(), node, stack_title);

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
                move |this, event: &DragMoveEvent<DockDragPayload>, window, cx| {
                    let payload = event.drag(cx).clone();
                    this.update_leaf_drop_scene_from_render(
                        &payload,
                        node,
                        event.bounds,
                        event.event.position,
                        is_central,
                        window,
                        cx,
                    );
                },
            ));
        if let Some(probe) = self.render_viewport_drop_scene_fact_probe(move |bounds| {
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: node,
                target_tabs: node,
                bounds,
                is_central,
            })
        }) {
            tabs = tabs.child(probe);
        }

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
            .bg(rgb(0xe7ebf0))
            .on_drag(stack_payload, |payload, _, _, cx| {
                cx.new(|_| DockDragPreview::new(payload.title()))
            });

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
            let target_index = index;
            let tab_item = item.clone();
            let mut tab = div()
                .id(selector.clone())
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
                    move |this, event: &DragMoveEvent<DockDragPayload>, window, cx| {
                        let payload = event.drag(cx).clone();
                        this.update_tab_label_drop_scene_from_render(
                            &payload,
                            node,
                            target_index,
                            event.bounds,
                            event.event.position,
                            is_central,
                            window,
                            cx,
                        );
                    },
                ))
                .on_drag(payload, |payload, _, _, cx| {
                    cx.new(|_| DockDragPreview::new(payload.title()))
                });
            if let Some(probe) = self.render_viewport_drop_scene_fact_probe(move |bounds| {
                DockHostDropSceneFact::TabLabel(DockTabLabelDropTarget {
                    target_tabs: node,
                    target_index,
                    bounds,
                    is_central,
                })
            }) {
                tab = tab.child(probe);
            }
            tab = tab.child(title);
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
                    .id(close_selector.clone())
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
                    }))
                    .child("x");
                tab = tab.child(close);
            }
            tab_bar = tab_bar.child(tab);
        }

        tabs.child(tab_bar)
            .child(self.render_panel(&active_item, session, cx))
            .into_any_element()
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
