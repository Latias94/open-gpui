use super::model::{
    CanvasPaintInteractionState, CanvasPaintModel, CanvasPaintOptions, CanvasPaintTheme,
};
use super::style::{parse_color, positive_pixels};
use crate::geometry_facts::{CanvasGeometryFacts, connection_hit_options};
use crate::record_scope::{CanvasRecordScopeOptions, resolve_selection_scope};
use crate::routing::{
    CanvasDefaultEdgeRouter, CanvasEdgeRouter, CanvasRoutePath, CanvasRouteRequest,
    CanvasRouteSegment,
};
use crate::tool::RECONNECT_HANDLE_VIEW_SIZE;
use crate::{
    CanvasConnectionEndpointRole, CanvasEdge, CanvasEdgeRoute, CanvasEdgeRouteKind, CanvasEndpoint,
    CanvasKindLabel, CanvasRecordId, CanvasSelection, CanvasSnapAxis, CanvasSnapGuide,
    CanvasTransformHandle, CanvasTransformTarget, CanvasViewport, EdgeId, HitOptions, HitTarget,
    canvas_transform_handles,
};
use open_gpui::{
    Bounds, Hsla, Pixels, Point, SharedString, TextRun, Window, WrappedLine, px, size,
};

mod feedback;
mod labels;
mod overlay;
mod scene;

pub use feedback::*;
pub use overlay::*;
pub use scene::*;

#[cfg(test)]
pub(super) use labels::label_line_clamp;
use labels::{paint_record_label, prepare_label};

const CONNECTION_TARGET_FEEDBACK_VIEW_SIZE: Pixels = px(18.0);
const RECONNECT_HANDLE_VISUAL_SIZE: Pixels = px(11.0);

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

    pub fn scene_frame(&self) -> CanvasSceneFrame {
        CanvasSceneFrame::from_paint_frame(self)
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
    pub hovered: bool,
    pub selected: bool,
    pub structurally_selected: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintEdgeGeometry {
    pub view_path: CanvasRoutePath,
    pub visual_state: CanvasPaintWireVisualState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasPaintWireVisualState {
    Committed,
    Hovered,
    Selected,
    SelectedHovered,
    PreviewFree,
    PreviewValidTarget,
    PreviewInvalidTarget,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPaintLabel {
    pub text: String,
    pub document_bounds: Bounds<Pixels>,
    pub view_bounds: Bounds<Pixels>,
    pub color: Option<Hsla>,
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
            let selected = options.include_interaction_feedback
                && selection_scope.as_ref().is_some_and(|scope| {
                    target_is_selected(&record.target, scope.normalized_selection())
                });
            let structurally_selected =
                target_is_structurally_selected(&record.target, selection_scope.as_ref());
            let hovered = options.include_interaction_feedback
                && model
                    .interaction
                    .hovered_target()
                    .is_some_and(|hovered| hovered == &target);
            let edge_geometry = paint_edge_geometry(
                model,
                &target,
                committed_wire_visual_state(selected || structurally_selected, hovered),
            );
            CanvasPaintRecord {
                label: paint_record_label(model, &target, record.bounds),
                target,
                document_bounds: record.bounds,
                view_bounds: model.viewport.document_bounds_to_view(record.bounds),
                edge_geometry,
                z_index: record.z_index,
                hidden: record.hidden,
                locked: record.locked,
                hovered,
                selected,
                structurally_selected,
            }
        })
        .collect();

    CanvasPaintFrame {
        visible_document_bounds,
        records,
        interaction: if options.include_interaction_feedback {
            interaction_frame(model, options, selection_scope.as_ref())
        } else {
            CanvasPaintInteractionFrame::default()
        },
    }
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

fn paint_edge_geometry(
    model: &CanvasPaintModel,
    target: &HitTarget,
    visual_state: CanvasPaintWireVisualState,
) -> Option<CanvasPaintEdgeGeometry> {
    let HitTarget::Edge(id) = target else {
        return None;
    };
    let geometry = model.runtime.edge_geometry(id)?;
    Some(CanvasPaintEdgeGeometry {
        view_path: route_path_to_view(&geometry.path, model.viewport),
        visual_state,
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
