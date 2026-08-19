use crate::DockPolicy;
use open_gpui::{Bounds, Pixels};

use super::{
    DockDropResolution, DockDropTargetValidator, DockResolvedDropTarget,
    validate_resolved_drop_target,
};

type RankedResolution = (Bounds<Pixels>, u8, usize, DockDropResolution);

pub(super) struct DockDropCandidateAccumulator<'a> {
    policy: &'a DockPolicy,
    target_validator: Option<&'a DockDropTargetValidator<'a>>,
    next_order: usize,
    best_valid: Option<RankedResolution>,
    best_rejection: Option<RankedResolution>,
}

impl<'a> DockDropCandidateAccumulator<'a> {
    pub(super) fn new(
        policy: &'a DockPolicy,
        target_validator: Option<&'a DockDropTargetValidator<'a>>,
    ) -> Self {
        Self {
            policy,
            target_validator,
            next_order: 0,
            best_valid: None,
            best_rejection: None,
        }
    }

    pub(super) fn push(&mut self, target: DockResolvedDropTarget, hit_bounds: Bounds<Pixels>) {
        self.push_with_priority(target, hit_bounds, 0);
    }

    pub(super) fn push_with_priority(
        &mut self,
        target: DockResolvedDropTarget,
        hit_bounds: Bounds<Pixels>,
        priority: u8,
    ) {
        let order = self.next_order;
        self.next_order += 1;
        let resolution = validate_resolved_drop_target(target, self.policy, self.target_validator);
        let slot = if resolution.is_valid() {
            &mut self.best_valid
        } else {
            &mut self.best_rejection
        };

        if candidate_beats_current(hit_bounds, priority, order, slot.as_ref()) {
            *slot = Some((hit_bounds, priority, order, resolution));
        }
    }

    pub(super) fn finish(self) -> Option<DockDropResolution> {
        self.best_valid
            .or(self.best_rejection)
            .map(|(_, _, _, resolution)| resolution)
    }
}

fn candidate_beats_current(
    hit_bounds: Bounds<Pixels>,
    priority: u8,
    order: usize,
    current: Option<&RankedResolution>,
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
