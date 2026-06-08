use crate::{
    CanvasDefaultEdgeRouter, CanvasDocument, CanvasEdge, CanvasEdgeRouter, CanvasEditor,
    CanvasEndpoint, CanvasEvent, CanvasGeometryResolver, CanvasKey, CanvasKeyModifiers,
    CanvasKindLabel, CanvasKindPaint, CanvasKindRegistry, CanvasNode, CanvasRouteSegment,
    CanvasRuntime, CanvasSelection, CanvasShape, CanvasSnapAxis, CanvasSnapGuide, CanvasStyle,
    CanvasTransformHandle, CanvasTransformTarget, CanvasViewport, HitOptions, HitTarget,
    PointerButton, ToolState, canvas_transform_handles, connection_hit_options,
};
use open_gpui::{
    App, Bounds, Canvas, ContentMask, Hsla, KeyDownEvent, Keystroke, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PathBuilder, Pixels, Point, ScrollWheelEvent,
    SharedString, TextAlign, TextRun, Window, canvas, px, quad, rgb,
};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct CanvasPaintModel {
    document: Arc<CanvasDocument>,
    runtime: Arc<CanvasRuntime>,
    kind_registry: Arc<CanvasKindRegistry>,
    viewport: CanvasViewport,
    interaction: CanvasPaintInteraction,
}

impl CanvasPaintModel {
    pub fn new(document: CanvasDocument, viewport: CanvasViewport) -> Self {
        Self::new_with_router(document, viewport, &CanvasDefaultEdgeRouter)
    }

    pub fn new_with_kind_registry(
        document: CanvasDocument,
        viewport: CanvasViewport,
        kind_registry: CanvasKindRegistry,
    ) -> Self {
        Self::new_with_router_and_kind_registry(
            document,
            viewport,
            &CanvasDefaultEdgeRouter,
            kind_registry,
        )
    }

    pub fn new_with_router<R>(
        document: CanvasDocument,
        viewport: CanvasViewport,
        router: &R,
    ) -> Self
    where
        R: CanvasEdgeRouter + ?Sized,
    {
        Self::new_with_router_and_kind_registry(
            document,
            viewport,
            router,
            CanvasKindRegistry::open(),
        )
    }

    pub fn new_with_router_and_kind_registry<R>(
        document: CanvasDocument,
        viewport: CanvasViewport,
        router: &R,
        kind_registry: CanvasKindRegistry,
    ) -> Self
    where
        R: CanvasEdgeRouter + ?Sized,
    {
        let runtime =
            CanvasRuntime::rebuild_with_router_and_kind_registry(&document, router, &kind_registry);
        Self {
            document: Arc::new(document),
            runtime: Arc::new(runtime),
            kind_registry: Arc::new(kind_registry),
            viewport,
            interaction: CanvasPaintInteraction::default(),
        }
    }

    pub fn document(&self) -> &CanvasDocument {
        self.document.as_ref()
    }

    pub fn runtime(&self) -> &CanvasRuntime {
        self.runtime.as_ref()
    }

    pub fn kind_registry(&self) -> &CanvasKindRegistry {
        self.kind_registry.as_ref()
    }

    pub fn viewport(&self) -> CanvasViewport {
        self.viewport
    }

    pub fn interaction(&self) -> &CanvasPaintInteraction {
        &self.interaction
    }

    pub fn interaction_mut(&mut self) -> &mut CanvasPaintInteraction {
        &mut self.interaction
    }
}

