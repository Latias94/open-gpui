use crate::{DockItemId, DockNodeId, DockSpaceId};
use open_gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div, rgb, white};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockTabDragPayload {
    pub(crate) source_space: DockSpaceId,
    pub(crate) source_tabs: DockNodeId,
    pub(crate) item: DockItemId,
    title: String,
}

impl DockTabDragPayload {
    pub(crate) fn new(
        source_space: DockSpaceId,
        source_tabs: DockNodeId,
        item: DockItemId,
        title: String,
    ) -> Self {
        Self {
            source_space,
            source_tabs,
            item,
            title,
        }
    }

    pub(crate) fn title(&self) -> &str {
        &self.title
    }
}

pub(crate) struct DockTabDragPreview {
    title: String,
}

impl DockTabDragPreview {
    pub(crate) fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
        }
    }
}

impl Render for DockTabDragPreview {
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
