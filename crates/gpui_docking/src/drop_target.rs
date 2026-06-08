use crate::{DockNodeId, DropZone};
use open_gpui::{Bounds, Pixels, Point};

const MAX_EDGE_BAND: f32 = 48.0;
const MIN_EDGE_BAND: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DockDropIntent {
    pub(crate) target_tabs: DockNodeId,
    pub(crate) zone: DropZone,
}

pub(crate) fn resolve_tabs_drop(
    target_tabs: DockNodeId,
    bounds: Bounds<Pixels>,
    position: Point<Pixels>,
) -> Option<DockDropIntent> {
    if !bounds.contains(&position) {
        return None;
    }

    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return None;
    }

    let edge_band = edge_band(width, height);
    let x = f32::from(position.x - bounds.origin.x);
    let y = f32::from(position.y - bounds.origin.y);
    let distances = [
        (DropZone::Left, x),
        (DropZone::Right, width - x),
        (DropZone::Top, y),
        (DropZone::Bottom, height - y),
    ];

    let zone = distances
        .into_iter()
        .filter(|(_, distance)| *distance <= edge_band)
        .min_by(|(_, left), (_, right)| left.total_cmp(right))
        .map(|(zone, _)| zone)
        .unwrap_or(DropZone::Center);

    Some(DockDropIntent { target_tabs, zone })
}

fn edge_band(width: f32, height: f32) -> f32 {
    let shortest = width.min(height);
    (shortest * 0.25)
        .clamp(MIN_EDGE_BAND, MAX_EDGE_BAND)
        .min(width / 3.0)
        .min(height / 3.0)
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

    #[test]
    fn center_point_resolves_to_center_zone() {
        let intent = resolve_tabs_drop(tabs(), bounds(300.0, 200.0), point(px(160.0), px(120.0)))
            .expect("point should resolve");

        assert_eq!(intent.zone, DropZone::Center);
    }

    #[test]
    fn edge_points_resolve_to_matching_zones() {
        let bounds = bounds(300.0, 200.0);

        assert_eq!(
            resolve_tabs_drop(tabs(), bounds, point(px(12.0), px(120.0))).map(|intent| intent.zone),
            Some(DropZone::Left)
        );
        assert_eq!(
            resolve_tabs_drop(tabs(), bounds, point(px(308.0), px(120.0)))
                .map(|intent| intent.zone),
            Some(DropZone::Right)
        );
        assert_eq!(
            resolve_tabs_drop(tabs(), bounds, point(px(160.0), px(22.0))).map(|intent| intent.zone),
            Some(DropZone::Top)
        );
        assert_eq!(
            resolve_tabs_drop(tabs(), bounds, point(px(160.0), px(218.0)))
                .map(|intent| intent.zone),
            Some(DropZone::Bottom)
        );
    }

    #[test]
    fn outside_points_do_not_resolve() {
        assert!(
            resolve_tabs_drop(tabs(), bounds(300.0, 200.0), point(px(500.0), px(120.0))).is_none()
        );
    }

    #[test]
    fn small_targets_still_leave_center_space() {
        let bounds = bounds(36.0, 36.0);

        assert_eq!(
            resolve_tabs_drop(tabs(), bounds, point(px(28.0), px(38.0))).map(|intent| intent.zone),
            Some(DropZone::Center)
        );
    }
}
