use super::*;

pub(super) fn prepare_label(
    label: &CanvasPaintLabel,
    canvas_bounds: Bounds<Pixels>,
    theme: CanvasPaintTheme,
    window: &mut Window,
) -> Option<CanvasPreparedPaintLabel> {
    let text = label.text.trim();
    if text.is_empty()
        || label.view_bounds.size.width <= Pixels::ZERO
        || label.view_bounds.size.height <= Pixels::ZERO
        || !positive_pixels(theme.label_font_size)
        || !positive_pixels(theme.label_line_height)
    {
        return None;
    }

    let mut text_style = window.text_style();
    text_style.color = label.color.unwrap_or(theme.label_color);
    let run = TextRun {
        len: text.len(),
        ..text_style.to_run(text.len())
    };

    let Some(lines) = window
        .text_system()
        .shape_text(
            SharedString::new(text),
            theme.label_font_size,
            &[run],
            Some(label.view_bounds.size.width),
            label_line_clamp(theme, label.view_bounds),
        )
        .ok()
    else {
        return None;
    };

    let text_height = lines.iter().fold(Pixels::ZERO, |height, line| {
        height + line.size(theme.label_line_height).height
    });
    let clip = SubtreeClip::try_rect(label.view_bounds).ok()?;
    let clip = window.prepare_subtree_clip(&clip, canvas_bounds);

    Some(CanvasPreparedPaintLabel {
        view_bounds: label.view_bounds,
        clip,
        lines: lines.into_iter().collect(),
        text_height,
    })
}

pub(crate) fn label_line_clamp(theme: CanvasPaintTheme, bounds: Bounds<Pixels>) -> Option<usize> {
    let max_lines_by_height =
        (bounds.size.height.as_f32() / theme.label_line_height.as_f32()).floor() as usize;
    let max_lines = max_lines_by_height.max(1);
    Some(
        theme
            .label_line_clamp
            .map_or(max_lines, |clamp| clamp.max(1).min(max_lines)),
    )
}

pub(super) fn paint_record_label(
    model: &CanvasPaintModel,
    target: &HitTarget,
    document_bounds: Bounds<Pixels>,
) -> Option<CanvasPaintLabel> {
    let label = match target {
        HitTarget::Node(id) => model
            .document
            .node(id)
            .and_then(|node| model.kind_registry.node_label(node)),
        HitTarget::Shape(id) => model
            .document
            .shape(id)
            .and_then(|shape| model.kind_registry.shape_label(shape)),
        HitTarget::Edge(_) | HitTarget::Handle { .. } => None,
    }?;

    Some(resolve_paint_label(model, label, document_bounds))
}

fn resolve_paint_label(
    model: &CanvasPaintModel,
    label: CanvasKindLabel,
    document_bounds: Bounds<Pixels>,
) -> CanvasPaintLabel {
    let document_bounds = label_document_bounds(document_bounds, label.inset);
    CanvasPaintLabel {
        text: label.text,
        document_bounds,
        view_bounds: model.viewport.document_bounds_to_view(document_bounds),
        color: label.color.as_deref().and_then(parse_color),
    }
}

fn label_document_bounds(bounds: Bounds<Pixels>, inset: Pixels) -> Bounds<Pixels> {
    if !inset.as_f32().is_finite() || inset <= Pixels::ZERO {
        return bounds;
    }

    let max_inset = bounds.size.width.min(bounds.size.height) * 0.5;
    bounds.inset(inset.min(max_inset))
}
