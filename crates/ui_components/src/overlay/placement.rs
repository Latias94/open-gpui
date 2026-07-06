//! GPUI placement mapping for renderer-neutral overlay placement.

use open_gpui::{Anchor, Edges, Pixels, Point, point, px};
use open_gpui_ui_core::{
    OverlayAnchorInput, OverlayPlacementAlignment, OverlayPlacementFit, OverlayPlacementInput,
    OverlayPlacementResolution, OverlayPlacementSide, OverlayPlacementTrace, Rect, UiPx,
    resolve_overlay_placement,
};

use crate::geometry::{gpui_point_from_ui, gpui_px_from_ui, ui_point_from_gpui};

/// Resolved GPUI placement state for an anchored overlay.
#[derive(Debug, Clone, PartialEq)]
pub struct GpuiOverlayPlacement {
    anchor: Anchor,
    position: Option<Point<Pixels>>,
    offset: Point<Pixels>,
    snap_margin: Pixels,
    resolution: OverlayPlacementResolution,
}

impl GpuiOverlayPlacement {
    /// Resolves GPUI placement fields from renderer-neutral placement input.
    pub fn resolve(input: OverlayPlacementInput, snap_margin: Pixels) -> Self {
        let has_anchor_position = input.preferred_anchor_bounds().is_some();
        let resolution = resolve_overlay_placement(input);

        Self {
            anchor: gpui_anchor(resolution.side(), resolution.alignment()),
            position: has_anchor_position.then(|| gpui_point_from_ui(resolution.anchor_point())),
            offset: gpui_offset(resolution.side(), resolution.offset()),
            snap_margin,
            resolution,
        }
    }

    /// Returns the GPUI anchor.
    pub const fn anchor(&self) -> Anchor {
        self.anchor
    }

    /// Returns the preferred window position.
    pub const fn position(&self) -> Option<Point<Pixels>> {
        self.position
    }

    /// Returns the GPUI offset.
    pub const fn offset(&self) -> Point<Pixels> {
        self.offset
    }

    /// Returns the snap-to-window margin.
    pub const fn snap_margin(&self) -> Pixels {
        self.snap_margin
    }

    /// Returns the snap margin as GPUI edges.
    pub fn snap_edges(&self) -> Edges<Pixels> {
        self.snap_margin.into()
    }

    /// Returns the original safe bounds, when provided.
    pub const fn safe_bounds(&self) -> Option<Rect> {
        self.resolution.safe_bounds()
    }

    /// Returns the renderer-neutral placement resolution.
    pub const fn resolution(&self) -> &OverlayPlacementResolution {
        &self.resolution
    }

    /// Returns the selected fit category.
    pub const fn fit(&self) -> OverlayPlacementFit {
        self.resolution.fit()
    }

    /// Returns the diagnostic placement trace.
    pub const fn trace(&self) -> &OverlayPlacementTrace {
        self.resolution.trace()
    }
}

/// Converts renderer-neutral placement into a GPUI anchor.
pub const fn gpui_anchor(
    side: OverlayPlacementSide,
    alignment: OverlayPlacementAlignment,
) -> Anchor {
    match (side, alignment) {
        (OverlayPlacementSide::Top, OverlayPlacementAlignment::Start) => Anchor::BottomLeft,
        (OverlayPlacementSide::Top, OverlayPlacementAlignment::Center) => Anchor::BottomCenter,
        (OverlayPlacementSide::Top, OverlayPlacementAlignment::End) => Anchor::BottomRight,
        (OverlayPlacementSide::Right, _) => Anchor::LeftCenter,
        (OverlayPlacementSide::Bottom, OverlayPlacementAlignment::Start) => Anchor::TopLeft,
        (OverlayPlacementSide::Bottom, OverlayPlacementAlignment::Center) => Anchor::TopCenter,
        (OverlayPlacementSide::Bottom, OverlayPlacementAlignment::End) => Anchor::TopRight,
        (OverlayPlacementSide::Left, _) => Anchor::RightCenter,
    }
}

fn gpui_offset(side: OverlayPlacementSide, offset: UiPx) -> Point<Pixels> {
    let offset = gpui_px_from_ui(offset);
    match side {
        OverlayPlacementSide::Top => point(px(0.0), -offset),
        OverlayPlacementSide::Right => point(offset, px(0.0)),
        OverlayPlacementSide::Bottom => point(px(0.0), offset),
        OverlayPlacementSide::Left => point(-offset, px(0.0)),
    }
}

/// Creates a point anchor placement input for context-menu-like adapters.
pub fn point_anchor_placement(
    point: Point<Pixels>,
    content_size: open_gpui_ui_core::OverlaySize,
) -> OverlayPlacementInput {
    OverlayPlacementInput::new(
        OverlayAnchorInput::from_point(ui_point_from_gpui(point)),
        content_size,
    )
}