impl From<&CanvasEditor> for CanvasPaintModel {
    fn from(editor: &CanvasEditor) -> Self {
        Self {
            document: Arc::new(editor.document().clone()),
            runtime: Arc::new(editor.runtime().clone()),
            kind_registry: Arc::new(editor.kind_registry().clone()),
            viewport: editor.viewport(),
            interaction: CanvasPaintInteraction {
                selection: editor.selection().clone(),
                state: editor.state().clone(),
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
    pub snap_guide_stroke: Hsla,
    pub snap_guide_stroke_width: Pixels,
    pub label_color: Hsla,
    pub label_font_size: Pixels,
    pub label_line_height: Pixels,
    pub label_line_clamp: Option<usize>,
    pub label_text_align: TextAlign,
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
            snap_guide_stroke: Hsla::from(rgb(0xbf8700)).alpha(0.9),
            snap_guide_stroke_width: px(1.0),
            label_color: Hsla::from(rgb(0x24292f)),
            label_font_size: px(14.0),
            label_line_height: px(18.0),
            label_line_clamp: Some(3),
            label_text_align: TextAlign::Center,
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
    pub label: Option<CanvasPaintLabel>,
    pub z_index: i32,
    pub hidden: bool,
    pub locked: bool,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintLabel {
    pub text: String,
    pub document_bounds: Bounds<Pixels>,
    pub view_bounds: Bounds<Pixels>,
    pub color: Option<Hsla>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CanvasPaintInteractionFrame {
    pub selection_bounds: Option<Bounds<Pixels>>,
    pub connection_preview: Option<CanvasPaintConnectionPreview>,
    pub transform_handles: Vec<CanvasPaintTransformHandle>,
    pub snap_guides: Vec<CanvasPaintSnapGuide>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintConnectionPreview {
    pub source_view_position: Point<Pixels>,
    pub target_view_position: Point<Pixels>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintTransformHandle {
    pub target: CanvasTransformTarget,
    pub handle: crate::CanvasResizeHandle,
    pub view_bounds: Bounds<Pixels>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintSnapGuide {
    pub axis: CanvasSnapAxis,
    pub view_start: Point<Pixels>,
    pub view_end: Point<Pixels>,
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
            modifiers: Self::modifiers(event.modifiers),
        })
    }

    pub fn mouse_move(&self, event: &MouseMoveEvent) -> Option<CanvasEvent> {
        Some(CanvasEvent::PointerMove {
            position: self.local_position(event.position)?,
            modifiers: Self::modifiers(event.modifiers),
        })
    }

    pub fn mouse_up(&self, event: &MouseUpEvent) -> Option<CanvasEvent> {
        Some(CanvasEvent::PointerUp {
            position: self.local_position(event.position)?,
            button: pointer_button(event.button)?,
            modifiers: Self::modifiers(event.modifiers),
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

    pub fn key_down(&self, event: &KeyDownEvent) -> CanvasEvent {
        Self::key_down_event(event)
    }

    pub fn key_down_event(event: &KeyDownEvent) -> CanvasEvent {
        let key = canvas_key(&event.keystroke);
        if key == CanvasKey::Escape {
            return CanvasEvent::Cancel;
        }

        CanvasEvent::KeyDown {
            key,
            modifiers: Self::modifiers(event.keystroke.modifiers),
            repeat: event.is_held,
        }
    }

    pub fn modifiers(modifiers: Modifiers) -> CanvasKeyModifiers {
        canvas_key_modifiers(modifiers)
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
        move |bounds, frame, window, cx| {
            paint_canvas_frame(bounds, &model, &frame, theme, window, cx);
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
        include_locked: true,
        include_handles: options.include_handles,
        margin: Pixels::ZERO,
    };
    let records = model
        .runtime
        .query_with_options(visible_document_bounds, hit_options)
        .map(|record| {
            let target = record.target.clone();
            CanvasPaintRecord {
                label: paint_record_label(model, &target, record.bounds),
                target,
                document_bounds: record.bounds,
                view_bounds: model.viewport.document_bounds_to_view(record.bounds),
                z_index: record.z_index,
                hidden: record.hidden,
                locked: record.locked,
                selected: options.include_interaction_feedback
                    && target_is_selected(&record.target, &model.interaction.selection),
            }
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
    cx: &mut App,
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
                let Some(edge) = model.document.edges.get(id) else {
                    continue;
                };
                let style = edge_paint_style(model, edge, theme);
                paint_edge(
                    window,
                    canvas_bounds,
                    model,
                    edge,
                    style.stroke,
                    style.stroke_width,
                );
            }
        }

        if let Some(label) = &record.label {
            paint_label(canvas_bounds, label, theme, window, cx);
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

    for guide in &frame.interaction.snap_guides {
        paint_line(
            window,
            canvas_bounds,
            guide.view_start,
            guide.view_end,
            theme.snap_guide_stroke,
            theme.snap_guide_stroke_width,
        );
    }

    for handle in &frame.interaction.transform_handles {
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
}

fn interaction_frame(model: &CanvasPaintModel) -> CanvasPaintInteractionFrame {
    match &model.interaction.state {
        ToolState::Selecting {
            origin, current, ..
        } => CanvasPaintInteractionFrame {
            selection_bounds: Some(
                model
                    .viewport
                    .document_bounds_to_view(bounds_from_points(*origin, *current)),
            ),
            connection_preview: None,
            transform_handles: Vec::new(),
            snap_guides: Vec::new(),
        },
        ToolState::Connecting { source, current } => CanvasPaintInteractionFrame {
            selection_bounds: None,
            connection_preview: connection_preview(model, source, *current),
            transform_handles: Vec::new(),
            snap_guides: Vec::new(),
        },
        ToolState::Translating { snap_guides, .. } | ToolState::Resizing { snap_guides, .. } => {
            CanvasPaintInteractionFrame {
                selection_bounds: None,
                connection_preview: None,
                transform_handles: transform_handles_for_model(model),
                snap_guides: paint_snap_guides(model, snap_guides),
            }
        }
        _ => CanvasPaintInteractionFrame {
            selection_bounds: None,
            connection_preview: None,
            transform_handles: transform_handles_for_model(model),
            snap_guides: Vec::new(),
        },
    }
}

fn paint_snap_guides(
    model: &CanvasPaintModel,
    guides: &[CanvasSnapGuide],
) -> Vec<CanvasPaintSnapGuide> {
    guides
        .iter()
        .map(|guide| CanvasPaintSnapGuide {
            axis: guide.axis,
            view_start: model.viewport.document_to_view(guide.document_start),
            view_end: model.viewport.document_to_view(guide.document_end),
        })
        .collect()
}

fn transform_handles_for_model(model: &CanvasPaintModel) -> Vec<CanvasPaintTransformHandle> {
    canvas_transform_handles(
        model.document.as_ref(),
        &model.interaction.selection,
        model.viewport,
        Some(model.kind_registry.as_ref()),
    )
    .into_iter()
    .map(|handle: CanvasTransformHandle| CanvasPaintTransformHandle {
        target: handle.target,
        handle: handle.handle,
        view_bounds: model
            .viewport
            .document_bounds_to_view(handle.document_bounds),
    })
    .collect()
}

fn connection_preview(
    model: &CanvasPaintModel,
    source: &CanvasEndpoint,
    current: Point<Pixels>,
) -> Option<CanvasPaintConnectionPreview> {
    let resolver = CanvasGeometryResolver::with_kind_registry(
        model.document.as_ref(),
        model.kind_registry.as_ref(),
    );
    let source = resolver.endpoint_position(source).ok()?;
    let target = connection_preview_target_position(model, source, current).unwrap_or(current);
    Some(CanvasPaintConnectionPreview {
        source_view_position: model.viewport.document_to_view(source),
        target_view_position: model.viewport.document_to_view(target),
    })
}

fn connection_preview_target_position(
    model: &CanvasPaintModel,
    source: Point<Pixels>,
    current: Point<Pixels>,
) -> Option<Point<Pixels>> {
    let resolver = CanvasGeometryResolver::with_kind_registry(
        model.document.as_ref(),
        model.kind_registry.as_ref(),
    );
    resolver.connection_preview_target(
        model
            .runtime
            .precise_hit_test_with_resolver(resolver, current, connection_hit_options()),
        source,
        current,
    )
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

fn paint_label(
    canvas_bounds: Bounds<Pixels>,
    label: &CanvasPaintLabel,
    theme: CanvasPaintTheme,
    window: &mut Window,
    cx: &mut App,
) {
    let text = label.text.trim();
    let label_bounds = label.view_bounds + canvas_bounds.origin;
    if text.is_empty()
        || label_bounds.size.width <= Pixels::ZERO
        || label_bounds.size.height <= Pixels::ZERO
        || !positive_pixels(theme.label_font_size)
        || !positive_pixels(theme.label_line_height)
    {
        return;
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
            Some(label_bounds.size.width),
            label_line_clamp(theme, label_bounds),
        )
        .ok()
    else {
        return;
    };

    let text_height = lines.iter().fold(Pixels::ZERO, |height, line| {
        height + line.size(theme.label_line_height).height
    });
    let vertical_offset = ((label_bounds.size.height - text_height) / 2.0).max(Pixels::ZERO);
    let mut origin = Point::new(label_bounds.left(), label_bounds.top() + vertical_offset);

    window.with_content_mask(
        Some(ContentMask {
            bounds: label_bounds,
        }),
        |window| {
            for line in &lines {
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

fn label_line_clamp(theme: CanvasPaintTheme, bounds: Bounds<Pixels>) -> Option<usize> {
    let max_lines_by_height =
        (bounds.size.height.as_f32() / theme.label_line_height.as_f32()).floor() as usize;
    let max_lines = max_lines_by_height.max(1);
    Some(
        theme
            .label_line_clamp
            .map_or(max_lines, |clamp| clamp.max(1).min(max_lines)),
    )
}

fn paint_record_label(
    model: &CanvasPaintModel,
    target: &HitTarget,
    document_bounds: Bounds<Pixels>,
) -> Option<CanvasPaintLabel> {
    let label = match target {
        HitTarget::Node(id) => model
            .document
            .nodes
            .get(id)
            .and_then(|node| model.kind_registry.node_label(node)),
        HitTarget::Shape(id) => model
            .document
            .shapes
            .get(id)
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

fn paint_edge(
    window: &mut Window,
    canvas_bounds: Bounds<Pixels>,
    model: &CanvasPaintModel,
    edge: &CanvasEdge,
    stroke: Hsla,
    stroke_width: Pixels,
) {
    let mut builder = PathBuilder::stroke(stroke_width);
    let Some(path) = paint_edge_route_path(model, edge).cloned() else {
        return;
    };

    let mut current = None;
    for segment in path.segments {
        match segment {
            CanvasRouteSegment::Line { from, to } => {
                let from = document_to_window_point(model, canvas_bounds, from);
                if current != Some(from) {
                    builder.move_to(from);
                }
                let to = document_to_window_point(model, canvas_bounds, to);
                builder.line_to(to);
                current = Some(to);
            }
            CanvasRouteSegment::CubicBezier {
                from,
                control_1,
                control_2,
                to,
            } => {
                let from = document_to_window_point(model, canvas_bounds, from);
                if current != Some(from) {
                    builder.move_to(from);
                }
                let to = document_to_window_point(model, canvas_bounds, to);
                builder.cubic_bezier_to(
                    to,
                    document_to_window_point(model, canvas_bounds, control_1),
                    document_to_window_point(model, canvas_bounds, control_2),
                );
                current = Some(to);
            }
        }
    }

    if let Ok(path) = builder.build() {
        window.paint_path(path, stroke);
    }
}

fn paint_edge_route_path<'a>(
    model: &'a CanvasPaintModel,
    edge: &CanvasEdge,
) -> Option<&'a crate::CanvasRoutePath> {
    model
        .runtime
        .edge_geometry(&edge.id)
        .map(|geometry| &geometry.path)
}

fn document_to_window_point(
    model: &CanvasPaintModel,
    canvas_bounds: Bounds<Pixels>,
    point: Point<Pixels>,
) -> Point<Pixels> {
    model.viewport.document_to_view(point) + canvas_bounds.origin
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CanvasResolvedPaintStyle {
    fill: Hsla,
    stroke: Hsla,
    stroke_width: Pixels,
    corner_radius: Pixels,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CanvasResolvedEdgePaintStyle {
    stroke: Hsla,
    stroke_width: Pixels,
}

fn node_paint_style(
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

fn shape_paint_style(
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

fn edge_paint_style(
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

fn positive_pixels(value: Pixels) -> bool {
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

fn style_color(value: &Option<String>) -> Option<Hsla> {
    value.as_deref().and_then(parse_color)
}

fn parse_color(value: &str) -> Option<Hsla> {
    open_gpui::Rgba::try_from(value).ok().map(Hsla::from)
}

fn pointer_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        MouseButton::Navigate(_) => None,
    }
}

fn canvas_key(keystroke: &Keystroke) -> CanvasKey {
    match keystroke.key.as_str() {
        "delete" | "del" => CanvasKey::Delete,
        "backspace" => CanvasKey::Backspace,
        "escape" | "esc" => CanvasKey::Escape,
        "enter" | "return" => CanvasKey::Enter,
        key if key.chars().count() == 1 => CanvasKey::Character(
            keystroke
                .key_char
                .clone()
                .unwrap_or_else(|| key.to_string()),
        ),
        key => CanvasKey::Named(key.to_string()),
    }
}

fn canvas_key_modifiers(modifiers: Modifiers) -> CanvasKeyModifiers {
    CanvasKeyModifiers {
        shift: modifiers.shift,
        alt: modifiers.alt,
        control: modifiers.control,
        platform: modifiers.platform,
        function: modifiers.function,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CanvasEdgeKind, CanvasHandle, CanvasKindRegistry, CanvasNode, CanvasNodeKind,
        CanvasRoutePath, CanvasRouteRequest, CanvasSelectionMode, CanvasShapeKind,
        CanvasToolEffect, EdgeId, HandleRole,
    };
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
    fn collect_visible_records_keeps_large_canvas_frame_bounded() {
        let document = large_grid_document(128, 96);
        let total_records = document.nodes.len();
        let model = CanvasPaintModel::new(
            document,
            CanvasViewport::new(point(px(2_400.0), px(1_800.0)), 1.0).unwrap(),
        );

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(800.0), px(600.0))),
            CanvasPaintOptions::default(),
        );

        assert_eq!(total_records, 12_288);
        assert!(!frame.records.is_empty());
        assert!(frame.records.len() < 80);
        assert!(frame.records.iter().all(|record| {
            frame
                .visible_document_bounds
                .intersects(&record.document_bounds)
        }));
    }

    #[test]
    fn collect_visible_records_keeps_locked_records_visible() {
        let mut node = CanvasNode::new("locked", point(px(0.0), px(0.0)), size(px(20.0), px(20.0)));
        node.locked = true;
        let mut document = CanvasDocument::default();
        document.insert_node(node).unwrap();
        let model = CanvasPaintModel::new(document, CanvasViewport::default());

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
            CanvasPaintOptions::default(),
        );

        assert_eq!(frame.records.len(), 1);
        assert_eq!(
            frame.records[0].target,
            HitTarget::Node(crate::NodeId::from("locked"))
        );
        assert!(frame.records[0].locked);
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
    fn paint_model_culls_edges_with_custom_router_geometry() {
        let document = connected_edge_document();
        let model = CanvasPaintModel::new_with_router(
            document,
            CanvasViewport::default(),
            &VerticalDetourRouter,
        );

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(76.0)), size(px(12.0), px(12.0))),
            CanvasPaintOptions::default(),
        );

        assert!(frame.records.iter().any(|record| {
            record.target == HitTarget::Edge(EdgeId::from("a-b"))
                && record.document_bounds.origin == point(px(-1.0), px(-1.0))
                && record.document_bounds.size == size(px(32.0), px(87.0))
        }));
        assert_eq!(
            model
                .runtime
                .edge_geometry(&EdgeId::from("a-b"))
                .unwrap()
                .path
                .document_points(),
            vec![
                point(px(5.0), px(5.0)),
                point(px(5.0), px(80.0)),
                point(px(25.0), px(5.0)),
            ]
        );
    }

    #[test]
    fn edge_paint_route_comes_only_from_runtime_geometry() {
        let document = connected_edge_document();
        let model = CanvasPaintModel::new_with_router(
            document,
            CanvasViewport::default(),
            &VerticalDetourRouter,
        );
        let edge = model.document().edges.get(&EdgeId::from("a-b")).unwrap();

        assert_eq!(
            paint_edge_route_path(&model, edge)
                .unwrap()
                .document_points(),
            vec![
                point(px(5.0), px(5.0)),
                point(px(5.0), px(80.0)),
                point(px(25.0), px(5.0)),
            ]
        );

        let mut unresolved_edge = edge.clone();
        unresolved_edge.id = EdgeId::from("missing");
        assert!(paint_edge_route_path(&model, &unresolved_edge).is_none());
    }

    #[test]
    fn paint_model_uses_kind_registry_bounds_in_frame_records() {
        let mut document = CanvasDocument::default();
        let mut node = CanvasNode::new("wide", point(px(10.0), px(10.0)), size(px(20.0), px(20.0)));
        node.kind = "wide".to_string();
        document.insert_node(node).unwrap();
        let model = CanvasPaintModel::new_with_kind_registry(
            document,
            CanvasViewport::default(),
            geometry_registry(),
        );

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
            CanvasPaintOptions::default(),
        );

        let record = frame
            .records
            .iter()
            .find(|record| record.target == HitTarget::Node(crate::NodeId::from("wide")))
            .unwrap();
        assert_eq!(
            record.document_bounds,
            Bounds::new(point(px(5.0), px(5.0)), size(px(30.0), px(30.0)))
        );
        assert_eq!(record.view_bounds, record.document_bounds);
    }

    #[test]
    fn paint_frame_carries_kind_label_metadata_for_nodes_and_shapes() {
        let mut document = CanvasDocument::default();
        let mut node = CanvasNode::new(
            "painted",
            point(px(10.0), px(20.0)),
            size(px(100.0), px(80.0)),
        );
        node.kind = "painted-node".to_string();
        document.insert_node(node).unwrap();
        let mut shape = CanvasShape::new(
            "shape",
            Bounds::new(point(px(150.0), px(20.0)), size(px(90.0), px(70.0))),
        );
        shape.kind = "painted-shape".to_string();
        document.insert_shape(shape).unwrap();
        let model = CanvasPaintModel::new_with_kind_registry(
            document,
            CanvasViewport::new(point(px(10.0), px(10.0)), 2.0).unwrap(),
            paint_registry(),
        );

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(1_000.0), px(1_000.0))),
            CanvasPaintOptions::default(),
        );

        let node_label = frame
            .records
            .iter()
            .find(|record| record.target == HitTarget::Node(crate::NodeId::from("painted")))
            .and_then(|record| record.label.as_ref())
            .unwrap();
        assert_eq!(node_label.text, "Node label");
        assert_eq!(
            node_label.document_bounds,
            Bounds::new(point(px(18.0), px(28.0)), size(px(84.0), px(64.0)))
        );
        assert_eq!(
            node_label.view_bounds,
            Bounds::new(point(px(16.0), px(36.0)), size(px(168.0), px(128.0)))
        );
        assert_eq!(node_label.color, parse_color("#24292f"));

        let shape_label = frame
            .records
            .iter()
            .find(|record| record.target == HitTarget::Shape(crate::ShapeId::from("shape")))
            .and_then(|record| record.label.as_ref())
            .unwrap();
        assert_eq!(shape_label.text, "Shape label");
        assert_eq!(
            shape_label.document_bounds,
            Bounds::new(point(px(154.0), px(24.0)), size(px(82.0), px(62.0)))
        );
        assert_eq!(shape_label.color, parse_color("#0969da"));
    }

    #[test]
    fn paint_theme_defaults_include_bounded_label_text() {
        let theme = CanvasPaintTheme::default();

        assert_eq!(theme.label_color, parse_color("#24292f").unwrap());
        assert_eq!(theme.label_font_size, px(14.0));
        assert_eq!(theme.label_line_height, px(18.0));
        assert_eq!(theme.label_line_clamp, Some(3));
        assert_eq!(theme.label_text_align, TextAlign::Center);
    }

    #[test]
    fn label_line_clamp_uses_theme_and_available_height() {
        let mut theme = CanvasPaintTheme {
            label_line_height: px(10.0),
            label_line_clamp: Some(5),
            ..CanvasPaintTheme::default()
        };
        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(26.0)));

        assert_eq!(label_line_clamp(theme, bounds), Some(2));

        theme.label_line_clamp = None;
        assert_eq!(label_line_clamp(theme, bounds), Some(2));

        theme.label_line_clamp = Some(0);
        assert_eq!(label_line_clamp(theme, bounds), Some(1));
    }

    #[test]
    fn paint_style_uses_record_style_then_kind_fallback_then_theme() {
        let mut document = CanvasDocument::default();
        let mut node = CanvasNode::new(
            "painted",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(80.0)),
        );
        node.kind = "painted-node".to_string();
        document.insert_node(node).unwrap();
        let mut shape = CanvasShape::new(
            "shape",
            Bounds::new(point(px(120.0), px(0.0)), size(px(100.0), px(80.0))),
        );
        shape.kind = "painted-shape".to_string();
        document.insert_shape(shape).unwrap();
        document
            .insert_node(CanvasNode::new(
                "source",
                point(px(0.0), px(160.0)),
                size(px(100.0), px(80.0)),
            ))
            .unwrap();
        document
            .insert_node(CanvasNode::new(
                "target",
                point(px(180.0), px(160.0)),
                size(px(100.0), px(80.0)),
            ))
            .unwrap();
        let mut edge = CanvasEdge::new(
            "edge",
            CanvasEndpoint::new("source", None::<&str>),
            CanvasEndpoint::new("target", None::<&str>),
        );
        edge.kind = "painted-edge".to_string();
        document.insert_edge(edge).unwrap();
        let model = CanvasPaintModel::new_with_kind_registry(
            document,
            CanvasViewport::default(),
            paint_registry(),
        );
        let theme = CanvasPaintTheme::default();

        let node = model
            .document()
            .nodes
            .get(&crate::NodeId::from("painted"))
            .unwrap();
        let node_style = node_paint_style(&model, node, theme);
        assert_eq!(node_style.fill, parse_color("#fff8c5").unwrap());
        assert_eq!(node_style.stroke, parse_color("#bf8700").unwrap());
        assert_eq!(node_style.stroke_width, px(2.0));
        assert_eq!(node_style.corner_radius, px(10.0));

        let shape = model
            .document()
            .shapes
            .get(&crate::ShapeId::from("shape"))
            .unwrap();
        let shape_style = shape_paint_style(&model, shape, theme);
        assert_eq!(shape_style.fill, parse_color("#ddf4ff").unwrap());
        assert_eq!(shape_style.stroke, parse_color("#0969da").unwrap());
        assert_eq!(shape_style.stroke_width, px(3.0));
        assert_eq!(shape_style.corner_radius, px(4.0));

        let edge = model
            .document()
            .edges
            .get(&crate::EdgeId::from("edge"))
            .unwrap();
        let edge_style = edge_paint_style(&model, edge, theme);
        assert_eq!(edge_style.stroke, parse_color("#d1242f").unwrap());
        assert_eq!(edge_style.stroke_width, px(5.0));

        let mut explicit = node.clone();
        explicit.style = CanvasStyle {
            fill: Some("#6f42c1".to_string()),
            stroke: Some("#1a7f37".to_string()),
            stroke_width: px(7.0),
        };
        let explicit_style = node_paint_style(&model, &explicit, theme);
        assert_eq!(explicit_style.fill, parse_color("#6f42c1").unwrap());
        assert_eq!(explicit_style.stroke, parse_color("#1a7f37").unwrap());
        assert_eq!(explicit_style.stroke_width, px(7.0));
        assert_eq!(explicit_style.corner_radius, px(10.0));

        let mut explicit_edge = edge.clone();
        explicit_edge.style.stroke = Some("#6f42c1".to_string());
        explicit_edge.style.stroke_width = px(9.0);
        let explicit_edge_style = edge_paint_style(&model, &explicit_edge, theme);
        assert_eq!(explicit_edge_style.stroke, parse_color("#6f42c1").unwrap());
        assert_eq!(explicit_edge_style.stroke_width, px(9.0));

        let unknown = CanvasNode::new(
            "unknown",
            point(px(240.0), px(0.0)),
            size(px(100.0), px(80.0)),
        );
        assert_eq!(
            node_paint_style(&model, &unknown, theme),
            CanvasResolvedPaintStyle {
                fill: theme.node_fill,
                stroke: theme.node_stroke,
                stroke_width: theme.node_stroke_width,
                corner_radius: theme.node_corner_radius,
            }
        );

        let unknown_edge = CanvasEdge::new(
            "unknown-edge",
            CanvasEndpoint::new("source", None::<&str>),
            CanvasEndpoint::new("target", None::<&str>),
        );
        assert_eq!(
            edge_paint_style(&model, &unknown_edge, theme),
            CanvasResolvedEdgePaintStyle {
                stroke: theme.edge_stroke,
                stroke_width: theme.edge_stroke_width,
            }
        );
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
            .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Node(
                crate::NodeId::from("selected"),
            )))
            .unwrap();
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
    fn selected_records_add_transform_handles_to_paint_frame() {
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
            .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Node(
                crate::NodeId::from("selected"),
            )))
            .unwrap();
        let model = CanvasPaintModel::from(&editor);

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(100.0))),
            CanvasPaintOptions::default(),
        );

        assert_eq!(frame.interaction.transform_handles.len(), 4);
        assert!(frame.interaction.transform_handles.iter().any(|handle| {
            handle.target == CanvasTransformTarget::Node(crate::NodeId::from("selected"))
                && handle.handle == crate::CanvasResizeHandle::BottomRight
                && handle.view_bounds.contains(&point(px(50.0), px(30.0)))
        }));
    }

    #[test]
    fn translating_state_adds_snap_guides_to_paint_frame() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "selected",
                point(px(10.0), px(10.0)),
                size(px(40.0), px(20.0)),
            ))
            .unwrap();
        let mut model = CanvasPaintModel::new(
            document,
            CanvasViewport::new(point(px(10.0), px(20.0)), 2.0).unwrap(),
        );
        model
            .interaction
            .selection
            .nodes
            .insert(crate::NodeId::from("selected"));
        model.interaction.state = ToolState::Translating {
            origin: point(px(10.0), px(10.0)),
            last: point(px(20.0), px(20.0)),
            constraint_axis: None,
            node_ids: vec![crate::NodeId::from("selected")],
            snap_guides: vec![CanvasSnapGuide {
                axis: CanvasSnapAxis::Horizontal,
                document_start: point(px(40.0), px(10.0)),
                document_end: point(px(40.0), px(90.0)),
            }],
        };

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(200.0))),
            CanvasPaintOptions::default(),
        );

        assert_eq!(
            frame.interaction.snap_guides,
            vec![CanvasPaintSnapGuide {
                axis: CanvasSnapAxis::Horizontal,
                view_start: point(px(60.0), px(-20.0)),
                view_end: point(px(60.0), px(140.0)),
            }]
        );
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
            .apply_tool_effects([
                CanvasToolEffect::AddSelection(HitTarget::Node(crate::NodeId::from("selected"))),
                CanvasToolEffect::SetState(ToolState::Selecting {
                    origin: point(px(10.0), px(10.0)),
                    current: point(px(40.0), px(50.0)),
                    selection_mode: CanvasSelectionMode::Replace,
                    base_selection: CanvasSelection::default(),
                }),
            ])
            .unwrap();
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
            selection_mode: CanvasSelectionMode::Replace,
            base_selection: CanvasSelection::default(),
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
        editor
            .apply_tool_effect(CanvasToolEffect::SetState(ToolState::Connecting {
                source: CanvasEndpoint::new("source", Some("out")),
                current: point(px(180.0), px(120.0)),
            }))
            .unwrap();
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
    fn connecting_preview_uses_kind_registry_endpoint_positions() {
        let mut source =
            CanvasNode::new("source", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        source.kind = "wide".to_string();
        let mut source_handle = CanvasHandle::new("out", point(px(10.0), px(5.0)));
        source_handle.role = HandleRole::Source;
        source.handles.push(source_handle);

        let mut target =
            CanvasNode::new("target", point(px(60.0), px(0.0)), size(px(10.0), px(10.0)));
        target.kind = "wide".to_string();
        let mut target_handle = CanvasHandle::new("in", point(px(0.0), px(5.0)));
        target_handle.role = HandleRole::Target;
        target.handles.push(target_handle);

        let mut document = CanvasDocument::default();
        document.insert_node(source).unwrap();
        document.insert_node(target).unwrap();
        let mut model = CanvasPaintModel::new_with_kind_registry(
            document,
            CanvasViewport::default(),
            geometry_registry(),
        );
        model.interaction.state = ToolState::Connecting {
            source: CanvasEndpoint::new("source", Some("out")),
            current: point(px(40.0), px(5.0)),
        };

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
            CanvasPaintOptions::default(),
        );

        assert_eq!(
            frame.interaction.connection_preview,
            Some(CanvasPaintConnectionPreview {
                source_view_position: point(px(30.0), px(5.0)),
                target_view_position: point(px(40.0), px(5.0)),
            })
        );
    }

    #[test]
    fn connecting_preview_snaps_to_valid_target_handle() {
        let mut source = CanvasNode::new(
            "source",
            point(px(10.0), px(20.0)),
            size(px(100.0), px(80.0)),
        );
        let mut source_handle = CanvasHandle::new("out", point(px(100.0), px(40.0)));
        source_handle.role = HandleRole::Source;
        source.handles.push(source_handle);

        let mut target = CanvasNode::new(
            "target",
            point(px(200.0), px(20.0)),
            size(px(100.0), px(80.0)),
        );
        let mut target_handle = CanvasHandle::new("in", point(px(0.0), px(40.0)));
        target_handle.role = HandleRole::Target;
        target.handles.push(target_handle);

        let mut document = CanvasDocument::default();
        document.insert_node(source).unwrap();
        document.insert_node(target).unwrap();
        let mut editor = CanvasEditor::new(document);
        editor
            .apply_tool_effect(CanvasToolEffect::SetState(ToolState::Connecting {
                source: CanvasEndpoint::new("source", Some("out")),
                current: point(px(204.0), px(64.0)),
            }))
            .unwrap();
        let model = CanvasPaintModel::from(&editor);

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(320.0), px(140.0))),
            CanvasPaintOptions::default(),
        );

        assert_eq!(
            frame.interaction.connection_preview,
            Some(CanvasPaintConnectionPreview {
                source_view_position: point(px(110.0), px(60.0)),
                target_view_position: point(px(200.0), px(60.0)),
            })
        );
    }

    #[test]
    fn connecting_preview_does_not_snap_to_invalid_target_handle() {
        let mut source = CanvasNode::new(
            "source",
            point(px(10.0), px(20.0)),
            size(px(100.0), px(80.0)),
        );
        let mut source_handle = CanvasHandle::new("out", point(px(100.0), px(40.0)));
        source_handle.role = HandleRole::Source;
        source.handles.push(source_handle);

        let mut target = CanvasNode::new(
            "target",
            point(px(200.0), px(20.0)),
            size(px(100.0), px(80.0)),
        );
        let mut invalid_target_handle = CanvasHandle::new("out", point(px(0.0), px(40.0)));
        invalid_target_handle.role = HandleRole::Source;
        target.handles.push(invalid_target_handle);

        let mut document = CanvasDocument::default();
        document.insert_node(source).unwrap();
        document.insert_node(target).unwrap();
        let mut editor = CanvasEditor::new(document);
        editor
            .apply_tool_effect(CanvasToolEffect::SetState(ToolState::Connecting {
                source: CanvasEndpoint::new("source", Some("out")),
                current: point(px(204.0), px(64.0)),
            }))
            .unwrap();
        let model = CanvasPaintModel::from(&editor);

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(320.0), px(140.0))),
            CanvasPaintOptions::default(),
        );

        assert_eq!(
            frame.interaction.connection_preview,
            Some(CanvasPaintConnectionPreview {
                source_view_position: point(px(110.0), px(60.0)),
                target_view_position: point(px(204.0), px(64.0)),
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
                modifiers: Modifiers {
                    shift: true,
                    ..Modifiers::default()
                },
                ..MouseDownEvent::default()
            }),
            Some(CanvasEvent::PointerDown {
                position: point(px(20.0), px(30.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers {
                    shift: true,
                    ..CanvasKeyModifiers::default()
                },
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
                modifiers: CanvasKeyModifiers::default(),
            })
        );
        assert_eq!(
            mapper.mouse_move(&MouseMoveEvent {
                position: point(px(150.0), px(95.0)),
                modifiers: Modifiers {
                    shift: true,
                    ..Modifiers::default()
                },
                ..MouseMoveEvent::default()
            }),
            Some(CanvasEvent::PointerMove {
                position: point(px(50.0), px(45.0)),
                modifiers: CanvasKeyModifiers {
                    shift: true,
                    ..CanvasKeyModifiers::default()
                },
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

    #[test]
    fn input_mapper_converts_key_down_events() {
        let mapper = CanvasInputMapper::new(Bounds::new(
            point(px(100.0), px(50.0)),
            size(px(200.0), px(120.0)),
        ));

        assert_eq!(
            mapper.key_down(&KeyDownEvent {
                keystroke: Keystroke::parse("backspace").unwrap(),
                is_held: false,
                prefer_character_input: false,
            }),
            CanvasEvent::KeyDown {
                key: CanvasKey::Backspace,
                modifiers: CanvasKeyModifiers::default(),
                repeat: false,
            }
        );
        assert_eq!(
            mapper.key_down(&KeyDownEvent {
                keystroke: Keystroke::parse("ctrl-a").unwrap(),
                is_held: true,
                prefer_character_input: false,
            }),
            CanvasEvent::KeyDown {
                key: CanvasKey::Character("a".to_string()),
                modifiers: CanvasKeyModifiers {
                    control: true,
                    ..CanvasKeyModifiers::default()
                },
                repeat: true,
            }
        );
        assert_eq!(
            CanvasInputMapper::key_down_event(&KeyDownEvent {
                keystroke: Keystroke::parse("escape").unwrap(),
                is_held: false,
                prefer_character_input: false,
            }),
            CanvasEvent::Cancel
        );
    }

    fn large_grid_document(columns: usize, rows: usize) -> CanvasDocument {
        let mut document = CanvasDocument::default();

        for row in 0..rows {
            for column in 0..columns {
                document
                    .insert_node(CanvasNode::new(
                        format!("node-{row}-{column}"),
                        point(px(column as f32 * 160.0), px(row as f32 * 120.0)),
                        size(px(96.0), px(56.0)),
                    ))
                    .unwrap();
            }
        }

        document
    }

    fn connected_edge_document() -> CanvasDocument {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();
        document
            .insert_node(CanvasNode::new(
                "b",
                point(px(20.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();
        document
            .insert_edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .unwrap();
        document
    }

    fn geometry_registry() -> CanvasKindRegistry {
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind("wide", WideNodeKind);
        registry
    }

    fn paint_registry() -> CanvasKindRegistry {
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind("painted-node", PaintedNodeKind);
        registry.register_edge_kind("painted-edge", PaintedEdgeKind);
        registry.register_shape_kind("painted-shape", PaintedShapeKind);
        registry
    }

    struct WideNodeKind;

    impl CanvasNodeKind for WideNodeKind {
        fn node_bounds(&self, node: &CanvasNode) -> Option<Bounds<open_gpui::Pixels>> {
            Some(node.bounds().dilate(px(5.0)))
        }

        fn handle_position(
            &self,
            node: &CanvasNode,
            handle_id: &crate::HandleId,
        ) -> Option<open_gpui::Point<open_gpui::Pixels>> {
            match handle_id.as_str() {
                "out" => Some(point(
                    node.position.x + node.size.width + px(20.0),
                    node.position.y + px(5.0),
                )),
                "in" => Some(point(node.position.x - px(20.0), node.position.y + px(5.0))),
                _ => None,
            }
        }
    }

    struct PaintedNodeKind;

    impl CanvasNodeKind for PaintedNodeKind {
        fn node_paint(&self, _node: &CanvasNode) -> Option<CanvasKindPaint> {
            Some(CanvasKindPaint {
                fill: Some("#fff8c5".to_string()),
                stroke: Some("#bf8700".to_string()),
                stroke_width: Some(px(2.0)),
                corner_radius: Some(px(10.0)),
            })
        }

        fn node_label(&self, _node: &CanvasNode) -> Option<CanvasKindLabel> {
            Some(
                CanvasKindLabel::new("Node label")
                    .with_inset(px(8.0))
                    .with_color("#24292f"),
            )
        }
    }

    struct PaintedEdgeKind;

    impl CanvasEdgeKind for PaintedEdgeKind {
        fn edge_paint(&self, _edge: &CanvasEdge) -> Option<CanvasKindPaint> {
            Some(CanvasKindPaint {
                fill: None,
                stroke: Some("#d1242f".to_string()),
                stroke_width: Some(px(5.0)),
                corner_radius: None,
            })
        }
    }

    struct PaintedShapeKind;

    impl CanvasShapeKind for PaintedShapeKind {
        fn shape_paint(&self, _shape: &CanvasShape) -> Option<CanvasKindPaint> {
            Some(CanvasKindPaint {
                fill: Some("#ddf4ff".to_string()),
                stroke: Some("#0969da".to_string()),
                stroke_width: Some(px(3.0)),
                corner_radius: Some(px(4.0)),
            })
        }

        fn shape_label(&self, _shape: &CanvasShape) -> Option<CanvasKindLabel> {
            Some(
                CanvasKindLabel::new("Shape label")
                    .with_inset(px(4.0))
                    .with_color("#0969da"),
            )
        }
    }

    struct VerticalDetourRouter;

    impl CanvasEdgeRouter for VerticalDetourRouter {
        fn route_edge(&self, request: CanvasRouteRequest<'_>) -> CanvasRoutePath {
            CanvasRoutePath::polyline([
                request.source,
                point(request.source.x, px(80.0)),
                request.target,
            ])
        }
    }
}
