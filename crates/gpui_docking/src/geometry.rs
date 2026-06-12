use crate::{DropZone, SplitAxis, split_fraction};
use open_gpui::{Bounds, Pixels, Point, point, px, size};

const ASSUMED_DOCK_WIDGET_FONT_SIZE: f32 = 16.0;
const MAX_SPLIT_PREVIEW_EXTENT: f32 = 48.0;
const MIN_SPLIT_PREVIEW_EXTENT: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockDropGeometry {
    pub(crate) drop_box: DockDropBox,
}

impl DockDropGeometry {
    pub(crate) fn zone(&self) -> DropZone {
        self.drop_box.kind.zone()
    }

    pub(crate) fn preview_bounds(&self) -> Bounds<Pixels> {
        self.drop_box.preview_bounds
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockDropBox {
    pub(crate) kind: DockDropBoxKind,
    pub(crate) hit_bounds: Bounds<Pixels>,
    pub(crate) preview_bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockDropBoxKind {
    Center,
    InnerEdge(DropZone),
    OuterEdge(DropZone),
}

impl DockDropBoxKind {
    pub(crate) fn zone(self) -> DropZone {
        match self {
            Self::Center => DropZone::Center,
            Self::InnerEdge(zone) | Self::OuterEdge(zone) => zone,
        }
    }

    pub(crate) fn is_center(self) -> bool {
        matches!(self, Self::Center)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockDropBoxSet {
    Inner,
    Outer,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DockDropBoxMetrics {
    center_half: f32,
    inner_side_half_long: f32,
    inner_side_half_short: f32,
    inner_offset: f32,
    outer_side_half_long: f32,
    outer_side_half_short: f32,
    outer_offset_x: f32,
    outer_offset_y: f32,
    split_preview_extent: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct LocalRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl LocalRect {
    fn from_center(center_x: f32, center_y: f32, half_width: f32, half_height: f32) -> Self {
        Self {
            x: center_x - half_width,
            y: center_y - half_height,
            width: half_width * 2.0,
            height: half_height * 2.0,
        }
    }

    fn clamp_to_bounds(self, width: f32, height: f32) -> Option<Self> {
        let min_x = self.x.clamp(0.0, width);
        let min_y = self.y.clamp(0.0, height);
        let max_x = (self.x + self.width).clamp(0.0, width);
        let max_y = (self.y + self.height).clamp(0.0, height);
        if max_x <= min_x || max_y <= min_y {
            return None;
        }
        Some(Self {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockSplitGeometry {
    pub(crate) pane_bounds: Vec<Bounds<Pixels>>,
    pub(crate) handle_hit_bounds: Vec<Bounds<Pixels>>,
    pub(crate) handle_centers: Vec<Pixels>,
    pub(crate) shares: Vec<f32>,
    pub(crate) extent: Pixels,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockSplitLayout {
    shares: Vec<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockSplitHandleLayout {
    pub(crate) index: usize,
    pub(crate) center_share: f32,
}

pub(crate) fn resolve_inner_drop_geometry(
    bounds: Bounds<Pixels>,
    position: Point<Pixels>,
) -> Option<DockDropGeometry> {
    resolve_drop_geometry(bounds, position, DockDropBoxSet::Inner)
}

pub(crate) fn resolve_outer_drop_geometry(
    bounds: Bounds<Pixels>,
    position: Point<Pixels>,
) -> Option<DockDropGeometry> {
    resolve_drop_geometry(bounds, position, DockDropBoxSet::Outer)
}

pub(crate) fn drop_boxes(bounds: Bounds<Pixels>, set: DockDropBoxSet) -> Vec<DockDropBox> {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    if !valid_extent(width) || !valid_extent(height) {
        return Vec::new();
    }

    let metrics = drop_box_metrics(width, height);
    let mut boxes = Vec::new();
    match set {
        DockDropBoxSet::Inner => {
            boxes.push(drop_box(
                bounds,
                DockDropBoxKind::Center,
                LocalRect::from_center(
                    width / 2.0,
                    height / 2.0,
                    metrics.center_half,
                    metrics.center_half,
                ),
                preview_bounds(
                    DropZone::Center,
                    width,
                    height,
                    metrics.split_preview_extent,
                ),
            ));
            boxes.extend(edge_drop_boxes(bounds, DockDropBoxKind::InnerEdge, metrics));
        }
        DockDropBoxSet::Outer => {
            boxes.extend(edge_drop_boxes(bounds, DockDropBoxKind::OuterEdge, metrics));
        }
    }
    boxes
}

fn resolve_drop_geometry(
    bounds: Bounds<Pixels>,
    position: Point<Pixels>,
    set: DockDropBoxSet,
) -> Option<DockDropGeometry> {
    if !bounds.contains(&position) {
        return None;
    }

    drop_boxes(bounds, set)
        .into_iter()
        .find(|drop_box| drop_box.hit_bounds.contains(&position))
        .map(|drop_box| DockDropGeometry { drop_box })
}

fn split_shares(child_count: usize, fractions: &[f32]) -> Vec<f32> {
    split_fraction::cleaned_shares(child_count, fractions)
}

fn split_shares_with_central(
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

impl DockSplitLayout {
    pub(crate) fn from_fractions(
        child_count: usize,
        fractions: &[f32],
        central_child_index: Option<usize>,
    ) -> Self {
        Self {
            shares: split_shares_with_central(child_count, fractions, central_child_index),
        }
    }

    pub(crate) fn child_share(&self, index: usize) -> Option<f32> {
        self.shares.get(index).copied()
    }

    pub(crate) fn handles(&self) -> Vec<DockSplitHandleLayout> {
        let mut cursor = 0.0_f32;
        self.shares
            .iter()
            .take(self.shares.len().saturating_sub(1))
            .enumerate()
            .map(|(index, share)| {
                cursor += *share;
                DockSplitHandleLayout {
                    index,
                    center_share: cursor,
                }
            })
            .collect()
    }

    pub(crate) fn geometry(
        &self,
        axis: SplitAxis,
        split_bounds: Bounds<Pixels>,
        handle_thickness: Pixels,
    ) -> DockSplitGeometry {
        let extent = split_extent(axis, split_bounds);
        let handle_centers = split_handle_centers(axis, split_bounds, self.handles());
        let pane_bounds = split_pane_bounds(axis, split_bounds, &self.shares);
        let handle_hit_bounds = handle_centers
            .iter()
            .copied()
            .map(|center| split_handle_hit_bounds(axis, split_bounds, center, handle_thickness))
            .collect();

        DockSplitGeometry {
            pane_bounds,
            handle_hit_bounds,
            handle_centers,
            shares: self.shares.clone(),
            extent,
        }
    }
}

fn clean_fraction(value: f32) -> f32 {
    if value.is_finite() && value >= 0.0 {
        value
    } else {
        0.0
    }
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
    handles: Vec<DockSplitHandleLayout>,
) -> Vec<Pixels> {
    let origin = axis_origin(axis, split_bounds);
    let extent = split_extent(axis, split_bounds);
    handles
        .into_iter()
        .map(|handle| origin + extent * handle.center_share)
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

fn drop_box_metrics(width: f32, height: f32) -> DockDropBoxMetrics {
    let shortest = width.min(height);
    let central_half = (ASSUMED_DOCK_WIDGET_FONT_SIZE * 1.5)
        .min((ASSUMED_DOCK_WIDGET_FONT_SIZE * 0.5).max(shortest / 8.0));
    let center_half = central_half.min(width / 2.0).min(height / 2.0);
    let inner_side_half_long = center_half;
    let inner_side_half_short = (center_half * 0.9).min(width / 2.0).min(height / 2.0);
    let inner_offset = center_half * 2.4;
    let outer_side_half_long = (central_half * 1.5).min(width / 2.0).min(height / 2.0);
    let outer_side_half_short = (central_half * 0.8).min(width / 2.0).min(height / 2.0);
    let outer_offset_x = (width / 2.0 - outer_side_half_short).max(0.0);
    let outer_offset_y = (height / 2.0 - outer_side_half_short).max(0.0);
    let split_preview_extent = (shortest * 0.25)
        .clamp(MIN_SPLIT_PREVIEW_EXTENT, MAX_SPLIT_PREVIEW_EXTENT)
        .min(width / 3.0)
        .min(height / 3.0);
    DockDropBoxMetrics {
        center_half,
        inner_side_half_long,
        inner_side_half_short,
        inner_offset,
        outer_side_half_long,
        outer_side_half_short,
        outer_offset_x,
        outer_offset_y,
        split_preview_extent,
    }
}

fn edge_drop_boxes(
    bounds: Bounds<Pixels>,
    kind: fn(DropZone) -> DockDropBoxKind,
    metrics: DockDropBoxMetrics,
) -> impl Iterator<Item = DockDropBox> {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    [
        edge_drop_box(bounds, kind(DropZone::Left), metrics),
        edge_drop_box(bounds, kind(DropZone::Right), metrics),
        edge_drop_box(bounds, kind(DropZone::Top), metrics),
        edge_drop_box(bounds, kind(DropZone::Bottom), metrics),
    ]
    .into_iter()
    .flatten()
    .filter(move |drop_box| {
        let hit = drop_box.hit_bounds;
        f32::from(hit.size.width) > 0.0
            && f32::from(hit.size.height) > 0.0
            && f32::from(hit.origin.x - bounds.origin.x) <= width
            && f32::from(hit.origin.y - bounds.origin.y) <= height
    })
}

fn edge_drop_box(
    bounds: Bounds<Pixels>,
    kind: DockDropBoxKind,
    metrics: DockDropBoxMetrics,
) -> Option<DockDropBox> {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    let center_x = width / 2.0;
    let center_y = height / 2.0;
    let zone = kind.zone();
    let hit = match kind {
        DockDropBoxKind::Center => return None,
        DockDropBoxKind::InnerEdge(DropZone::Left) => LocalRect::from_center(
            center_x - metrics.inner_offset,
            center_y,
            metrics.inner_side_half_short,
            metrics.inner_side_half_long,
        ),
        DockDropBoxKind::InnerEdge(DropZone::Right) => LocalRect::from_center(
            center_x + metrics.inner_offset,
            center_y,
            metrics.inner_side_half_short,
            metrics.inner_side_half_long,
        ),
        DockDropBoxKind::InnerEdge(DropZone::Top) => LocalRect::from_center(
            center_x,
            center_y - metrics.inner_offset,
            metrics.inner_side_half_long,
            metrics.inner_side_half_short,
        ),
        DockDropBoxKind::InnerEdge(DropZone::Bottom) => LocalRect::from_center(
            center_x,
            center_y + metrics.inner_offset,
            metrics.inner_side_half_long,
            metrics.inner_side_half_short,
        ),
        DockDropBoxKind::OuterEdge(DropZone::Left) => LocalRect::from_center(
            center_x - metrics.outer_offset_x,
            center_y,
            metrics.outer_side_half_short,
            metrics.outer_side_half_long,
        ),
        DockDropBoxKind::OuterEdge(DropZone::Right) => LocalRect::from_center(
            center_x + metrics.outer_offset_x,
            center_y,
            metrics.outer_side_half_short,
            metrics.outer_side_half_long,
        ),
        DockDropBoxKind::OuterEdge(DropZone::Top) => LocalRect::from_center(
            center_x,
            center_y - metrics.outer_offset_y,
            metrics.outer_side_half_long,
            metrics.outer_side_half_short,
        ),
        DockDropBoxKind::OuterEdge(DropZone::Bottom) => LocalRect::from_center(
            center_x,
            center_y + metrics.outer_offset_y,
            metrics.outer_side_half_long,
            metrics.outer_side_half_short,
        ),
        DockDropBoxKind::InnerEdge(DropZone::Center)
        | DockDropBoxKind::OuterEdge(DropZone::Center) => return None,
    };
    let hit_bounds = local_bounds(bounds.origin, hit.clamp_to_bounds(width, height)?);
    let preview_bounds = offset_bounds(
        bounds.origin,
        preview_bounds(zone, width, height, metrics.split_preview_extent),
    );
    Some(DockDropBox {
        kind,
        hit_bounds,
        preview_bounds,
    })
}

fn drop_box(
    bounds: Bounds<Pixels>,
    kind: DockDropBoxKind,
    hit: LocalRect,
    preview: Bounds<Pixels>,
) -> DockDropBox {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    let hit_bounds = local_bounds(
        bounds.origin,
        hit.clamp_to_bounds(width, height)
            .expect("center drop box should fit inside valid bounds"),
    );
    DockDropBox {
        kind,
        hit_bounds,
        preview_bounds: offset_bounds(bounds.origin, preview),
    }
}

fn preview_bounds(
    zone: DropZone,
    width: f32,
    height: f32,
    split_preview_extent: f32,
) -> Bounds<Pixels> {
    match zone {
        DropZone::Center => Bounds::new(point(px(0.0), px(0.0)), size(px(width), px(height))),
        DropZone::Left => Bounds::new(
            point(px(0.0), px(0.0)),
            size(px(split_preview_extent), px(height)),
        ),
        DropZone::Right => Bounds::new(
            point(px(width - split_preview_extent), px(0.0)),
            size(px(split_preview_extent), px(height)),
        ),
        DropZone::Top => Bounds::new(
            point(px(0.0), px(0.0)),
            size(px(width), px(split_preview_extent)),
        ),
        DropZone::Bottom => Bounds::new(
            point(px(0.0), px(height - split_preview_extent)),
            size(px(width), px(split_preview_extent)),
        ),
    }
}

fn local_bounds(origin: Point<Pixels>, rect: LocalRect) -> Bounds<Pixels> {
    Bounds::new(
        point(origin.x + px(rect.x), origin.y + px(rect.y)),
        size(px(rect.width), px(rect.height)),
    )
}

fn offset_bounds(origin: Point<Pixels>, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        point(origin.x + bounds.origin.x, origin.y + bounds.origin.y),
        bounds.size,
    )
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
        let bounds = bounds(300.0, 200.0);
        let geometry = resolve_inner_drop_geometry(bounds, point(px(160.0), px(120.0)))
            .expect("point should resolve");

        assert_eq!(geometry.drop_box.kind, DockDropBoxKind::Center);
        assert_eq!(geometry.zone(), DropZone::Center);
        assert_eq!(geometry.preview_bounds().origin, bounds.origin);
        assert_eq!(geometry.preview_bounds().size, size(px(300.0), px(200.0)));
    }

    #[test]
    fn inner_drop_geometry_resolves_only_explicit_edge_boxes() {
        let bounds = bounds(300.0, 200.0);
        assert!(
            resolve_inner_drop_geometry(bounds, point(px(12.0), px(120.0))).is_none(),
            "near-edge points outside an explicit side box must not split"
        );

        let left = resolve_inner_drop_geometry(
            bounds,
            drop_box_center(
                bounds,
                DockDropBoxSet::Inner,
                DockDropBoxKind::InnerEdge(DropZone::Left),
            ),
        )
        .expect("left edge should resolve");
        let right = resolve_inner_drop_geometry(
            bounds,
            drop_box_center(
                bounds,
                DockDropBoxSet::Inner,
                DockDropBoxKind::InnerEdge(DropZone::Right),
            ),
        )
        .expect("right edge should resolve");
        let top = resolve_inner_drop_geometry(
            bounds,
            drop_box_center(
                bounds,
                DockDropBoxSet::Inner,
                DockDropBoxKind::InnerEdge(DropZone::Top),
            ),
        )
        .expect("top edge should resolve");
        let bottom = resolve_inner_drop_geometry(
            bounds,
            drop_box_center(
                bounds,
                DockDropBoxSet::Inner,
                DockDropBoxKind::InnerEdge(DropZone::Bottom),
            ),
        )
        .expect("bottom edge should resolve");

        assert_eq!(
            left.drop_box.kind,
            DockDropBoxKind::InnerEdge(DropZone::Left)
        );
        assert_eq!(
            right.drop_box.kind,
            DockDropBoxKind::InnerEdge(DropZone::Right)
        );
        assert_eq!(top.drop_box.kind, DockDropBoxKind::InnerEdge(DropZone::Top));
        assert_eq!(
            bottom.drop_box.kind,
            DockDropBoxKind::InnerEdge(DropZone::Bottom)
        );
        assert_eq!(left.preview_bounds().origin, bounds.origin);
        assert!(right.preview_bounds().origin.x > left.preview_bounds().origin.x);
        assert_eq!(top.preview_bounds().origin.x, bounds.origin.x);
        assert!(bottom.preview_bounds().origin.y > top.preview_bounds().origin.y);
    }

    #[test]
    fn outer_drop_geometry_resolves_only_outer_edge_boxes() {
        let bounds = bounds(300.0, 200.0);
        assert!(resolve_outer_drop_geometry(bounds, point(px(160.0), px(120.0))).is_none());

        let left = resolve_outer_drop_geometry(
            bounds,
            drop_box_center(
                bounds,
                DockDropBoxSet::Outer,
                DockDropBoxKind::OuterEdge(DropZone::Left),
            ),
        )
        .expect("left outer edge should resolve");

        assert_eq!(
            left.drop_box.kind,
            DockDropBoxKind::OuterEdge(DropZone::Left)
        );
        assert_eq!(left.zone(), DropZone::Left);
    }

    #[test]
    fn inner_box_corners_use_explicit_candidate_order() {
        let bounds = bounds(300.0, 200.0);
        let left = drop_boxes(bounds, DockDropBoxSet::Inner)
            .into_iter()
            .find(|drop_box| drop_box.kind == DockDropBoxKind::InnerEdge(DropZone::Left))
            .expect("left box should exist");
        let corner = point(left.hit_bounds.origin.x, left.hit_bounds.origin.y);
        let geometry = resolve_inner_drop_geometry(bounds, corner).expect("corner should resolve");

        assert_eq!(
            geometry.drop_box.kind,
            DockDropBoxKind::InnerEdge(DropZone::Left)
        );
    }

    #[test]
    fn overlapping_corner_points_prefer_first_outer_candidate() {
        let bounds = bounds(30.0, 30.0);
        let boxes = drop_boxes(bounds, DockDropBoxSet::Outer);
        let left = boxes
            .iter()
            .find(|drop_box| drop_box.kind == DockDropBoxKind::OuterEdge(DropZone::Left))
            .expect("left box should exist");
        let top = boxes
            .iter()
            .find(|drop_box| drop_box.kind == DockDropBoxKind::OuterEdge(DropZone::Top))
            .expect("top box should exist");
        let overlap = point(
            left.hit_bounds.origin.x + px(4.0),
            top.hit_bounds.origin.y + px(4.0),
        );
        let geometry =
            resolve_outer_drop_geometry(bounds, overlap).expect("overlap should resolve");

        assert_eq!(
            geometry.drop_box.kind,
            DockDropBoxKind::OuterEdge(DropZone::Left)
        );
    }

    #[test]
    fn small_targets_keep_center_space() {
        let geometry = resolve_inner_drop_geometry(bounds(36.0, 36.0), point(px(28.0), px(38.0)))
            .expect("point should resolve");

        assert_eq!(geometry.drop_box.kind, DockDropBoxKind::Center);
    }

    #[test]
    fn invalid_drop_bounds_do_not_resolve() {
        assert!(
            resolve_inner_drop_geometry(bounds(0.0, 36.0), point(px(10.0), px(20.0))).is_none()
        );
    }

    fn drop_box_center(
        bounds: Bounds<Pixels>,
        set: DockDropBoxSet,
        kind: DockDropBoxKind,
    ) -> Point<Pixels> {
        drop_boxes(bounds, set)
            .into_iter()
            .find(|drop_box| drop_box.kind == kind)
            .map(|drop_box| drop_box.hit_bounds.center())
            .unwrap_or_else(|| panic!("{kind:?} should exist"))
    }

    #[test]
    fn splitter_handle_geometry_matches_fraction_boundaries() {
        let geometry = DockSplitLayout::from_fractions(2, &[0.25, 0.75], None).geometry(
            SplitAxis::Horizontal,
            bounds(400.0, 100.0),
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
        let geometry = DockSplitLayout::from_fractions(2, &[0.25, 0.75], None).geometry(
            SplitAxis::Vertical,
            bounds(200.0, 400.0),
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
        let geometry = DockSplitLayout::from_fractions(3, &[f32::NAN], None).geometry(
            SplitAxis::Horizontal,
            bounds(300.0, 100.0),
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
        let geometry = DockSplitLayout::from_fractions(3, &[0.2, 0.0, 0.3], Some(1)).geometry(
            SplitAxis::Horizontal,
            bounds(1000.0, 100.0),
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
        let geometry = DockSplitLayout::from_fractions(3, &[0.8, 0.0, 0.7], Some(1)).geometry(
            SplitAxis::Horizontal,
            bounds(1000.0, 100.0),
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
