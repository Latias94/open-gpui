use open_gpui::{BoxShadow, hsla, point, px};
use open_gpui_ui_core::ThemeElevationLayer;

pub(crate) fn gpui_elevation_shadow(layers: [ThemeElevationLayer; 2]) -> Vec<BoxShadow> {
    layers
        .into_iter()
        .map(|layer| BoxShadow {
            color: hsla(0.0, 0.0, 0.0, f32::from(layer.opacity_percent()) / 100.0),
            offset: point(px(layer.offset_x().as_f32()), px(layer.offset_y().as_f32())),
            blur_radius: px(layer.blur_radius().as_f32()),
            spread_radius: px(layer.spread_radius().as_f32()),
            inset: false,
        })
        .collect()
}
