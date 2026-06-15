//! Overlay foundation page metadata.

use open_gpui::{Pixels, point, px, size};
use open_gpui_ui_core::{
    Rect, anchor_rect_from_point, outer_bounds_with_window_margin, prefer_visual_bounds, rect,
};

/// Page title.
pub const TITLE: &str = "Overlay";
/// Page summary.
pub const SUMMARY: &str = "Anchor rectangles, visual bounds, and window-margin geometry helpers.";
/// Foundation signals rendered by this page.
pub const SIGNALS: &[&str] = &[
    "anchor_rect_from_point()",
    "prefer_visual_bounds()",
    "outer_bounds_with_window_margin()",
    "OverlayEdges",
    "OverlaySize",
];

/// Geometry used by the overlay demo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayDemoGeometry {
    /// Point selected as the trigger anchor.
    pub trigger_point: open_gpui::Point<Pixels>,
    /// 1x1 anchor rect produced from the trigger point.
    pub anchor_rect: Rect,
    /// Layout rect that approximates the trigger bounds.
    pub layout_rect: Rect,
    /// Visual rect preferred for overlay positioning.
    pub visual_rect: Rect,
    /// Preferred rect resolved from visual/layout candidates.
    pub preferred_rect: Rect,
    /// Window bounds after applying the safe overlay margin.
    pub safe_window_rect: Rect,
}

/// Returns deterministic overlay geometry for the gallery.
pub fn demo_geometry() -> OverlayDemoGeometry {
    let trigger_point = point(px(312.0), px(168.0));
    let anchor_rect = anchor_rect_from_point(trigger_point);
    let layout_rect = rect(point(px(288.0), px(144.0)), size(px(176.0), px(40.0)));
    let visual_rect = rect(point(px(284.0), px(140.0)), size(px(184.0), px(48.0)));
    let preferred_rect = prefer_visual_bounds(Some(visual_rect), Some(layout_rect))
        .expect("visual or layout rect should be present");
    let safe_window_rect = outer_bounds_with_window_margin(
        rect(point(px(0.0), px(0.0)), size(px(640.0), px(360.0))),
        px(12.0),
    );

    OverlayDemoGeometry {
        trigger_point,
        anchor_rect,
        layout_rect,
        visual_rect,
        preferred_rect,
        safe_window_rect,
    }
}
