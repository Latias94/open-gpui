use super::*;

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
