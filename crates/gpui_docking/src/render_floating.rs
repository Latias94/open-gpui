use crate::{
    DockFloatingContainer, DockHost, DockNodeId, debug::DockDebugRegion, drag::DockTabDragPayload,
    host_render_session::DockHostRenderSession,
};
use open_gpui::{
    AnyElement, Context, DragMoveEvent, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Styled, canvas, div, px, rgb,
    white,
};

impl DockHost {
    pub(crate) fn render_floating_node(
        &mut self,
        node: DockNodeId,
        child: DockNodeId,
        session: &DockHostRenderSession,
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
            .child(self.render_node(child, session, cx))
            .into_any_element()
    }

    pub(crate) fn render_floating_container(
        &mut self,
        container: DockFloatingContainer,
        session: &DockHostRenderSession,
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
        let content = child
            .map(|child| self.render_node(child, session, cx))
            .unwrap_or_else(|| self.render_missing_node(container.node, session));
        let title = child
            .map(|child| session.floating_title(child))
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
            .child(self.render_floating_handle(container, title, session, cx))
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
        session: &DockHostRenderSession,
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
        let target_tabs = session.first_tabs_in_subtree(floating);

        let mut handle = div()
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
            .cursor_pointer();

        if let Some(target_tabs) = target_tabs {
            handle = handle.on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DockTabDragPayload>, _, cx| {
                    this.update_floating_title_bar_drop_scene_from_render(
                        floating,
                        target_tabs,
                        event.bounds,
                        bounds,
                        event.event.position,
                        cx,
                    );
                },
            ));
        }

        handle
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
}
