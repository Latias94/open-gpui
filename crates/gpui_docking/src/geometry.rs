use crate::DropZone;
use open_gpui::{Bounds, Pixels, Point, point, px, size};
use open_gpui_ui_core::{UiRect, ui_point, ui_px, ui_rect, ui_size};

const DEFAULT_DROP_GUIDE_FONT_SIZE: f32 = 16.0;
const DEFAULT_MIN_SPLIT_PREVIEW_EXTENT: f32 = 8.0;
const DEFAULT_MAX_SPLIT_PREVIEW_EXTENT: f32 = 48.0;

pub(crate) fn ui_rect_from_bounds(bounds: Bounds<Pixels>) -> UiRect {
    ui_rect(
        ui_point(
            ui_px(f32::from(bounds.origin.x)),
            ui_px(f32::from(bounds.origin.y)),
        ),
        ui_size(
            ui_px(f32::from(bounds.size.width)),
            ui_px(f32::from(bounds.size.height)),
        ),
    )
}

pub(crate) fn bounds_from_ui_rect(rect: UiRect) -> Bounds<Pixels> {
    Bounds::new(
        point(px(rect.origin.x.as_f32()), px(rect.origin.y.as_f32())),
        size(px(rect.size.width.as_f32()), px(rect.size.height.as_f32())),
    )
}

/// Style inputs used to calculate dock drop guide hit rectangles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockDropGuideStyle {
    /// Font-size equivalent used by the guide cluster sizing formula.
    pub font_size: Pixels,
    /// Minimum preview strip extent for edge split guides.
    pub min_split_preview_extent: Pixels,
    /// Maximum preview strip extent for edge split guides.
    pub max_split_preview_extent: Pixels,
}

impl Default for DockDropGuideStyle {
    fn default() -> Self {
        Self {
            font_size: px(DEFAULT_DROP_GUIDE_FONT_SIZE),
            min_split_preview_extent: px(DEFAULT_MIN_SPLIT_PREVIEW_EXTENT),
            max_split_preview_extent: px(DEFAULT_MAX_SPLIT_PREVIEW_EXTENT),
        }
    }
}

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
    pub(crate) draw_bounds: Bounds<Pixels>,
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

    fn expand(self, amount: f32) -> Self {
        if !amount.is_finite() || amount <= 0.0 {
            return self;
        }
        Self {
            x: self.x - amount,
            y: self.y - amount,
            width: self.width + amount * 2.0,
            height: self.height + amount * 2.0,
        }
    }
}

pub(crate) fn resolve_inner_drop_geometry_with_style(
    bounds: Bounds<Pixels>,
    position: Point<Pixels>,
    style: DockDropGuideStyle,
) -> Option<DockDropGeometry> {
    resolve_drop_geometry(bounds, position, DockDropBoxSet::Inner, style)
}

#[cfg(test)]
fn resolve_inner_drop_geometry(
    bounds: Bounds<Pixels>,
    position: Point<Pixels>,
) -> Option<DockDropGeometry> {
    resolve_inner_drop_geometry_with_style(bounds, position, DockDropGuideStyle::default())
}

pub(crate) fn resolve_outer_drop_geometry_with_style(
    bounds: Bounds<Pixels>,
    position: Point<Pixels>,
    style: DockDropGuideStyle,
) -> Option<DockDropGeometry> {
    resolve_drop_geometry(bounds, position, DockDropBoxSet::Outer, style)
}

