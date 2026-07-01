use crate::{
    DockHost, DockNodeId, SplitAxis,
    accessibility_scene::{DockAccessibilityScene, gpui_accessible_action_from_ui},
    debug::DockDebugRegion,
    host_render_session::DockHostRenderSession,
    render::DockViewportHostSceneFrameSlot,
};
use open_gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px, relative, rgb,
};
use open_gpui_ui_core::{AccessibleAction, resolve_split_fractions_with_fill_child};

pub(crate) struct DockRenderSplitInput {
    node: DockNodeId,
    axis: SplitAxis,
    children: Vec<DockNodeId>,
    fractions: Vec<f32>,
}

impl DockRenderSplitInput {
    pub(crate) fn new(
        node: DockNodeId,
        axis: SplitAxis,
        children: Vec<DockNodeId>,
        fractions: Vec<f32>,
    ) -> Self {
        Self {
            node,
            axis,
            children,
            fractions,
        }
    }
}

impl DockHost {
    pub(crate) fn render_split(
        &mut self,
        input: DockRenderSplitInput,
        session: &DockHostRenderSession,
        viewport_host_scene_frame: &DockViewportHostSceneFrameSlot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let DockRenderSplitInput {
            node,
            axis,
            children,
            fractions,
        } = input;

        if children.is_empty() {
            return self.render_missing_node(node, session);
        }

        let selector = self.record_debug_selector(
            DockDebugRegion::Split { node },
            format!("{}:split:{}", session.selector_prefix(), node.as_u64()),
        );
        let shares = resolve_split_fractions_with_fill_child(
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
                    .child(self.render_node(child, session, viewport_host_scene_frame, window, cx)),
            );
        }

        if shares.len() >= 2 {
            let handle_size = session.splitter_handle_size();
            let handle_offset = -handle_size / 2.0;
            let mut handle_center_share = 0.0_f32;
            for (handle_index, share) in shares.iter().take(shares.len() - 1).enumerate() {
                handle_center_share += *share;
                let selector = self.record_debug_selector(
                    DockDebugRegion::SplitterHandle {
                        split: node,
                        index: handle_index,
                    },
                    format!(
                        "{}:split:{}:handle:{}",
                        session.selector_prefix(),
                        node.as_u64(),
                        handle_index
                    ),
                );
                let accessible = DockAccessibilityScene::splitter_element_for_render(
                    node,
                    axis,
                    handle_index,
                    handle_center_share,
                );
                let increment_entity = cx.entity();
                let decrement_entity = cx.entity();
                let mut handle = div()
                    .id(accessible.id_str().to_string())
                    .debug_selector(move || selector)
                    .absolute()
                    .bg(rgb(0xc8d0dc))
                    .hover(|this| this.bg(rgb(0x94a3b8)))
                    .cursor_pointer()
                    .on_a11y_action(
                        gpui_accessible_action_from_ui(AccessibleAction::Increment),
                        move |_, _, cx| {
                            increment_entity.update(cx, |host, cx| {
                                host.resize_splitter_from_accessibility(
                                    node,
                                    axis,
                                    handle_index,
                                    AccessibleAction::Increment,
                                    cx,
                                );
                            });
                        },
                    )
                    .on_a11y_action(
                        gpui_accessible_action_from_ui(AccessibleAction::Decrement),
                        move |_, _, cx| {
                            decrement_entity.update(cx, |host, cx| {
                                host.resize_splitter_from_accessibility(
                                    node,
                                    axis,
                                    handle_index,
                                    AccessibleAction::Decrement,
                                    cx,
                                );
                            });
                        },
                    );
                handle = accessible.apply_to(handle);

                handle = match axis {
                    SplitAxis::Horizontal => handle
                        .left(relative(handle_center_share))
                        .top(px(0.0))
                        .ml(handle_offset)
                        .h_full()
                        .w(handle_size),
                    SplitAxis::Vertical => handle
                        .top(relative(handle_center_share))
                        .left(px(0.0))
                        .mt(handle_offset)
                        .w_full()
                        .h(handle_size),
                };

                split = split.child(handle);
            }
        }

        split.into_any_element()
    }
}
