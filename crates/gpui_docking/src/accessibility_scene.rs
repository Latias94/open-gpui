use crate::{
    DockItemId, DockNodeId, DropZone, SplitAxis,
    overlay_scene::{DockOverlayLayerKind, DockOverlayScene},
    presentation_scene::DockPresentationScene,
};
use open_gpui::{Bounds, Pixels};
use open_gpui_ui_core::{AccessibleAction, Orientation};

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockAccessibilityScene {
    pub(crate) descriptors: Vec<DockAccessibilityDescriptor>,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockAccessibilityDescriptor {
    pub(crate) role: DockAccessibilityRole,
    pub(crate) node: Option<DockNodeId>,
    pub(crate) item: Option<DockItemId>,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) label: Option<String>,
    pub(crate) zone: Option<DropZone>,
    pub(crate) active: bool,
    pub(crate) orientation: Option<Orientation>,
    pub(crate) selected: Option<bool>,
    pub(crate) disabled: Option<bool>,
    pub(crate) actions: Vec<AccessibleAction>,
}

#[cfg_attr(not(test), allow(dead_code))]
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
    #[cfg_attr(not(test), allow(dead_code))]
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
                orientation: None,
                selected: None,
                disabled: Some(false),
                actions: vec![AccessibleAction::Focus],
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
                orientation: Some(Orientation::Horizontal),
                selected: None,
                disabled: Some(false),
                actions: Vec::new(),
            });
            descriptors.push(DockAccessibilityDescriptor {
                role: DockAccessibilityRole::DropDestination,
                node: Some(tab_bar.tabs),
                item: None,
                bounds: tab_bar.bounds,
                label: Some("Tab drop destination".to_string()),
                zone: Some(DropZone::Center),
                active: true,
                orientation: Some(Orientation::Horizontal),
                selected: None,
                disabled: Some(false),
                actions: vec![AccessibleAction::CustomAction],
            });
        }

        for tab in &scene.tab_labels {
            let selected = scene
                .focus_regions
                .iter()
                .any(|focus| focus.tabs == tab.tabs && focus.item == tab.item);
            descriptors.push(DockAccessibilityDescriptor {
                role: DockAccessibilityRole::Tab,
                node: Some(tab.tabs),
                item: Some(tab.item.clone()),
                bounds: tab.bounds,
                label: Some(tab.title.clone()),
                zone: None,
                active: true,
                orientation: None,
                selected: Some(selected),
                disabled: Some(false),
                actions: vec![AccessibleAction::Click, AccessibleAction::Focus],
            });
            descriptors.push(DockAccessibilityDescriptor {
                role: DockAccessibilityRole::DragSource,
                node: Some(tab.tabs),
                item: Some(tab.item.clone()),
                bounds: tab.bounds,
                label: Some(format!("Drag {}", tab.title)),
                zone: None,
                active: true,
                orientation: None,
                selected: Some(selected),
                disabled: Some(false),
                actions: vec![AccessibleAction::CustomAction],
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
                orientation: None,
                selected: Some(true),
                disabled: Some(false),
                actions: vec![AccessibleAction::Focus],
            });
            descriptors.push(DockAccessibilityDescriptor {
                role: DockAccessibilityRole::FocusRegion,
                node: Some(focus.tabs),
                item: Some(focus.item.clone()),
                bounds: focus.bounds,
                label: Some("Focused pane".to_string()),
                zone: None,
                active: true,
                orientation: None,
                selected: Some(true),
                disabled: Some(false),
                actions: vec![AccessibleAction::Focus],
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
                orientation: Some(orientation_for_axis(splitter.axis)),
                selected: None,
                disabled: Some(false),
                actions: vec![AccessibleAction::Increment, AccessibleAction::Decrement],
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
                orientation: None,
                selected: None,
                disabled: Some(false),
                actions: vec![AccessibleAction::Focus],
            });
        }

        Self { descriptors }
    }

    #[cfg_attr(not(test), allow(dead_code))]
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
                        orientation: None,
                        selected: None,
                        disabled: Some(!layer.active),
                        actions: vec![AccessibleAction::CustomAction],
                    });
                    self.descriptors.push(DockAccessibilityDescriptor {
                        role: DockAccessibilityRole::DropDestination,
                        node: layer.target_node,
                        item: None,
                        bounds: layer.bounds,
                        label: Some("Dock drop destination".to_string()),
                        zone: layer.zone,
                        active: layer.active,
                        orientation: None,
                        selected: None,
                        disabled: Some(!layer.active),
                        actions: vec![AccessibleAction::CustomAction],
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
                        orientation: None,
                        selected: None,
                        disabled: Some(!layer.active),
                        actions: vec![AccessibleAction::CustomAction],
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
                        orientation: None,
                        selected: None,
                        disabled: Some(true),
                        actions: Vec::new(),
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

#[cfg_attr(not(test), allow(dead_code))]
fn orientation_for_axis(axis: SplitAxis) -> Orientation {
    match axis {
        SplitAxis::Horizontal => Orientation::Horizontal,
        SplitAxis::Vertical => Orientation::Vertical,
    }
}
