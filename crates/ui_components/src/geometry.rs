//! Internal GPUI adapter conversions for UI-core geometry.

use open_gpui::{Pixels, Point, Size, point, px};
use open_gpui_ui_core::{UiPoint, UiPx, UiSize};

pub(crate) fn ui_px_from_gpui(value: Pixels) -> UiPx {
    UiPx::new(value.as_f32())
}

pub(crate) fn ui_point_from_gpui(value: Point<Pixels>) -> UiPoint {
    UiPoint::new(ui_px_from_gpui(value.x), ui_px_from_gpui(value.y))
}

pub(crate) fn ui_size_from_gpui(width: Pixels, height: Pixels) -> UiSize {
    UiSize::new(ui_px_from_gpui(width), ui_px_from_gpui(height))
}

pub(crate) fn ui_size_from_gpui_size(value: Size<Pixels>) -> UiSize {
    ui_size_from_gpui(value.width, value.height)
}

pub(crate) fn gpui_px_from_ui(value: UiPx) -> Pixels {
    px(value.as_f32())
}

pub(crate) fn gpui_point_from_ui(value: UiPoint) -> Point<Pixels> {
    point(gpui_px_from_ui(value.x), gpui_px_from_ui(value.y))
}
