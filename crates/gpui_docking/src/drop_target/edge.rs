use crate::{
    DockEdgeDockSizing, DockNodeId, DropZone, SplitAxis,
    geometry::{self, DockDropBox, DockDropGeometry, DockDropGuideMetrics},
};
use open_gpui::{Bounds, Pixels, Point, Size, px, size};

use super::candidate::bounds_area;
use super::{
    DockDropResolveSource, DockDropResolverInput, DockEdgePlanResolver, DockLeafDropTarget,
    DockResolvedDropTarget, DockResolvedDropTargetAvailability, DockResolvedDropTargetKind,
    DockRootDropTarget,
};

pub(super) struct DockEdgeDropMetadata {
    pub(super) drop_box: DockDropBox,
    pub(super) preview_bounds: Bounds<Pixels>,
    pub(super) edge_sizing: DockEdgeDockSizing,
}

pub(super) fn resolve_root_edge_drop(
    input: &DockDropResolverInput<'_>,
    leaf: Option<&DockLeafDropTarget>,
) -> Option<DockResolvedDropTarget> {
    let root = input.root?;
    let leaf_tabs = match leaf {
        Some(leaf) if leaf.root == root.root => Some(leaf.target_tabs),
        Some(_) => return None,
        None => None,
    };
    let is_central_region =
        leaf.is_some_and(|leaf| leaf.is_central && leaf.root == leaf.target_tabs);

    let geometry = geometry::resolve_outer_drop_geometry_with_style(
        root.bounds,
        input.position,
        input.drop_guide_metrics,
    )?;

    let metadata = edge_drop_metadata(root.bounds, geometry.drop_box, input.payload_size);
    let edge_plan = input
        .edge_plan_resolver
        .and_then(|resolver| resolver(root.root, geometry.zone(), metadata.edge_sizing));
    let inner_target_bounds = leaf.map(|leaf| leaf.bounds);

    Some(DockResolvedDropTarget {
        kind: DockResolvedDropTargetKind::RootEdge {
            root: root.root,
            leaf_tabs,
            zone: geometry.zone(),
        },
        source: DockDropResolveSource::RootEdge,
        target_bounds: Some(root.bounds),
        inner_target_bounds,
        availability: DockResolvedDropTargetAvailability {
            center: false,
            sides: true,
        },
        drop_box: Some(metadata.drop_box),
        hit_bounds: Some(metadata.drop_box.hit_bounds),
        preview_bounds: Some(metadata.preview_bounds),
        tab_insertion_bounds: None,
        edge_sizing: Some(metadata.edge_sizing),
        edge_plan,
        is_central_region,
    })
}

pub(super) fn root_guide_target(
    root: DockRootDropTarget,
    leaf: Option<&DockLeafDropTarget>,
) -> DockResolvedDropTarget {
    DockResolvedDropTarget {
        kind: DockResolvedDropTargetKind::RootEdge {
            root: root.root,
            leaf_tabs: leaf.map(|leaf| leaf.target_tabs),
            zone: DropZone::Left,
        },
        source: DockDropResolveSource::RootEdge,
        target_bounds: Some(root.bounds),
        inner_target_bounds: leaf.map(|leaf| leaf.bounds),
        availability: DockResolvedDropTargetAvailability {
            center: false,
            sides: true,
        },
        drop_box: None,
        hit_bounds: Some(root.bounds),
        preview_bounds: Some(root.bounds),
        tab_insertion_bounds: None,
        edge_sizing: None,
        edge_plan: None,
        is_central_region: leaf
            .is_some_and(|leaf| leaf.is_central && leaf.root == leaf.target_tabs),
    }
}

pub(super) fn resolve_leaf_drop(
    leaf: &DockLeafDropTarget,
    position: Point<Pixels>,
    payload_size: Option<Size<Pixels>>,
    drop_guide_metrics: DockDropGuideMetrics,
    edge_plan_resolver: Option<&DockEdgePlanResolver<'_>>,
) -> Option<DockResolvedDropTarget> {
    let geometry = geometry::resolve_inner_drop_geometry_with_style(
        leaf.bounds,
        position,
        drop_guide_metrics,
    )?;
    target_from_leaf_geometry(leaf, geometry, payload_size, edge_plan_resolver)
}

