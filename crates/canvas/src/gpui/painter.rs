use super::frame::{
    CanvasPaintConnectionTargetFeedback, CanvasPaintConnectionTargetState, CanvasPaintEdgeGeometry,
    CanvasPaintReconnectHandle, CanvasPaintReconnectHandleShape, CanvasPaintRecord,
    CanvasPreparedPaintFrame, CanvasPreparedPaintLabel, CanvasSceneLayerPhase,
};
use super::model::{CanvasPaintModel, CanvasPaintTheme};
use super::style::{edge_paint_style, node_paint_style, shape_paint_style};
use crate::{HitTarget, routing::CanvasRouteSegment};
use open_gpui::{App, Bounds, Hsla, PathBuilder, Pixels, Point, Window, px, quad, size};

mod chrome;
mod primitives;
mod records;

use chrome::paint_interaction_chrome;
use primitives::{paint_edge, paint_endpoint_affordance, paint_label, paint_line, paint_rect};
use records::{
    paint_frame_record_body, paint_frame_record_label, paint_hovered_edge_feedback,
    paint_record_selection_feedback, paint_selected_edge_feedback,
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
        paint_record_selection_feedback(record, canvas_bounds, model, theme, window);
    }

    paint_hovered_edge_feedback(canvas_bounds, model, frame, theme, window);
    paint_interaction_chrome(canvas_bounds, frame, theme, window);
}

pub fn paint_canvas_scene_phase(
    canvas_bounds: Bounds<Pixels>,
    model: &CanvasPaintModel,
    frame: &CanvasPreparedPaintFrame,
    phase: CanvasSceneLayerPhase,
    theme: CanvasPaintTheme,
    window: &mut Window,
    cx: &mut App,
) {
    let scene = frame.frame().scene_frame();
    match phase {
        CanvasSceneLayerPhase::DocumentUnderlay => {
            if let Some(background) = theme.background {
                window.paint_quad(open_gpui::fill(canvas_bounds, background));
            }
        }
        CanvasSceneLayerPhase::EdgeBehindNodes => {
            for item in scene.edge_items() {
                paint_frame_record_body(
                    item.record_index,
                    canvas_bounds,
                    model,
                    frame,
                    theme,
                    window,
                    cx,
                );
            }
        }
        CanvasSceneLayerPhase::RecordBody => {
            for group in scene.record_groups() {
                if group.has_phase(CanvasSceneLayerPhase::RecordBody) {
                    paint_frame_record_body(
                        group.record_index,
                        canvas_bounds,
                        model,
                        frame,
                        theme,
                        window,
                        cx,
                    );
                }
            }
        }
        CanvasSceneLayerPhase::RecordWidget => {}
        CanvasSceneLayerPhase::RecordChrome => {
            for group in scene.record_groups() {
                if !group.has_phase(CanvasSceneLayerPhase::RecordChrome) {
                    continue;
                }
                let Some(record) = frame.frame.records.get(group.record_index) else {
                    continue;
                };
                paint_frame_record_label(
                    group.record_index,
                    canvas_bounds,
                    frame,
                    theme,
                    window,
                    cx,
                );
                paint_record_selection_feedback(record, canvas_bounds, model, theme, window);
            }
        }
        CanvasSceneLayerPhase::EdgeAboveNodes => {
            paint_selected_edge_feedback(canvas_bounds, model, frame, theme, window);
            paint_hovered_edge_feedback(canvas_bounds, model, frame, theme, window);
        }
        CanvasSceneLayerPhase::ToolChrome | CanvasSceneLayerPhase::HostPortal => {
            paint_interaction_chrome(canvas_bounds, frame, theme, window);
        }
    }
}
