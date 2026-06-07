use crate::{
    CanvasDocument, CanvasEdge, CanvasEdgeRouteKind, CanvasEditor, CanvasEndpoint, CanvasEvent,
    CanvasSelection, CanvasViewport, HitOptions, HitTarget, PointerButton, SpatialIndex, ToolState,
};
use open_gpui::{
    Bounds, Canvas, Hsla, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PathBuilder,
    Pixels, Point, ScrollWheelEvent, Window, canvas, px, quad, rgb,
};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct CanvasPaintModel {
    pub document: Arc<CanvasDocument>,
    pub index: Arc<SpatialIndex>,
    pub viewport: CanvasViewport,
    pub interaction: CanvasPaintInteraction,
}

impl CanvasPaintModel {
    pub fn new(document: CanvasDocument, viewport: CanvasViewport) -> Self {
        let index = SpatialIndex::rebuild(&document);
        Self {
            document: Arc::new(document),
            index: Arc::new(index),
            viewport,
            interaction: CanvasPaintInteraction::default(),
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
            interaction: CanvasPaintInteraction::default(),
        }
    }

    pub fn from_parts_with_interaction(
        document: Arc<CanvasDocument>,
        index: Arc<SpatialIndex>,
        viewport: CanvasViewport,
        interaction: CanvasPaintInteraction,
    ) -> Self {
        Self {
            document,
            index,
            viewport,
            interaction,
        }
    }
}

