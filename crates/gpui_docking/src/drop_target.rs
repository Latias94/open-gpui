use crate::{
    DockNodeId, DockPolicy, DockPolicyError, DropZone,
    geometry::{self, DockDropGeometry},
};
use open_gpui::{Bounds, Pixels, Point};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockDropIntent {
    pub(crate) target_tabs: DockNodeId,
    pub(crate) zone: DropZone,
    pub(crate) preview_bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum DockDropResolution {
    Valid(DockDropIntent),
    Rejected(DockDropRejection),
}

impl DockDropResolution {
    pub(crate) fn intent(self) -> Option<DockDropIntent> {
        match self {
            Self::Valid(intent) => Some(intent),
            Self::Rejected(rejection) => {
                let _ = (
                    rejection.target_tabs,
                    rejection.zone,
                    rejection.preview_bounds,
                    rejection.reason,
                );
                None
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockDropRejection {
    pub(crate) target_tabs: DockNodeId,
    pub(crate) zone: DropZone,
    pub(crate) preview_bounds: Bounds<Pixels>,
    pub(crate) reason: DockPolicyError,
}

pub(crate) fn resolve_tabs_drop(
    target_tabs: DockNodeId,
    bounds: Bounds<Pixels>,
    position: Point<Pixels>,
    policy: &DockPolicy,
) -> Option<DockDropResolution> {
    let geometry = geometry::resolve_drop_geometry(bounds, position)?;
    Some(match policy.validate_drop_zone(geometry.zone) {
        Ok(()) => DockDropResolution::Valid(intent_from_geometry(target_tabs, geometry)),
        Err(reason) => DockDropResolution::Rejected(DockDropRejection {
            target_tabs,
            zone: geometry.zone,
            preview_bounds: geometry.preview_bounds,
            reason,
        }),
    })
}

fn intent_from_geometry(target_tabs: DockNodeId, geometry: DockDropGeometry) -> DockDropIntent {
    DockDropIntent {
        target_tabs,
        zone: geometry.zone,
        preview_bounds: geometry.preview_bounds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui::{point, px, size};
    use slotmap::Key;

    fn tabs() -> DockNodeId {
        DockNodeId::null()
    }

    fn bounds(width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(10.0), px(20.0)), size(px(width), px(height)))
    }

    fn policy() -> DockPolicy {
        DockPolicy::default()
    }

    #[test]
    fn center_point_resolves_to_center_zone() {
        let intent = resolve_tabs_drop(
            tabs(),
            bounds(300.0, 200.0),
            point(px(160.0), px(120.0)),
            &policy(),
        )
        .and_then(DockDropResolution::intent)
        .expect("point should resolve");

        assert_eq!(intent.zone, DropZone::Center);
        assert_eq!(intent.preview_bounds.size, size(px(300.0), px(200.0)));
    }

    #[test]
    fn edge_points_resolve_to_matching_zones() {
        let bounds = bounds(300.0, 200.0);

        assert_eq!(
            resolve_tabs_drop(tabs(), bounds, point(px(12.0), px(120.0)), &policy())
                .and_then(DockDropResolution::intent)
                .map(|intent| intent.zone),
            Some(DropZone::Left)
        );
        assert_eq!(
            resolve_tabs_drop(tabs(), bounds, point(px(308.0), px(120.0)), &policy())
                .and_then(DockDropResolution::intent)
                .map(|intent| intent.zone),
            Some(DropZone::Right)
        );
        assert_eq!(
            resolve_tabs_drop(tabs(), bounds, point(px(160.0), px(22.0)), &policy())
                .and_then(DockDropResolution::intent)
                .map(|intent| intent.zone),
            Some(DropZone::Top)
        );
        assert_eq!(
            resolve_tabs_drop(tabs(), bounds, point(px(160.0), px(218.0)), &policy())
                .and_then(DockDropResolution::intent)
                .map(|intent| intent.zone),
            Some(DropZone::Bottom)
        );
    }

    #[test]
    fn outside_points_do_not_resolve() {
        assert!(
            resolve_tabs_drop(
                tabs(),
                bounds(300.0, 200.0),
                point(px(500.0), px(120.0)),
                &policy()
            )
            .is_none()
        );
    }

    #[test]
    fn small_targets_still_leave_center_space() {
        let bounds = bounds(36.0, 36.0);

        assert_eq!(
            resolve_tabs_drop(tabs(), bounds, point(px(28.0), px(38.0)), &policy())
                .and_then(DockDropResolution::intent)
                .map(|intent| intent.zone),
            Some(DropZone::Center)
        );
    }

    #[test]
    fn disabled_edge_split_returns_rejection_without_intent() {
        let mut policy = DockPolicy::default();
        policy.set_allow_edge_split(false);
        let resolution = resolve_tabs_drop(
            tabs(),
            bounds(300.0, 200.0),
            point(px(12.0), px(120.0)),
            &policy,
        )
        .expect("edge point should resolve to a policy result");

        let DockDropResolution::Rejected(rejection) = resolution else {
            panic!("edge split should be rejected");
        };
        assert_eq!(rejection.zone, DropZone::Left);
        assert_eq!(rejection.reason, DockPolicyError::EdgeSplitDisabled);
    }
}
