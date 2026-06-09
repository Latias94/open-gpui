use crate::{
    DockHost, DockNode, DockNodeId, debug::DockDebugRegion,
    host_render_session::DockHostRenderSession,
};
use open_gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window,
    black, div, rgb, rgba,
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
            .text_color(black());

        if session.empty_central_passthrough() {
            host = host.bg(rgba(0x00000000));
        } else {
            host = host.bg(rgb(0xf7f8fa));
        }

        if let Some(root) = session.root() {
            host = host.child(self.render_node(root, &session, cx));
        } else if session.empty_central_passthrough() {
            host = host.child(self.render_passthrough_empty_central_space(&session));
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

    fn render_passthrough_empty_central_space(
        &mut self,
        session: &DockHostRenderSession,
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
}