impl From<&CanvasEditor> for CanvasPaintModel {
    fn from(editor: &CanvasEditor) -> Self {
        Self {
            document: Arc::new(editor.document.clone()),
            index: Arc::new(editor.index.clone()),
            viewport: editor.viewport,
            interaction: CanvasPaintInteraction {
                selection: editor.selection.clone(),
                state: editor.state.clone(),
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintInteraction {
    pub selection: CanvasSelection,
    pub state: ToolState,
}

impl Default for CanvasPaintInteraction {
    fn default() -> Self {
        Self {
            selection: CanvasSelection::default(),
            state: ToolState::Idle,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasPaintOptions {
    pub include_hidden: bool,
    pub include_handles: bool,
    pub include_interaction_feedback: bool,
    pub cull_margin: Pixels,
}

impl Default for CanvasPaintOptions {
    fn default() -> Self {
        Self {
            include_hidden: false,
            include_handles: false,
            include_interaction_feedback: true,
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
    pub selection_fill: Hsla,
    pub selection_stroke: Hsla,
    pub selection_stroke_width: Pixels,
    pub selection_corner_radius: Pixels,
    pub selection_bounds_fill: Hsla,
    pub selection_bounds_stroke: Hsla,
    pub selection_bounds_stroke_width: Pixels,
    pub connection_preview_stroke: Hsla,
    pub connection_preview_stroke_width: Pixels,
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
            selection_fill: Hsla::from(rgb(0x0969da)).alpha(0.08),
            selection_stroke: Hsla::from(rgb(0x0969da)),
            selection_stroke_width: px(2.0),
            selection_corner_radius: px(7.0),
            selection_bounds_fill: Hsla::from(rgb(0x0969da)).alpha(0.08),
            selection_bounds_stroke: Hsla::from(rgb(0x0969da)).alpha(0.7),
            selection_bounds_stroke_width: px(1.0),
            connection_preview_stroke: Hsla::from(rgb(0x0969da)).alpha(0.7),
            connection_preview_stroke_width: px(2.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintFrame {
    pub visible_document_bounds: Bounds<Pixels>,
    pub records: Vec<CanvasPaintRecord>,
    pub interaction: CanvasPaintInteractionFrame,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintRecord {
    pub target: HitTarget,
    pub document_bounds: Bounds<Pixels>,
    pub view_bounds: Bounds<Pixels>,
    pub z_index: i32,
    pub hidden: bool,
    pub selected: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CanvasPaintInteractionFrame {
    pub selection_bounds: Option<Bounds<Pixels>>,
    pub connection_preview: Option<CanvasPaintConnectionPreview>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintConnectionPreview {
    pub source_view_position: Point<Pixels>,
    pub target_view_position: Point<Pixels>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasInputMapper {
    pub bounds: Bounds<Pixels>,
    pub line_height: Pixels,
}

impl CanvasInputMapper {
    pub fn new(bounds: Bounds<Pixels>) -> Self {
        Self {
            bounds,
            line_height: px(16.0),
        }
    }

    pub fn with_line_height(mut self, line_height: Pixels) -> Self {
        self.line_height = line_height;
        self
    }

    pub fn mouse_down(&self, event: &MouseDownEvent) -> Option<CanvasEvent> {
        Some(CanvasEvent::PointerDown {
            position: self.local_position(event.position)?,
            button: pointer_button(event.button)?,
        })
    }

    pub fn mouse_move(&self, event: &MouseMoveEvent) -> Option<CanvasEvent> {
        Some(CanvasEvent::PointerMove {
            position: self.local_position(event.position)?,
        })
    }

    pub fn mouse_up(&self, event: &MouseUpEvent) -> Option<CanvasEvent> {
        Some(CanvasEvent::PointerUp {
            position: self.local_position(event.position)?,
            button: pointer_button(event.button)?,
        })
    }

    pub fn scroll_wheel(&self, event: &ScrollWheelEvent) -> Option<CanvasEvent> {
        if self.local_position(event.position).is_none() {
            return None;
        }

        Some(CanvasEvent::Wheel {
            delta: event.delta.pixel_delta(self.line_height),
        })
    }

    pub fn local_position(&self, position: Point<Pixels>) -> Option<Point<Pixels>> {
        self.bounds
            .contains(&position)
            .then(|| position - self.bounds.origin)
    }
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
            selected: options.include_interaction_feedback
                && target_is_selected(&record.target, &model.interaction.selection),
        })
        .collect();

    CanvasPaintFrame {
        visible_document_bounds,
        records,
        interaction: if options.include_interaction_feedback {
            interaction_frame(model)
        } else {
            CanvasPaintInteractionFrame::default()
        },
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

    for record in &frame.records {
        if !record.selected {
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
                let Some(edge) = model.document.edges.get(id) else {
                    continue;
                };
                paint_edge(
                    window,
                    canvas_bounds,
                    model,
                    edge,
                    theme.selection_stroke,
                    theme.selection_stroke_width,
                );
            }
        }
    }

    if let Some(bounds) = frame.interaction.selection_bounds {
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

    if let Some(preview) = &frame.interaction.connection_preview {
        paint_line(
            window,
            canvas_bounds,
            preview.source_view_position,
            preview.target_view_position,
            theme.connection_preview_stroke,
            theme.connection_preview_stroke_width,
        );
    }
}

fn interaction_frame(model: &CanvasPaintModel) -> CanvasPaintInteractionFrame {
    match &model.interaction.state {
        ToolState::Selecting { origin, current } => CanvasPaintInteractionFrame {
            selection_bounds: Some(
                model
                    .viewport
                    .document_bounds_to_view(bounds_from_points(*origin, *current)),
            ),
            connection_preview: None,
        },
        ToolState::Connecting { source, current } => CanvasPaintInteractionFrame {
            selection_bounds: None,
            connection_preview: connection_preview(model, source, *current),
        },
        _ => CanvasPaintInteractionFrame::default(),
    }
}

fn connection_preview(
    model: &CanvasPaintModel,
    source: &CanvasEndpoint,
    current: Point<Pixels>,
) -> Option<CanvasPaintConnectionPreview> {
    let source = model.document.endpoint_position(source).ok()?;
    Some(CanvasPaintConnectionPreview {
        source_view_position: model.viewport.document_to_view(source),
        target_view_position: model.viewport.document_to_view(current),
    })
}

fn target_is_selected(target: &HitTarget, selection: &CanvasSelection) -> bool {
    match target {
        HitTarget::Node(id) => selection.nodes.contains(id),
        HitTarget::Handle { node_id, handle_id } => selection.handles.contains(&CanvasEndpoint {
            node_id: node_id.clone(),
            handle_id: Some(handle_id.clone()),
        }),
        HitTarget::Edge(id) => selection.edges.contains(id),
        HitTarget::Shape(id) => selection.shapes.contains(id),
    }
}

fn selection_corner_radius(record: &CanvasPaintRecord, theme: CanvasPaintTheme) -> Pixels {
    match &record.target {
        HitTarget::Node(_) => theme.selection_corner_radius,
        HitTarget::Handle { .. } => theme.handle_corner_radius,
        HitTarget::Shape(_) | HitTarget::Edge(_) => px(0.0),
    }
}

fn bounds_from_points(a: Point<Pixels>, b: Point<Pixels>) -> Bounds<Pixels> {
    Bounds::from_corners(
        Point::new(a.x.min(b.x), a.y.min(b.y)),
        Point::new(a.x.max(b.x), a.y.max(b.y)),
    )
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

fn pointer_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        MouseButton::Navigate(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanvasHandle, CanvasNode};
    use open_gpui::{Bounds, ScrollDelta, point, px, size};

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
    fn selected_records_are_marked_in_paint_frame() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "selected",
                point(px(10.0), px(10.0)),
                size(px(40.0), px(20.0)),
            ))
            .unwrap();
        document
            .insert_node(CanvasNode::new(
                "plain",
                point(px(70.0), px(10.0)),
                size(px(40.0), px(20.0)),
            ))
            .unwrap();
        let mut editor = CanvasEditor::new(document);
        editor
            .selection
            .nodes
            .insert(crate::NodeId::from("selected"));
        let model = CanvasPaintModel::from(&editor);

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(100.0))),
            CanvasPaintOptions::default(),
        );

        assert!(frame.records.iter().any(|record| {
            record.target == HitTarget::Node(crate::NodeId::from("selected")) && record.selected
        }));
        assert!(frame.records.iter().any(|record| {
            record.target == HitTarget::Node(crate::NodeId::from("plain")) && !record.selected
        }));
    }

    #[test]
    fn interaction_feedback_can_be_disabled() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "selected",
                point(px(10.0), px(10.0)),
                size(px(40.0), px(20.0)),
            ))
            .unwrap();
        let mut editor = CanvasEditor::new(document);
        editor
            .selection
            .nodes
            .insert(crate::NodeId::from("selected"));
        editor.state = ToolState::Selecting {
            origin: point(px(10.0), px(10.0)),
            current: point(px(40.0), px(50.0)),
        };
        let model = CanvasPaintModel::from(&editor);

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(100.0))),
            CanvasPaintOptions {
                include_interaction_feedback: false,
                ..CanvasPaintOptions::default()
            },
        );

        assert!(frame.records.iter().all(|record| !record.selected));
        assert_eq!(frame.interaction, CanvasPaintInteractionFrame::default());
    }

    #[test]
    fn selecting_state_adds_selection_bounds_feedback() {
        let mut model = CanvasPaintModel::new(
            CanvasDocument::default(),
            CanvasViewport::new(point(px(10.0), px(20.0)), 2.0).unwrap(),
        );
        model.interaction.state = ToolState::Selecting {
            origin: point(px(40.0), px(80.0)),
            current: point(px(20.0), px(50.0)),
        };

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(200.0))),
            CanvasPaintOptions::default(),
        );

