use crate::{
    DockItemId, DockNodeId,
    presentation_scene::{DockPresentationPaneKind, DockPresentationScene},
};
use open_gpui::{Bounds, Pixels};

const EPSILON: f32 = 0.001;

/// Direction used by docking pane spatial focus commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockSpatialDirection {
    /// Move focus to the nearest pane left of the current pane.
    Left,
    /// Move focus to the nearest pane right of the current pane.
    Right,
    /// Move focus to the nearest pane above the current pane.
    Up,
    /// Move focus to the nearest pane below the current pane.
    Down,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockSpatialNavigationTarget {
    pub(crate) tabs: DockNodeId,
    pub(crate) item: Option<DockItemId>,
    pub(crate) bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, PartialEq)]
struct DockSpatialCandidate {
    target: DockSpatialNavigationTarget,
    overlap: f32,
    distance: f32,
}

pub(crate) fn resolve_neighbor(
    scene: &DockPresentationScene,
    current_tabs: DockNodeId,
    direction: DockSpatialDirection,
) -> Option<DockSpatialNavigationTarget> {
    let current = scene
        .panes
        .iter()
        .find(|pane| {
            pane.kind == DockPresentationPaneKind::Tabs && pane.node == Some(current_tabs)
        })?
        .bounds;

    scene
        .panes
        .iter()
        .filter_map(|pane| {
            let tabs = pane.node?;
            if pane.kind != DockPresentationPaneKind::Tabs || tabs == current_tabs {
                return None;
            }
            score_candidate(scene, tabs, pane.bounds, current, direction)
        })
        .min_by(compare_candidates)
        .map(|candidate| candidate.target)
}

fn score_candidate(
    scene: &DockPresentationScene,
    tabs: DockNodeId,
    bounds: Bounds<Pixels>,
    current: Bounds<Pixels>,
    direction: DockSpatialDirection,
) -> Option<DockSpatialCandidate> {
    if !is_in_direction(bounds, current, direction) {
        return None;
    }

    let overlap = match direction {
        DockSpatialDirection::Left | DockSpatialDirection::Right => {
            overlap_1d(min_y(bounds), max_y(bounds), min_y(current), max_y(current))
        }
        DockSpatialDirection::Up | DockSpatialDirection::Down => {
            overlap_1d(min_x(bounds), max_x(bounds), min_x(current), max_x(current))
        }
    };
    let distance = match direction {
        DockSpatialDirection::Left => min_x(current) - max_x(bounds),
        DockSpatialDirection::Right => min_x(bounds) - max_x(current),
        DockSpatialDirection::Up => min_y(current) - max_y(bounds),
        DockSpatialDirection::Down => min_y(bounds) - max_y(current),
    }
    .max(0.0);

    Some(DockSpatialCandidate {
        target: DockSpatialNavigationTarget {
            tabs,
            item: selected_item_for_tabs(scene, tabs),
            bounds,
        },
        overlap,
        distance,
    })
}

fn compare_candidates(a: &DockSpatialCandidate, b: &DockSpatialCandidate) -> std::cmp::Ordering {
    b.overlap
        .total_cmp(&a.overlap)
        .then_with(|| a.distance.total_cmp(&b.distance))
        .then_with(|| a.target.tabs.as_u64().cmp(&b.target.tabs.as_u64()))
}

fn is_in_direction(
    bounds: Bounds<Pixels>,
    current: Bounds<Pixels>,
    direction: DockSpatialDirection,
) -> bool {
    match direction {
        DockSpatialDirection::Left => max_x(bounds) <= min_x(current) + EPSILON,
        DockSpatialDirection::Right => min_x(bounds) >= max_x(current) - EPSILON,
        DockSpatialDirection::Up => max_y(bounds) <= min_y(current) + EPSILON,
        DockSpatialDirection::Down => min_y(bounds) >= max_y(current) - EPSILON,
    }
}

fn selected_item_for_tabs(scene: &DockPresentationScene, tabs: DockNodeId) -> Option<DockItemId> {
    scene
        .focus_regions
        .iter()
        .find(|focus| focus.tabs == tabs)
        .map(|focus| focus.item.clone())
}

fn overlap_1d(a_min: f32, a_max: f32, b_min: f32, b_max: f32) -> f32 {
    (a_max.min(b_max) - a_min.max(b_min)).max(0.0)
}

fn min_x(bounds: Bounds<Pixels>) -> f32 {
    f32::from(bounds.origin.x)
}

fn max_x(bounds: Bounds<Pixels>) -> f32 {
    min_x(bounds) + f32::from(bounds.size.width)
}

fn min_y(bounds: Bounds<Pixels>) -> f32 {
    f32::from(bounds.origin.y)
}

fn max_y(bounds: Bounds<Pixels>) -> f32 {
    min_y(bounds) + f32::from(bounds.size.height)
}
