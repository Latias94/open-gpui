use super::*;

pub(super) fn paint_frame_record_body(
    record_index: usize,
    canvas_bounds: Bounds<Pixels>,
    model: &CanvasPaintModel,
    frame: &CanvasPreparedPaintFrame,
    theme: CanvasPaintTheme,
    window: &mut Window,
    _cx: &mut App,
) {
    let Some(record) = frame.frame.records.get(record_index) else {
        return;
    };

    match &record.target {
        HitTarget::Node(id) => {
            let Some(node) = model.document.node(id) else {
                return;
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
                return;
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
            if model.document.edge(id).is_none() {
                return;
            }
            let Some(edge_geometry) = &record.edge_geometry else {
                return;
            };
            let Some(edge) = model.document.edge(id) else {
                return;
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
}

pub(super) fn paint_frame_record_label(
    record_index: usize,
    canvas_bounds: Bounds<Pixels>,
    frame: &CanvasPreparedPaintFrame,
    theme: CanvasPaintTheme,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(label_index) = frame
        .label_indices
        .get(record_index)
        .and_then(|index| *index)
    else {
        return;
    };
    paint_label(canvas_bounds, &frame.labels[label_index], theme, window, cx);
}

pub(super) fn paint_record_selection_feedback(
    record: &CanvasPaintRecord,
    canvas_bounds: Bounds<Pixels>,
    model: &CanvasPaintModel,
    theme: CanvasPaintTheme,
    window: &mut Window,
) {
    if !record_has_selection_feedback(record) {
        return;
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
                return;
            }
            let Some(edge_geometry) = &record.edge_geometry else {
                return;
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

pub(super) fn paint_selected_edge_feedback(
    canvas_bounds: Bounds<Pixels>,
    model: &CanvasPaintModel,
    frame: &CanvasPreparedPaintFrame,
    theme: CanvasPaintTheme,
    window: &mut Window,
) {
    for record in &frame.frame.records {
        if !matches!(record.target, HitTarget::Edge(_)) {
            continue;
        }
        paint_record_selection_feedback(record, canvas_bounds, model, theme, window);
    }
}

pub(super) fn paint_hovered_edge_feedback(
    canvas_bounds: Bounds<Pixels>,
    model: &CanvasPaintModel,
    frame: &CanvasPreparedPaintFrame,
    theme: CanvasPaintTheme,
    window: &mut Window,
) {
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
