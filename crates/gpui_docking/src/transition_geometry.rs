#![allow(dead_code)]

use crate::{
    DockNodeId, DropZone, SplitAxis,
    overlay_scene::{DockOverlayLayerKind, DockOverlayScene},
    presentation_scene::{
        DockPresentationFocusRegion, DockPresentationPane, DockPresentationScene,
    },
    zoom_state::DockZoomScene,
};
use open_gpui::{Bounds, Pixels, point};
use open_gpui_ui_core::MotionPreference;
use std::collections::{HashMap, HashSet};

/// Descriptor plan for docking presentation, divider, and overlay transitions.
#[derive(Debug, Clone, PartialEq)]
pub struct DockTransitionPlan {
    pub(crate) preference: DockMotionPreference,
    pub(crate) final_scene: DockPresentationScene,
    pub(crate) pane_transitions: Vec<DockPaneTransition>,
    pub(crate) divider_transitions: Vec<DockDividerTransition>,
    pub(crate) overlay_transitions: Vec<DockOverlayTransition>,
}

pub(crate) type DockMotionPreference = MotionPreference;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPaneTransition {
    pub(crate) node: DockNodeId,
    pub(crate) kind: DockPaneTransitionKind,
    pub(crate) from: Option<Bounds<Pixels>>,
    pub(crate) to: Option<Bounds<Pixels>>,
    pub(crate) slide: Option<DockSlideTransition>,
    pub(crate) immediate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockPaneTransitionKind {
    Unchanged,
    Entering,
    Leaving,
    Moving,
    Resizing,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockSlideTransition {
    pub(crate) edge: DockTransitionEdge,
    pub(crate) source_bounds: Bounds<Pixels>,
    pub(crate) final_bounds: Bounds<Pixels>,
    pub(crate) occlusion_bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockTransitionEdge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockDividerTransition {
    pub(crate) split: DockNodeId,
    pub(crate) index: usize,
    pub(crate) axis: SplitAxis,
    pub(crate) kind: DockDividerTransitionKind,
    pub(crate) from: Option<Bounds<Pixels>>,
    pub(crate) to: Bounds<Pixels>,
    pub(crate) immediate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockDividerTransitionKind {
    Unchanged,
    Appearing,
    Moving,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockOverlayTransition {
    pub(crate) kind: DockOverlayTransitionKind,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) target_node: Option<DockNodeId>,
    pub(crate) zone: Option<DropZone>,
    pub(crate) payload_index: Option<usize>,
    pub(crate) immediate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockOverlayTransitionKind {
    RouteMarker,
    TabInsertion,
    PayloadGhost,
    FocusRing,
    RejectedNoop,
}

impl DockTransitionPlan {
    pub(crate) fn between(
        previous: &DockPresentationScene,
        next: &DockPresentationScene,
        preference: DockMotionPreference,
    ) -> Self {
        Self {
            preference,
            final_scene: next.clone(),
            pane_transitions: pane_transitions(previous, next, preference),
            divider_transitions: divider_transitions(previous, next, preference),
            overlay_transitions: Vec::new(),
        }
    }

    pub(crate) fn from_overlay_scene(
        final_scene: &DockPresentationScene,
        overlay_scene: &DockOverlayScene,
        preference: DockMotionPreference,
    ) -> Self {
        Self {
            preference,
            final_scene: final_scene.clone(),
            pane_transitions: Vec::new(),
            divider_transitions: Vec::new(),
            overlay_transitions: overlay_scene
                .layers
                .iter()
                .filter_map(|layer| {
                    let kind = match layer.kind {
                        DockOverlayLayerKind::RouteMarker => DockOverlayTransitionKind::RouteMarker,
                        DockOverlayLayerKind::TabInsertion => {
                            DockOverlayTransitionKind::TabInsertion
                        }
                        DockOverlayLayerKind::PayloadGhost => {
                            DockOverlayTransitionKind::PayloadGhost
                        }
                        DockOverlayLayerKind::RejectedState => {
                            DockOverlayTransitionKind::RejectedNoop
                        }
                        DockOverlayLayerKind::TargetBody
                        | DockOverlayLayerKind::GuideBox
                        | DockOverlayLayerKind::PayloadTab
                        | DockOverlayLayerKind::FocusRing => return None,
                    };
                    Some(DockOverlayTransition {
                        kind,
                        bounds: layer.bounds,
                        target_node: layer.target_node,
                        zone: layer.zone,
                        payload_index: layer.payload_index,
                        immediate: preference.is_immediate(),
                    })
                })
                .collect(),
        }
    }

    pub(crate) fn from_zoom_scene(
        previous: &DockPresentationScene,
        zoom: &DockZoomScene,
        preference: DockMotionPreference,
    ) -> Self {
        let mut plan = Self::between(previous, &zoom.scene, preference);
        for egress in &zoom.egress {
            if let Some(transition) = plan
                .pane_transitions
                .iter_mut()
                .find(|transition| transition.node == egress.node)
            {
                let source_bounds = egress.edge.source_bounds(egress.from, previous.bounds);
                transition.from = Some(egress.from);
                transition.to = Some(source_bounds);
                transition.slide = Some(DockSlideTransition {
                    edge: egress.edge,
                    source_bounds,
                    final_bounds: egress.from,
                    occlusion_bounds: egress.from,
                });
            }
        }
        if let Some(focus) = zoom.focus.as_ref() {
            plan.overlay_transitions
                .push(focus_ring_transition(focus, preference));
        }
        plan
    }

    pub(crate) fn from_focus_region(
        final_scene: &DockPresentationScene,
        focus: &DockPresentationFocusRegion,
        preference: DockMotionPreference,
    ) -> Self {
        Self {
            preference,
            final_scene: final_scene.clone(),
            pane_transitions: Vec::new(),
            divider_transitions: Vec::new(),
            overlay_transitions: vec![focus_ring_transition(focus, preference)],
        }
    }

    pub(crate) fn is_immediate(&self) -> bool {
        self.preference.is_immediate()
            && self.pane_transitions.iter().all(|item| item.immediate)
            && self.divider_transitions.iter().all(|item| item.immediate)
            && self.overlay_transitions.iter().all(|item| item.immediate)
    }
}

fn focus_ring_transition(
    focus: &DockPresentationFocusRegion,
    preference: DockMotionPreference,
) -> DockOverlayTransition {
    DockOverlayTransition {
        kind: DockOverlayTransitionKind::FocusRing,
        bounds: focus.bounds,
        target_node: Some(focus.tabs),
        zone: None,
        payload_index: None,
        immediate: preference.is_immediate(),
    }
}

fn pane_transitions(
    previous: &DockPresentationScene,
    next: &DockPresentationScene,
    preference: DockMotionPreference,
) -> Vec<DockPaneTransition> {
    let previous_panes = pane_map(previous);
    let next_panes = pane_map(next);
    let mut transitions = Vec::new();
    let mut seen = HashSet::new();

    for (node, next_pane) in &next_panes {
        seen.insert(*node);
        let transition = match previous_panes.get(node) {
            Some(previous_pane) => pane_transition_for_existing(
                *node,
                previous_pane.bounds,
                next_pane.bounds,
                preference,
            ),
            None => {
                let slide = slide_transition(next_pane.bounds, next.bounds);
                DockPaneTransition {
                    node: *node,
                    kind: DockPaneTransitionKind::Entering,
                    from: Some(slide.source_bounds),
                    to: Some(next_pane.bounds),
                    slide: Some(slide),
                    immediate: preference.is_immediate(),
                }
            }
        };
        transitions.push(transition);
    }

    for (node, previous_pane) in previous_panes {
        if seen.contains(&node) {
            continue;
        }
        let slide = slide_transition(previous_pane.bounds, previous.bounds);
        transitions.push(DockPaneTransition {
            node,
            kind: DockPaneTransitionKind::Leaving,
            from: Some(previous_pane.bounds),
            to: Some(slide.source_bounds),
            slide: Some(slide),
            immediate: preference.is_immediate(),
        });
    }

    transitions
}

fn pane_transition_for_existing(
    node: DockNodeId,
    from: Bounds<Pixels>,
    to: Bounds<Pixels>,
    preference: DockMotionPreference,
) -> DockPaneTransition {
    let kind = if from == to {
        DockPaneTransitionKind::Unchanged
    } else if from.size != to.size {
        DockPaneTransitionKind::Resizing
    } else {
        DockPaneTransitionKind::Moving
    };
    DockPaneTransition {
        node,
        kind,
        from: Some(from),
        to: Some(to),
        slide: None,
        immediate: preference.is_immediate(),
    }
}

fn pane_map(scene: &DockPresentationScene) -> HashMap<DockNodeId, &DockPresentationPane> {
    scene
        .panes
        .iter()
        .filter_map(|pane| pane.node.map(|node| (node, pane)))
        .collect()
}

fn divider_transitions(
    previous: &DockPresentationScene,
    next: &DockPresentationScene,
    preference: DockMotionPreference,
) -> Vec<DockDividerTransition> {
    let previous_dividers: HashMap<_, _> = previous
        .splitters
        .iter()
        .map(|splitter| ((splitter.split, splitter.index), splitter))
        .collect();

    next.splitters
        .iter()
        .map(|splitter| {
            let from = previous_dividers
                .get(&(splitter.split, splitter.index))
                .map(|previous| previous.bounds);
            let kind = match from {
                None => DockDividerTransitionKind::Appearing,
                Some(bounds) if bounds == splitter.bounds => DockDividerTransitionKind::Unchanged,
                Some(_) => DockDividerTransitionKind::Moving,
            };
            DockDividerTransition {
                split: splitter.split,
                index: splitter.index,
                axis: splitter.axis,
                kind,
                from,
                to: splitter.bounds,
                immediate: preference.is_immediate(),
            }
        })
        .collect()
}

fn slide_transition(
    final_bounds: Bounds<Pixels>,
    scene_bounds: Bounds<Pixels>,
) -> DockSlideTransition {
    let edge = preferred_transition_edge(final_bounds, scene_bounds);
    DockSlideTransition {
        edge,
        source_bounds: edge.source_bounds(final_bounds, scene_bounds),
        final_bounds,
        occlusion_bounds: final_bounds,
    }
}

pub(crate) fn preferred_transition_edge(
    bounds: Bounds<Pixels>,
    scene_bounds: Bounds<Pixels>,
) -> DockTransitionEdge {
    let left = f32::from((bounds.origin.x - scene_bounds.origin.x).abs());
    let right = f32::from((scene_bounds.right() - bounds.right()).abs());
    let top = f32::from((bounds.origin.y - scene_bounds.origin.y).abs());
    let bottom = f32::from((scene_bounds.bottom() - bounds.bottom()).abs());
    let touching_epsilon = 0.5_f32;

    if left <= touching_epsilon {
        return DockTransitionEdge::Left;
    }
    if right <= touching_epsilon {
        return DockTransitionEdge::Right;
    }
    if top <= touching_epsilon {
        return DockTransitionEdge::Top;
    }
    if bottom <= touching_epsilon {
        return DockTransitionEdge::Bottom;
    }

    [
        (DockTransitionEdge::Left, left),
        (DockTransitionEdge::Right, right),
        (DockTransitionEdge::Top, top),
        (DockTransitionEdge::Bottom, bottom),
    ]
    .into_iter()
    .min_by(|(_, a), (_, b)| a.total_cmp(b))
    .map(|(edge, _)| edge)
    .unwrap_or(DockTransitionEdge::Left)
}

impl DockTransitionEdge {
    fn source_bounds(
        self,
        final_bounds: Bounds<Pixels>,
        scene_bounds: Bounds<Pixels>,
    ) -> Bounds<Pixels> {
        let origin = match self {
            Self::Left => point(
                scene_bounds.origin.x - final_bounds.size.width,
                final_bounds.origin.y,
            ),
            Self::Right => point(scene_bounds.right(), final_bounds.origin.y),
            Self::Top => point(
                final_bounds.origin.x,
                scene_bounds.origin.y - final_bounds.size.height,
            ),
            Self::Bottom => point(final_bounds.origin.x, scene_bounds.bottom()),
        };
        Bounds::new(origin, final_bounds.size)
    }
}
