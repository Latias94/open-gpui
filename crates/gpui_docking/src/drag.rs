use crate::{DockItemId, DockNodeId, DockSpaceId};
use open_gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div, rgb, white};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockDragPayload {
    pub(crate) source_space: DockSpaceId,
    pub(crate) source_tabs: DockNodeId,
    pub(crate) kind: DockDragPayloadKind,
    title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DockDragPayloadKind {
    Item { item: DockItemId },
    Tabs,
}

impl DockDragPayload {
    pub(crate) fn new_item(
        source_space: DockSpaceId,
        source_tabs: DockNodeId,
        item: DockItemId,
        title: String,
    ) -> Self {
        Self {
            source_space,
            source_tabs,
            kind: DockDragPayloadKind::Item { item },
            title,
        }
    }

    pub(crate) fn new_tabs(
        source_space: DockSpaceId,
        source_tabs: DockNodeId,
        title: String,
    ) -> Self {
        Self {
            source_space,
            source_tabs,
            kind: DockDragPayloadKind::Tabs,
            title,
        }
    }

    pub(crate) fn item(&self) -> Option<&DockItemId> {
        match &self.kind {
            DockDragPayloadKind::Item { item } => Some(item),
            DockDragPayloadKind::Tabs => None,
        }
    }

    pub(crate) fn is_tabs_stack(&self) -> bool {
        matches!(self.kind, DockDragPayloadKind::Tabs)
    }

    pub(crate) fn title(&self) -> &str {
        &self.title
    }
}

pub(crate) struct DockDragPreview {
    title: String,
}

impl DockDragPreview {
    pub(crate) fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
        }
    }
}

impl Render for DockDragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(rgb(0x334155))
            .text_color(white())
            .text_sm()
            .shadow_md()
            .child(self.title.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slotmap::Key;

    #[test]
    fn drag_payload_represents_item_or_tabs_stack() {
        let source_space = DockSpaceId::from("main");
        let source_tabs = DockNodeId::null();
        let item_payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            DockItemId::from("a"),
            "Panel A".to_string(),
        );
        let tabs_payload =
            DockDragPayload::new_tabs(source_space, source_tabs, "Stack".to_string());

        assert_eq!(item_payload.item(), Some(&DockItemId::from("a")));
        assert!(!item_payload.is_tabs_stack());
        assert_eq!(tabs_payload.item(), None);
        assert!(tabs_payload.is_tabs_stack());
    }
}
