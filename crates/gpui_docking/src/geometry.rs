use crate::{DropZone, SplitAxis, split_fraction};
use open_gpui::{Bounds, Pixels, Point, point, px, size};

const MAX_EDGE_BAND: f32 = 48.0;
const MIN_EDGE_BAND: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockDropGeometry {
    pub(crate) zone: DropZone,
    pub(crate) preview_bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockSplitGeometry {
    pub(crate) pane_bounds: Vec<Bounds<Pixels>>,
    pub(crate) handle_hit_bounds: Vec<Bounds<Pixels>>,
    pub(crate) handle_centers: Vec<Pixels>,
    pub(crate) shares: Vec<f32>,
    pub(crate) extent: Pixels,
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

pub(crate) fn split_shares(child_count: usize, fractions: &[f32]) -> Vec<f32> {
    split_fraction::cleaned_shares(child_count, fractions)
}

pub(crate) fn split_shares_with_central(
    child_count: usize,
    fractions: &[f32],
    central_child_index: Option<usize>,
) -> Vec<f32> {
    let Some(central_child_index) = central_child_index else {
        return split_shares(child_count, fractions);
    };
    if child_count == 0 || central_child_index >= child_count {
        return split_shares(child_count, fractions);
    }
    if child_count == 1 {
        return vec![1.0];
    }

    let mut shares: Vec<f32> = (0..child_count)
        .map(|index| {
            if index == central_child_index {
                0.0
            } else {
                clean_fraction(fractions.get(index).copied().unwrap_or(0.0))
            }
        })
        .collect();

    let non_central_sum: f32 = shares.iter().sum();
    if non_central_sum > 1.0 {
        for (index, share) in shares.iter_mut().enumerate() {
            if index != central_child_index {
                *share /= non_central_sum;
            }
        }
        shares[central_child_index] = 0.0;
    } else {
        shares[central_child_index] = 1.0 - non_central_sum;
    }

    shares
}

pub(crate) fn split_handle_positions(shares: &[f32]) -> Vec<f32> {
    let mut cursor = 0.0_f32;
    shares
        .iter()
        .take(shares.len().saturating_sub(1))
        .map(|share| {
            cursor += *share;
            cursor
        })
        .collect()
}

pub(crate) fn split_geometry(
    axis: SplitAxis,
    split_bounds: Bounds<Pixels>,
    child_count: usize,
    fractions: &[f32],
    handle_thickness: Pixels,
) -> DockSplitGeometry {
    split_geometry_with_central(
        axis,
        split_bounds,
        child_count,
        fractions,
        None,
        handle_thickness,
    )
}

pub(crate) fn split_geometry_with_central(
    axis: SplitAxis,
    split_bounds: Bounds<Pixels>,
    child_count: usize,
    fractions: &[f32],
    central_child_index: Option<usize>,
    handle_thickness: Pixels,
) -> DockSplitGeometry {
    let shares = split_shares_with_central(child_count, fractions, central_child_index);
    let extent = split_extent(axis, split_bounds);
    let handle_centers = split_handle_centers(axis, split_bounds, &shares);
    let pane_bounds = split_pane_bounds(axis, split_bounds, &shares);
    let handle_hit_bounds = handle_centers
        .iter()
        .copied()
        .map(|center| split_handle_hit_bounds(axis, split_bounds, center, handle_thickness))
        .collect();

    DockSplitGeometry {
        pane_bounds,
        handle_hit_bounds,
        handle_centers,
        shares,
        extent,
    }
}

fn clean_fraction(value: f32) -> f32 {
    if value.is_finite() && value >= 0.0 {
        value
    } else {
        0.0
    }
}

pub(crate) fn splitter_handle_bounds(
    axis: SplitAxis,
    split_bounds: Bounds<Pixels>,
    shares: &[f32],
    handle_thickness: Pixels,
) -> Vec<Bounds<Pixels>> {
    split_geometry(axis, split_bounds, shares.len(), shares, handle_thickness).handle_hit_bounds
}

pub(crate) fn resize_adjacent_split_fractions(
    fractions: &[f32],
    child_count: usize,
    handle_index: usize,
    split_extent: Pixels,
    delta: Pixels,
    min_pane_size: Pixels,
) -> Option<Vec<f32>> {
    if child_count < 2 || handle_index + 1 >= child_count {
        return None;
    }

    let extent = f32::from(split_extent);
    if !extent.is_finite() || extent <= f32::EPSILON {
        return None;
    }

    let mut shares = split_shares(child_count, fractions);
    let pair_total = shares[handle_index] + shares[handle_index + 1];
    if !pair_total.is_finite() || pair_total <= f32::EPSILON {
        return None;
    }

    let min_fraction = (f32::from(min_pane_size).max(0.0) / extent).clamp(0.0, pair_total / 2.0);
    let delta_fraction = f32::from(delta) / extent;
    let next_first =
        (shares[handle_index] + delta_fraction).clamp(min_fraction, pair_total - min_fraction);

    shares[handle_index] = next_first;
    shares[handle_index + 1] = pair_total - next_first;
    split_fraction::normalize_shares(&mut shares);
    Some(shares)
}

fn valid_extent(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

fn split_extent(axis: SplitAxis, split_bounds: Bounds<Pixels>) -> Pixels {
    match axis {
        SplitAxis::Horizontal => split_bounds.size.width,
        SplitAxis::Vertical => split_bounds.size.height,
    }
}

fn split_pane_bounds(
    axis: SplitAxis,
    split_bounds: Bounds<Pixels>,
    shares: &[f32],
) -> Vec<Bounds<Pixels>> {
    let mut cursor = axis_origin(axis, split_bounds);
    let extent = split_extent(axis, split_bounds);
    shares
        .iter()
        .map(|share| {
            let pane_extent = extent * *share;
            let bounds = match axis {
                SplitAxis::Horizontal => Bounds::new(
                    point(cursor, split_bounds.origin.y),
                    size(pane_extent, split_bounds.size.height),
                ),
                SplitAxis::Vertical => Bounds::new(
                    point(split_bounds.origin.x, cursor),
                    size(split_bounds.size.width, pane_extent),
                ),
            };
            cursor += pane_extent;
            bounds
        })
        .collect()
}

fn split_handle_centers(
    axis: SplitAxis,
    split_bounds: Bounds<Pixels>,
    shares: &[f32],
) -> Vec<Pixels> {
    let origin = axis_origin(axis, split_bounds);
    let extent = split_extent(axis, split_bounds);
    split_handle_positions(shares)
        .into_iter()
        .map(|position| origin + extent * position)
        .collect()
}

fn split_handle_hit_bounds(
    axis: SplitAxis,
    split_bounds: Bounds<Pixels>,
    center: Pixels,
    handle_thickness: Pixels,
) -> Bounds<Pixels> {
    let half_thickness = handle_thickness / 2.0;
    match axis {
        SplitAxis::Horizontal => Bounds::new(
            point(center - half_thickness, split_bounds.origin.y),
            size(handle_thickness, split_bounds.size.height),
        ),
        SplitAxis::Vertical => Bounds::new(
            point(split_bounds.origin.x, center - half_thickness),
            size(split_bounds.size.width, handle_thickness),
        ),
    }
}

fn axis_origin(axis: SplitAxis, split_bounds: Bounds<Pixels>) -> Pixels {
    match axis {
        SplitAxis::Horizontal => split_bounds.origin.x,
        SplitAxis::Vertical => split_bounds.origin.y,
    }
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
        let geometry = split_geometry(
            SplitAxis::Horizontal,
            bounds(400.0, 100.0),
            2,
            &[0.25, 0.75],
            px(6.0),
        );

        assert_eq!(geometry.pane_bounds.len(), 2);
        assert_eq!(geometry.pane_bounds[0].origin.x, px(10.0));
        assert_eq!(geometry.pane_bounds[0].size.width, px(100.0));
        assert_eq!(geometry.pane_bounds[1].origin.x, px(110.0));
        assert_eq!(geometry.pane_bounds[1].size.width, px(300.0));
        assert_eq!(geometry.handle_centers, vec![px(110.0)]);
        assert_eq!(geometry.handle_hit_bounds.len(), 1);
        assert_eq!(geometry.handle_hit_bounds[0].origin.x, px(107.0));
        assert_eq!(geometry.handle_hit_bounds[0].size.width, px(6.0));
        assert_eq!(geometry.handle_hit_bounds[0].size.height, px(100.0));
    }

    #[test]
    fn vertical_split_geometry_matches_fraction_boundaries() {
        let geometry = split_geometry(
            SplitAxis::Vertical,
            bounds(200.0, 400.0),
            2,
            &[0.25, 0.75],
            px(8.0),
        );

        assert_eq!(geometry.pane_bounds[0].origin.y, px(20.0));
        assert_eq!(geometry.pane_bounds[0].size.height, px(100.0));
        assert_eq!(geometry.pane_bounds[1].origin.y, px(120.0));
        assert_eq!(geometry.pane_bounds[1].size.height, px(300.0));
        assert_eq!(geometry.handle_centers, vec![px(120.0)]);
        assert_eq!(geometry.handle_hit_bounds[0].origin.y, px(116.0));
        assert_eq!(geometry.handle_hit_bounds[0].size.height, px(8.0));
    }

    #[test]
    fn split_geometry_repairs_fraction_input_once() {
        let geometry = split_geometry(
            SplitAxis::Horizontal,
            bounds(300.0, 100.0),
            3,
            &[f32::NAN],
            px(6.0),
        );

        assert_eq!(geometry.pane_bounds.len(), 3);
        assert_eq!(geometry.handle_hit_bounds.len(), 2);
        assert_close(geometry.shares.iter().sum(), 1.0);
        assert_close(geometry.shares[0], 0.0);
        assert_close(geometry.shares[1], 0.5);
        assert_close(geometry.shares[2], 0.5);
    }

    #[test]
    fn central_split_child_receives_remaining_space() {
        let geometry = split_geometry_with_central(
            SplitAxis::Horizontal,
            bounds(1000.0, 100.0),
            3,
            &[0.2, 0.0, 0.3],
            Some(1),
            px(6.0),
        );

        assert_close(geometry.shares[0], 0.2);
        assert_close(geometry.shares[1], 0.5);
        assert_close(geometry.shares[2], 0.3);
        assert_eq!(geometry.pane_bounds[0].size.width, px(200.0));
        assert_eq!(geometry.pane_bounds[1].size.width, px(500.0));
        assert_eq!(geometry.pane_bounds[2].size.width, px(300.0));
    }

    #[test]
    fn central_split_child_yields_space_when_neighbors_over_allocate() {
        let geometry = split_geometry_with_central(
            SplitAxis::Horizontal,
            bounds(1000.0, 100.0),
            3,
            &[0.8, 0.0, 0.7],
            Some(1),
            px(6.0),
        );

        assert_close(geometry.shares[0], 0.5333);
        assert_close(geometry.shares[1], 0.0);
        assert_close(geometry.shares[2], 0.4667);
        assert_close(geometry.shares.iter().sum(), 1.0);
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 0.001,
            "expected {actual} to be close to {expected}"
        );
    }

    #[test]
    fn positive_resize_delta_grows_first_adjacent_pane() {
        let next =
            resize_adjacent_split_fractions(&[0.25, 0.75], 2, 0, px(400.0), px(40.0), px(48.0))
                .expect("resize should be valid");

        assert_close(next[0], 0.35);
        assert_close(next[1], 0.65);
    }

    #[test]
    fn negative_resize_delta_shrinks_first_adjacent_pane() {
        let next =
            resize_adjacent_split_fractions(&[0.5, 0.5], 2, 0, px(400.0), px(-80.0), px(48.0))
                .expect("resize should be valid");

        assert_close(next[0], 0.3);
        assert_close(next[1], 0.7);
    }

    #[test]
    fn resize_clamps_at_minimum_pane_size() {
        let next =
            resize_adjacent_split_fractions(&[0.5, 0.5], 2, 0, px(400.0), px(-300.0), px(100.0))
                .expect("resize should be valid");

        assert_close(next[0], 0.25);
        assert_close(next[1], 0.75);
    }

    #[test]
    fn impossible_minimum_splits_adjacent_pair_evenly() {
        let next =
            resize_adjacent_split_fractions(&[0.5, 0.5], 2, 0, px(120.0), px(100.0), px(80.0))
                .expect("resize should be valid");

        assert_close(next[0], 0.5);
        assert_close(next[1], 0.5);
    }

    #[test]
    fn invalid_resize_handle_index_returns_none() {
        assert!(
            resize_adjacent_split_fractions(&[0.5, 0.5], 2, 1, px(400.0), px(10.0), px(48.0))
                .is_none()
        );
    }
}
