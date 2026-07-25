use crate::{
    DockHost, DockItemId, DockNode, DockNodeId, DockSpaceId, SplitAxis,
    chrome_geometry::{
        dock_floating_chrome_bounds, dock_presentation_tab_label_bounds, dock_tab_bar_bounds,
    },
    host_render_session::DockHostPresentationSession,
    split_geometry::resolve_dock_split_layout,
};
#[cfg(test)]
use open_gpui::Context;
use open_gpui::{Bounds, Pixels};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPresentationScene {
    pub(crate) space: DockSpaceId,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) root: Option<DockNodeId>,
    pub(crate) panes: Vec<DockPresentationPane>,
    pub(crate) tab_bars: Vec<DockPresentationTabBar>,
    pub(crate) tab_labels: Vec<DockPresentationTabLabel>,
    pub(crate) splitters: Vec<DockPresentationSplitter>,
    pub(crate) floating_containers: Vec<DockPresentationFloatingContainer>,
    pub(crate) focus_regions: Vec<DockPresentationFocusRegion>,
    pub(crate) overlay_anchors: Vec<DockPresentationOverlayAnchor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockPresentationPaneKind {
    Tabs,
    EmptyCentral,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPresentationPane {
    pub(crate) node: Option<DockNodeId>,
    pub(crate) kind: DockPresentationPaneKind,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) floating: Option<DockNodeId>,
    pub(crate) is_central: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPresentationTabBar {
    pub(crate) tabs: DockNodeId,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) floating: Option<DockNodeId>,
    pub(crate) is_central: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPresentationTabLabel {
    pub(crate) tabs: DockNodeId,
    pub(crate) item: DockItemId,
    pub(crate) index: usize,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) title: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPresentationSplitter {
    pub(crate) split: DockNodeId,
    pub(crate) index: usize,
    pub(crate) axis: SplitAxis,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) before: DockNodeId,
    pub(crate) after: DockNodeId,
    pub(crate) extent: Pixels,
    pub(crate) shares: Vec<f32>,
    pub(crate) floating: Option<DockNodeId>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPresentationFloatingContainer {
    pub(crate) node: DockNodeId,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) title_bar_bounds: Bounds<Pixels>,
    pub(crate) content_bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPresentationFocusRegion {
    pub(crate) tabs: DockNodeId,
    pub(crate) item: DockItemId,
    pub(crate) bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockPresentationOverlayAnchorKind {
    Root,
    Pane,
    TabBar,
    EmptyCentral,
    Floating,
    FloatingTitleBar,
    Splitter,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPresentationOverlayAnchor {
    pub(crate) kind: DockPresentationOverlayAnchorKind,
    pub(crate) node: Option<DockNodeId>,
    pub(crate) bounds: Bounds<Pixels>,
}

impl DockPresentationScene {
    pub(crate) fn from_presentation_session(
        session: &DockHostPresentationSession,
        bounds: Bounds<Pixels>,
    ) -> Self {
        let mut scene = Self {
            space: session.space().clone(),
            bounds,
            root: session.root(),
            panes: Vec::new(),
            tab_bars: Vec::new(),
            tab_labels: Vec::new(),
            splitters: Vec::new(),
            floating_containers: Vec::new(),
            focus_regions: Vec::new(),
            overlay_anchors: vec![DockPresentationOverlayAnchor {
                kind: DockPresentationOverlayAnchorKind::Root,
                node: session.root(),
                bounds,
            }],
        };

        if let Some(root) = session.root() {
            scene.collect_node(session, root, bounds, None);
        } else if session.has_empty_central_region() {
            scene.push_empty_central(bounds);
        }

        for container in session.floating_containers() {
            let floating =
                DockPresentationFloatingContainer::from_bounds(container.node, container.bounds);
            scene.overlay_anchors.push(DockPresentationOverlayAnchor {
                kind: DockPresentationOverlayAnchorKind::Floating,
                node: Some(container.node),
                bounds: container.bounds,
            });
            scene.overlay_anchors.push(DockPresentationOverlayAnchor {
                kind: DockPresentationOverlayAnchorKind::FloatingTitleBar,
                node: Some(container.node),
                bounds: floating.title_bar_bounds,
            });
            scene.collect_node(
                session,
                container.node,
                floating.content_bounds,
                Some(container.node),
            );
            scene.floating_containers.push(floating);
        }

        scene
    }

    pub(crate) fn pane_for_node(&self, node: DockNodeId) -> Option<&DockPresentationPane> {
        self.panes.iter().find(|pane| pane.node == Some(node))
    }

    #[cfg(test)]
    pub(crate) fn tab_bar_for_node(&self, node: DockNodeId) -> Option<&DockPresentationTabBar> {
        self.tab_bars.iter().find(|tab_bar| tab_bar.tabs == node)
    }

    fn collect_node(
        &mut self,
        session: &DockHostPresentationSession,
        node_id: DockNodeId,
        bounds: Bounds<Pixels>,
        floating: Option<DockNodeId>,
    ) {
        let Some(node) = session.node(node_id).cloned() else {
            return;
        };

        match node {
            DockNode::Split {
                axis,
                children,
                fractions,
            } => self.collect_split(
                session, node_id, axis, children, fractions, bounds, floating,
            ),
            DockNode::Tabs { items, selected } => {
                self.collect_tabs(session, node_id, items, selected, bounds, floating);
            }
            DockNode::Floating { child } => {
                self.collect_node(session, child, bounds, floating.or(Some(node_id)));
            }
        }
    }

    fn collect_split(
        &mut self,
        session: &DockHostPresentationSession,
        split: DockNodeId,
        axis: SplitAxis,
        children: Vec<DockNodeId>,
        fractions: Vec<f32>,
        bounds: Bounds<Pixels>,
        floating: Option<DockNodeId>,
    ) {
        if children.is_empty() {
            return;
        }

        let layout = resolve_dock_split_layout(
            split,
            axis,
            &children,
            &fractions,
            session.central_child_index(&children),
            bounds,
            session.splitter_handle_size(),
        );

        for handle in layout.handles() {
            self.splitters.push(DockPresentationSplitter {
                split,
                index: handle.index,
                axis: handle.axis,
                bounds: handle.bounds,
                before: handle.before,
                after: handle.after,
                extent: handle.extent,
                shares: layout.shares().to_vec(),
                floating,
            });
            self.overlay_anchors.push(DockPresentationOverlayAnchor {
                kind: DockPresentationOverlayAnchorKind::Splitter,
                node: Some(split),
                bounds: handle.bounds,
            });
        }

        for panel in layout.panels() {
            self.collect_node(session, panel.child, panel.bounds, floating);
        }
    }

    fn collect_tabs(
        &mut self,
        session: &DockHostPresentationSession,
        tabs: DockNodeId,
        items: Vec<DockItemId>,
        selected: Option<DockItemId>,
        bounds: Bounds<Pixels>,
        floating: Option<DockNodeId>,
    ) {
        let is_central = session.is_central_tabs(tabs);
        self.panes.push(DockPresentationPane {
            node: Some(tabs),
            kind: DockPresentationPaneKind::Tabs,
            bounds,
            floating,
            is_central,
        });
        self.overlay_anchors.push(DockPresentationOverlayAnchor {
            kind: DockPresentationOverlayAnchorKind::Pane,
            node: Some(tabs),
            bounds,
        });

        let tab_bar_bounds = dock_tab_bar_bounds(bounds);
        self.tab_bars.push(DockPresentationTabBar {
            tabs,
            bounds: tab_bar_bounds,
            floating,
            is_central,
        });
        self.overlay_anchors.push(DockPresentationOverlayAnchor {
            kind: DockPresentationOverlayAnchorKind::TabBar,
            node: Some(tabs),
            bounds: tab_bar_bounds,
        });

        for (index, item) in items.iter().cloned().enumerate() {
            self.tab_labels.push(DockPresentationTabLabel {
                tabs,
                item: item.clone(),
                index,
                bounds: dock_presentation_tab_label_bounds(tab_bar_bounds, items.len(), index),
                title: session.panel_title(&item),
            });
        }

        if let Some(selected) = selected
            && items.iter().any(|item| item == &selected)
        {
            self.focus_regions.push(DockPresentationFocusRegion {
                tabs,
                item: selected,
                bounds,
            });
        }
    }

    fn push_empty_central(&mut self, bounds: Bounds<Pixels>) {
        self.panes.push(DockPresentationPane {
            node: None,
            kind: DockPresentationPaneKind::EmptyCentral,
            bounds,
            floating: None,
            is_central: true,
        });
        self.overlay_anchors.push(DockPresentationOverlayAnchor {
            kind: DockPresentationOverlayAnchorKind::EmptyCentral,
            node: None,
            bounds,
        });
    }
}

impl DockPresentationFloatingContainer {
    fn from_bounds(node: DockNodeId, bounds: Bounds<Pixels>) -> Self {
        let chrome = dock_floating_chrome_bounds(bounds);
        Self {
            node,
            bounds,
            title_bar_bounds: chrome.title_bar_bounds,
            content_bounds: chrome.content_bounds,
        }
    }
}

impl DockHost {
    #[cfg(test)]
    pub(crate) fn presentation_scene(
        &self,
        bounds: Bounds<Pixels>,
        cx: &Context<Self>,
    ) -> DockPresentationScene {
        let session = self.presentation_session(cx);
        let base = DockPresentationScene::from_presentation_session(&session, bounds);
        self.zoom_state()
            .resolve(&base, session.motion_preference())
            .map(|zoom| zoom.scene)
            .unwrap_or(base)
    }

    #[cfg(test)]
    pub(crate) fn presentation_scene_for_test(
        &self,
        bounds: Bounds<Pixels>,
        cx: &Context<Self>,
    ) -> DockPresentationScene {
        self.presentation_scene(bounds, cx)
    }
}
