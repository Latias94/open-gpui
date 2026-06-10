use crate::transform::{CanvasResizeHandle, resize_bounds_by_handle};
use crate::{
    CanvasDefaultEdgeRouter, CanvasDocument, CanvasGeometryFacts, CanvasKindRegistry,
    CanvasRecordGeometry, CanvasRecordId, CanvasSelection,
};
use indexmap::IndexSet;
use open_gpui::{Bounds, Pixels, Point, px};
use serde::{Deserialize, Serialize};

pub const DEFAULT_SNAP_THRESHOLD: Pixels = px(6.0);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CanvasSnapAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasSnapGuide {
    pub axis: CanvasSnapAxis,
    pub document_start: Point<Pixels>,
    pub document_end: Point<Pixels>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CanvasSnapResult {
    pub delta: Point<Pixels>,
    pub guides: Vec<CanvasSnapGuide>,
}

pub fn snap_delta_for_selection(
    document: &CanvasDocument,
    selection: &CanvasSelection,
    delta: Point<Pixels>,
    threshold: Pixels,
    kind_registry: Option<&CanvasKindRegistry>,
) -> CanvasSnapResult {
    let Some(active_bounds) = selection_bounds(document, selection, kind_registry) else {
        return CanvasSnapResult {
            delta,
            guides: Vec::new(),
        };
    };

    let moved_bounds = Bounds::new(active_bounds.origin + delta, active_bounds.size);
    let selected_record_ids = selected_record_ids(selection);
    let mut horizontal = None;
    let mut vertical = None;

    for candidate in candidate_bounds(document, kind_registry) {
        if selected_record_ids.contains(candidate.record_id()) {
            continue;
        }

        horizontal = best_snap(
            horizontal,
            snap_axis(
                moved_bounds,
                candidate.bounds(),
                CanvasSnapAxis::Horizontal,
                threshold,
            ),
        );
        vertical = best_snap(
            vertical,
            snap_axis(
                moved_bounds,
                candidate.bounds(),
                CanvasSnapAxis::Vertical,
                threshold,
            ),
        );
    }

    let mut snapped_delta = delta;
    let mut guides = Vec::new();
    if let Some(snap) = horizontal {
        snapped_delta.x += snap.adjustment;
        guides.push(snap.guide);
    }
    if let Some(snap) = vertical {
        snapped_delta.y += snap.adjustment;
        guides.push(snap.guide);
    }

    CanvasSnapResult {
        delta: snapped_delta,
        guides,
    }
}

pub fn snap_delta_for_resize_selection(
    document: &CanvasDocument,
    selection: &CanvasSelection,
    handle: CanvasResizeHandle,
    delta: Point<Pixels>,
    threshold: Pixels,
    kind_registry: Option<&CanvasKindRegistry>,
) -> CanvasSnapResult {
    let Some(active_bounds) = selection_bounds(document, selection, kind_registry) else {
        return CanvasSnapResult {
            delta,
            guides: Vec::new(),
        };
    };

    let resized_bounds = resize_bounds_by_handle(active_bounds, handle, delta);
    let selected_record_ids = selected_record_ids(selection);
    let mut horizontal = None;
    let mut vertical = None;

    for candidate in candidate_bounds(document, kind_registry) {
        if selected_record_ids.contains(candidate.record_id()) {
            continue;
        }

        horizontal = best_snap(
            horizontal,
            snap_resize_axis(
                resized_bounds,
                candidate.bounds(),
                CanvasSnapAxis::Horizontal,
                handle,
                threshold,
            ),
        );
        vertical = best_snap(
            vertical,
            snap_resize_axis(
                resized_bounds,
                candidate.bounds(),
                CanvasSnapAxis::Vertical,
                handle,
                threshold,
            ),
        );
    }

    let mut snapped_delta = delta;
    let mut guides = Vec::new();
    if let Some(snap) = horizontal {
        snapped_delta.x += snap.adjustment;
        guides.push(snap.guide);
    }
    if let Some(snap) = vertical {
        snapped_delta.y += snap.adjustment;
        guides.push(snap.guide);
    }

    CanvasSnapResult {
        delta: snapped_delta,
        guides,
    }
}

fn selected_record_ids(selection: &CanvasSelection) -> IndexSet<CanvasRecordId> {
    selection
        .selected_nodes()
        .map(|id| CanvasRecordId::Node(id.clone()))
        .chain(
            selection
                .selected_shapes()
                .map(|id| CanvasRecordId::Shape(id.clone())),
        )
        .collect()
}

fn selection_bounds(
    document: &CanvasDocument,
    selection: &CanvasSelection,
    kind_registry: Option<&CanvasKindRegistry>,
) -> Option<Bounds<Pixels>> {
    let facts = CanvasGeometryFacts::with_router_and_kind_registry(
        document,
        CanvasDefaultEdgeRouter,
        kind_registry,
    );
    facts.selected_bounds(selection)
}

#[derive(Clone, Debug)]
struct CandidateBounds(CanvasRecordGeometry);

impl CandidateBounds {
    fn record_id(&self) -> &CanvasRecordId {
        &self.0.id
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.0.bounds
    }
}

fn candidate_bounds(
    document: &CanvasDocument,
    kind_registry: Option<&CanvasKindRegistry>,
) -> Vec<CandidateBounds> {
    let facts = CanvasGeometryFacts::with_router_and_kind_registry(
        document,
        CanvasDefaultEdgeRouter,
        kind_registry,
    );
    facts
        .record_geometries()
        .into_iter()
        .filter(CanvasRecordGeometry::is_node_or_shape)
        .filter(CanvasRecordGeometry::is_visible_unlocked)
        .map(CandidateBounds)
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
struct SnapCandidate {
    distance: Pixels,
    adjustment: Pixels,
    guide: CanvasSnapGuide,
}

fn best_snap(current: Option<SnapCandidate>, next: Option<SnapCandidate>) -> Option<SnapCandidate> {
    match (current, next) {
        (None, None) => None,
        (Some(current), None) => Some(current),
        (None, Some(next)) => Some(next),
        (Some(current), Some(next)) => {
            if next.distance < current.distance {
                Some(next)
            } else {
                Some(current)
            }
        }
    }
}

fn snap_axis(
    active: Bounds<Pixels>,
    candidate: Bounds<Pixels>,
    axis: CanvasSnapAxis,
    threshold: Pixels,
) -> Option<SnapCandidate> {
    let mut best = None;
    for active_anchor in anchors(active, axis) {
        for candidate_anchor in anchors(candidate, axis) {
            let adjustment = candidate_anchor.value - active_anchor.value;
            let distance = adjustment.abs();
            if distance <= threshold {
                best = best_snap(
                    best,
                    Some(SnapCandidate {
                        distance,
                        adjustment,
                        guide: snap_guide(axis, active, candidate, candidate_anchor.value),
                    }),
                );
            }
        }
    }
    best
}

fn snap_resize_axis(
    active: Bounds<Pixels>,
    candidate: Bounds<Pixels>,
    axis: CanvasSnapAxis,
    handle: CanvasResizeHandle,
    threshold: Pixels,
) -> Option<SnapCandidate> {
    let active_anchor = resize_anchor(active, axis, handle);
    let mut best = None;
    for candidate_anchor in anchors(candidate, axis) {
        let adjustment = candidate_anchor.value - active_anchor.value;
        let distance = adjustment.abs();
        if distance <= threshold {
            best = best_snap(
                best,
                Some(SnapCandidate {
                    distance,
                    adjustment,
                    guide: snap_guide(axis, active, candidate, candidate_anchor.value),
                }),
            );
        }
    }
    best
}

#[derive(Clone, Copy, Debug)]
struct AnchorValue {
    value: Pixels,
}

fn anchors(bounds: Bounds<Pixels>, axis: CanvasSnapAxis) -> [AnchorValue; 3] {
    match axis {
        CanvasSnapAxis::Horizontal => {
            let left = bounds.origin.x;
            let right = bounds.origin.x + bounds.size.width;
            [
                AnchorValue { value: left },
                AnchorValue {
                    value: left + bounds.size.width * 0.5,
                },
                AnchorValue { value: right },
            ]
        }
        CanvasSnapAxis::Vertical => {
            let top = bounds.origin.y;
            let bottom = bounds.origin.y + bounds.size.height;
            [
                AnchorValue { value: top },
                AnchorValue {
                    value: top + bounds.size.height * 0.5,
                },
                AnchorValue { value: bottom },
            ]
        }
    }
}

fn resize_anchor(
    bounds: Bounds<Pixels>,
    axis: CanvasSnapAxis,
    handle: CanvasResizeHandle,
) -> AnchorValue {
    match (axis, handle) {
        (
            CanvasSnapAxis::Horizontal,
            CanvasResizeHandle::TopLeft | CanvasResizeHandle::BottomLeft,
        ) => AnchorValue {
            value: bounds.origin.x,
        },
        (
            CanvasSnapAxis::Horizontal,
            CanvasResizeHandle::TopRight | CanvasResizeHandle::BottomRight,
        ) => AnchorValue {
            value: bounds.origin.x + bounds.size.width,
        },
        (CanvasSnapAxis::Vertical, CanvasResizeHandle::TopLeft | CanvasResizeHandle::TopRight) => {
            AnchorValue {
                value: bounds.origin.y,
            }
        }
        (
            CanvasSnapAxis::Vertical,
            CanvasResizeHandle::BottomLeft | CanvasResizeHandle::BottomRight,
        ) => AnchorValue {
            value: bounds.origin.y + bounds.size.height,
        },
    }
}

fn snap_guide(
    axis: CanvasSnapAxis,
    active: Bounds<Pixels>,
    candidate: Bounds<Pixels>,
    value: Pixels,
) -> CanvasSnapGuide {
    match axis {
        CanvasSnapAxis::Horizontal => {
            let top = active.origin.y.min(candidate.origin.y);
            let bottom = (active.origin.y + active.size.height)
                .max(candidate.origin.y + candidate.size.height);
            CanvasSnapGuide {
                axis,
                document_start: Point::new(value, top),
                document_end: Point::new(value, bottom),
            }
        }
        CanvasSnapAxis::Vertical => {
            let left = active.origin.x.min(candidate.origin.x);
            let right = (active.origin.x + active.size.width)
                .max(candidate.origin.x + candidate.size.width);
            CanvasSnapGuide {
                axis,
                document_start: Point::new(left, value),
                document_end: Point::new(right, value),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CanvasNode, CanvasNodeGeometryPolicy, CanvasNodeKind, CanvasShape, NodeId, ShapeId,
    };
    use open_gpui::{point, size};

    #[test]
    fn snaps_selected_bounds_to_nearby_left_edge() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "active",
                point(px(0.0), px(0.0)),
                size(px(40.0), px(40.0)),
            ))
            .unwrap();
        document
            .insert_node(CanvasNode::new(
                "target",
                point(px(100.0), px(0.0)),
                size(px(40.0), px(40.0)),
            ))
            .unwrap();
        let mut selection = CanvasSelection::default();
        selection.insert_node(NodeId::from("active"));

        let result = snap_delta_for_selection(
            &document,
            &selection,
            point(px(96.0), px(0.0)),
            DEFAULT_SNAP_THRESHOLD,
            None,
        );

        assert_eq!(result.delta, point(px(100.0), px(0.0)));
        assert!(
            result
                .guides
                .iter()
                .any(|guide| guide.axis == CanvasSnapAxis::Horizontal)
        );
    }

    #[test]
    fn ignores_selected_and_locked_records() {
        let mut active =
            CanvasNode::new("active", point(px(0.0), px(0.0)), size(px(40.0), px(40.0)));
        active.locked = false;
        let mut locked = CanvasNode::new(
            "locked",
            point(px(100.0), px(0.0)),
            size(px(40.0), px(40.0)),
        );
        locked.locked = true;
        let mut document = CanvasDocument::default();
        document.insert_node(active).unwrap();
        document.insert_node(locked).unwrap();
        let mut selection = CanvasSelection::default();
        selection.insert_node(NodeId::from("active"));

        let result = snap_delta_for_selection(
            &document,
            &selection,
            point(px(96.0), px(0.0)),
            DEFAULT_SNAP_THRESHOLD,
            None,
        );

        assert_eq!(result.delta, point(px(96.0), px(0.0)));
        assert!(result.guides.is_empty());
    }

    #[test]
    fn kind_registry_bounds_are_used_for_selection_and_candidates() {
        let mut active =
            CanvasNode::new("active", point(px(0.0), px(0.0)), size(px(40.0), px(40.0)));
        active.kind = "wide".to_string();
        let target = CanvasNode::new(
            "target",
            point(px(100.0), px(80.0)),
            size(px(40.0), px(40.0)),
        );
        let mut document = CanvasDocument::default();
        document.insert_node(active).unwrap();
        document.insert_node(target).unwrap();
        let mut selection = CanvasSelection::default();
        selection.insert_node(NodeId::from("active"));
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind(
            "wide",
            CanvasNodeKind::new().with_geometry_policy(WideNodeKind),
        );

        let without_registry = snap_delta_for_selection(
            &document,
            &selection,
            point(px(9.0), px(0.0)),
            DEFAULT_SNAP_THRESHOLD,
            None,
        );
        let with_registry = snap_delta_for_selection(
            &document,
            &selection,
            point(px(9.0), px(0.0)),
            DEFAULT_SNAP_THRESHOLD,
            Some(&registry),
        );

        assert_eq!(without_registry.delta, point(px(9.0), px(0.0)));
        assert!(without_registry.guides.is_empty());
        assert_eq!(with_registry.delta, point(px(10.0), px(0.0)));
        assert!(!with_registry.guides.is_empty());
    }

    #[test]
    fn resize_snaps_only_the_dragged_edges() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "active",
                point(px(100.0), px(100.0)),
                size(px(40.0), px(40.0)),
            ))
            .unwrap();
        document
            .insert_shape(CanvasShape::new(
                "target",
                Bounds::new(point(px(50.0), px(60.0)), size(px(32.0), px(24.0))),
            ))
            .unwrap();
        let mut selection = CanvasSelection::default();
        selection.insert_node(NodeId::from("active"));
        selection.insert_shape(ShapeId::from("target"));

        let result = snap_delta_for_resize_selection(
            &document,
            &selection,
            CanvasResizeHandle::TopLeft,
            point(px(-46.0), px(-35.0)),
            DEFAULT_SNAP_THRESHOLD,
            None,
        );

        assert_eq!(result.delta, point(px(-46.0), px(-35.0)));
        assert!(result.guides.is_empty());

        selection.clear_shapes();
        let result = snap_delta_for_resize_selection(
            &document,
            &selection,
            CanvasResizeHandle::TopLeft,
            point(px(-46.0), px(-35.0)),
            DEFAULT_SNAP_THRESHOLD,
            None,
        );

        assert_eq!(result.delta, point(px(-50.0), px(-40.0)));
        assert!(
            result
                .guides
                .iter()
                .any(|guide| guide.axis == CanvasSnapAxis::Horizontal)
        );
        assert!(
            result
                .guides
                .iter()
                .any(|guide| guide.axis == CanvasSnapAxis::Vertical)
        );
    }

    struct WideNodeKind;

    impl CanvasNodeGeometryPolicy for WideNodeKind {
        fn node_bounds(&self, node: &CanvasNode) -> Option<Bounds<Pixels>> {
            Some(Bounds::new(
                node.position,
                size(node.size.width + px(50.0), node.size.height),
            ))
        }
    }
}
