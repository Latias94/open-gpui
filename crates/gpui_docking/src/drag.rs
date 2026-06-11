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
    Floating { floating: DockNodeId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockDragPayloadIdentity {
    source_space: DockSpaceId,
    source_tabs: DockNodeId,
    kind: DockDragPayloadKind,
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

    pub(crate) fn new_floating(
        source_space: DockSpaceId,
        floating: DockNodeId,
        source_tabs: DockNodeId,
        title: String,
    ) -> Self {
        Self {
            source_space,
            source_tabs,
            kind: DockDragPayloadKind::Floating { floating },
            title,
        }
    }

    #[cfg(test)]
    pub(crate) fn item(&self) -> Option<&DockItemId> {
        match &self.kind {
            DockDragPayloadKind::Item { item } => Some(item),
            DockDragPayloadKind::Tabs | DockDragPayloadKind::Floating { .. } => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn is_tabs_stack(&self) -> bool {
        matches!(self.kind, DockDragPayloadKind::Tabs)
    }

    #[cfg(test)]
    pub(crate) fn floating(&self) -> Option<DockNodeId> {
        match self.kind {
            DockDragPayloadKind::Floating { floating } => Some(floating),
            DockDragPayloadKind::Item { .. } | DockDragPayloadKind::Tabs => None,
        }
    }

    pub(crate) fn title(&self) -> &str {
        &self.title
    }

    pub(crate) fn identity(&self) -> DockDragPayloadIdentity {
        DockDragPayloadIdentity {
            source_space: self.source_space.clone(),
            source_tabs: self.source_tabs,
            kind: self.kind.clone(),
        }
    }

    pub(crate) fn excluded_tabs_for_drop_scene(&self) -> Option<DockNodeId> {
        match self.kind {
            DockDragPayloadKind::Item { .. } => None,
            DockDragPayloadKind::Tabs | DockDragPayloadKind::Floating { .. } => {
                Some(self.source_tabs)
            }
        }
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
        let floating = DockNodeId::null();
        let tabs_payload =
            DockDragPayload::new_tabs(source_space.clone(), source_tabs, "Stack".to_string());
        let floating_payload = DockDragPayload::new_floating(
            source_space,
            floating,
            source_tabs,
            "Floating".to_string(),
        );

        assert_eq!(item_payload.item(), Some(&DockItemId::from("a")));
        assert!(!item_payload.is_tabs_stack());
        assert_eq!(tabs_payload.item(), None);
        assert!(tabs_payload.is_tabs_stack());
        assert_eq!(floating_payload.floating(), Some(floating));
        assert_eq!(
            floating_payload.excluded_tabs_for_drop_scene(),
            Some(source_tabs)
        );
    }

    #[test]
    fn drag_payload_identity_ignores_preview_title() {
        let source_space = DockSpaceId::from("main");
        let source_tabs = DockNodeId::null();
        let original = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            DockItemId::from("a"),
            "Original title".to_string(),
        );
        let renamed = DockDragPayload::new_item(
            source_space,
            source_tabs,
            DockItemId::from("a"),
            "Renamed title".to_string(),
        );

        assert_eq!(original.identity(), renamed.identity());
        assert_ne!(original, renamed);
    }
}
