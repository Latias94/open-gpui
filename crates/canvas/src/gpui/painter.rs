use super::frame::{
    CanvasPaintConnectionTargetFeedback, CanvasPaintConnectionTargetState, CanvasPaintEdgeGeometry,
    CanvasPaintReconnectHandle, CanvasPaintReconnectHandleShape, CanvasPaintRecord,
    CanvasPreparedPaintFrame, CanvasPreparedPaintLabel,
};
use super::model::{CanvasPaintModel, CanvasPaintTheme};
use super::style::{edge_paint_style, node_paint_style, shape_paint_style};
use crate::{CanvasRouteSegment, HitTarget};
use open_gpui::{
    App, Bounds, ContentMask, Hsla, PathBuilder, Pixels, Point, Window, px, quad, size,
};

pub fn paint_canvas_frame(
    canvas_bounds: Bounds<Pixels>,
    model: &CanvasPaintModel,
    frame: &CanvasPreparedPaintFrame,
    theme: CanvasPaintTheme,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(background) = theme.background {
        window.paint_quad(open_gpui::fill(canvas_bounds, background));
    }

    for (record_index, record) in frame.frame.records.iter().enumerate() {
        match &record.target {
            HitTarget::Node(id) => {
                let Some(node) = model.document.node(id) else {
                    continue;
                };
                let style = node_paint_style(model, node, theme);
                paint_rect(
                    window,
                    canvas_bounds,
                    record.view_bounds,
                    style.fill,
                    style.stroke,
                    style.stroke_width,
                    style.corner_radius,
                );
            }
            HitTarget::Handle { .. } => {
                paint_endpoint_affordance(window, canvas_bounds, record.view_bounds, theme);
            }
            HitTarget::Shape(id) => {
                let Some(shape) = model.document.shape(id) else {
                    continue;
                };
                let style = shape_paint_style(model, shape, theme);
                paint_rect(
                    window,
                    canvas_bounds,
                    record.view_bounds,
                    style.fill,
                    style.stroke,
                    style.stroke_width,
                    style.corner_radius,
                );
            }
            HitTarget::Edge(id) => {
                let Some(edge) = model.document.edge(id) else {
                    continue;
                };
                let Some(edge_geometry) = &record.edge_geometry else {
                    continue;
                };
                let style = edge_paint_style(model, edge, theme);
                paint_edge(
                    window,
                    canvas_bounds,
                    edge_geometry,
                    style.stroke,
                    style.stroke_width,
                );
            }
        }

        if let Some(label_index) = frame
            .label_indices
            .get(record_index)
            .and_then(|index| *index)
        {
            paint_label(canvas_bounds, &frame.labels[label_index], theme, window, cx);
        }
    }

    for record in &frame.frame.records {
        if !record_has_selection_feedback(record) {
            continue;
        }

        match &record.target {
            HitTarget::Node(_) | HitTarget::Shape(_) | HitTarget::Handle { .. } => {
                paint_rect(
                    window,
                    canvas_bounds,
                    record.view_bounds.dilate(px(2.0)),
                    theme.selection_fill,
                    theme.selection_stroke,
                    theme.selection_stroke_width,
                    selection_corner_radius(record, theme),
                );
            }
            HitTarget::Edge(id) => {
                if model.document.edge(id).is_none() {
                    continue;
                }
                let Some(edge_geometry) = &record.edge_geometry else {
                    continue;
                };
                paint_edge(
                    window,
                    canvas_bounds,
                    edge_geometry,
                    theme.selection_stroke,
                    theme.selection_stroke_width,
                );
            }
        }
    }

    for record in &frame.frame.records {
        if !record.hovered {
            continue;
        }
        let HitTarget::Edge(id) = &record.target else {
            continue;
        };
        if model.document.edge(id).is_none() {
            continue;
        }
        let Some(edge_geometry) = &record.edge_geometry else {
            continue;
        };
        paint_edge(
            window,
            canvas_bounds,
            edge_geometry,
            theme.connection_preview_stroke,
            theme.connection_preview_stroke_width,
        );
    }

    if let Some(bounds) = frame.frame.interaction.structural_selection_bounds {
        paint_rect(
            window,
            canvas_bounds,
            bounds.dilate(px(3.0)),
            theme.selection_bounds_fill,
            theme.selection_bounds_stroke,
            theme.selection_bounds_stroke_width,
            px(2.0),
        );
    }

    if let Some(bounds) = frame.frame.interaction.selection_bounds {
        paint_rect(
            window,
            canvas_bounds,
            bounds,
            theme.selection_bounds_fill,
            theme.selection_bounds_stroke,
            theme.selection_bounds_stroke_width,
            px(2.0),
        );
    }

    if let Some(preview) = &frame.frame.interaction.connection_preview {
        let (preview_stroke, preview_stroke_width) =
            connection_preview_style(&preview.target_feedback, theme);
        paint_edge(
            window,
            canvas_bounds,
            &preview.edge_geometry,
            preview_stroke,
            preview_stroke_width,
        );
        paint_connection_target_feedback(window, canvas_bounds, &preview.target_feedback, theme);
    }

    for guide in &frame.frame.interaction.snap_guides {
        paint_line(
            window,
            canvas_bounds,
            guide.view_start,
            guide.view_end,
            theme.snap_guide_stroke,
            theme.snap_guide_stroke_width,
        );
    }

    for handle in &frame.frame.interaction.transform_handles {
        paint_rect(
            window,
            canvas_bounds,
            handle.view_bounds,
            theme.handle_fill,
            theme.handle_stroke,
            theme.handle_stroke_width,
            theme.handle_corner_radius,
        );
    }

    for handle in &frame.frame.interaction.reconnect_handles {
        paint_reconnect_handle(window, canvas_bounds, handle, theme);
    }
}

