use crate::DockPolicy;
use open_gpui::{Bounds, Pixels};

use super::{
    DockDropResolution, DockDropTargetValidator, DockResolvedDropTarget,
    validate_resolved_drop_target,
};

#[derive(Debug)]
pub(super) struct DockDropCandidate {
    target: DockResolvedDropTarget,
    hit_bounds: Bounds<Pixels>,
    priority: u8,
    order: usize,
}

pub(super) fn push_drop_candidate(
    candidates: &mut Vec<DockDropCandidate>,
    order: &mut usize,
    target: DockResolvedDropTarget,
    hit_bounds: Bounds<Pixels>,
) {
    candidates.push(DockDropCandidate {
        target,
        hit_bounds,
        priority: 0,
        order: *order,
    });
    *order += 1;
}

pub(super) fn push_prioritized_drop_candidate(
    candidates: &mut Vec<DockDropCandidate>,
    order: &mut usize,
    target: DockResolvedDropTarget,
    hit_bounds: Bounds<Pixels>,
    priority: u8,
) {
    candidates.push(DockDropCandidate {
        target,
        hit_bounds,
        priority,
        order: *order,
    });
    *order += 1;
}

pub(super) fn choose_drop_candidate(
    candidates: Vec<DockDropCandidate>,
    policy: &DockPolicy,
    target_validator: Option<&DockDropTargetValidator<'_>>,
) -> Option<DockDropResolution> {
    let mut best_valid = None;
    let mut best_rejection = None;

    for candidate in candidates {
        let hit_bounds = candidate.hit_bounds;
        let priority = candidate.priority;
        let order = candidate.order;
        let resolution = validate_resolved_drop_target(candidate.target, policy, target_validator);
        let slot = if resolution.is_valid() {
            &mut best_valid
        } else {
            &mut best_rejection
        };

        if candidate_beats_current(hit_bounds, priority, order, slot.as_ref()) {
            *slot = Some((hit_bounds, priority, order, resolution));
        }
    }

    best_valid
        .or(best_rejection)
        .map(|(_, _, _, resolution)| resolution)
}

fn candidate_beats_current(
    hit_bounds: Bounds<Pixels>,
    priority: u8,
    order: usize,
    current: Option<&(Bounds<Pixels>, u8, usize, DockDropResolution)>,
) -> bool {
    let Some((current_bounds, current_priority, current_order, _)) = current else {
        return true;
    };
    if priority != *current_priority {
        return priority > *current_priority;
    }
    let area = bounds_area(hit_bounds);
    let current_area = bounds_area(*current_bounds);
    area < current_area || (area == current_area && order > *current_order)
}

pub(super) fn bounds_area(bounds: Bounds<Pixels>) -> f32 {
    let width = f32::from(bounds.size.width).max(0.0);
    let height = f32::from(bounds.size.height).max(0.0);
    let area = width * height;
    if area.is_finite() {
        area
    } else {
        f32::INFINITY
    }
}