pub(super) fn leaf_guide_target(leaf: &DockLeafDropTarget) -> DockResolvedDropTarget {
    DockResolvedDropTarget {
        kind: DockResolvedDropTargetKind::LeafCenter {
            root: leaf.root,
            target_tabs: leaf.target_tabs,
        },
        source: DockDropResolveSource::LeafBody,
        target_bounds: Some(leaf.bounds),
        inner_target_bounds: None,
        availability: DockResolvedDropTargetAvailability::all(),
        drop_box: None,
        hit_bounds: Some(leaf.bounds),
        preview_bounds: Some(leaf.bounds),
        tab_insertion_bounds: None,
        edge_sizing: None,
        edge_plan: None,
        is_central_region: leaf.is_central,
    }
}

fn target_from_leaf_geometry(
    leaf: &DockLeafDropTarget,
    geometry: DockDropGeometry,
    payload_size: Option<Size<Pixels>>,
    edge_plan_resolver: Option<&DockEdgePlanResolver<'_>>,
) -> Option<DockResolvedDropTarget> {
    let kind = if geometry.drop_box.kind.is_center() {
        DockResolvedDropTargetKind::LeafCenter {
            root: leaf.root,
            target_tabs: leaf.target_tabs,
        }
    } else {
        DockResolvedDropTargetKind::InnerEdge {
            root: leaf.root,
            target_tabs: leaf.target_tabs,
            zone: geometry.zone(),
        }
    };
    let source = if geometry.drop_box.kind.is_center() {
        DockDropResolveSource::LeafBody
    } else {
        DockDropResolveSource::InnerEdge
    };

    if geometry.drop_box.kind.is_center() {
        return Some(DockResolvedDropTarget {
            kind,
            source,
            target_bounds: Some(leaf.bounds),
            inner_target_bounds: None,
            availability: DockResolvedDropTargetAvailability::all(),
            drop_box: Some(geometry.drop_box),
            hit_bounds: Some(geometry.drop_box.hit_bounds),
            preview_bounds: Some(geometry.preview_bounds()),
            tab_insertion_bounds: None,
            edge_sizing: None,
            edge_plan: None,
            is_central_region: leaf.is_central,
        });
    }

    // Match ImGui: only the root central leaf suppresses inner side splits in favor of
    // host/root outer guides. Nested central leaves still allow inner side docking.
    if leaf.is_central && leaf.root == leaf.target_tabs {
        return None;
    }

    let metadata = edge_drop_metadata(leaf.bounds, geometry.drop_box, payload_size);
    let edge_plan = edge_plan_resolver
        .and_then(|resolver| resolver(leaf.target_tabs, geometry.zone(), metadata.edge_sizing));

    Some(DockResolvedDropTarget {
        kind,
        source,
        target_bounds: Some(leaf.bounds),
        inner_target_bounds: None,
        availability: DockResolvedDropTargetAvailability::all(),
        drop_box: Some(metadata.drop_box),
        hit_bounds: Some(metadata.drop_box.hit_bounds),
        preview_bounds: Some(metadata.preview_bounds),
        tab_insertion_bounds: None,
        edge_sizing: Some(metadata.edge_sizing),
        edge_plan,
        is_central_region: leaf.is_central,
    })
}

fn edge_drop_metadata(
    target_bounds: Bounds<Pixels>,
    mut drop_box: DockDropBox,
    payload_size: Option<Size<Pixels>>,
) -> DockEdgeDropMetadata {
    let zone = drop_box.kind.zone();
    let preview_bounds = edge_preview_bounds(zone, target_bounds, payload_size);
    drop_box.preview_bounds = preview_bounds;
    let axis = zone_axis(zone);
    let sizing = DockEdgeDockSizing::from_extents(
        split_extent(axis, preview_bounds),
        split_extent(axis, target_bounds),
    );
    DockEdgeDropMetadata {
        drop_box,
        preview_bounds,
        edge_sizing: sizing,
    }
}

