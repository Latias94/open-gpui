use super::*;

pub(super) fn paint_rect(
    window: &mut Window,
    canvas_bounds: Bounds<Pixels>,
    local_bounds: Bounds<Pixels>,
    fill: Hsla,
    stroke: Hsla,
    stroke_width: Pixels,
    corner_radius: Pixels,
) {
    window.paint_quad(quad(
        local_bounds + canvas_bounds.origin,
        corner_radius,
        fill,
        stroke_width,
        stroke,
        Default::default(),
    ));
}

pub(super) fn paint_endpoint_affordance(
    window: &mut Window,
    canvas_bounds: Bounds<Pixels>,
    hit_bounds: Bounds<Pixels>,
    theme: CanvasPaintTheme,
) {
    let radius = hit_bounds.size.width.min(hit_bounds.size.height) * 0.5;
    paint_rect(
        window,
        canvas_bounds,
        hit_bounds,
        theme.handle_fill.alpha(0.10),
        theme.handle_stroke.alpha(0.75),
        theme.handle_stroke_width,
        radius,
    );

    let visual_size = hit_bounds
        .size
        .width
        .min(hit_bounds.size.height)
        .min(px(10.0));
    let visual_bounds = Bounds::centered_at(hit_bounds.center(), size(visual_size, visual_size));
    paint_rect(
        window,
        canvas_bounds,
        visual_bounds,
        theme.handle_fill,
        theme.handle_stroke,
        theme.handle_stroke_width,
        visual_size * 0.5,
    );
}

pub(super) fn paint_line(
    window: &mut Window,
    canvas_bounds: Bounds<Pixels>,
    start: Point<Pixels>,
    end: Point<Pixels>,
    stroke: Hsla,
    stroke_width: Pixels,
) {
    let mut builder = PathBuilder::stroke(stroke_width);
    builder.move_to(start + canvas_bounds.origin);
    builder.line_to(end + canvas_bounds.origin);
    if let Ok(path) = builder.build() {
        window.paint_path(path, stroke);
    }
}

pub(super) fn paint_label(
    canvas_bounds: Bounds<Pixels>,
    label: &CanvasPreparedPaintLabel,
    theme: CanvasPaintTheme,
    window: &mut Window,
    cx: &mut App,
) {
    let label_bounds = label.view_bounds + canvas_bounds.origin;
    let vertical_offset = ((label_bounds.size.height - label.text_height) / 2.0).max(Pixels::ZERO);
    let mut origin = Point::new(label_bounds.left(), label_bounds.top() + vertical_offset);

    window.with_content_mask(
        Some(ContentMask {
            bounds: label_bounds,
        }),
        |window| {
            for line in &label.lines {
                if origin.y >= label_bounds.bottom() {
                    break;
                }

                let line_height = line.size(theme.label_line_height).height;
                line.paint(
                    origin,
                    theme.label_line_height,
                    theme.label_text_align,
                    Some(label_bounds),
                    window,
                    cx,
                )
                .ok();
                origin.y += line_height;
            }
        },
    );
}

pub(super) fn paint_edge(
    window: &mut Window,
    canvas_bounds: Bounds<Pixels>,
    geometry: &CanvasPaintEdgeGeometry,
    stroke: Hsla,
    stroke_width: Pixels,
) {
    let mut builder = PathBuilder::stroke(stroke_width);

    let mut current = None;
    for segment in &geometry.view_path.segments {
        match segment {
            CanvasRouteSegment::Line { from, to } => {
                let from = *from + canvas_bounds.origin;
                if current != Some(from) {
                    builder.move_to(from);
                }
                let to = *to + canvas_bounds.origin;
                builder.line_to(to);
                current = Some(to);
            }
            CanvasRouteSegment::CubicBezier {
                from,
                control_1,
                control_2,
                to,
            } => {
                let from = *from + canvas_bounds.origin;
                if current != Some(from) {
                    builder.move_to(from);
                }
                let to = *to + canvas_bounds.origin;
                builder.cubic_bezier_to(
                    to,
                    *control_1 + canvas_bounds.origin,
                    *control_2 + canvas_bounds.origin,
                );
                current = Some(to);
            }
        }
    }

    if let Ok(path) = builder.build() {
        window.paint_path(path, stroke);
    }
}
