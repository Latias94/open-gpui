#![allow(dead_code)]

use crate::{
    DockNodeId, DockSpaceId,
    presentation_scene::{
        DockPresentationFocusRegion, DockPresentationOverlayAnchor,
        DockPresentationOverlayAnchorKind, DockPresentationPane, DockPresentationScene,
        dock_presentation_tab_label_bounds,
    },
    transition_geometry::{DockMotionPreference, DockTransitionEdge, preferred_transition_edge},
};
use open_gpui::{Bounds, Pixels};
use std::collections::HashMap;

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct DockZoomState {
    zoomed: HashMap<DockSpaceId, DockZoomPresentation>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockZoomPresentation {
    pub(crate) target: DockNodeId,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockZoomScene {
    pub(crate) scene: DockPresentationScene,
    pub(crate) target: DockNodeId,
    pub(crate) egress: Vec<DockZoomPaneEgress>,
    pub(crate) focus: Option<DockPresentationFocusRegion>,
    pub(crate) immediate: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockZoomPaneEgress {
    pub(crate) node: DockNodeId,
    pub(crate) from: Bounds<Pixels>,
    pub(crate) edge: DockTransitionEdge,
}

impl DockZoomState {
    pub(crate) fn zoom(&mut self, space: DockSpaceId, target: DockNodeId) {
        self.zoomed.insert(space, DockZoomPresentation { target });
    }

    pub(crate) fn unzoom(&mut self, space: &DockSpaceId) -> Option<DockZoomPresentation> {
        self.zoomed.remove(space)
    }

    pub(crate) fn target(&self, space: &DockSpaceId) -> Option<DockNodeId> {
        self.zoomed.get(space).map(|zoom| zoom.target)
    }

    pub(crate) fn clear_missing_target(
        &mut self,
        space: &DockSpaceId,
        scene: &DockPresentationScene,
    ) -> bool {
        let Some(target) = self.target(space) else {
            return false;
        };
        if scene.pane_for_node(target).is_some() {
            return false;
        }
        self.zoomed.remove(space);
        true
    }

    pub(crate) fn resolve(
        &self,
        scene: &DockPresentationScene,
        preference: DockMotionPreference,
    ) -> Option<DockZoomScene> {
        let target = self.target(&scene.space)?;
        DockZoomScene::from_scene(scene, target, preference)
    }
}

impl DockZoomScene {
    pub(crate) fn from_scene(
        scene: &DockPresentationScene,
        target: DockNodeId,
        preference: DockMotionPreference,
    ) -> Option<Self> {
        let target_pane = scene.pane_for_node(target)?;
        let mut zoomed = scene.clone();
        zoomed.panes = vec![DockPresentationPane {
            bounds: scene.bounds,
            ..target_pane.clone()
        }];
        zoomed.tab_bars.retain(|tab_bar| tab_bar.tabs == target);
        for tab_bar in &mut zoomed.tab_bars {
            tab_bar.bounds.origin = scene.bounds.origin;
            tab_bar.bounds.size.width = scene.bounds.size.width;
        }
        zoomed.tab_labels.retain(|label| label.tabs == target);
        let tab_label_count = zoomed.tab_labels.len();
        if let Some(tab_bar) = zoomed.tab_bars.first() {
            for (index, label) in zoomed.tab_labels.iter_mut().enumerate() {
                label.index = index;
                label.bounds =
                    dock_presentation_tab_label_bounds(tab_bar.bounds, tab_label_count, index);
            }
        }
        zoomed.splitters.clear();
        zoomed.floating_containers.clear();
        zoomed.focus_regions.retain(|focus| focus.tabs == target);
        for focus in &mut zoomed.focus_regions {
            focus.bounds = scene.bounds;
        }
        zoomed.overlay_anchors = vec![DockPresentationOverlayAnchor {
            kind: DockPresentationOverlayAnchorKind::Root,
            node: scene.root,
            bounds: scene.bounds,
        }];
        zoomed.overlay_anchors.push(DockPresentationOverlayAnchor {
            kind: DockPresentationOverlayAnchorKind::Pane,
            node: Some(target),
            bounds: scene.bounds,
        });
        zoomed
            .overlay_anchors
            .extend(
                zoomed
                    .tab_bars
                    .iter()
                    .map(|tab_bar| DockPresentationOverlayAnchor {
                        kind: DockPresentationOverlayAnchorKind::TabBar,
                        node: Some(tab_bar.tabs),
                        bounds: tab_bar.bounds,
                    }),
            );

        let egress = scene
            .panes
            .iter()
            .filter_map(|pane| {
                let node = pane.node?;
                (node != target).then(|| DockZoomPaneEgress {
                    node,
                    from: pane.bounds,
                    edge: preferred_transition_edge(pane.bounds, scene.bounds),
                })
            })
            .collect();

        let focus = zoomed
            .focus_regions
            .iter()
            .find(|focus| focus.tabs == target)
            .cloned();

        Some(Self {
            scene: zoomed,
            target,
            egress,
            focus,
            immediate: matches!(preference, DockMotionPreference::Reduced),
        })
    }
}
