use crate::{
    DockHost, DockNodeId, SplitAxis, debug::DockDebugRegion, geometry::DockSplitLayout,
    host_render_session::DockHostRenderSession, render::DockViewportHostSceneFrameSlot,
};
use open_gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Styled, canvas, div, px, relative, rgb,
};

impl DockHost {
    pub(crate) fn render_split(
        &mut self,
        node: DockNodeId,
        axis: SplitAxis,
        children: Vec<DockNodeId>,
        fractions: Vec<f32>,
        session: &DockHostRenderSession,
        viewport_host_scene_frame: Option<&DockViewportHostSceneFrameSlot>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if children.is_empty() {
            return self.render_missing_node(node, session);
        }

        let selector = self.record_debug_selector(
            DockDebugRegion::Split { node },
            format!("{}:split:{}", session.selector_prefix(), node.as_u64()),
        );
        let layout = DockSplitLayout::from_fractions(
            children.len(),
            &fractions,
            session.central_child_index(&children),
        );
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
                    session.selector_prefix(),
                    node.as_u64(),
                    index
                ),
            );
            let share = layout.child_share(index).unwrap_or(1.0);
            split = split.child(
                div()
                    .id(selector.clone())
                    .debug_selector(move || selector)
                    .flex()
                    .flex_grow(share)
                    .flex_shrink_1()
                    .flex_basis(relative(0.0))
                    .overflow_hidden()
                    .child(self.render_node(child, session, viewport_host_scene_frame, cx)),
            );
        }

        let handles = layout.handles();
        if !handles.is_empty() {
            let handle_size = session.splitter_handle_size();
            let handle_offset = -handle_size / 2.0;
            for handle_layout in &handles {
                let selector = self.record_debug_selector(
                    DockDebugRegion::SplitterHandle {
                        split: node,
                        index: handle_layout.index,
                    },
                    format!(
                        "{}:split:{}:handle:{}",
                        session.selector_prefix(),
                        node.as_u64(),
                        handle_layout.index
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
                        .left(relative(handle_layout.center_share))
                        .top(px(0.0))
                        .ml(handle_offset)
                        .h_full()
                        .w(handle_size),
                    SplitAxis::Vertical => handle
                        .top(relative(handle_layout.center_share))
                        .left(px(0.0))
                        .mt(handle_offset)
                        .w_full()
                        .h(handle_size),
                };

                split = split.child(handle);
            }
        }

        split = split.child(self.render_splitter_event_layer(
            node,
            axis,
            layout,
            session.splitter_handle_size(),
            cx,
        ));

        split.into_any_element()
    }

    fn render_splitter_event_layer(
        &self,
        node: DockNodeId,
        axis: SplitAxis,
        layout: DockSplitLayout,
        handle_size: Pixels,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entity = cx.entity();

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
                            let layout = layout.clone();
                            move |event: &MouseDownEvent, _, _, app| {
                                if event.button != MouseButton::Left {
                                    return;
                                }

                                let geometry = layout.geometry(axis, split_bounds, handle_size);
                                let Some(handle_index) = geometry
                                    .handle_hit_bounds
                                    .iter()
                                    .position(|bounds| bounds.contains(&event.position))
                                else {
                                    return;
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
                                        geometry.extent,
                                        geometry.shares.clone(),
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
}