#[cfg(test)]
fn resolve_outer_drop_geometry(
    bounds: Bounds<Pixels>,
    position: Point<Pixels>,
) -> Option<DockDropGeometry> {
    resolve_outer_drop_geometry_with_style(bounds, position, DockDropGuideStyle::default())
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn drop_boxes(bounds: Bounds<Pixels>, set: DockDropBoxSet) -> Vec<DockDropBox> {
    drop_boxes_with_style(bounds, set, DockDropGuideStyle::default())
}

pub(crate) fn drop_boxes_with_style(
    bounds: Bounds<Pixels>,
    set: DockDropBoxSet,
    style: DockDropGuideStyle,
) -> Vec<DockDropBox> {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    if !valid_extent(width) || !valid_extent(height) {
        return Vec::new();
    }

    let metrics = drop_box_metrics(width, height, style);
    let mut boxes = Vec::new();
    match set {
        DockDropBoxSet::Inner => {
            boxes.push(drop_box(
                bounds,
                DockDropBoxKind::Center,
                {
                    let draw = LocalRect::from_center(
                        width / 2.0,
                        height / 2.0,
                        metrics.center_half,
                        metrics.center_half,
                    );
                    (draw, draw.expand(inner_hit_expand(metrics)))
                },
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
    style: DockDropGuideStyle,
) -> Option<DockDropGeometry> {
    if !bounds.contains(&position) {
        return None;
    }

    let boxes = drop_boxes_with_style(bounds, set, style);
    boxes
        .iter()
        .copied()
        .find(|drop_box| drop_box_contains_position(bounds, *drop_box, position, set, style))
        .map(|drop_box| DockDropGeometry { drop_box })
}

fn valid_extent(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

fn drop_box_metrics(width: f32, height: f32, style: DockDropGuideStyle) -> DockDropBoxMetrics {
    let shortest = width.min(height);
    let font_size = drop_guide_font_size(style);
    let central_half = (font_size * 1.5).min((font_size * 0.5).max(shortest / 8.0));
    let center_half = central_half.min(width / 2.0).min(height / 2.0);
    let inner_side_half_long = center_half;
    let inner_side_half_short = (center_half * 0.9).min(width / 2.0).min(height / 2.0);
    let inner_offset = center_half * 2.4;
    let outer_side_half_long = (central_half * 1.5).min(width / 2.0).min(height / 2.0);
    let outer_side_half_short = (central_half * 0.8).min(width / 2.0).min(height / 2.0);
    let outer_offset_x = (width / 2.0 - outer_side_half_short).max(0.0);
    let outer_offset_y = (height / 2.0 - outer_side_half_short).max(0.0);
    let (min_split_preview_extent, max_split_preview_extent) = split_preview_extent_limits(style);
    let split_preview_extent = (shortest * 0.25)
        .clamp(min_split_preview_extent, max_split_preview_extent)
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

fn drop_guide_font_size(style: DockDropGuideStyle) -> f32 {
    positive_or_default(style.font_size, DEFAULT_DROP_GUIDE_FONT_SIZE)
}

fn split_preview_extent_limits(style: DockDropGuideStyle) -> (f32, f32) {
    let min = positive_or_default(
        style.min_split_preview_extent,
        DEFAULT_MIN_SPLIT_PREVIEW_EXTENT,
    );
    let max = positive_or_default(
        style.max_split_preview_extent,
        DEFAULT_MAX_SPLIT_PREVIEW_EXTENT,
    );
    if min <= max { (min, max) } else { (max, min) }
}

fn positive_or_default(value: Pixels, default: f32) -> f32 {
    let value = f32::from(value);
    if value.is_finite() && value > 0.0 {
        value
    } else {
        default
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
    let draw = match kind {
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
    let hit = if matches!(kind, DockDropBoxKind::InnerEdge(_)) {
        draw.expand(inner_hit_expand(metrics))
    } else {
        draw
    };
    let draw_bounds = local_bounds(bounds.origin, draw.clamp_to_bounds(width, height)?);
    let hit_bounds = local_bounds(bounds.origin, hit.clamp_to_bounds(width, height)?);
    let preview_bounds = offset_bounds(
        bounds.origin,
        preview_bounds(zone, width, height, metrics.split_preview_extent),
    );
    Some(DockDropBox {
        kind,
        hit_bounds,
        draw_bounds,
        preview_bounds,
    })
}

fn drop_box_contains_position(
    bounds: Bounds<Pixels>,
    drop_box: DockDropBox,
    position: Point<Pixels>,
    set: DockDropBoxSet,
    style: DockDropGuideStyle,
) -> bool {
    if set == DockDropBoxSet::Inner
        && let Some(kind) = inner_radial_drop_box_kind(bounds, position, style)
    {
        return drop_box.kind == kind;
    }

    drop_box.hit_bounds.contains(&position)
}

fn inner_radial_drop_box_kind(
    bounds: Bounds<Pixels>,
    position: Point<Pixels>,
    style: DockDropGuideStyle,
) -> Option<DockDropBoxKind> {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    let metrics = drop_box_metrics(width, height, style);
    let center = bounds.center();
    let local_x = f32::from(position.x - bounds.origin.x);
    let local_y = f32::from(position.y - bounds.origin.y);
    let center_x = f32::from(center.x - bounds.origin.x);
    let center_y = f32::from(center.y - bounds.origin.y);
    let delta_x = local_x - center_x;
    let delta_y = local_y - center_y;
    let distance_squared = delta_x * delta_x + delta_y * delta_y;
    let center_threshold = metrics.center_half * 1.4;
    if distance_squared < center_threshold * center_threshold {
        return Some(DockDropBoxKind::Center);
    }

    let side_threshold = metrics.center_half * (1.4 + 1.2);
    if distance_squared < side_threshold * side_threshold {
        return Some(DockDropBoxKind::InnerEdge(quadrant_zone(delta_x, delta_y)));
    }

    None
}

fn inner_hit_expand(metrics: DockDropBoxMetrics) -> f32 {
    metrics.center_half * 0.30
}

fn quadrant_zone(delta_x: f32, delta_y: f32) -> DropZone {
    if delta_x.abs() > delta_y.abs() {
        if delta_x < 0.0 {
            DropZone::Left
        } else {
            DropZone::Right
        }
    } else if delta_y < 0.0 {
        DropZone::Top
    } else {
        DropZone::Bottom
    }
}

fn drop_box(
    bounds: Bounds<Pixels>,
    kind: DockDropBoxKind,
    rects: (LocalRect, LocalRect),
    preview: Bounds<Pixels>,
) -> DockDropBox {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    let (draw, hit) = rects;
    let hit_bounds = local_bounds(
        bounds.origin,
        hit.clamp_to_bounds(width, height)
            .expect("center drop box should fit inside valid bounds"),
    );
    let draw_bounds = local_bounds(
        bounds.origin,
        draw.clamp_to_bounds(width, height)
            .expect("center drop box should fit inside valid bounds"),
    );
    DockDropBox {
        kind,
        hit_bounds,
        draw_bounds,
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

    fn area(bounds: Bounds<Pixels>) -> f32 {
        f32::from(bounds.size.width) * f32::from(bounds.size.height)
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
    fn inner_drop_geometry_resolves_expanded_visible_guide_hits() {
        let bounds = bounds(300.0, 200.0);
        assert!(
            resolve_inner_drop_geometry(bounds, point(px(12.0), px(120.0))).is_none(),
            "near-edge points outside the visible guide cluster must not split"
        );

        let left_box = drop_boxes(bounds, DockDropBoxSet::Inner)
            .into_iter()
            .find(|drop_box| drop_box.kind == DockDropBoxKind::InnerEdge(DropZone::Left))
            .expect("left box should exist");
        let expanded_only_left = point(
            left_box.draw_bounds.origin.x - px(1.0),
            left_box.draw_bounds.center().y,
        );
        assert!(
            !left_box.draw_bounds.contains(&expanded_only_left),
            "expanded-only point should be outside the visible guide"
        );
        assert!(
            left_box.hit_bounds.contains(&expanded_only_left),
            "expanded-only point should remain inside the ImGui-style hit area"
        );
        let expanded_left = resolve_inner_drop_geometry(bounds, expanded_only_left)
            .expect("expanded hit area should resolve");
        assert_eq!(
            expanded_left.drop_box.kind,
            DockDropBoxKind::InnerEdge(DropZone::Left)
        );

        let left_hit = point(px(125.0), px(120.0));
        let near_left = resolve_inner_drop_geometry(bounds, left_hit)
            .expect("near-left guide region should resolve without requiring exact rect hit");
        assert_eq!(
            near_left.drop_box.kind,
            DockDropBoxKind::InnerEdge(DropZone::Left)
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
    fn drop_boxes_expose_separate_draw_and_hit_bounds() {
        let bounds = bounds(300.0, 200.0);
        let box_set = drop_boxes(bounds, DockDropBoxSet::Inner);

        for drop_box in box_set {
            assert!(
                bounds.contains(&drop_box.draw_bounds.center()),
                "draw bounds should remain inside the target container for {:?}",
                drop_box.kind
            );
            assert!(
                area(drop_box.hit_bounds) > area(drop_box.draw_bounds),
                "inner {:?} should use an expanded hit target around the visible guide",
                drop_box.kind
            );
        }
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

        for drop_box in drop_boxes(bounds, DockDropBoxSet::Outer) {
            assert_eq!(
                drop_box.hit_bounds, drop_box.draw_bounds,
                "outer {:?} should keep exact visible hit bounds",
                drop_box.kind
            );
        }
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
}
