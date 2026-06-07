use crate::{
    CanvasDocument, CanvasEdge, CanvasEdgeRouteKind, CanvasEditor, CanvasViewport, HitOptions,
    HitTarget, SpatialIndex,
};
use open_gpui::{Bounds, Canvas, Hsla, PathBuilder, Pixels, Point, Window, canvas, px, quad, rgb};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct CanvasPaintModel {
    pub document: Arc<CanvasDocument>,
    pub index: Arc<SpatialIndex>,
    pub viewport: CanvasViewport,
}

impl CanvasPaintModel {
    pub fn new(document: CanvasDocument, viewport: CanvasViewport) -> Self {
        let index = SpatialIndex::rebuild(&document);
        Self {
            document: Arc::new(document),
            index: Arc::new(index),
            viewport,
        }
    }

    pub fn from_parts(
        document: Arc<CanvasDocument>,
        index: Arc<SpatialIndex>,
        viewport: CanvasViewport,
    ) -> Self {
        Self {
            document,
            index,
            viewport,
        }
    }
}

impl From<&CanvasEditor> for CanvasPaintModel {
    fn from(editor: &CanvasEditor) -> Self {
        Self {
            document: Arc::new(editor.document.clone()),
            index: Arc::new(editor.index.clone()),
            viewport: editor.viewport,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasPaintOptions {
    pub include_hidden: bool,
    pub include_handles: bool,
    pub cull_margin: Pixels,
}

impl Default for CanvasPaintOptions {
    fn default() -> Self {
        Self {
            include_hidden: false,
            include_handles: false,
            cull_margin: Pixels::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasPaintTheme {
    pub background: Option<Hsla>,
    pub node_fill: Hsla,
    pub node_stroke: Hsla,
    pub node_stroke_width: Pixels,
    pub node_corner_radius: Pixels,
    pub shape_fill: Hsla,
    pub shape_stroke: Hsla,
    pub shape_stroke_width: Pixels,
    pub edge_stroke: Hsla,
    pub edge_stroke_width: Pixels,
    pub handle_fill: Hsla,
    pub handle_stroke: Hsla,
    pub handle_stroke_width: Pixels,
    pub handle_corner_radius: Pixels,
}

impl Default for CanvasPaintTheme {
    fn default() -> Self {
        Self {
            background: None,
            node_fill: Hsla::from(rgb(0xffffff)),
            node_stroke: Hsla::from(rgb(0xd0d7de)),
            node_stroke_width: px(1.0),
            node_corner_radius: px(6.0),
            shape_fill: Hsla::from(rgb(0xf6f8fa)),
            shape_stroke: Hsla::from(rgb(0xd0d7de)),
            shape_stroke_width: px(1.0),
            edge_stroke: Hsla::from(rgb(0x57606a)),
            edge_stroke_width: px(2.0),
            handle_fill: Hsla::from(rgb(0x0969da)),
            handle_stroke: Hsla::from(rgb(0xffffff)),
            handle_stroke_width: px(1.0),
            handle_corner_radius: px(6.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintFrame {
    pub visible_document_bounds: Bounds<Pixels>,
    pub records: Vec<CanvasPaintRecord>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintRecord {
    pub target: HitTarget,
    pub document_bounds: Bounds<Pixels>,
    pub view_bounds: Bounds<Pixels>,
    pub z_index: i32,
    pub hidden: bool,
}

pub fn canvas_view(
    model: CanvasPaintModel,
    options: CanvasPaintOptions,
    theme: CanvasPaintTheme,
) -> Canvas<CanvasPaintFrame> {
    let prepaint_model = model.clone();
    canvas(
        move |bounds, _, _| collect_visible_records(&prepaint_model, bounds, options),
        move |bounds, frame, window, _| {
            paint_canvas_frame(bounds, &model, &frame, theme, window);
        },
    )
}

pub fn collect_visible_records(
    model: &CanvasPaintModel,
    canvas_bounds: Bounds<Pixels>,
    options: CanvasPaintOptions,
) -> CanvasPaintFrame {
    let mut visible_document_bounds = model
        .viewport
        .view_bounds_to_document(Bounds::new(Point::default(), canvas_bounds.size));
    if options.cull_margin > Pixels::ZERO {
        visible_document_bounds = visible_document_bounds.dilate(options.cull_margin);
    }

    let hit_options = HitOptions {
        include_hidden: options.include_hidden,
        include_handles: options.include_handles,
        margin: Pixels::ZERO,
    };
    let records = model
        .index
        .query_with_options(visible_document_bounds, hit_options)
        .map(|record| CanvasPaintRecord {
            target: record.target.clone(),
            document_bounds: record.bounds,
            view_bounds: model.viewport.document_bounds_to_view(record.bounds),
            z_index: record.z_index,
            hidden: record.hidden,
        })
        .collect();

    CanvasPaintFrame {
        visible_document_bounds,
        records,
    }
}

pub fn paint_canvas_frame(
    canvas_bounds: Bounds<Pixels>,
    model: &CanvasPaintModel,
    frame: &CanvasPaintFrame,
    theme: CanvasPaintTheme,
    window: &mut Window,
) {
    if let Some(background) = theme.background {
        window.paint_quad(open_gpui::fill(canvas_bounds, background));
    }

    for record in &frame.records {
        match &record.target {
            HitTarget::Node(id) => {
                let Some(node) = model.document.nodes.get(id) else {
                    continue;
                };
                let fill = style_color(&node.style.fill).unwrap_or(theme.node_fill);
                let stroke = style_color(&node.style.stroke).unwrap_or(theme.node_stroke);
                let stroke_width =
                    positive_pixels_or(node.style.stroke_width, theme.node_stroke_width);
                paint_rect(
                    window,
                    canvas_bounds,
                    record.view_bounds,
                    fill,
                    stroke,
                    stroke_width,
                    theme.node_corner_radius,
                );
            }
            HitTarget::Handle { .. } => {
                paint_rect(
                    window,
                    canvas_bounds,
                    record.view_bounds,
                    theme.handle_fill,
                    theme.handle_stroke,
                    theme.handle_stroke_width,
                    theme.handle_corner_radius,
                );
            }
            HitTarget::Shape(id) => {
                let Some(shape) = model.document.shapes.get(id) else {
                    continue;
                };
                let fill = style_color(&shape.style.fill).unwrap_or(theme.shape_fill);
                let stroke = style_color(&shape.style.stroke).unwrap_or(theme.shape_stroke);
                let stroke_width =
                    positive_pixels_or(shape.style.stroke_width, theme.shape_stroke_width);
                paint_rect(
                    window,
                    canvas_bounds,
                    record.view_bounds,
                    fill,
                    stroke,
                    stroke_width,
                    px(0.0),
                );
            }
            HitTarget::Edge(id) => {
                let Some(edge) = model.document.edges.get(id) else {
                    continue;
                };
                let stroke = style_color(&edge.style.stroke).unwrap_or(theme.edge_stroke);
                let stroke_width =
                    positive_pixels_or(edge.style.stroke_width, theme.edge_stroke_width);
                paint_edge(window, canvas_bounds, model, edge, stroke, stroke_width);
            }
        }
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

fn paint_edge(
    window: &mut Window,
    canvas_bounds: Bounds<Pixels>,
    model: &CanvasPaintModel,
    edge: &CanvasEdge,
    stroke: Hsla,
    stroke_width: Pixels,
) {
    let mut builder = PathBuilder::stroke(stroke_width);
    if edge.route.kind.as_str() == CanvasEdgeRouteKind::CUBIC_BEZIER
        && edge.route.control_points.len() >= 2
    {
        let Ok(source) = model.document.endpoint_position(&edge.source) else {
            return;
        };
        let Ok(target) = model.document.endpoint_position(&edge.target) else {
            return;
        };
        builder.move_to(document_to_window_point(model, canvas_bounds, source));
        builder.cubic_bezier_to(
            document_to_window_point(model, canvas_bounds, target),
            document_to_window_point(model, canvas_bounds, edge.route.control_points[0]),
            document_to_window_point(model, canvas_bounds, edge.route.control_points[1]),
        );
    } else {
        let Ok(points) = model.document.edge_route_points(edge) else {
            return;
        };
        for (index, point) in points.into_iter().enumerate() {
            let point = document_to_window_point(model, canvas_bounds, point);
            if index == 0 {
                builder.move_to(point);
            } else {
                builder.line_to(point);
            }
        }
    }

    if let Ok(path) = builder.build() {
        window.paint_path(path, stroke);
    }
}

fn document_to_window_point(
    model: &CanvasPaintModel,
    canvas_bounds: Bounds<Pixels>,
    point: Point<Pixels>,
) -> Point<Pixels> {
    model.viewport.document_to_view(point) + canvas_bounds.origin
}

fn positive_pixels_or(value: Pixels, fallback: Pixels) -> Pixels {
    if value > Pixels::ZERO && value.as_f32().is_finite() {
        value
    } else {
        fallback
    }
}

fn style_color(value: &Option<String>) -> Option<Hsla> {
    value
        .as_deref()
        .and_then(|value| open_gpui::Rgba::try_from(value).ok())
        .map(Hsla::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanvasHandle, CanvasNode};
    use open_gpui::{Bounds, point, px, size};

    #[test]
    fn collect_visible_records_culls_and_transforms_bounds() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "inside",
                point(px(60.0), px(10.0)),
                size(px(20.0), px(10.0)),
            ))
            .unwrap();
        document
            .insert_node(CanvasNode::new(
                "outside",
                point(px(200.0), px(10.0)),
                size(px(20.0), px(10.0)),
            ))
            .unwrap();
        let model = CanvasPaintModel::new(
            document,
            CanvasViewport::new(point(px(50.0), px(0.0)), 2.0).unwrap(),
        );

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(100.0), px(100.0)), size(px(100.0), px(100.0))),
            CanvasPaintOptions::default(),
        );

        assert_eq!(frame.records.len(), 1);
        assert_eq!(
            frame.records[0].target,
            HitTarget::Node(crate::NodeId::from("inside"))
        );
        assert_eq!(
            frame.records[0].view_bounds,
            Bounds::new(point(px(20.0), px(20.0)), size(px(40.0), px(20.0)))
        );
    }

    #[test]
    fn handles_are_only_collected_when_requested() {
        let mut node = CanvasNode::new("node", point(px(0.0), px(0.0)), size(px(40.0), px(40.0)));
        node.handles
            .push(CanvasHandle::new("out", point(px(40.0), px(20.0))));
        let mut document = CanvasDocument::default();
        document.insert_node(node).unwrap();
        let model = CanvasPaintModel::new(document, CanvasViewport::default());
        let canvas_bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));

        let frame = collect_visible_records(&model, canvas_bounds, CanvasPaintOptions::default());
        assert_eq!(frame.records.len(), 1);

        let frame = collect_visible_records(
            &model,
            canvas_bounds,
            CanvasPaintOptions {
                include_handles: true,
                ..CanvasPaintOptions::default()
            },
        );

        assert!(frame.records.iter().any(|record| {
            matches!(
                &record.target,
                HitTarget::Handle { node_id, handle_id }
                    if node_id.as_str() == "node" && handle_id.as_str() == "out"
            )
        }));
    }

    #[test]
    fn parses_style_hex_colors() {
        assert_eq!(
            style_color(&Some("#0969da".to_string())),
            Some(Hsla::from(rgb(0x0969da)))
        );
        assert_eq!(style_color(&Some("not-a-color".to_string())), None);
    }
}
