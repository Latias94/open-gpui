#![allow(dead_code)]

use crate::{
    DockItemId, DockNodeId, DropZone,
    overlay_scene::{DockOverlayLayerKind, DockOverlayScene},
    presentation_scene::DockPresentationScene,
};
use open_gpui::{Bounds, Pixels};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockAccessibilityScene {
    pub(crate) descriptors: Vec<DockAccessibilityDescriptor>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockAccessibilityDescriptor {
    pub(crate) role: DockAccessibilityRole,
    pub(crate) node: Option<DockNodeId>,
    pub(crate) item: Option<DockItemId>,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) label: Option<String>,
    pub(crate) zone: Option<DropZone>,
    pub(crate) active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DockAccessibilityRole {
    Pane,
    TabList,
    Tab,
    TabPanel,
    Splitter,
    FloatingWindow,
    FocusRegion,
    DropTarget,
    DragSource,
    DropDestination,
    RejectedDropTarget,
}

impl DockAccessibilityScene {
    pub(crate) fn from_presentation(scene: &DockPresentationScene) -> Self {
        let mut descriptors = Vec::new();

        for pane in &scene.panes {
            descriptors.push(DockAccessibilityDescriptor {
                role: DockAccessibilityRole::Pane,
                node: pane.node,
                item: None,
                bounds: pane.bounds,
                label: pane.node.map(|node| format!("Pane {node:?}")),
                zone: None,
                active: true,
            });
        }

        for tab_bar in &scene.tab_bars {
            descriptors.push(DockAccessibilityDescriptor {
                role: DockAccessibilityRole::TabList,
                node: Some(tab_bar.tabs),
                item: None,
                bounds: tab_bar.bounds,
                label: Some("Tabs".to_string()),
                zone: None,
                active: true,
            });
            descriptors.push(DockAccessibilityDescriptor {
                role: DockAccessibilityRole::DropDestination,
                node: Some(tab_bar.tabs),
                item: None,
                bounds: tab_bar.bounds,
                label: Some("Tab drop destination".to_string()),
                zone: Some(DropZone::Center),
                active: true,
            });
        }

        for tab in &scene.tab_labels {
            descriptors.push(DockAccessibilityDescriptor {
                role: DockAccessibilityRole::Tab,
                node: Some(tab.tabs),
                item: Some(tab.item.clone()),
                bounds: tab.bounds,
                label: Some(tab.title.clone()),
                zone: None,
                active: true,
            });
            descriptors.push(DockAccessibilityDescriptor {
                role: DockAccessibilityRole::DragSource,
                node: Some(tab.tabs),
                item: Some(tab.item.clone()),
                bounds: tab.bounds,
                label: Some(format!("Drag {}", tab.title)),
                zone: None,
                active: true,
            });
        }

        for focus in &scene.focus_regions {
            descriptors.push(DockAccessibilityDescriptor {
                role: DockAccessibilityRole::TabPanel,
                node: Some(focus.tabs),
                item: Some(focus.item.clone()),
                bounds: focus.bounds,
                label: Some(format!("Panel {}", focus.item)),
                zone: None,
                active: true,
            });
            descriptors.push(DockAccessibilityDescriptor {
                role: DockAccessibilityRole::FocusRegion,
                node: Some(focus.tabs),
                item: Some(focus.item.clone()),
                bounds: focus.bounds,
                label: Some("Focused pane".to_string()),
                zone: None,
                active: true,
            });
        }

        for splitter in &scene.splitters {
            descriptors.push(DockAccessibilityDescriptor {
                role: DockAccessibilityRole::Splitter,
                node: Some(splitter.split),
                item: None,
                bounds: splitter.bounds,
                label: Some(format!("Splitter {}", splitter.index)),
                zone: None,
                active: true,
            });
        }

        for floating in &scene.floating_containers {
            descriptors.push(DockAccessibilityDescriptor {
                role: DockAccessibilityRole::FloatingWindow,
                node: Some(floating.node),
                item: None,
                bounds: floating.bounds,
                label: Some("Floating container".to_string()),
                zone: None,
                active: true,
            });
        }

        Self { descriptors }
    }

    pub(crate) fn with_overlay(mut self, overlay: &DockOverlayScene) -> Self {
        for layer in &overlay.layers {
            match layer.kind {
                DockOverlayLayerKind::GuideBox | DockOverlayLayerKind::TabInsertion => {
                    self.descriptors.push(DockAccessibilityDescriptor {
                        role: DockAccessibilityRole::DropTarget,
                        node: layer.target_node,
                        item: None,
                        bounds: layer.bounds,
                        label: Some("Dock drop target".to_string()),
                        zone: layer.zone,
                        active: layer.active,
                    });
                    self.descriptors.push(DockAccessibilityDescriptor {
                        role: DockAccessibilityRole::DropDestination,
                        node: layer.target_node,
                        item: None,
                        bounds: layer.bounds,
                        label: Some("Dock drop destination".to_string()),
                        zone: layer.zone,
                        active: layer.active,
                    });
                }
                DockOverlayLayerKind::PayloadTab | DockOverlayLayerKind::PayloadGhost => {
                    self.descriptors.push(DockAccessibilityDescriptor {
                        role: DockAccessibilityRole::DragSource,
                        node: layer.target_node,
                        item: None,
                        bounds: layer.bounds,
                        label: Some("Dock drag payload".to_string()),
                        zone: layer.zone,
                        active: layer.active,
                    });
                }
                DockOverlayLayerKind::RejectedState => {
                    self.descriptors.push(DockAccessibilityDescriptor {
                        role: DockAccessibilityRole::RejectedDropTarget,
                        node: layer.target_node,
                        item: None,
                        bounds: layer.bounds,
                        label: Some("Rejected dock target".to_string()),
                        zone: layer.zone,
                        active: true,
                    });
                }
                DockOverlayLayerKind::RouteMarker
                | DockOverlayLayerKind::TargetBody
                | DockOverlayLayerKind::FocusRing => {}
            }
        }
        self
    }
}