        assert_eq!(
            frame.interaction.selection_bounds,
            Some(Bounds::new(
                point(px(20.0), px(60.0)),
                size(px(40.0), px(60.0))
            ))
        );
    }

    #[test]
    fn connecting_state_adds_connection_preview_feedback() {
        let mut node = CanvasNode::new(
            "source",
            point(px(10.0), px(20.0)),
            size(px(100.0), px(80.0)),
        );
        node.handles
            .push(CanvasHandle::new("out", point(px(100.0), px(40.0))));
        let mut document = CanvasDocument::default();
        document.insert_node(node).unwrap();
        let mut editor = CanvasEditor::new(document);
        editor.state = ToolState::Connecting {
            source: CanvasEndpoint::new("source", Some("out")),
            current: point(px(180.0), px(120.0)),
        };
        let model = CanvasPaintModel::from(&editor);

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(240.0), px(180.0))),
            CanvasPaintOptions::default(),
        );

        assert_eq!(
            frame.interaction.connection_preview,
            Some(CanvasPaintConnectionPreview {
                source_view_position: point(px(110.0), px(60.0)),
                target_view_position: point(px(180.0), px(120.0)),
            })
        );
    }

    #[test]
    fn parses_style_hex_colors() {
        assert_eq!(
            style_color(&Some("#0969da".to_string())),
            Some(Hsla::from(rgb(0x0969da)))
        );
        assert_eq!(style_color(&Some("not-a-color".to_string())), None);
    }

    #[test]
    fn input_mapper_localizes_pointer_events() {
        let mapper = CanvasInputMapper::new(Bounds::new(
            point(px(100.0), px(50.0)),
            size(px(200.0), px(120.0)),
        ));

        assert_eq!(
            mapper.mouse_down(&MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(120.0), px(80.0)),
                ..MouseDownEvent::default()
            }),
            Some(CanvasEvent::PointerDown {
                position: point(px(20.0), px(30.0)),
                button: PointerButton::Primary,
            })
        );
        assert_eq!(
            mapper.mouse_up(&MouseUpEvent {
                button: MouseButton::Right,
                position: point(px(140.0), px(90.0)),
                ..MouseUpEvent::default()
            }),
            Some(CanvasEvent::PointerUp {
                position: point(px(40.0), px(40.0)),
                button: PointerButton::Secondary,
            })
        );
        assert_eq!(
            mapper.mouse_move(&MouseMoveEvent {
                position: point(px(150.0), px(95.0)),
                ..MouseMoveEvent::default()
            }),
            Some(CanvasEvent::PointerMove {
                position: point(px(50.0), px(45.0)),
            })
        );
    }

    #[test]
    fn input_mapper_filters_outside_or_unsupported_pointer_events() {
        let mapper = CanvasInputMapper::new(Bounds::new(
            point(px(100.0), px(50.0)),
            size(px(200.0), px(120.0)),
        ));

        assert_eq!(
            mapper.mouse_down(&MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(20.0), px(80.0)),
                ..MouseDownEvent::default()
            }),
            None
        );
        assert_eq!(
            mapper.mouse_down(&MouseDownEvent {
                button: MouseButton::Navigate(open_gpui::NavigationDirection::Back),
                position: point(px(120.0), px(80.0)),
                ..MouseDownEvent::default()
            }),
            None
        );
    }

    #[test]
    fn input_mapper_converts_scroll_delta_to_canvas_wheel() {
        let mapper = CanvasInputMapper::new(Bounds::new(
            point(px(100.0), px(50.0)),
            size(px(200.0), px(120.0)),
        ))
        .with_line_height(px(20.0));

        assert_eq!(
            mapper.scroll_wheel(&ScrollWheelEvent {
                position: point(px(120.0), px(80.0)),
                delta: ScrollDelta::Lines(point(1.0, -2.0)),
                ..ScrollWheelEvent::default()
            }),
            Some(CanvasEvent::Wheel {
                delta: point(px(20.0), px(-40.0)),
            })
        );
        assert_eq!(
            mapper.scroll_wheel(&ScrollWheelEvent {
                position: point(px(20.0), px(80.0)),
                delta: ScrollDelta::Pixels(point(px(1.0), px(2.0))),
                ..ScrollWheelEvent::default()
            }),
            None
        );
    }
}
