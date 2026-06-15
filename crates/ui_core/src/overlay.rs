//! Overlay geometry helpers for the Open GPUI component ecosystem.

use open_gpui::{Bounds, Edges, Pixels as Px, Point, Size, bounds, px, size};

/// A rectangle in device pixels.
pub type Rect = Bounds<Px>;

/// A size in device pixels.
pub type OverlaySize = Size<Px>;

/// Returns the preferred bounds when both visual and layout rects are available.
pub fn prefer_visual_bounds(visual: Option<Rect>, layout: Option<Rect>) -> Option<Rect> {
    visual.or(layout)
}

/// Returns a 1x1 rectangle anchor derived from a point.
pub fn anchor_rect_from_point(point: Point<Px>) -> Rect {
    bounds(point, size(px(1.0), px(1.0)))
}

/// Returns a rectangle inset by a uniform window margin.
pub fn outer_bounds_with_window_margin(bounds: Rect, window_margin: Px) -> Rect {
    bounds.inset(window_margin)
}

/// Returns a rectangle from the given origin and size.
pub fn rect(origin: Point<Px>, size: OverlaySize) -> Rect {
    bounds(origin, size)
}

/// Returns a rectangle inset by a uniform margin.
pub fn inset_rect(bounds: Rect, margin: Px) -> Rect {
    outer_bounds_with_window_margin(bounds, margin)
}

/// Re-export the geometry edge type so overlay callers do not need to depend on `open_gpui`
/// directly for the basic shape helpers.
pub type OverlayEdges = Edges<Px>;

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui::{point, px, size};

    #[test]
    fn prefer_visual_bounds_prefers_visual() {
        let visual = rect(point(px(10.0), px(20.0)), size(px(100.0), px(60.0)));
        let layout = rect(point(px(30.0), px(40.0)), size(px(120.0), px(80.0)));

        assert_eq!(
            prefer_visual_bounds(Some(visual), Some(layout)),
            Some(visual)
        );
    }

    #[test]
    fn prefer_visual_bounds_falls_back_to_layout() {
        let layout = rect(point(px(30.0), px(40.0)), size(px(120.0), px(80.0)));

        assert_eq!(prefer_visual_bounds(None, Some(layout)), Some(layout));
    }

    #[test]
    fn anchor_rect_from_point_creates_one_pixel_anchor() {
        let anchor = anchor_rect_from_point(point(px(12.0), px(34.0)));

        assert_eq!(anchor.origin.x, px(12.0));
        assert_eq!(anchor.origin.y, px(34.0));
        assert_eq!(anchor.size.width, px(1.0));
        assert_eq!(anchor.size.height, px(1.0));
    }

    #[test]
    fn outer_bounds_with_window_margin_insets_uniformly() {
        let input = rect(point(px(240.0), px(64.0)), size(px(220.0), px(190.0)));

        assert_eq!(
            outer_bounds_with_window_margin(input, px(10.0)),
            rect(point(px(250.0), px(74.0)), size(px(200.0), px(170.0)))
        );
    }
}
