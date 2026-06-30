#![allow(dead_code)]

use crate::{
    DockHost, DockItemId, DockNode, DockNodeId, DockSpaceId, SplitAxis, geometry::DockSplitLayout,
    host_render_session::DockHostRenderSession, transition_geometry::DockMotionPreference,
    zoom_state::DockZoomScene,
};
use open_gpui::{Bounds, Context, Pixels, point, px, size};
use open_gpui_ui_core::{
    Orientation, Size, SplitterHandlePlacement, SplitterLayoutScene, SplitterMetrics,
    SplitterPanelDescriptor, SplitterState, UiRect, ui_point, ui_px, ui_rect, ui_size,
};

const PRESENTATION_TAB_BAR_HEIGHT: f32 = 28.0;
const PRESENTATION_FLOATING_TITLE_HEIGHT: f32 = 24.0;

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
    pub(crate) fn from_render_session(
        session: &DockHostRenderSession,
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

    pub(crate) fn tab_bar_for_node(&self, node: DockNodeId) -> Option<&DockPresentationTabBar> {
        self.tab_bars.iter().find(|tab_bar| tab_bar.tabs == node)
    }

    fn collect_node(
        &mut self,
        session: &DockHostRenderSession,
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
        session: &DockHostRenderSession,
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

        let layout = DockSplitLayout::from_fractions(
            children.len(),
            &fractions,
            session.central_child_index(&children),
        );
        let scene = split_layout_scene(
            split,
            axis,
            &children,
            layout.shares(),
            bounds,
            session.splitter_handle_size(),
        );
        let extent = split_extent(axis, bounds);

        for handle in scene.handles() {
            let index = handle.index();
            if let (Some(before), Some(after)) = (children.get(index), children.get(index + 1)) {
                let handle_bounds = bounds_from_ui_rect(handle.bounds());
                self.splitters.push(DockPresentationSplitter {
                    split,
                    index,
                    axis,
                    bounds: handle_bounds,
                    before: *before,
                    after: *after,
                    extent,
                    shares: layout.shares().to_vec(),
                });
                self.overlay_anchors.push(DockPresentationOverlayAnchor {
                    kind: DockPresentationOverlayAnchorKind::Splitter,
                    node: Some(split),
                    bounds: handle_bounds,
                });
            }
        }

        for (child, panel) in children.into_iter().zip(scene.panels()) {
            let child_bounds = bounds_from_ui_rect(panel.bounds());
            self.collect_node(session, child, child_bounds, floating);
        }
    }

    fn collect_tabs(
        &mut self,
        session: &DockHostRenderSession,
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

        let tab_bar_bounds = tab_bar_bounds(bounds);
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

fn split_layout_scene(
    split: DockNodeId,
    axis: SplitAxis,
    children: &[DockNodeId],
    shares: &[f32],
    bounds: Bounds<Pixels>,
    handle_size: Pixels,
) -> SplitterLayoutScene {
    let orientation = match axis {
        SplitAxis::Horizontal => Orientation::Horizontal,
        SplitAxis::Vertical => Orientation::Vertical,
    };
    let state = SplitterState::resolve(
        format!("dock-split-{}", split.as_u64()),
        orientation,
        Size::Medium,
        false,
        children.iter().enumerate().map(|(index, child)| {
            SplitterPanelDescriptor::new(
                format!("dock-node-{}", child.as_u64()),
                shares.get(index).copied().unwrap_or(0.0),
            )
            .min_fraction(0.0)
        }),
    );
    let handle_size = ui_px(f32::from(handle_size).max(0.0));
    let metrics = SplitterMetrics::new(handle_size, handle_size, ui_px(0.0))
        .with_handle_placement(SplitterHandlePlacement::OverlayBoundary);
    SplitterLayoutScene::from_state_with_metrics(&state, ui_rect_from_bounds(bounds), metrics)
}

fn ui_rect_from_bounds(bounds: Bounds<Pixels>) -> UiRect {
    ui_rect(
        ui_point(
            ui_px(f32::from(bounds.origin.x)),
            ui_px(f32::from(bounds.origin.y)),
        ),
        ui_size(
            ui_px(f32::from(bounds.size.width)),
            ui_px(f32::from(bounds.size.height)),
        ),
    )
}

fn bounds_from_ui_rect(rect: UiRect) -> Bounds<Pixels> {
    Bounds::new(
        point(px(rect.origin.x.as_f32()), px(rect.origin.y.as_f32())),
        size(px(rect.size.width.as_f32()), px(rect.size.height.as_f32())),
    )
}

fn split_extent(axis: SplitAxis, bounds: Bounds<Pixels>) -> Pixels {
    match axis {
        SplitAxis::Horizontal => bounds.size.width,
        SplitAxis::Vertical => bounds.size.height,
    }
}

impl DockPresentationFloatingContainer {
    fn from_bounds(node: DockNodeId, bounds: Bounds<Pixels>) -> Self {
        let title_height =
            px(PRESENTATION_FLOATING_TITLE_HEIGHT).min(bounds.size.height.max(px(0.0)));
        let title_bar_bounds = Bounds::new(bounds.origin, size(bounds.size.width, title_height));
        let content_bounds = Bounds::new(
            point(bounds.origin.x, bounds.origin.y + title_height),
            size(
                bounds.size.width,
                (bounds.size.height - title_height).max(px(0.0)),
            ),
        );

        Self {
            node,
            bounds,
            title_bar_bounds,
            content_bounds,
        }
    }
}

fn tab_bar_bounds(bounds: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        bounds.origin,
        size(
            bounds.size.width,
            px(PRESENTATION_TAB_BAR_HEIGHT).min(bounds.size.height.max(px(0.0))),
        ),
    )
}

pub(crate) fn dock_presentation_tab_label_bounds(
    tab_bar_bounds: Bounds<Pixels>,
    tab_count: usize,
    index: usize,
) -> Bounds<Pixels> {
    if tab_count == 0 {
        return Bounds::new(
            tab_bar_bounds.origin,
            size(px(0.0), tab_bar_bounds.size.height),
        );
    }

    let width = tab_bar_bounds.size.width / tab_count as f32;
    Bounds::new(
        point(
            tab_bar_bounds.origin.x + width * index as f32,
            tab_bar_bounds.origin.y,
        ),
        size(width, tab_bar_bounds.size.height),
    )
}

impl DockHost {
    pub(crate) fn presentation_scene(
        &self,
        bounds: Bounds<Pixels>,
        cx: &Context<Self>,
    ) -> DockPresentationScene {
        let base = DockPresentationScene::from_render_session(&self.render_session(cx), bounds);
        self.zoom_state()
            .resolve(&base, DockMotionPreference::Animated)
            .map(|zoom| zoom.scene)
            .unwrap_or(base)
    }

    pub(crate) fn zoom_scene(
        &self,
        bounds: Bounds<Pixels>,
        preference: DockMotionPreference,
        cx: &Context<Self>,
    ) -> Option<DockZoomScene> {
        let base = DockPresentationScene::from_render_session(&self.render_session(cx), bounds);
        self.zoom_state().resolve(&base, preference)
    }

    pub(crate) fn presentation_scene_for_test(
        &self,
        bounds: Bounds<Pixels>,
        cx: &Context<Self>,
    ) -> DockPresentationScene {
        self.presentation_scene(bounds, cx)
    }
}
