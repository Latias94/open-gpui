use crate::{
    DockNodeId, DropZone, SplitAxis,
    geometry::{bounds_from_motion_rect, motion_rect_from_bounds},
    presentation_scene::{
        DockPresentationFocusRegion, DockPresentationPane, DockPresentationScene,
    },
    visual_affordance_scene::{
        DockVisualAffordanceId, DockVisualAffordanceKind, DockVisualAffordanceLayer,
        DockVisualAffordanceScene,
    },
    zoom_state::DockZoomScene,
};
use open_gpui::{Bounds, Pixels};
use open_gpui_motion::{MotionEdge, MotionPreference, motion_source_rect, preferred_motion_edge};
use std::collections::{HashMap, HashSet};

/// Descriptor plan for docking presentation, divider, and visual affordance transitions.
#[derive(Debug, Clone, PartialEq)]
pub struct DockTransitionPlan {
    pub(crate) preference: DockMotionPreference,
    pub(crate) final_scene: DockPresentationScene,
    pub(crate) pane_transitions: Vec<DockPaneTransition>,
    pub(crate) divider_transitions: Vec<DockDividerTransition>,
    pub(crate) visual_affordance_transitions: Vec<DockVisualAffordanceTransition>,
}

pub(crate) type DockMotionPreference = MotionPreference;
pub(crate) type DockTransitionEdge = MotionEdge;

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
pub(crate) struct DockVisualAffordanceTransition {
    pub(crate) motion_key: DockVisualAffordanceId,
    pub(crate) kind: DockVisualAffordanceTransitionKind,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) target_node: Option<DockNodeId>,
    pub(crate) zone: Option<DropZone>,
    pub(crate) payload_index: Option<usize>,
    pub(crate) immediate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DockVisualAffordanceTransitionKind {
    RouteMarker,
    TargetBody,
    GuideBox,
    TabInsertion,
    PayloadTab,
    PayloadGhost,
    FocusRing,
    RejectedNoop,
}

impl DockVisualAffordanceTransitionKind {
    pub(crate) fn from_affordance_kind(kind: DockVisualAffordanceKind) -> Self {
        match kind {
            DockVisualAffordanceKind::RouteMarker => Self::RouteMarker,
            DockVisualAffordanceKind::DropTargetBody => Self::TargetBody,
            DockVisualAffordanceKind::GuideBox => Self::GuideBox,
            DockVisualAffordanceKind::TabInsertionSlot => Self::TabInsertion,
            DockVisualAffordanceKind::PayloadTab => Self::PayloadTab,
            DockVisualAffordanceKind::PayloadGhost => Self::PayloadGhost,
            DockVisualAffordanceKind::FocusRing => Self::FocusRing,
            DockVisualAffordanceKind::RejectedTarget
            | DockVisualAffordanceKind::DividerHandle
            | DockVisualAffordanceKind::DividerCorner
            | DockVisualAffordanceKind::ZoomEgress => Self::RejectedNoop,
        }
    }
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
            visual_affordance_transitions: Vec::new(),
        }
    }

    pub(crate) fn from_visual_affordance_scene(
        final_scene: &DockPresentationScene,
        affordance_scene: &DockVisualAffordanceScene,
        preference: DockMotionPreference,
    ) -> Self {
        Self {
            preference,
            final_scene: final_scene.clone(),
            pane_transitions: Vec::new(),
            divider_transitions: Vec::new(),
            visual_affordance_transitions: affordance_scene
                .layers
                .iter()
                .map(|layer| visual_affordance_transition_from_layer(layer, preference))
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
                let source_bounds =
                    source_bounds_for_edge(egress.edge, egress.from, previous.bounds);
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
            plan.visual_affordance_transitions
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
            visual_affordance_transitions: vec![focus_ring_transition(focus, preference)],
        }
    }

    pub(crate) fn is_immediate(&self) -> bool {
        self.preference.is_immediate()
            && self.pane_transitions.iter().all(|item| item.immediate)
            && self.divider_transitions.iter().all(|item| item.immediate)
            && self
                .visual_affordance_transitions
                .iter()
                .all(|item| item.immediate)
    }
}

fn focus_ring_transition(
    focus: &DockPresentationFocusRegion,
    preference: DockMotionPreference,
) -> DockVisualAffordanceTransition {
    let motion_key = DockVisualAffordanceId {
        kind: DockVisualAffordanceKind::FocusRing,
        target_node: Some(focus.tabs),
        zone: None,
        layer_scope: crate::visual_affordance_scene::DockVisualLayerScope::Focus,
        payload_index: None,
        serial: None,
    };
    DockVisualAffordanceTransition {
        motion_key,
        kind: DockVisualAffordanceTransitionKind::FocusRing,
        bounds: focus.bounds,
        target_node: Some(focus.tabs),
        zone: None,
        payload_index: None,
        immediate: preference.is_immediate(),
    }
}

fn visual_affordance_transition_from_layer(
    layer: &DockVisualAffordanceLayer,
    preference: DockMotionPreference,
) -> DockVisualAffordanceTransition {
    DockVisualAffordanceTransition {
        motion_key: layer.motion_key.clone(),
        kind: DockVisualAffordanceTransitionKind::from_affordance_kind(layer.kind),
        bounds: layer.bounds,
        target_node: layer.target_node,
        zone: layer.zone,
        payload_index: layer.payload_index,
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
        source_bounds: source_bounds_for_edge(edge, final_bounds, scene_bounds),
        edge,
        final_bounds,
        occlusion_bounds: final_bounds,
    }
}

pub(crate) fn preferred_transition_edge(
    bounds: Bounds<Pixels>,
    scene_bounds: Bounds<Pixels>,
) -> DockTransitionEdge {
    preferred_motion_edge(
        motion_rect_from_bounds(bounds),
        motion_rect_from_bounds(scene_bounds),
    )
}

fn source_bounds_for_edge(
    edge: DockTransitionEdge,
    final_bounds: Bounds<Pixels>,
    scene_bounds: Bounds<Pixels>,
) -> Bounds<Pixels> {
    bounds_from_motion_rect(motion_source_rect(
        edge,
        motion_rect_from_bounds(final_bounds),
        motion_rect_from_bounds(scene_bounds),
    ))
}
