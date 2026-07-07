use super::*;

pub(super) fn paint_interaction_chrome(
    canvas_bounds: Bounds<Pixels>,
    frame: &CanvasPreparedPaintFrame,
    theme: CanvasPaintTheme,
    window: &mut Window,
) {
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
