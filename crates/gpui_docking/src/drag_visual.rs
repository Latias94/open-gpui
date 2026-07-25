use crate::DockDragVisualStyle;
use open_gpui::{Context, IntoElement, ParentElement, Render, SharedString, Styled, div, px};

/// Source-owned visual returned to GPUI for one active Dock payload drag.
pub(crate) struct DockDragVisual {
    title: SharedString,
    style: DockDragVisualStyle,
}

impl DockDragVisual {
    pub(crate) fn new(title: impl Into<SharedString>, style: DockDragVisualStyle) -> Self {
        Self {
            title: title.into(),
            style,
        }
    }
}

impl Render for DockDragVisual {
    fn render(
        &mut self,
        _window: &mut open_gpui::Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .min_w(px(120.0))
            .max_w(px(320.0))
            .px_3()
            .py_2()
            .border_1()
            .rounded_sm()
            .border_color(self.style.border)
            .bg(self.style.background)
            .text_color(self.style.text)
            .shadow(self.style.shadow.clone())
            .text_sm()
            .truncate()
            .child(self.title.clone())
    }
}