fn record_has_selection_feedback(record: &CanvasPaintRecord) -> bool {
    record.selected || record.structurally_selected
}

fn selection_corner_radius(record: &CanvasPaintRecord, theme: CanvasPaintTheme) -> Pixels {
    match &record.target {
        HitTarget::Node(_) => theme.selection_corner_radius,
        HitTarget::Handle { .. } => theme.handle_corner_radius,
        HitTarget::Shape(_) | HitTarget::Edge(_) => px(0.0),
    }
}

fn paint_rect(
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

fn paint_endpoint_affordance(
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

fn paint_reconnect_handle(
    window: &mut Window,
    canvas_bounds: Bounds<Pixels>,
    handle: &CanvasPaintReconnectHandle,
    theme: CanvasPaintTheme,
) {
    let radius = handle
        .hit_bounds
        .size
        .width
        .min(handle.hit_bounds.size.height)
        * 0.5;
    paint_rect(
        window,
        canvas_bounds,
        handle.hit_bounds,
        theme.handle_fill.alpha(0.10),
        theme.handle_stroke.alpha(0.75),
        theme.handle_stroke_width,
        radius,
    );

    let visual_radius = handle
        .visual_bounds
        .size
        .width
        .min(handle.visual_bounds.size.height)
        * 0.5;
    match handle.shape {
        CanvasPaintReconnectHandleShape::SourcePlug => paint_rect(
            window,
            canvas_bounds,
            handle.visual_bounds,
            theme.handle_fill,
            theme.handle_stroke,
            theme.handle_stroke_width,
            visual_radius,
        ),
        CanvasPaintReconnectHandleShape::TargetSocket => paint_rect(
            window,
            canvas_bounds,
            handle.visual_bounds,
            theme.handle_fill.alpha(0.04),
            theme.handle_fill,
            theme.handle_stroke_width,
            visual_radius,
        ),
    }
}

fn paint_connection_target_feedback(
    window: &mut Window,
    canvas_bounds: Bounds<Pixels>,
    feedback: &CanvasPaintConnectionTargetFeedback,
    theme: CanvasPaintTheme,
) {
    let (fill, stroke, stroke_width) = connection_target_style(feedback, theme);
    paint_rect(
        window,
        canvas_bounds,
        feedback.view_bounds,
        fill,
        stroke,
        stroke_width,
        feedback
            .view_bounds
            .size
            .width
            .min(feedback.view_bounds.size.height)
            * 0.5,
    );
}

fn connection_preview_style(
    feedback: &CanvasPaintConnectionTargetFeedback,
    theme: CanvasPaintTheme,
) -> (Hsla, Pixels) {
    let stroke = match feedback.state {
        CanvasPaintConnectionTargetState::Free => theme.connection_preview_stroke,
        CanvasPaintConnectionTargetState::Valid => theme.connection_valid_target_stroke,
        CanvasPaintConnectionTargetState::Invalid => theme.connection_invalid_target_stroke,
    };
    (stroke, theme.connection_preview_stroke_width)
}

fn connection_target_style(
    feedback: &CanvasPaintConnectionTargetFeedback,
    theme: CanvasPaintTheme,
) -> (Hsla, Hsla, Pixels) {
    match feedback.state {
        CanvasPaintConnectionTargetState::Free => (
            theme.connection_free_target_fill,
            theme.connection_free_target_stroke,
            theme.handle_stroke_width,
        ),
        CanvasPaintConnectionTargetState::Valid => (
            theme.connection_valid_target_fill,
            theme.connection_valid_target_stroke,
            theme.handle_stroke_width,
        ),
        CanvasPaintConnectionTargetState::Invalid => (
            theme.connection_invalid_target_fill,
            theme.connection_invalid_target_stroke,
            theme.handle_stroke_width,
        ),
    }
}

fn paint_line(
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

fn paint_label(
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

fn paint_edge(
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
