use crate::{
    DockHost, DockNode, DockNodeId, debug::DockDebugRegion, drag::DockDragPayload,
    drop_runtime::resolution_target, drop_target::DockDropResolution,
    host_render_session::DockHostRenderSession,
};
use open_gpui::{
    AnyElement, Context, DragMoveEvent, InteractiveElement, IntoElement, MouseButton, MouseUpEvent,
    ParentElement, Render, Styled, Window, black, div, rgb, rgba,
};

impl Render for DockHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.clear_debug_selectors();
        let session = self.render_session(cx);
        self.focus_pending_panel_from_render(&session, window, cx);
        let drop_target_space = session.space().clone();
        let outside_drop_target_space = session.space().clone();

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
            .text_color(black())
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DockDragPayload>, window, cx| {
                    let payload = event.drag(cx).clone();
                    this.begin_host_drop_scene_from_render(
                        &payload,
                        event.bounds,
                        event.event.position,
                        window,
                        cx,
                    );
                },
            ))
            .on_drop(
                cx.listener(move |this, payload: &DockDragPayload, window, cx| {
                    this.drop_payload_from_render(
                        payload,
                        drop_target_space.clone(),
                        window.mouse_position(),
                        window,
                        cx,
                    );
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseUpEvent, window, cx| {
                    if this.viewport_runtime().is_none() {
                        return;
                    }
                    let Some(payload) = cx.active_drag_value::<DockDragPayload>().cloned() else {
                        return;
                    };
                    this.drop_payload_from_render(
                        &payload,
                        outside_drop_target_space.clone(),
                        event.position,
                        window,
                        cx,
                    );
                    cx.stop_active_drag(window);
                    cx.stop_propagation();
                }),
            );

        if session.empty_central_passthrough() {
            host = host.bg(rgba(0x00000000));
        } else {
            host = host.bg(rgb(0xf7f8fa));
        }

        if let Some(root) = session.root() {
            host = host.child(self.render_root_node(root, &session, cx));
        } else if session.empty_central_passthrough() {
            host = host.child(self.render_passthrough_empty_central_space(&session, cx));
        } else {
            host = host.child(self.render_empty_space(&session, cx));
        }

        for floating in session.floating_containers() {
            host = host.child(self.render_floating_container(*floating, &session, cx));
        }

        if let Some(preview) = self.render_host_drop_preview(&session) {
            host = host.child(preview);
        }

        host
    }
}

impl DockHost {
    fn focus_pending_panel_from_render(
        &mut self,
        session: &DockHostRenderSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(item) = self.take_pending_panel_focus() else {
            return;
        };
        session.request_panel_focus(&item, window, cx);
    }

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

    fn render_root_node(
        &mut self,
        root: DockNodeId,
        session: &DockHostRenderSession,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let root_child = self.render_node(root, session, cx);
        div()
            .relative()
            .flex()
            .size_full()
            .overflow_hidden()
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DockDragPayload>, _, cx| {
                    let payload = event.drag(cx).clone();
                    this.update_root_drop_scene_from_render(
                        &payload,
                        root,
                        event.bounds,
                        event.event.position,
                        cx,
                    );
                },
            ))
            .child(root_child)
            .into_any_element()
    }

    fn render_empty_space(
        &mut self,
        session: &DockHostRenderSession,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DockDragPayload>, _, cx| {
                    let payload = event.drag(cx).clone();
                    this.update_empty_space_drop_scene_from_render(
                        &payload,
                        event.event.position,
                        event.bounds,
                        cx,
                    );
                },
            ))
            .child(session.empty_message().to_string())
            .into_any_element()
    }

    fn render_passthrough_empty_central_space(
        &mut self,
        session: &DockHostRenderSession,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::EmptySpace,
            format!("{}:empty-central", session.selector_prefix()),
        );
        div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .flex()
            .size_full()
            .bg(rgba(0x00000000))
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DockDragPayload>, _, cx| {
                    let payload = event.drag(cx).clone();
                    this.update_empty_space_drop_scene_from_render(
                        &payload,
                        event.event.position,
                        event.bounds,
                        cx,
                    );
                },
            ))
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

    fn render_host_drop_preview(&mut self, session: &DockHostRenderSession) -> Option<AnyElement> {
        let resolution = self.interaction().drop_resolution()?;
        let target = resolution_target(resolution)?;
        let bounds = target.preview_bounds?;
        let rejected = matches!(resolution, DockDropResolution::Rejected(_));
        let selector = self.record_debug_selector(
            DockDebugRegion::DropPreview,
            format!("{}:drop-preview", session.selector_prefix()),
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
                .border_color(if rejected {
                    rgb(0xdc2626)
                } else {
                    rgb(0x2563eb)
                })
                .bg(if rejected {
                    rgba(0xfca5a547)
                } else {
                    rgba(0x60a5fa47)
                })
                .into_any_element(),
        )
    }
}
