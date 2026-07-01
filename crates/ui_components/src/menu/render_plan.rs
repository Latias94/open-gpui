use open_gpui_ui_core::{
    OverlayAnchorInput, OverlayPlacementAlignment, OverlayPlacementInput, OverlayPlacementSide,
    Rect, UiPx, ui_point, ui_size,
};
/// Renderer-neutral surface plan for a submenu that may be rendered as a floating layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MenuSubmenuSurface {
    trigger_bounds: Rect,
    content_bounds: Rect,
    placement_input: OverlayPlacementInput,
    hover_corridor: MenuSafeHoverCorridor,
}

impl MenuSubmenuSurface {
    /// Creates a submenu surface plan from resolved trigger bounds and content size.
    pub fn resolve(
        trigger_bounds: Rect,
        content_size: open_gpui_ui_core::OverlaySize,
        side: OverlayPlacementSide,
        alignment: OverlayPlacementAlignment,
        offset: UiPx,
        safe_bounds: Option<Rect>,
    ) -> Self {
        let mut placement_input = OverlayPlacementInput::new(
            OverlayAnchorInput::from_layout_bounds(trigger_bounds),
            content_size,
        )
        .with_side(side)
        .with_alignment(alignment)
        .with_offset(offset);
        if let Some(safe_bounds) = safe_bounds {
            placement_input = placement_input.with_safe_bounds(safe_bounds);
        }

        let content_bounds =
            submenu_content_bounds(trigger_bounds, content_size, side, alignment, offset);
        let hover_corridor = MenuSafeHoverCorridor::between(trigger_bounds, content_bounds);

        Self {
            trigger_bounds,
            content_bounds,
            placement_input,
            hover_corridor,
        }
    }

    /// Returns bounds for the submenu trigger item.
    pub const fn trigger_bounds(self) -> Rect {
        self.trigger_bounds
    }

    /// Returns preferred bounds for the submenu content before renderer collision handling.
    pub const fn content_bounds(self) -> Rect {
        self.content_bounds
    }

    /// Returns renderer-neutral placement input for the submenu content.
    pub const fn placement_input(self) -> OverlayPlacementInput {
        self.placement_input
    }

    /// Returns the safe hover transition corridor between trigger and content.
    pub const fn hover_corridor(self) -> MenuSafeHoverCorridor {
        self.hover_corridor
    }
}

/// Renderer-neutral hover transition corridor between a submenu trigger and its floating surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MenuSafeHoverCorridor {
    bounds: Rect,
}

impl MenuSafeHoverCorridor {
    /// Creates the smallest axis-aligned corridor that connects trigger and submenu bounds.
    pub fn between(trigger_bounds: Rect, content_bounds: Rect) -> Self {
        Self {
            bounds: union_rect(trigger_bounds, content_bounds),
        }
    }

    /// Returns the corridor bounds.
    pub const fn bounds(self) -> Rect {
        self.bounds
    }

    /// Returns whether a pointer position is inside the corridor.
    pub fn contains_point(self, point: open_gpui_ui_core::UiPoint) -> bool {
        rect_contains_point(self.bounds, point)
    }
}

fn submenu_content_bounds(
    trigger_bounds: Rect,
    content_size: open_gpui_ui_core::OverlaySize,
    side: OverlayPlacementSide,
    alignment: OverlayPlacementAlignment,
    offset: UiPx,
) -> Rect {
    let trigger_left = trigger_bounds.origin.x;
    let trigger_top = trigger_bounds.origin.y;
    let trigger_right = trigger_bounds.origin.x + trigger_bounds.size.width;
    let trigger_bottom = trigger_bounds.origin.y + trigger_bounds.size.height;
    let trigger_center_x = trigger_bounds.origin.x + trigger_bounds.size.width.half();
    let trigger_center_y = trigger_bounds.origin.y + trigger_bounds.size.height.half();

    let x = match side {
        OverlayPlacementSide::Right => trigger_right + offset,
        OverlayPlacementSide::Left => trigger_left - offset - content_size.width,
        OverlayPlacementSide::Top | OverlayPlacementSide::Bottom => match alignment {
            OverlayPlacementAlignment::Start => trigger_left,
            OverlayPlacementAlignment::Center => trigger_center_x - content_size.width.half(),
            OverlayPlacementAlignment::End => trigger_right - content_size.width,
        },
    };
    let y = match side {
        OverlayPlacementSide::Bottom => trigger_bottom + offset,
        OverlayPlacementSide::Top => trigger_top - offset - content_size.height,
        OverlayPlacementSide::Left | OverlayPlacementSide::Right => match alignment {
            OverlayPlacementAlignment::Start => trigger_top,
            OverlayPlacementAlignment::Center => trigger_center_y - content_size.height.half(),
            OverlayPlacementAlignment::End => trigger_bottom - content_size.height,
        },
    };

    open_gpui_ui_core::rect(ui_point(x, y), content_size)
}

fn union_rect(a: Rect, b: Rect) -> Rect {
    let left = a.origin.x.min(b.origin.x);
    let top = a.origin.y.min(b.origin.y);
    let right = (a.origin.x + a.size.width).max(b.origin.x + b.size.width);
    let bottom = (a.origin.y + a.size.height).max(b.origin.y + b.size.height);
    open_gpui_ui_core::rect(ui_point(left, top), ui_size(right - left, bottom - top))
}

fn rect_contains_point(rect: Rect, point: open_gpui_ui_core::UiPoint) -> bool {
    let left = rect.origin.x.as_f32();
    let top = rect.origin.y.as_f32();
    let right = (rect.origin.x + rect.size.width).as_f32();
    let bottom = (rect.origin.y + rect.size.height).as_f32();
    let x = point.x.as_f32();
    let y = point.y.as_f32();

    x >= left && x <= right && y >= top && y <= bottom
}
