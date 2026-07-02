use super::model::{
    CanvasPaintInteractionState, CanvasPaintModel, CanvasPaintOptions, CanvasPaintTheme,
};
use super::style::{parse_color, positive_pixels};
use crate::{
    CanvasConnectionEndpointRole, CanvasEndpoint, CanvasGeometryFacts, CanvasKindLabel,
    CanvasRecordId, CanvasRecordScopeOptions, CanvasResolvedSelectionScope, CanvasRoutePath,
    CanvasRouteSegment, CanvasSelection, CanvasSnapAxis, CanvasSnapGuide, CanvasTransformHandle,
    CanvasTransformTarget, CanvasViewport, EdgeId, HitOptions, HitTarget, canvas_transform_handles,
    connection_hit_options, resolve_selection_scope,
};
use open_gpui::{
    Bounds, Hsla, Pixels, Point, SharedString, TextRun, Window, WrappedLine, px, size,
};

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintFrame {
    pub visible_document_bounds: Bounds<Pixels>,
    pub records: Vec<CanvasPaintRecord>,
    pub interaction: CanvasPaintInteractionFrame,
}

impl CanvasPaintFrame {
    pub fn widget_overlay_frame(
        &self,
        options: CanvasWidgetOverlayOptions,
    ) -> CanvasWidgetOverlayFrame {
        collect_widget_overlay_frame(self, options)
    }
}

#[derive(Debug)]
pub struct CanvasPreparedPaintFrame {
    pub(super) frame: CanvasPaintFrame,
    pub(super) labels: Vec<CanvasPreparedPaintLabel>,
    pub(super) label_indices: Vec<Option<usize>>,
}

impl CanvasPreparedPaintFrame {
    pub fn frame(&self) -> &CanvasPaintFrame {
        &self.frame
    }

    pub fn into_frame(self) -> CanvasPaintFrame {
        self.frame
    }

    pub fn prepared_label_count(&self) -> usize {
        self.labels.len()
    }

    pub fn record_count(&self) -> usize {
        self.frame.records.len()
    }

    pub fn has_prepared_label(&self, record_index: usize) -> bool {
        self.label_indices
            .get(record_index)
            .is_some_and(Option::is_some)
    }

    pub fn widget_overlay_frame(
        &self,
        options: CanvasWidgetOverlayOptions,
    ) -> CanvasWidgetOverlayFrame {
        self.frame.widget_overlay_frame(options)
    }
}

