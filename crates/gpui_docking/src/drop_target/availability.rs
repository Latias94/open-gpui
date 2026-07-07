use crate::{DockPolicy, DockPolicyError, DropZone};

use super::{
    DockDropRejection, DockDropResolution, DockDropTargetValidator, DockResolvedDropTarget,
    DockResolvedDropTargetAvailability, DockResolvedDropTargetKind,
};

pub(crate) fn validate_resolved_drop_target(
    mut target: DockResolvedDropTarget,
    policy: &DockPolicy,
    target_validator: Option<&DockDropTargetValidator<'_>>,
) -> DockDropResolution {
    target.availability = availability_allowed_by_policy(&target, policy);
    if target.is_central_dock_over_target()
        && let Err(reason) = policy.validate_central_region_dock_over()
    {
        target.availability = availability_rejected_by_reason(target.availability, &reason);
        return DockDropResolution::Rejected(DockDropRejection { target, reason });
    }

    if let Some(zone) = target.zone()
        && let Err(reason) = policy.validate_drop_zone(zone)
    {
        target.availability = availability_rejected_by_reason(target.availability, &reason);
        return DockDropResolution::Rejected(DockDropRejection { target, reason });
    }

    match target_validator.map(|validator| validator(&target)) {
        Some(Ok(())) | None => DockDropResolution::Valid(target),
        Some(Err(reason)) => {
            target.availability = availability_rejected_by_reason(target.availability, &reason);
            DockDropResolution::Rejected(DockDropRejection { target, reason })
        }
    }
}

fn availability_allowed_by_policy(
    target: &DockResolvedDropTarget,
    policy: &DockPolicy,
) -> DockResolvedDropTargetAvailability {
    let mut availability = target.availability;
    if availability.center && policy.validate_drop_zone(DropZone::Center).is_err() {
        availability.center = false;
    }
    if availability.center
        && target.is_central_dock_over_target()
        && policy.validate_central_region_dock_over().is_err()
    {
        availability.center = false;
    }
    if availability.sides && policy.validate_drop_zone(DropZone::Left).is_err() {
        availability.sides = false;
    }
    availability
}

fn availability_rejected_by_reason(
    mut availability: DockResolvedDropTargetAvailability,
    reason: &DockPolicyError,
) -> DockResolvedDropTargetAvailability {
    match reason {
        DockPolicyError::CenterMergeDisabled
        | DockPolicyError::SameStackCenterDropDisabled
        | DockPolicyError::SplitPayloadCenterMergeRejected
        | DockPolicyError::CentralRegionDockOverDisabled => {
            availability.center = false;
        }
        DockPolicyError::EdgeSplitDisabled => {
            availability.sides = false;
        }
        DockPolicyError::DockClassRejected { .. } => {
            availability.center = false;
            availability.sides = false;
        }
        DockPolicyError::FloatingDisabled
        | DockPolicyError::PlatformViewportsDisabled
        | DockPolicyError::SplitterResizeDisabled => {}
    }
    availability
}

impl DockResolvedDropTarget {
    fn is_central_dock_over_target(&self) -> bool {
        self.is_central_region
            && matches!(
                self.kind,
                DockResolvedDropTargetKind::TabBar { .. }
                    | DockResolvedDropTargetKind::LeafCenter { .. }
                    | DockResolvedDropTargetKind::EmptyDockSpace { .. }
            )
    }
}
