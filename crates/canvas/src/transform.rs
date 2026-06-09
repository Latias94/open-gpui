use crate::{
    CanvasDefaultEdgeRouter, CanvasDocument, CanvasGeometryFacts, CanvasKindRegistry,
    CanvasRecordGeometry, CanvasRecordId, CanvasSelection, NodeId, ShapeId,
};
use open_gpui::{Bounds, Pixels, Point, Size, px};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CanvasResizeHandle {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CanvasTransformTarget {
    Node(NodeId),
    Shape(ShapeId),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasTransformHandle {
    pub target: CanvasTransformTarget,
    pub handle: CanvasResizeHandle,
    pub document_bounds: Bounds<Pixels>,
}

pub fn canvas_transform_handles(
    document: &CanvasDocument,
    selection: &CanvasSelection,
    viewport: crate::CanvasViewport,
    kind_registry: Option<&CanvasKindRegistry>,
) -> Vec<CanvasTransformHandle> {
    let facts = CanvasGeometryFacts::with_router_and_kind_registry(
        document,
        CanvasDefaultEdgeRouter,
        kind_registry,
    );
    let handle_size = transform_handle_document_size(viewport.zoom);
    let mut handles = Vec::new();

    for geometry in facts.selected_record_geometries(selection) {
        if !geometry.is_visible_unlocked() {
            continue;
        }
        let Some(target) = transform_target_for_record(&geometry) else {
            continue;
        };
        handles.extend(transform_handles_for_bounds(
            target,
            geometry.bounds,
            handle_size,
        ));
    }

    handles
}

fn transform_target_for_record(geometry: &CanvasRecordGeometry) -> Option<CanvasTransformTarget> {
    match &geometry.id {
        CanvasRecordId::Node(id) => Some(CanvasTransformTarget::Node(id.clone())),
        CanvasRecordId::Shape(id) => Some(CanvasTransformTarget::Shape(id.clone())),
        CanvasRecordId::Edge(_) => None,
    }
}

pub(crate) fn resize_bounds_by_handle(
    bounds: Bounds<Pixels>,
    handle: CanvasResizeHandle,
    delta: Point<Pixels>,
) -> Bounds<Pixels> {
    let min_size = Size::new(px(8.0), px(8.0));
    let left = bounds.origin.x;
    let top = bounds.origin.y;
    let right = bounds.origin.x + bounds.size.width;
    let bottom = bounds.origin.y + bounds.size.height;

    let (new_left, new_top, new_right, new_bottom) = match handle {
        CanvasResizeHandle::TopLeft => (
            (left + delta.x).min(right - min_size.width),
            (top + delta.y).min(bottom - min_size.height),
            right,
            bottom,
        ),
        CanvasResizeHandle::TopRight => (
            left,
            (top + delta.y).min(bottom - min_size.height),
            (right + delta.x).max(left + min_size.width),
            bottom,
        ),
        CanvasResizeHandle::BottomLeft => (
            (left + delta.x).min(right - min_size.width),
            top,
            right,
            (bottom + delta.y).max(top + min_size.height),
        ),
        CanvasResizeHandle::BottomRight => (
            left,
            top,
            (right + delta.x).max(left + min_size.width),
            (bottom + delta.y).max(top + min_size.height),
        ),
    };

    Bounds::from_corners(
        Point::new(new_left, new_top),
        Point::new(new_right, new_bottom),
    )
}

fn transform_handles_for_bounds(
    target: CanvasTransformTarget,
    bounds: Bounds<Pixels>,
    handle_size: Size<Pixels>,
) -> Vec<CanvasTransformHandle> {
    resize_handle_centers(bounds)
        .into_iter()
        .map(|(handle, center)| CanvasTransformHandle {
            target: target.clone(),
            handle,
            document_bounds: Bounds::centered_at(center, handle_size),
        })
        .collect()
}

fn resize_handle_centers(bounds: Bounds<Pixels>) -> [(CanvasResizeHandle, Point<Pixels>); 4] {
    let left = bounds.origin.x;
    let top = bounds.origin.y;
    let right = bounds.origin.x + bounds.size.width;
    let bottom = bounds.origin.y + bounds.size.height;
    [
        (CanvasResizeHandle::TopLeft, Point::new(left, top)),
        (CanvasResizeHandle::TopRight, Point::new(right, top)),
        (CanvasResizeHandle::BottomLeft, Point::new(left, bottom)),
        (CanvasResizeHandle::BottomRight, Point::new(right, bottom)),
    ]
}

fn transform_handle_document_size(zoom: f32) -> Size<Pixels> {
    let scale = if zoom.is_finite() && zoom > 0.0 {
        1.0 / zoom
    } else {
        1.0
    };
    let edge = px(8.0) * scale;
    Size::new(edge, edge)
}
