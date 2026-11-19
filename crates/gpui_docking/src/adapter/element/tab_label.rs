//! Renderer resources for custom tabs, shared by measurement and painting.

use dockspace::model::ItemId;
use open_gpui::{
    App, AvailableSpace, ElementId, Hsla, IntoElement, Pixels, SharedString, Stateful,
    SubtreePresentation, SubtreePresentationExt, Window, prelude::*, size,
};

use crate::adapter::{DockPane, DockTabContentContext, GpuiDockspaceConfig, host::TabContent};

#[derive(Clone)]
pub(super) struct CustomTabLabel {
    item: ItemId,
    title: SharedString,
    content: TabContent,
}

impl CustomTabLabel {
    pub(super) fn new(pane: &DockPane) -> Option<Self> {
        Some(Self {
            item: pane.id,
            title: pane.title.clone(),
            content: pane.tab_content.clone()?,
        })
    }

    pub(super) fn element(
        &self,
        config: &GpuiDockspaceConfig,
        selected: bool,
        color: Hsla,
        height: Pixels,
        cx: &App,
    ) -> Stateful<open_gpui::Div> {
        let content = (self.content)(
            DockTabContentContext {
                item: self.item,
                title: self.title.as_ref(),
                selected,
                height,
            },
            cx,
        );
        super::tab_label_style(config, color)
            .id(ElementId::NamedInteger(
                "dockspace-tab-label".into(),
                self.item.get(),
            ))
            .h(height)
            .child(content)
    }

    pub(super) fn measure(
        &self,
        config: &GpuiDockspaceConfig,
        height: Pixels,
        window: &mut Window,
        cx: &mut App,
    ) -> f64 {
        // Selection can reuse core measurements. Reserve both appearances
        // before the tab is first painted.
        [false, true]
            .into_iter()
            .map(|selected| {
                let (namespace, color) = if selected {
                    ("dockspace-measure-selected-tab", config.colors.text)
                } else {
                    ("dockspace-measure-unselected-tab", config.colors.muted_text)
                };
                let bounds = window.with_element_namespace(namespace, |window| {
                    self.element(config, selected, color, height, cx)
                        .with_subtree_presentation(SubtreePresentation::Inert)
                        .into_any_element()
                        .layout_as_root(
                            size(AvailableSpace::MaxContent, AvailableSpace::Definite(height)),
                            window,
                            cx,
                        )
                });
                f64::from(f32::from(bounds.width))
            })
            .reduce(f64::max)
            .expect("the label has two selection states")
    }
}