#[derive(Debug)]
pub(super) struct CanvasPreparedPaintLabel {
    pub(super) view_bounds: Bounds<Pixels>,
    pub(super) lines: Vec<WrappedLine>,
    pub(super) text_height: Pixels,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintRecord {
    pub target: HitTarget,
    pub document_bounds: Bounds<Pixels>,
    pub view_bounds: Bounds<Pixels>,
    pub label: Option<CanvasPaintLabel>,
    pub edge_geometry: Option<CanvasPaintEdgeGeometry>,
    pub z_index: i32,
    pub hidden: bool,
    pub locked: bool,
    pub selected: bool,
    pub structurally_selected: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintEdgeGeometry {
    pub view_path: CanvasRoutePath,
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
    pub structural_selection_bounds: Option<Bounds<Pixels>>,
    pub connection_preview: Option<CanvasPaintConnectionPreview>,
    pub reconnect_handles: Vec<CanvasPaintReconnectHandle>,
    pub transform_handles: Vec<CanvasPaintTransformHandle>,
    pub snap_guides: Vec<CanvasPaintSnapGuide>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintConnectionPreview {
    pub source_view_position: Point<Pixels>,
    pub target_view_position: Point<Pixels>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasPaintReconnectEndpoint {
    Source,
    Target,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintReconnectHandle {
    pub edge_id: EdgeId,
    pub endpoint: CanvasPaintReconnectEndpoint,
    pub view_bounds: Bounds<Pixels>,
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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CanvasWidgetOverlayFrame {
    pub placements: Vec<CanvasWidgetOverlayPlacement>,
}

impl CanvasWidgetOverlayFrame {
    pub fn is_empty(&self) -> bool {
        self.placements.is_empty()
    }

    pub fn len(&self) -> usize {
        self.placements.len()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasWidgetOverlayPlacement {
    pub target: HitTarget,
    pub document_bounds: Bounds<Pixels>,
    pub view_bounds: Bounds<Pixels>,
    pub z_index: i32,
    pub hit_priority: CanvasWidgetOverlayHitPriority,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CanvasWidgetOverlayOptions {
    pub include_selected_nodes: bool,
    pub include_selected_shapes: bool,
    pub include_hidden: bool,
    pub include_locked: bool,
    pub hit_priority: CanvasWidgetOverlayHitPriority,
}

impl CanvasWidgetOverlayOptions {
    pub fn selected_nodes() -> Self {
        Self {
            include_selected_nodes: true,
            ..Self::default()
        }
    }

    pub fn selected_records() -> Self {
        Self {
            include_selected_nodes: true,
            include_selected_shapes: true,
            ..Self::default()
        }
    }

    pub fn with_locked(mut self, include_locked: bool) -> Self {
        self.include_locked = include_locked;
        self
    }

    pub fn with_hidden(mut self, include_hidden: bool) -> Self {
        self.include_hidden = include_hidden;
        self
    }

    pub fn with_hit_priority(mut self, hit_priority: CanvasWidgetOverlayHitPriority) -> Self {
        self.hit_priority = hit_priority;
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CanvasWidgetOverlayHitPriority {
    CanvasFirst,
    #[default]
    WidgetFirst,
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
    let selection_scope = options.include_interaction_feedback.then(|| {
        resolve_selection_scope(
            model.document.as_ref(),
            model.interaction.selection(),
            CanvasRecordScopeOptions::structural_with_internal_edges(),
        )
    });
    let records = model
        .runtime
        .query_with_options(visible_document_bounds, hit_options)
        .map(|record| {
            let target = record.target.clone();
            let edge_geometry = paint_edge_geometry(model, &target);
            CanvasPaintRecord {
                label: paint_record_label(model, &target, record.bounds),
                target,
                document_bounds: record.bounds,
                view_bounds: model.viewport.document_bounds_to_view(record.bounds),
                edge_geometry,
                z_index: record.z_index,
                hidden: record.hidden,
                locked: record.locked,
                selected: options.include_interaction_feedback
                    && selection_scope.as_ref().is_some_and(|scope| {
                        target_is_selected(&record.target, scope.normalized_selection())
                    }),
                structurally_selected: target_is_structurally_selected(
                    &record.target,
                    selection_scope.as_ref(),
                ),
            }
        })
        .collect();

    CanvasPaintFrame {
        visible_document_bounds,
        records,
        interaction: if options.include_interaction_feedback {
            interaction_frame(model, selection_scope.as_ref())
        } else {
            CanvasPaintInteractionFrame::default()
        },
    }
}

pub fn collect_widget_overlay_frame(
    frame: &CanvasPaintFrame,
    options: CanvasWidgetOverlayOptions,
) -> CanvasWidgetOverlayFrame {
    let placements = frame
        .records
        .iter()
        .filter(|record| record_requests_widget_overlay(record, options))
        .map(|record| CanvasWidgetOverlayPlacement {
            target: record.target.clone(),
            document_bounds: record.document_bounds,
            view_bounds: record.view_bounds,
            z_index: record.z_index,
            hit_priority: options.hit_priority,
        })
        .collect();

    CanvasWidgetOverlayFrame { placements }
}

pub fn prepaint_canvas_frame(
    model: &CanvasPaintModel,
    canvas_bounds: Bounds<Pixels>,
    options: CanvasPaintOptions,
    theme: CanvasPaintTheme,
    window: &mut Window,
) -> CanvasPreparedPaintFrame {
    let frame = collect_visible_records(model, canvas_bounds, options);
    prepare_canvas_frame(frame, theme, window)
}

pub fn prepare_canvas_frame(
    frame: CanvasPaintFrame,
    theme: CanvasPaintTheme,
    window: &mut Window,
) -> CanvasPreparedPaintFrame {
    let mut labels = Vec::new();
    let label_indices = frame
        .records
        .iter()
        .map(|record| {
            record.label.as_ref().and_then(|label| {
                let prepared = prepare_label(label, theme, window)?;
                let index = labels.len();
                labels.push(prepared);
                Some(index)
            })
        })
        .collect();

    CanvasPreparedPaintFrame {
        frame,
        labels,
        label_indices,
    }
}

fn interaction_frame(
    model: &CanvasPaintModel,
    selection_scope: Option<&crate::CanvasResolvedSelectionScope>,
) -> CanvasPaintInteractionFrame {
    match model.interaction.state() {
        CanvasPaintInteractionState::Selecting { origin, current } => CanvasPaintInteractionFrame {
            selection_bounds: Some(
                model
                    .viewport
                    .document_bounds_to_view(bounds_from_points(*origin, *current)),
            ),
            structural_selection_bounds: None,
            connection_preview: None,
            reconnect_handles: Vec::new(),
            transform_handles: Vec::new(),
            snap_guides: Vec::new(),
        },
        CanvasPaintInteractionState::Connecting { source, current } => {
            CanvasPaintInteractionFrame {
                selection_bounds: None,
                structural_selection_bounds: structural_selection_bounds(model, selection_scope),
                connection_preview: connection_preview(model, source, *current),
                reconnect_handles: Vec::new(),
                transform_handles: Vec::new(),
                snap_guides: Vec::new(),
            }
        }
        CanvasPaintInteractionState::Reconnecting {
            endpoint,
            fixed,
            current,
        } => CanvasPaintInteractionFrame {
            selection_bounds: None,
            structural_selection_bounds: structural_selection_bounds(model, selection_scope),
            connection_preview: reconnect_preview(model, *endpoint, fixed, *current),
            reconnect_handles: Vec::new(),
            transform_handles: Vec::new(),
            snap_guides: Vec::new(),
        },
        CanvasPaintInteractionState::Transforming { snap_guides } => CanvasPaintInteractionFrame {
            selection_bounds: None,
            structural_selection_bounds: structural_selection_bounds(model, selection_scope),
            connection_preview: None,
            reconnect_handles: reconnect_handles_for_model(model),
            transform_handles: transform_handles_for_model(model),
            snap_guides: paint_snap_guides(model, snap_guides),
        },
        CanvasPaintInteractionState::Idle => CanvasPaintInteractionFrame {
            selection_bounds: None,
            structural_selection_bounds: structural_selection_bounds(model, selection_scope),
            connection_preview: None,
            reconnect_handles: reconnect_handles_for_model(model),
            transform_handles: transform_handles_for_model(model),
            snap_guides: Vec::new(),
        },
    }
}

fn structural_selection_bounds(
    model: &CanvasPaintModel,
    selection_scope: Option<&crate::CanvasResolvedSelectionScope>,
) -> Option<Bounds<Pixels>> {
    let selection_scope = selection_scope?;
    let facts = CanvasGeometryFacts::with_kind_registry(
        model.document.as_ref(),
        model.kind_registry.as_ref(),
    );
    let structural_bounds =
        facts.node_shape_bounds_for_records(selection_scope.paint_structural_records())?;
    if Some(structural_bounds) == facts.selected_bounds(model.interaction.selection()) {
        return None;
    }

    Some(model.viewport.document_bounds_to_view(structural_bounds))
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
        model.interaction.selection(),
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

fn reconnect_handles_for_model(model: &CanvasPaintModel) -> Vec<CanvasPaintReconnectHandle> {
    let facts = CanvasGeometryFacts::with_kind_registry(
        model.document.as_ref(),
        model.kind_registry.as_ref(),
    );
    model
        .interaction
        .selection()
        .selected_edges()
        .filter_map(|edge_id| {
            let edge = model.document.edge(edge_id)?;
            let source = facts.endpoint_position(&edge.source).ok()?;
            let target = facts.endpoint_position(&edge.target).ok()?;
            Some([
                CanvasPaintReconnectHandle {
                    edge_id: edge_id.clone(),
                    endpoint: CanvasPaintReconnectEndpoint::Source,
                    view_bounds: Bounds::centered_at(
                        model.viewport.document_to_view(source),
                        size(px(14.0), px(14.0)),
                    ),
                },
                CanvasPaintReconnectHandle {
                    edge_id: edge_id.clone(),
                    endpoint: CanvasPaintReconnectEndpoint::Target,
                    view_bounds: Bounds::centered_at(
                        model.viewport.document_to_view(target),
                        size(px(14.0), px(14.0)),
                    ),
                },
            ])
        })
        .flatten()
        .collect()
}

fn connection_preview(
    model: &CanvasPaintModel,
    source: &CanvasEndpoint,
    current: Point<Pixels>,
) -> Option<CanvasPaintConnectionPreview> {
    let facts = CanvasGeometryFacts::with_kind_registry(
        model.document.as_ref(),
        model.kind_registry.as_ref(),
    );
    let source = facts.endpoint_position(source).ok()?;
    let target = connection_preview_endpoint_position(
        model,
        CanvasConnectionEndpointRole::Target,
        source,
        current,
    )
    .unwrap_or(current);
    Some(CanvasPaintConnectionPreview {
        source_view_position: model.viewport.document_to_view(source),
        target_view_position: model.viewport.document_to_view(target),
    })
}

fn reconnect_preview(
    model: &CanvasPaintModel,
    endpoint: CanvasConnectionEndpointRole,
    fixed: &CanvasEndpoint,
    current: Point<Pixels>,
) -> Option<CanvasPaintConnectionPreview> {
    let facts = CanvasGeometryFacts::with_kind_registry(
        model.document.as_ref(),
        model.kind_registry.as_ref(),
    );
    let fixed = facts.endpoint_position(fixed).ok()?;
    let moving =
        connection_preview_endpoint_position(model, endpoint, fixed, current).unwrap_or(current);
    Some(match endpoint {
        CanvasConnectionEndpointRole::Source => CanvasPaintConnectionPreview {
            source_view_position: model.viewport.document_to_view(moving),
            target_view_position: model.viewport.document_to_view(fixed),
        },
        CanvasConnectionEndpointRole::Target => CanvasPaintConnectionPreview {
            source_view_position: model.viewport.document_to_view(fixed),
            target_view_position: model.viewport.document_to_view(moving),
        },
    })
}

fn connection_preview_endpoint_position(
    model: &CanvasPaintModel,
    role: CanvasConnectionEndpointRole,
    fixed: Point<Pixels>,
    current: Point<Pixels>,
) -> Option<Point<Pixels>> {
    let facts = CanvasGeometryFacts::with_kind_registry(
        model.document.as_ref(),
        model.kind_registry.as_ref(),
    );
    let endpoint = facts.connection_endpoint_at(
        model
            .runtime
            .precise_hit_test_with_facts(facts, current, connection_hit_options()),
        role,
    )?;
    let position = facts.endpoint_position(&endpoint).ok()?;
    (position != fixed).then_some(position)
}

fn target_is_selected(target: &HitTarget, selection: &CanvasSelection) -> bool {
    selection.contains_target(target)
}

fn target_is_structurally_selected(
    target: &HitTarget,
    scope: Option<&CanvasResolvedSelectionScope>,
) -> bool {
    let Some(scope) = scope else {
        return false;
    };
    let Some(record_id) = record_id_for_target(target) else {
        return false;
    };

    scope.contains_paint_structural_record(&record_id)
}

fn record_id_for_target(target: &HitTarget) -> Option<CanvasRecordId> {
    match target {
        HitTarget::Node(id) => Some(CanvasRecordId::Node(id.clone())),
        HitTarget::Edge(id) => Some(CanvasRecordId::Edge(id.clone())),
        HitTarget::Shape(id) => Some(CanvasRecordId::Shape(id.clone())),
        HitTarget::Handle { .. } => None,
    }
}

fn record_requests_widget_overlay(
    record: &CanvasPaintRecord,
    options: CanvasWidgetOverlayOptions,
) -> bool {
    if !record.selected
        || (record.hidden && !options.include_hidden)
        || (record.locked && !options.include_locked)
    {
        return false;
    }

    match &record.target {
        HitTarget::Node(_) => options.include_selected_nodes,
        HitTarget::Shape(_) => options.include_selected_shapes,
        HitTarget::Edge(_) | HitTarget::Handle { .. } => false,
    }
}

fn bounds_from_points(a: Point<Pixels>, b: Point<Pixels>) -> Bounds<Pixels> {
    Bounds::from_corners(
        Point::new(a.x.min(b.x), a.y.min(b.y)),
        Point::new(a.x.max(b.x), a.y.max(b.y)),
    )
}

fn prepare_label(
    label: &CanvasPaintLabel,
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

    Some(CanvasPreparedPaintLabel {
        view_bounds: label.view_bounds,
        lines: lines.into_iter().collect(),
        text_height,
    })
}

pub(super) fn label_line_clamp(theme: CanvasPaintTheme, bounds: Bounds<Pixels>) -> Option<usize> {
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

fn paint_edge_geometry(
    model: &CanvasPaintModel,
    target: &HitTarget,
) -> Option<CanvasPaintEdgeGeometry> {
    let HitTarget::Edge(id) = target else {
        return None;
    };
    let geometry = model.runtime.edge_geometry(id)?;
    Some(CanvasPaintEdgeGeometry {
        view_path: route_path_to_view(&geometry.path, model.viewport),
    })
}

fn route_path_to_view(path: &CanvasRoutePath, viewport: CanvasViewport) -> CanvasRoutePath {
    CanvasRoutePath::new(path.segments.iter().map(|segment| match segment {
        CanvasRouteSegment::Line { from, to } => CanvasRouteSegment::Line {
            from: viewport.document_to_view(*from),
            to: viewport.document_to_view(*to),
        },
        CanvasRouteSegment::CubicBezier {
            from,
            control_1,
            control_2,
            to,
        } => CanvasRouteSegment::CubicBezier {
            from: viewport.document_to_view(*from),
            control_1: viewport.document_to_view(*control_1),
            control_2: viewport.document_to_view(*control_2),
            to: viewport.document_to_view(*to),
        },
    }))
}
