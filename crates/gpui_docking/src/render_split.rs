use crate::{
    DockHost, DockNodeId, SplitAxis, debug::DockDebugRegion, geometry,
    host_render_session::DockHostRenderSession, split_fraction,
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
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if children.is_empty() {
            return self.render_missing_node(node, session);
        }

        let selector = self.record_debug_selector(
            DockDebugRegion::Split { node },
            format!("{}:split:{}", session.selector_prefix(), node.as_u64()),
        );
        let shares = split_fraction::cleaned_shares(children.len(), &fractions);
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
                    .child(self.render_node(child, session, cx)),
            );
        }

        if shares.len() >= 2 {
            let handle_size = session.splitter_handle_size();
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
                        session.selector_prefix(),
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

        split = split.child(self.render_splitter_event_layer(
            node,
            axis,
            shares,
            session.splitter_handle_size(),
            cx,
        ));

        split.into_any_element()
    }

    fn render_splitter_event_layer(
        &self,
        node: DockNodeId,
        axis: SplitAxis,
        shares: Vec<f32>,
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
}
