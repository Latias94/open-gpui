use crate::{DropZone, SplitAxis};
use open_gpui::{Bounds, Pixels, Point, point, px, size};

const MAX_EDGE_BAND: f32 = 48.0;
const MIN_EDGE_BAND: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockDropGeometry {
    pub(crate) zone: DropZone,
    pub(crate) preview_bounds: Bounds<Pixels>,
}

pub(crate) fn resolve_drop_geometry(
    bounds: Bounds<Pixels>,
    position: Point<Pixels>,
) -> Option<DockDropGeometry> {
    if !bounds.contains(&position) {
        return None;
    }

    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    if !valid_extent(width) || !valid_extent(height) {
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

    Some(DockDropGeometry {
        zone,
        preview_bounds: preview_bounds(zone, width, height, edge_band),
    })
}

pub(crate) fn splitter_handle_bounds(
    axis: SplitAxis,
    split_bounds: Bounds<Pixels>,
    shares: &[f32],
    handle_thickness: Pixels,
) -> Vec<Bounds<Pixels>> {
    if shares.len() < 2 {
        return Vec::new();
    }

    let half_thickness = handle_thickness / 2.0;
    let mut cursor = 0.0_f32;
    let mut handles = Vec::with_capacity(shares.len().saturating_sub(1));

    for share in shares.iter().take(shares.len().saturating_sub(1)) {
        cursor += *share;
        match axis {
            SplitAxis::Horizontal => {
                let x = split_bounds.origin.x + split_bounds.size.width * cursor - half_thickness;
                handles.push(Bounds::new(
                    point(x, split_bounds.origin.y),
                    size(handle_thickness, split_bounds.size.height),
                ));
            }
            SplitAxis::Vertical => {
                let y = split_bounds.origin.y + split_bounds.size.height * cursor - half_thickness;
                handles.push(Bounds::new(
                    point(split_bounds.origin.x, y),
                    size(split_bounds.size.width, handle_thickness),
                ));
            }
        }
    }

    handles
}

fn valid_extent(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

fn edge_band(width: f32, height: f32) -> f32 {
    let shortest = width.min(height);
    (shortest * 0.25)
        .clamp(MIN_EDGE_BAND, MAX_EDGE_BAND)
        .min(width / 3.0)
        .min(height / 3.0)
}

fn preview_bounds(zone: DropZone, width: f32, height: f32, edge_band: f32) -> Bounds<Pixels> {
    match zone {
        DropZone::Center => Bounds::new(point(px(0.0), px(0.0)), size(px(width), px(height))),
        DropZone::Left => Bounds::new(point(px(0.0), px(0.0)), size(px(edge_band), px(height))),
        DropZone::Right => Bounds::new(
            point(px(width - edge_band), px(0.0)),
            size(px(edge_band), px(height)),
        ),
        DropZone::Top => Bounds::new(point(px(0.0), px(0.0)), size(px(width), px(edge_band))),
        DropZone::Bottom => Bounds::new(
            point(px(0.0), px(height - edge_band)),
            size(px(width), px(edge_band)),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui::{point, px, size};

    fn bounds(width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(10.0), px(20.0)), size(px(width), px(height)))
    }

    #[test]
    fn drop_geometry_resolves_center_and_preview_bounds() {
        let geometry = resolve_drop_geometry(bounds(300.0, 200.0), point(px(160.0), px(120.0)))
            .expect("point should resolve");

        assert_eq!(geometry.zone, DropZone::Center);
        assert_eq!(geometry.preview_bounds.origin, point(px(0.0), px(0.0)));
        assert_eq!(geometry.preview_bounds.size, size(px(300.0), px(200.0)));
    }

    #[test]
    fn drop_geometry_resolves_edges_and_preview_bounds() {
        let bounds = bounds(300.0, 200.0);
        let left = resolve_drop_geometry(bounds, point(px(12.0), px(120.0)))
            .expect("left edge should resolve");
        let right = resolve_drop_geometry(bounds, point(px(308.0), px(120.0)))
            .expect("right edge should resolve");
        let top = resolve_drop_geometry(bounds, point(px(160.0), px(22.0)))
            .expect("top edge should resolve");
        let bottom = resolve_drop_geometry(bounds, point(px(160.0), px(218.0)))
            .expect("bottom edge should resolve");

        assert_eq!(left.zone, DropZone::Left);
        assert_eq!(right.zone, DropZone::Right);
        assert_eq!(top.zone, DropZone::Top);
        assert_eq!(bottom.zone, DropZone::Bottom);
        assert!(f32::from(left.preview_bounds.size.width) > 0.0);
        assert!(f32::from(right.preview_bounds.origin.x) > 0.0);
        assert!(f32::from(top.preview_bounds.size.height) > 0.0);
        assert!(f32::from(bottom.preview_bounds.origin.y) > 0.0);
    }

    #[test]
    fn small_targets_keep_center_space() {
        let geometry = resolve_drop_geometry(bounds(36.0, 36.0), point(px(28.0), px(38.0)))
            .expect("point should resolve");

        assert_eq!(geometry.zone, DropZone::Center);
    }

    #[test]
    fn invalid_drop_bounds_do_not_resolve() {
        assert!(resolve_drop_geometry(bounds(0.0, 36.0), point(px(10.0), px(20.0))).is_none());
    }

    #[test]
    fn splitter_handle_geometry_matches_fraction_boundaries() {
        let handles = splitter_handle_bounds(
            SplitAxis::Horizontal,
            bounds(400.0, 100.0),
            &[0.25, 0.75],
            px(6.0),
        );

        assert_eq!(handles.len(), 1);
        assert_eq!(handles[0].origin.x, px(107.0));
        assert_eq!(handles[0].size.width, px(6.0));
        assert_eq!(handles[0].size.height, px(100.0));
    }
}