fn edge_preview_bounds(
    zone: DropZone,
    target_bounds: Bounds<Pixels>,
    payload_size: Option<Size<Pixels>>,
) -> Bounds<Pixels> {
    let axis = zone_axis(zone);
    let target_extent = split_extent(axis, target_bounds);
    let desired_extent = payload_size
        .map(|size| split_size_extent(axis, size))
        .filter(|extent| {
            let extent = f32::from(*extent);
            extent.is_finite() && extent > 0.0 && extent <= f32::from(target_extent) * 0.5
        })
        .unwrap_or_else(|| px(f32::from(target_extent) * 0.5));

    match zone {
        DropZone::Left => Bounds::new(
            target_bounds.origin,
            size(desired_extent, target_bounds.size.height),
        ),
        DropZone::Right => Bounds::new(
            Point::new(
                target_bounds.origin.x + target_bounds.size.width - desired_extent,
                target_bounds.origin.y,
            ),
            size(desired_extent, target_bounds.size.height),
        ),
        DropZone::Top => Bounds::new(
            target_bounds.origin,
            size(target_bounds.size.width, desired_extent),
        ),
        DropZone::Bottom => Bounds::new(
            Point::new(
                target_bounds.origin.x,
                target_bounds.origin.y + target_bounds.size.height - desired_extent,
            ),
            size(target_bounds.size.width, desired_extent),
        ),
        DropZone::Center => target_bounds,
    }
}

fn zone_axis(zone: DropZone) -> SplitAxis {
    match zone {
        DropZone::Left | DropZone::Right => SplitAxis::Horizontal,
        DropZone::Top | DropZone::Bottom => SplitAxis::Vertical,
        DropZone::Center => SplitAxis::Horizontal,
    }
}

fn split_extent(axis: SplitAxis, bounds: Bounds<Pixels>) -> Pixels {
    match axis {
        SplitAxis::Horizontal => bounds.size.width,
        SplitAxis::Vertical => bounds.size.height,
    }
}

fn split_size_extent(axis: SplitAxis, size: Size<Pixels>) -> Pixels {
    match axis {
        SplitAxis::Horizontal => size.width,
        SplitAxis::Vertical => size.height,
    }
}

pub(super) fn best_leaf_for_root_containing(
    leaves: &[DockLeafDropTarget],
    position: Point<Pixels>,
    root: DockNodeId,
) -> Option<&DockLeafDropTarget> {
    let mut best: Option<(&DockLeafDropTarget, f32, usize)> = None;
    for (index, leaf) in leaves.iter().enumerate() {
        if leaf.root != root || !leaf.bounds.contains(&position) {
            continue;
        }
        let area = bounds_area(leaf.bounds);
        let is_better = match best {
            None => true,
            Some((_, best_area, best_index)) => {
                area < best_area || (area == best_area && index > best_index)
            }
        };
        if is_better {
            best = Some((leaf, area, index));
        }
    }
    best.map(|(leaf, _, _)| leaf)
}

pub(super) fn best_leaf_containing(
    leaves: &[DockLeafDropTarget],
    position: Point<Pixels>,
) -> Option<&DockLeafDropTarget> {
    let mut best: Option<(&DockLeafDropTarget, f32, usize)> = None;
    for (index, leaf) in leaves.iter().enumerate() {
        if !leaf.bounds.contains(&position) {
            continue;
        }
        let area = bounds_area(leaf.bounds);
        let is_better = match best {
            None => true,
            Some((_, best_area, best_index)) => {
                area < best_area || (area == best_area && index > best_index)
            }
        };
        if is_better {
            best = Some((leaf, area, index));
        }
    }
    best.map(|(leaf, _, _)| leaf)
}

pub(super) fn leaf_bounds_for_tabs(
    leaves: &[DockLeafDropTarget],
    target_tabs: DockNodeId,
) -> Option<Bounds<Pixels>> {
    leaves
        .iter()
        .find(|leaf| leaf.target_tabs == target_tabs)
        .map(|leaf| leaf.bounds)
}
