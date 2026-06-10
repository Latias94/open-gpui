use super::model::{CanvasPaintModel, CanvasPaintTheme};
use crate::{CanvasEdge, CanvasKindPaint, CanvasNode, CanvasShape, CanvasStyle};
use open_gpui::{Hsla, Pixels};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CanvasResolvedPaintStyle {
    pub(super) fill: Hsla,
    pub(super) stroke: Hsla,
    pub(super) stroke_width: Pixels,
    pub(super) corner_radius: Pixels,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CanvasResolvedEdgePaintStyle {
    pub(super) stroke: Hsla,
    pub(super) stroke_width: Pixels,
}

pub(super) fn node_paint_style(
    model: &CanvasPaintModel,
    node: &CanvasNode,
    theme: CanvasPaintTheme,
) -> CanvasResolvedPaintStyle {
    record_paint_style(
        &node.style,
        model.kind_registry.node_paint(node).as_ref(),
        CanvasResolvedPaintStyle {
            fill: theme.node_fill,
            stroke: theme.node_stroke,
            stroke_width: theme.node_stroke_width,
            corner_radius: theme.node_corner_radius,
        },
    )
}

pub(super) fn shape_paint_style(
    model: &CanvasPaintModel,
    shape: &CanvasShape,
    theme: CanvasPaintTheme,
) -> CanvasResolvedPaintStyle {
    record_paint_style(
        &shape.style,
        model.kind_registry.shape_paint(shape).as_ref(),
        CanvasResolvedPaintStyle {
            fill: theme.shape_fill,
            stroke: theme.shape_stroke,
            stroke_width: theme.shape_stroke_width,
            corner_radius: Pixels::ZERO,
        },
    )
}

pub(super) fn edge_paint_style(
    model: &CanvasPaintModel,
    edge: &CanvasEdge,
    theme: CanvasPaintTheme,
) -> CanvasResolvedEdgePaintStyle {
    let fallback = model.kind_registry.edge_paint(edge);
    CanvasResolvedEdgePaintStyle {
        stroke: paint_color(
            &edge.style.stroke,
            fallback.as_ref().and_then(|paint| paint.stroke.as_deref()),
            theme.edge_stroke,
        ),
        stroke_width: paint_pixels(
            edge.style.stroke_width,
            fallback.as_ref().and_then(|paint| paint.stroke_width),
            theme.edge_stroke_width,
        ),
    }
}

fn record_paint_style(
    style: &CanvasStyle,
    fallback: Option<&CanvasKindPaint>,
    theme: CanvasResolvedPaintStyle,
) -> CanvasResolvedPaintStyle {
    CanvasResolvedPaintStyle {
        fill: paint_color(
            &style.fill,
            fallback.and_then(|paint| paint.fill.as_deref()),
            theme.fill,
        ),
        stroke: paint_color(
            &style.stroke,
            fallback.and_then(|paint| paint.stroke.as_deref()),
            theme.stroke,
        ),
        stroke_width: paint_pixels(
            style.stroke_width,
            fallback.and_then(|paint| paint.stroke_width),
            theme.stroke_width,
        ),
        corner_radius: fallback
            .and_then(|paint| paint.corner_radius)
            .filter(|value| positive_pixels(*value))
            .unwrap_or(theme.corner_radius),
    }
}

pub(super) fn positive_pixels(value: Pixels) -> bool {
    value > Pixels::ZERO && value.as_f32().is_finite()
}

fn paint_pixels(value: Pixels, fallback: Option<Pixels>, theme: Pixels) -> Pixels {
    if positive_pixels(value) {
        value
    } else {
        fallback
            .filter(|value| positive_pixels(*value))
            .unwrap_or(theme)
    }
}

fn paint_color(value: &Option<String>, fallback: Option<&str>, theme: Hsla) -> Hsla {
    style_color(value)
        .or_else(|| fallback.and_then(parse_color))
        .unwrap_or(theme)
}

pub(super) fn style_color(value: &Option<String>) -> Option<Hsla> {
    value.as_deref().and_then(parse_color)
}

pub(super) fn parse_color(value: &str) -> Option<Hsla> {
    open_gpui::Rgba::try_from(value).ok().map(Hsla::from)
}
