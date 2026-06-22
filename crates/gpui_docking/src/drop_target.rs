use crate::{
    DockEdgeDockPlan, DockEdgeDockSizing, DockNodeId, DockPolicy, DockPolicyError, DockSpaceId,
    DropZone, SplitAxis,
    geometry::{self, DockDropBox, DockDropBoxKind, DockDropGeometry, DockDropGuideStyle},
};
use open_gpui::{Bounds, Pixels, Point, Size, px, size};

pub(crate) type DockDropTargetValidator<'a> =
    dyn Fn(&DockResolvedDropTarget) -> Result<(), DockPolicyError> + 'a;
pub(crate) type DockEdgePlanResolver<'a> =
    dyn Fn(DockNodeId, DropZone, DockEdgeDockSizing) -> Option<DockEdgeDockPlan> + 'a;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockResolvedDropTarget {
    pub(crate) kind: DockResolvedDropTargetKind,
    pub(crate) source: DockDropResolveSource,
    pub(crate) drop_box: Option<DockDropBox>,
    pub(crate) preview_bounds: Option<Bounds<Pixels>>,
    pub(crate) edge_sizing: Option<DockEdgeDockSizing>,
    pub(crate) edge_plan: Option<DockEdgeDockPlan>,
    pub(crate) is_central_region: bool,
}

impl DockResolvedDropTarget {
    pub(crate) fn target_key(&self) -> DockDropTargetKey {
        DockDropTargetKey {
            kind: self.kind.clone(),
            source: self.source,
            drop_box_kind: self.drop_box.map(|drop_box| drop_box.kind),
            edge_sizing: self.edge_sizing,
            edge_plan: self.edge_plan,
        }
    }

    pub(crate) fn zone(&self) -> Option<DropZone> {
        match self.kind {
            DockResolvedDropTargetKind::TabBar { .. }
            | DockResolvedDropTargetKind::LeafCenter { .. }
            | DockResolvedDropTargetKind::FloatingTitleBar { .. } => Some(DropZone::Center),
            DockResolvedDropTargetKind::InnerEdge { zone, .. }
            | DockResolvedDropTargetKind::RootEdge { zone, .. } => Some(zone),
            DockResolvedDropTargetKind::EmptyDockSpace { .. } => None,
        }
    }

    pub(crate) fn target_space<'a>(&'a self, default_space: &'a DockSpaceId) -> &'a DockSpaceId {
        match &self.kind {
            DockResolvedDropTargetKind::EmptyDockSpace { space, .. } => space,
            DockResolvedDropTargetKind::TabBar { .. }
            | DockResolvedDropTargetKind::LeafCenter { .. }
            | DockResolvedDropTargetKind::InnerEdge { .. }
            | DockResolvedDropTargetKind::RootEdge { .. }
            | DockResolvedDropTargetKind::FloatingTitleBar { .. } => default_space,
        }
    }

    pub(crate) fn center_target_tabs(&self) -> Option<DockNodeId> {
        match self.kind {
            DockResolvedDropTargetKind::TabBar { target_tabs, .. }
            | DockResolvedDropTargetKind::LeafCenter { target_tabs, .. }
            | DockResolvedDropTargetKind::FloatingTitleBar { target_tabs, .. } => Some(target_tabs),
            DockResolvedDropTargetKind::InnerEdge { .. }
            | DockResolvedDropTargetKind::RootEdge { .. }
            | DockResolvedDropTargetKind::EmptyDockSpace { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockDropTargetKey {
    kind: DockResolvedDropTargetKind,
    source: DockDropResolveSource,
    drop_box_kind: Option<DockDropBoxKind>,
    edge_sizing: Option<DockEdgeDockSizing>,
    edge_plan: Option<DockEdgeDockPlan>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockResolvedDropTargetKind {
    TabBar {
        target_tabs: DockNodeId,
        insert_index: usize,
    },
    LeafCenter {
        root: DockNodeId,
        target_tabs: DockNodeId,
    },
    InnerEdge {
        root: DockNodeId,
        target_tabs: DockNodeId,
        zone: DropZone,
    },
    RootEdge {
        root: DockNodeId,
        leaf_tabs: Option<DockNodeId>,
        zone: DropZone,
    },
    FloatingTitleBar {
        floating: DockNodeId,
        target_tabs: DockNodeId,
    },
    EmptyDockSpace {
        space: DockSpaceId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockDropResolveSource {
    TabBar,
    LeafBody,
    InnerEdge,
    RootEdge,
    FloatingTitleBar,
    EmptyDockSpace,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockTabLabelDropTarget {
    pub(crate) target_tabs: DockNodeId,
    pub(crate) target_index: usize,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) is_central: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockTabBarDropTarget {
    pub(crate) target_tabs: DockNodeId,
    pub(crate) insert_index: usize,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) is_central: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockLeafDropTarget {
    pub(crate) root: DockNodeId,
    pub(crate) target_tabs: DockNodeId,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) is_central: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockRootDropTarget {
    pub(crate) root: DockNodeId,
    pub(crate) bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockFloatingTitleBarDropTarget {
    pub(crate) floating: DockNodeId,
    pub(crate) target_tabs: DockNodeId,
    pub(crate) title_bounds: Bounds<Pixels>,
    pub(crate) preview_bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockEmptySpaceDropTarget {
    pub(crate) space: DockSpaceId,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) is_central: bool,
}

pub(crate) struct DockDropResolverInput<'a> {
    pub(crate) position: Point<Pixels>,
    pub(crate) payload_size: Option<Size<Pixels>>,
    pub(crate) drop_guide_style: DockDropGuideStyle,
    pub(crate) policy: &'a DockPolicy,
    pub(crate) target_validator: Option<&'a DockDropTargetValidator<'a>>,
    pub(crate) edge_plan_resolver: Option<&'a DockEdgePlanResolver<'a>>,
    pub(crate) tab_labels: &'a [DockTabLabelDropTarget],
    pub(crate) tab_bars: &'a [DockTabBarDropTarget],
    pub(crate) leaves: &'a [DockLeafDropTarget],
    pub(crate) root: Option<DockRootDropTarget>,
    pub(crate) floating_title_bars: &'a [DockFloatingTitleBarDropTarget],
    pub(crate) empty_spaces: &'a [DockEmptySpaceDropTarget],
}

impl<'a> DockDropResolverInput<'a> {
    #[cfg(test)]
    pub(crate) fn new(position: Point<Pixels>, policy: &'a DockPolicy) -> Self {
        Self {
            position,
            payload_size: None,
            drop_guide_style: DockDropGuideStyle::default(),
            policy,
            target_validator: None,
            edge_plan_resolver: None,
            tab_labels: &[],
            tab_bars: &[],
            leaves: &[],
            root: None,
            floating_title_bars: &[],
            empty_spaces: &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockDropResolution {
    Valid(DockResolvedDropTarget),
    Rejected(DockDropRejection),
}

impl DockDropResolution {
    #[cfg(test)]
    pub(crate) fn target(self) -> Option<DockResolvedDropTarget> {
        match self {
            Self::Valid(target) => Some(target),
            Self::Rejected(_) => None,
        }
    }

    fn is_valid(&self) -> bool {
        matches!(self, Self::Valid(_))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockDropRejection {
    pub(crate) target: DockResolvedDropTarget,
    pub(crate) reason: DockPolicyError,
}

#[derive(Debug)]
struct DockDropCandidate {
    target: DockResolvedDropTarget,
    hit_bounds: Bounds<Pixels>,
    order: usize,
}

pub(crate) fn resolve_layout_drop(input: DockDropResolverInput<'_>) -> Option<DockDropResolution> {
    let candidates = collect_drop_candidates(&input);
    choose_drop_candidate(candidates, input.policy, input.target_validator)
}

fn collect_drop_candidates(input: &DockDropResolverInput<'_>) -> Vec<DockDropCandidate> {
    let mut candidates = Vec::new();
    let mut order = 0;

    for target in input
        .empty_spaces
        .iter()
        .filter(|target| target.bounds.contains(&input.position))
    {
        push_drop_candidate(
            &mut candidates,
            &mut order,
            DockResolvedDropTarget {
                kind: DockResolvedDropTargetKind::EmptyDockSpace {
                    space: target.space.clone(),
                },
                source: DockDropResolveSource::EmptyDockSpace,
                drop_box: None,
                preview_bounds: Some(target.bounds),
                edge_sizing: None,
                edge_plan: None,
                is_central_region: target.is_central,
            },
            target.bounds,
        );
    }

    for leaf in input
        .leaves
        .iter()
        .filter(|leaf| leaf.bounds.contains(&input.position))
    {
        let Some(target) = resolve_leaf_drop(
            leaf,
            input.position,
            input.payload_size,
            input.drop_guide_style,
            input.edge_plan_resolver,
        ) else {
            continue;
        };
        let hit_bounds = target
            .drop_box
            .map_or(leaf.bounds, |drop_box| drop_box.hit_bounds);
        push_drop_candidate(&mut candidates, &mut order, target, hit_bounds);
    }

    for target in input
        .tab_bars
        .iter()
        .filter(|target| target.bounds.contains(&input.position))
    {
        push_drop_candidate(
            &mut candidates,
            &mut order,
            DockResolvedDropTarget {
                kind: DockResolvedDropTargetKind::TabBar {
                    target_tabs: target.target_tabs,
                    insert_index: target.insert_index,
                },
                source: DockDropResolveSource::TabBar,
                drop_box: None,
                preview_bounds: Some(target.bounds),
                edge_sizing: None,
                edge_plan: None,
                is_central_region: target.is_central,
            },
            target.bounds,
        );
    }

    for target in input
        .tab_labels
        .iter()
        .filter(|target| target.bounds.contains(&input.position))
    {
        let insert_index = if input.position.x < target.bounds.center().x {
            target.target_index
        } else {
            target.target_index.saturating_add(1)
        };
        push_drop_candidate(
            &mut candidates,
            &mut order,
            DockResolvedDropTarget {
                kind: DockResolvedDropTargetKind::TabBar {
                    target_tabs: target.target_tabs,
                    insert_index,
                },
                source: DockDropResolveSource::TabBar,
                drop_box: None,
                preview_bounds: Some(target.bounds),
                edge_sizing: None,
                edge_plan: None,
                is_central_region: target.is_central,
            },
            target.bounds,
        );
    }

    for target in input
        .floating_title_bars
        .iter()
        .filter(|target| target.title_bounds.contains(&input.position))
    {
        push_drop_candidate(
            &mut candidates,
            &mut order,
            DockResolvedDropTarget {
                kind: DockResolvedDropTargetKind::FloatingTitleBar {
                    floating: target.floating,
                    target_tabs: target.target_tabs,
                },
                source: DockDropResolveSource::FloatingTitleBar,
                drop_box: None,
                preview_bounds: Some(target.preview_bounds),
                edge_sizing: None,
                edge_plan: None,
                is_central_region: false,
            },
            target.title_bounds,
        );
    }

    let leaf = input
        .root
        .and_then(|root| best_leaf_for_root_containing(input.leaves, input.position, root.root));
    if let Some(target) = resolve_root_edge_drop(input, leaf) {
        let root_bounds = input.root.expect("root edge target requires root").bounds;
        let hit_bounds = target
            .drop_box
            .map_or(root_bounds, |drop_box| drop_box.hit_bounds);
        push_drop_candidate(&mut candidates, &mut order, target, hit_bounds);
    }

    candidates
}

fn push_drop_candidate(
    candidates: &mut Vec<DockDropCandidate>,
    order: &mut usize,
    target: DockResolvedDropTarget,
    hit_bounds: Bounds<Pixels>,
) {
    candidates.push(DockDropCandidate {
        target,
        hit_bounds,
        order: *order,
    });
    *order += 1;
}

fn choose_drop_candidate(
    candidates: Vec<DockDropCandidate>,
    policy: &DockPolicy,
    target_validator: Option<&DockDropTargetValidator<'_>>,
) -> Option<DockDropResolution> {
    let mut best_valid = None;
    let mut best_rejection = None;

    for candidate in candidates {
        let hit_bounds = candidate.hit_bounds;
        let order = candidate.order;
        let resolution = validate_resolved_drop_target(candidate.target, policy, target_validator);
        let slot = if resolution.is_valid() {
            &mut best_valid
        } else {
            &mut best_rejection
        };

        if candidate_beats_current(hit_bounds, order, slot.as_ref()) {
            *slot = Some((hit_bounds, order, resolution));
        }
    }

    best_valid
        .or(best_rejection)
        .map(|(_, _, resolution)| resolution)
}

fn candidate_beats_current(
    hit_bounds: Bounds<Pixels>,
    order: usize,
    current: Option<&(Bounds<Pixels>, usize, DockDropResolution)>,
) -> bool {
    let Some((current_bounds, current_order, _)) = current else {
        return true;
    };
    let area = bounds_area(hit_bounds);
    let current_area = bounds_area(*current_bounds);
    area < current_area || (area == current_area && order > *current_order)
}

fn bounds_area(bounds: Bounds<Pixels>) -> f32 {
    let width = f32::from(bounds.size.width).max(0.0);
    let height = f32::from(bounds.size.height).max(0.0);
    let area = width * height;
    if area.is_finite() {
        area
    } else {
        f32::INFINITY
    }
}

fn resolve_root_edge_drop(
    input: &DockDropResolverInput<'_>,
    leaf: Option<&DockLeafDropTarget>,
) -> Option<DockResolvedDropTarget> {
    let root = input.root?;
    let leaf_tabs = match leaf {
        Some(leaf) if leaf.root == root.root => Some(leaf.target_tabs),
        Some(_) => return None,
        None => None,
    };

    let geometry = geometry::resolve_outer_drop_geometry_with_style(
        root.bounds,
        input.position,
        input.drop_guide_style,
    )?;

    let (drop_box, preview_bounds, edge_sizing) =
        edge_drop_metadata(root.bounds, geometry.drop_box, input.payload_size);
    let edge_plan = input
        .edge_plan_resolver
        .and_then(|resolver| resolver(root.root, geometry.zone(), edge_sizing));

    Some(DockResolvedDropTarget {
        kind: DockResolvedDropTargetKind::RootEdge {
            root: root.root,
            leaf_tabs,
            zone: geometry.zone(),
        },
        source: DockDropResolveSource::RootEdge,
        drop_box: Some(drop_box),
        preview_bounds: Some(preview_bounds),
        edge_sizing: Some(edge_sizing),
        edge_plan,
        is_central_region: false,
    })
}

fn resolve_leaf_drop(
    leaf: &DockLeafDropTarget,
    position: Point<Pixels>,
    payload_size: Option<Size<Pixels>>,
    drop_guide_style: DockDropGuideStyle,
    edge_plan_resolver: Option<&DockEdgePlanResolver<'_>>,
) -> Option<DockResolvedDropTarget> {
    let geometry =
        geometry::resolve_inner_drop_geometry_with_style(leaf.bounds, position, drop_guide_style)?;
    target_from_leaf_geometry(leaf, geometry, payload_size, edge_plan_resolver)
}

pub(crate) fn validate_resolved_drop_target(
    target: DockResolvedDropTarget,
    policy: &DockPolicy,
    target_validator: Option<&DockDropTargetValidator<'_>>,
) -> DockDropResolution {
    if target.is_central_dock_over_target()
        && let Err(reason) = policy.validate_central_region_dock_over()
    {
        return DockDropResolution::Rejected(DockDropRejection { target, reason });
    }

    if let Some(zone) = target.zone()
        && let Err(reason) = policy.validate_drop_zone(zone)
    {
        return DockDropResolution::Rejected(DockDropRejection { target, reason });
    }

    match target_validator.map(|validator| validator(&target)) {
        Some(Ok(())) | None => DockDropResolution::Valid(target),
        Some(Err(reason)) => DockDropResolution::Rejected(DockDropRejection { target, reason }),
    }
}

impl DockResolvedDropTarget {
    fn is_central_dock_over_target(&self) -> bool {
        self.is_central_region
            && matches!(
                self.kind,
                DockResolvedDropTargetKind::TabBar { .. }
                    | DockResolvedDropTargetKind::LeafCenter { .. }
                    | DockResolvedDropTargetKind::EmptyDockSpace { .. }
            )
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
            drop_box: Some(geometry.drop_box),
            preview_bounds: Some(geometry.preview_bounds()),
            edge_sizing: None,
            edge_plan: None,
            is_central_region: leaf.is_central,
        });
    }

    if leaf.is_central {
        return None;
    }

    let (drop_box, preview_bounds, edge_sizing) =
        edge_drop_metadata(leaf.bounds, geometry.drop_box, payload_size);
    let edge_plan = edge_plan_resolver
        .and_then(|resolver| resolver(leaf.target_tabs, geometry.zone(), edge_sizing));

    Some(DockResolvedDropTarget {
        kind,
        source,
        drop_box: Some(drop_box),
        preview_bounds: Some(preview_bounds),
        edge_sizing: Some(edge_sizing),
        edge_plan,
        is_central_region: leaf.is_central,
    })
}

fn edge_drop_metadata(
    target_bounds: Bounds<Pixels>,
    mut drop_box: DockDropBox,
    payload_size: Option<Size<Pixels>>,
) -> (DockDropBox, Bounds<Pixels>, DockEdgeDockSizing) {
    let zone = drop_box.kind.zone();
    let preview_bounds = edge_preview_bounds(zone, target_bounds, payload_size);
    drop_box.preview_bounds = preview_bounds;
    let axis = zone_axis(zone);
    let sizing = DockEdgeDockSizing::from_extents(
        split_extent(axis, preview_bounds),
        split_extent(axis, target_bounds),
    );
    (drop_box, preview_bounds, sizing)
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

fn best_leaf_for_root_containing(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockGraph, DockItemId, DockNode, SplitAxis,
        geometry::{DockDropBoxKind, DockDropBoxSet},
    };
    use open_gpui::{point, px, size};
    use slotmap::Key;

    fn tabs() -> DockNodeId {
        DockNodeId::null()
    }

    fn bounds(width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(10.0), px(20.0)), size(px(width), px(height)))
    }

    fn policy() -> DockPolicy {
        DockPolicy::default()
    }

    fn resolve_tabs_drop_with_central(
        target_tabs: DockNodeId,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        is_central: bool,
        policy: &DockPolicy,
    ) -> Option<DockDropResolution> {
        let leaf = [DockLeafDropTarget {
            root: target_tabs,
            target_tabs,
            bounds,
            is_central,
        }];
        resolve_layout_drop(DockDropResolverInput {
            leaves: &leaf,
            ..DockDropResolverInput::new(position, policy)
        })
    }

    fn resolve_tab_reorder_drop_with_central(
        target_tabs: DockNodeId,
        target_index: usize,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        is_central: bool,
        policy: &DockPolicy,
    ) -> Option<DockDropResolution> {
        let tab = [DockTabLabelDropTarget {
            target_tabs,
            target_index,
            bounds,
            is_central,
        }];
        resolve_layout_drop(DockDropResolverInput {
            tab_labels: &tab,
            ..DockDropResolverInput::new(position, policy)
        })
    }

    fn resolve_tab_bar_empty_drop_with_central(
        target_tabs: DockNodeId,
        insert_index: usize,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        is_central: bool,
        policy: &DockPolicy,
    ) -> Option<DockDropResolution> {
        let tab_bar = [DockTabBarDropTarget {
            target_tabs,
            insert_index,
            bounds,
            is_central,
        }];
        resolve_layout_drop(DockDropResolverInput {
            tab_bars: &tab_bar,
            ..DockDropResolverInput::new(position, policy)
        })
    }

    fn leaf(root: DockNodeId, target_tabs: DockNodeId) -> DockLeafDropTarget {
        DockLeafDropTarget {
            root,
            target_tabs,
            bounds: bounds(300.0, 200.0),
            is_central: false,
        }
    }

    fn two_node_ids() -> (DockNodeId, DockNodeId) {
        let mut graph = DockGraph::new();
        let first = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("first")],
            selected: Some(DockItemId::from("first")),
        });
        let second = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("second")],
            selected: Some(DockItemId::from("second")),
        });
        (first, second)
    }

    fn drop_box_center(
        bounds: Bounds<Pixels>,
        set: DockDropBoxSet,
        kind: DockDropBoxKind,
    ) -> Point<Pixels> {
        geometry::drop_boxes(bounds, set)
            .into_iter()
            .find(|drop_box| drop_box.kind == kind)
            .map(|drop_box| drop_box.hit_bounds.center())
            .unwrap_or_else(|| panic!("{kind:?} should exist"))
    }

    #[test]
    fn center_point_resolves_to_center_zone() {
        let target = resolve_tabs_drop_with_central(
            tabs(),
            bounds(300.0, 200.0),
            point(px(160.0), px(120.0)),
            false,
            &policy(),
        )
        .and_then(DockDropResolution::target)
        .expect("point should resolve");

        assert_eq!(target.zone(), Some(DropZone::Center));
        assert_eq!(
            target.preview_bounds.map(|bounds| bounds.size),
            Some(size(px(300.0), px(200.0)))
        );
    }

    #[test]
    fn points_outside_explicit_side_boxes_do_not_split() {
        assert!(
            resolve_tabs_drop_with_central(
                tabs(),
                bounds(300.0, 200.0),
                point(px(12.0), px(120.0)),
                false,
                &policy()
            )
            .is_none()
        );
    }

    #[test]
    fn inner_side_boxes_resolve_to_matching_zones() {
        let bounds = bounds(300.0, 200.0);

        assert_eq!(
            resolve_tabs_drop_with_central(
                tabs(),
                bounds,
                drop_box_center(
                    bounds,
                    DockDropBoxSet::Inner,
                    DockDropBoxKind::InnerEdge(DropZone::Left)
                ),
                false,
                &policy()
            )
            .and_then(DockDropResolution::target)
            .and_then(|target| target.zone()),
            Some(DropZone::Left)
        );
        assert_eq!(
            resolve_tabs_drop_with_central(
                tabs(),
                bounds,
                drop_box_center(
                    bounds,
                    DockDropBoxSet::Inner,
                    DockDropBoxKind::InnerEdge(DropZone::Right)
                ),
                false,
                &policy()
            )
            .and_then(DockDropResolution::target)
            .and_then(|target| target.zone()),
            Some(DropZone::Right)
        );
        assert_eq!(
            resolve_tabs_drop_with_central(
                tabs(),
                bounds,
                drop_box_center(
                    bounds,
                    DockDropBoxSet::Inner,
                    DockDropBoxKind::InnerEdge(DropZone::Top)
                ),
                false,
                &policy()
            )
            .and_then(DockDropResolution::target)
            .and_then(|target| target.zone()),
            Some(DropZone::Top)
        );
        assert_eq!(
            resolve_tabs_drop_with_central(
                tabs(),
                bounds,
                drop_box_center(
                    bounds,
                    DockDropBoxSet::Inner,
                    DockDropBoxKind::InnerEdge(DropZone::Bottom)
                ),
                false,
                &policy()
            )
            .and_then(DockDropResolution::target)
            .and_then(|target| target.zone()),
            Some(DropZone::Bottom)
        );
    }

    #[test]
    fn outside_points_do_not_resolve() {
        assert!(
            resolve_tabs_drop_with_central(
                tabs(),
                bounds(300.0, 200.0),
                point(px(500.0), px(120.0)),
                false,
                &policy()
            )
            .is_none()
        );
    }

    #[test]
    fn small_targets_still_leave_center_space() {
        let bounds = bounds(36.0, 36.0);

        assert_eq!(
            resolve_tabs_drop_with_central(
                tabs(),
                bounds,
                point(px(28.0), px(38.0)),
                false,
                &policy()
            )
            .and_then(DockDropResolution::target)
            .and_then(|target| target.zone()),
            Some(DropZone::Center)
        );
    }

    #[test]
    fn disabled_edge_split_returns_rejection_without_preview_projection() {
        let mut policy = DockPolicy::default();
        policy.set_allow_edge_split(false);
        let bounds = bounds(300.0, 200.0);
        let resolution = resolve_tabs_drop_with_central(
            tabs(),
            bounds,
            drop_box_center(
                bounds,
                DockDropBoxSet::Inner,
                DockDropBoxKind::InnerEdge(DropZone::Left),
            ),
            false,
            &policy,
        )
        .expect("edge point should resolve to a policy result");

        let DockDropResolution::Rejected(rejection) = resolution else {
            panic!("edge split should be rejected");
        };
        assert_eq!(rejection.target.zone(), Some(DropZone::Left));
        assert_eq!(rejection.reason, DockPolicyError::EdgeSplitDisabled);
    }

    #[test]
    fn tab_reorder_drop_uses_target_tab_half_as_insert_index() {
        let bounds = bounds(100.0, 24.0);

        let before = resolve_tab_reorder_drop_with_central(
            tabs(),
            2,
            bounds,
            point(px(24.0), px(28.0)),
            false,
            &policy(),
        )
        .and_then(DockDropResolution::target)
        .expect("left half of the tab should resolve");
        assert_eq!(before.zone(), Some(DropZone::Center));
        assert_eq!(
            before.kind,
            DockResolvedDropTargetKind::TabBar {
                target_tabs: tabs(),
                insert_index: 2,
            }
        );

        let after = resolve_tab_reorder_drop_with_central(
            tabs(),
            2,
            bounds,
            point(px(90.0), px(28.0)),
            false,
            &policy(),
        )
        .and_then(DockDropResolution::target)
        .expect("right half of the tab should resolve");
        assert_eq!(after.zone(), Some(DropZone::Center));
        assert_eq!(
            after.kind,
            DockResolvedDropTargetKind::TabBar {
                target_tabs: tabs(),
                insert_index: 3,
            }
        );
    }

    #[test]
    fn tab_bar_empty_space_appends_to_target_tabs() {
        let target = tabs();
        let target = resolve_tab_bar_empty_drop_with_central(
            target,
            3,
            bounds(300.0, 28.0),
            point(px(260.0), px(30.0)),
            false,
            &policy(),
        )
        .and_then(DockDropResolution::target)
        .expect("empty tab bar area should resolve as an append target");

        assert_eq!(target.source, DockDropResolveSource::TabBar);
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::TabBar {
                target_tabs: tabs(),
                insert_index: 3,
            }
        );
    }

    #[test]
    fn tab_reorder_drop_respects_center_merge_policy() {
        let mut policy = DockPolicy::default();
        policy.set_allow_center_merge(false);

        let DockDropResolution::Rejected(rejection) = resolve_tab_reorder_drop_with_central(
            tabs(),
            0,
            bounds(100.0, 24.0),
            point(px(24.0), px(28.0)),
            false,
            &policy,
        )
        .expect("point inside the tab should resolve to a policy result") else {
            panic!("disabled center merge should reject tab reorder target");
        };

        assert_eq!(rejection.target.zone(), Some(DropZone::Center));
        assert_eq!(rejection.reason, DockPolicyError::CenterMergeDisabled);
    }

    #[test]
    fn layout_resolver_prefers_tab_bar_reorder_before_leaf_body() {
        let root = tabs();
        let tab = DockTabLabelDropTarget {
            target_tabs: root,
            target_index: 2,
            bounds: bounds(100.0, 24.0),
            is_central: false,
        };
        let leaf = leaf(root, root);
        let resolution = resolve_layout_drop(DockDropResolverInput {
            tab_labels: &[tab],
            leaves: &[leaf],
            ..DockDropResolverInput::new(point(px(90.0), px(28.0)), &policy())
        })
        .and_then(DockDropResolution::target)
        .expect("tab label should resolve");

        assert_eq!(resolution.source, DockDropResolveSource::TabBar);
        assert_eq!(
            resolution.kind,
            DockResolvedDropTargetKind::TabBar {
                target_tabs: root,
                insert_index: 3,
            }
        );
    }

    #[test]
    fn leaf_body_center_resolves_to_center_merge_target() {
        let root = tabs();
        let target = resolve_layout_drop(DockDropResolverInput {
            leaves: &[leaf(root, root)],
            ..DockDropResolverInput::new(point(px(160.0), px(120.0)), &policy())
        })
        .and_then(DockDropResolution::target)
        .expect("leaf body should resolve");

        assert_eq!(target.source, DockDropResolveSource::LeafBody);
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::LeafCenter {
                root,
                target_tabs: root,
            }
        );
    }

    #[test]
    fn overlapping_leaf_hits_choose_smallest_area_regardless_of_order() {
        let (background_tabs, foreground_tabs) = two_node_ids();
        let background_bounds = bounds(180.0, 180.0);
        let foreground_bounds = Bounds::new(point(px(50.0), px(60.0)), size(px(100.0), px(100.0)));
        let position = background_bounds.center();
        for leaves in [
            [
                DockLeafDropTarget {
                    root: background_tabs,
                    target_tabs: background_tabs,
                    bounds: background_bounds,
                    is_central: false,
                },
                DockLeafDropTarget {
                    root: foreground_tabs,
                    target_tabs: foreground_tabs,
                    bounds: foreground_bounds,
                    is_central: false,
                },
            ],
            [
                DockLeafDropTarget {
                    root: foreground_tabs,
                    target_tabs: foreground_tabs,
                    bounds: foreground_bounds,
                    is_central: false,
                },
                DockLeafDropTarget {
                    root: background_tabs,
                    target_tabs: background_tabs,
                    bounds: background_bounds,
                    is_central: false,
                },
            ],
        ] {
            let target = resolve_layout_drop(DockDropResolverInput {
                leaves: &leaves,
                ..DockDropResolverInput::new(position, &policy())
            })
            .and_then(DockDropResolution::target)
            .expect("overlapping leaves should resolve");

            assert_eq!(target.source, DockDropResolveSource::LeafBody);
            assert_eq!(
                target.kind,
                DockResolvedDropTargetKind::LeafCenter {
                    root: foreground_tabs,
                    target_tabs: foreground_tabs,
                }
            );
        }
    }

    #[test]
    fn rejected_foreground_leaf_falls_through_to_valid_background_leaf() {
        let (background_tabs, foreground_tabs) = two_node_ids();
        let background_bounds = bounds(180.0, 180.0);
        let foreground_bounds = Bounds::new(point(px(50.0), px(60.0)), size(px(100.0), px(100.0)));
        let position = background_bounds.center();
        let validator = move |target: &DockResolvedDropTarget| {
            if matches!(
                target.kind,
                DockResolvedDropTargetKind::LeafCenter {
                    target_tabs,
                    ..
                } if target_tabs == foreground_tabs
            ) {
                Err(DockPolicyError::DockClassRejected {
                    space: DockSpaceId::from("main"),
                    item: DockItemId::from("a"),
                    dock_class: None,
                })
            } else {
                Ok(())
            }
        };
        let target = resolve_layout_drop(DockDropResolverInput {
            leaves: &[
                DockLeafDropTarget {
                    root: background_tabs,
                    target_tabs: background_tabs,
                    bounds: background_bounds,
                    is_central: false,
                },
                DockLeafDropTarget {
                    root: foreground_tabs,
                    target_tabs: foreground_tabs,
                    bounds: foreground_bounds,
                    is_central: false,
                },
            ],
            target_validator: Some(&validator),
            ..DockDropResolverInput::new(position, &policy())
        })
        .and_then(DockDropResolution::target)
        .expect("rejected foreground target should fall through to the background leaf");

        assert_eq!(target.source, DockDropResolveSource::LeafBody);
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::LeafCenter {
                root: background_tabs,
                target_tabs: background_tabs,
            }
        );
    }

    #[test]
    fn all_rejected_candidates_preserve_smallest_rejection_for_preview() {
        let (background_tabs, foreground_tabs) = two_node_ids();
        let background_bounds = bounds(180.0, 180.0);
        let foreground_bounds = bounds(100.0, 100.0);
        let position = drop_box_center(
            foreground_bounds,
            DockDropBoxSet::Inner,
            DockDropBoxKind::Center,
        );
        let validator = |_: &DockResolvedDropTarget| {
            Err(DockPolicyError::DockClassRejected {
                space: DockSpaceId::from("main"),
                item: DockItemId::from("a"),
                dock_class: None,
            })
        };
        let DockDropResolution::Rejected(rejection) = resolve_layout_drop(DockDropResolverInput {
            leaves: &[
                DockLeafDropTarget {
                    root: background_tabs,
                    target_tabs: background_tabs,
                    bounds: background_bounds,
                    is_central: false,
                },
                DockLeafDropTarget {
                    root: foreground_tabs,
                    target_tabs: foreground_tabs,
                    bounds: foreground_bounds,
                    is_central: false,
                },
            ],
            target_validator: Some(&validator),
            ..DockDropResolverInput::new(position, &policy())
        })
        .expect("all rejected candidates should still produce a rejected preview") else {
            panic!("all candidates should be rejected");
        };

        assert_eq!(rejection.target.source, DockDropResolveSource::LeafBody);
        assert_eq!(
            rejection.target.kind,
            DockResolvedDropTargetKind::LeafCenter {
                root: foreground_tabs,
                target_tabs: foreground_tabs,
            }
        );
    }

    #[test]
    fn overlapping_tab_labels_choose_smallest_area_regardless_of_order() {
        let (wide_tabs, narrow_tabs) = two_node_ids();
        let wide = DockTabLabelDropTarget {
            target_tabs: wide_tabs,
            target_index: 0,
            bounds: bounds(160.0, 24.0),
            is_central: false,
        };
        let narrow = DockTabLabelDropTarget {
            target_tabs: narrow_tabs,
            target_index: 0,
            bounds: Bounds::new(point(px(30.0), px(20.0)), size(px(80.0), px(24.0))),
            is_central: false,
        };
        let position = point(px(50.0), px(28.0));

        for tab_labels in [[wide, narrow], [narrow, wide]] {
            let target = resolve_layout_drop(DockDropResolverInput {
                tab_labels: &tab_labels,
                ..DockDropResolverInput::new(position, &policy())
            })
            .and_then(DockDropResolution::target)
            .expect("overlapping tab labels should resolve");

            assert_eq!(
                target.kind,
                DockResolvedDropTargetKind::TabBar {
                    target_tabs: narrow_tabs,
                    insert_index: 0,
                }
            );
        }
    }

    #[test]
    fn leaf_edge_resolves_to_inner_edge_split_target() {
        let root = tabs();
        let bounds = bounds(300.0, 200.0);
        let target = resolve_layout_drop(DockDropResolverInput {
            leaves: &[DockLeafDropTarget {
                bounds,
                ..leaf(root, root)
            }],
            ..DockDropResolverInput::new(
                drop_box_center(
                    bounds,
                    DockDropBoxSet::Inner,
                    DockDropBoxKind::InnerEdge(DropZone::Left),
                ),
                &policy(),
            )
        })
        .and_then(DockDropResolution::target)
        .expect("leaf edge should resolve");

        assert_eq!(target.source, DockDropResolveSource::InnerEdge);
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::InnerEdge {
                root,
                target_tabs: root,
                zone: DropZone::Left,
            }
        );
    }

    fn root_bounds() -> Bounds<Pixels> {
        Bounds::new(point(px(0.0), px(0.0)), size(px(600.0), px(400.0)))
    }

    fn root_edge_position(zone: DropZone) -> Point<Pixels> {
        drop_box_center(
            root_bounds(),
            DockDropBoxSet::Outer,
            DockDropBoxKind::OuterEdge(zone),
        )
    }

    fn leaf_bounds_containing(position: Point<Pixels>) -> Bounds<Pixels> {
        Bounds::new(
            point(position.x - px(60.0), position.y - px(60.0)),
            size(px(120.0), px(120.0)),
        )
    }

    #[test]
    fn edge_preview_uses_payload_extent_when_payload_fits_half_host() {
        let root = tabs();
        let host = Bounds::new(point(px(0.0), px(0.0)), size(px(1000.0), px(600.0)));
        let target = resolve_layout_drop(DockDropResolverInput {
            payload_size: Some(size(px(240.0), px(180.0))),
            leaves: &[DockLeafDropTarget {
                root,
                target_tabs: root,
                bounds: host,
                is_central: false,
            }],
            ..DockDropResolverInput::new(
                drop_box_center(
                    host,
                    DockDropBoxSet::Inner,
                    DockDropBoxKind::InnerEdge(DropZone::Right),
                ),
                &policy(),
            )
        })
        .and_then(DockDropResolution::target)
        .expect("edge target should resolve");

        assert_eq!(
            target.preview_bounds,
            Some(Bounds::new(
                point(px(760.0), px(0.0)),
                size(px(240.0), px(600.0))
            ))
        );
        assert_eq!(
            target.edge_sizing.map(|sizing| sizing.new_child_share()),
            Some(0.24)
        );
    }

    #[test]
    fn edge_preview_falls_back_to_equal_split_when_payload_exceeds_half_host() {
        let root = tabs();
        let host = Bounds::new(point(px(0.0), px(0.0)), size(px(1000.0), px(600.0)));
        let target = resolve_layout_drop(DockDropResolverInput {
            payload_size: Some(size(px(640.0), px(180.0))),
            leaves: &[DockLeafDropTarget {
                root,
                target_tabs: root,
                bounds: host,
                is_central: false,
            }],
            ..DockDropResolverInput::new(
                drop_box_center(
                    host,
                    DockDropBoxSet::Inner,
                    DockDropBoxKind::InnerEdge(DropZone::Left),
                ),
                &policy(),
            )
        })
        .and_then(DockDropResolution::target)
        .expect("edge target should resolve");

        assert_eq!(
            target.preview_bounds,
            Some(Bounds::new(
                point(px(0.0), px(0.0)),
                size(px(500.0), px(600.0))
            ))
        );
        assert_eq!(
            target.edge_sizing.map(|sizing| sizing.new_child_share()),
            Some(0.5)
        );
    }

    #[test]
    fn root_outer_edges_do_not_override_smaller_leaf_targets() {
        let mut graph = DockGraph::new();
        let leaf_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("a")],
            selected: Some(DockItemId::from("a")),
        });
        let sibling = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("b")],
            selected: Some(DockItemId::from("b")),
        });
        let root = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Horizontal,
            children: vec![leaf_tabs, sibling],
            fractions: vec![0.5, 0.5],
        });
        for zone in [
            DropZone::Left,
            DropZone::Right,
            DropZone::Top,
            DropZone::Bottom,
        ] {
            let position = root_edge_position(zone);
            let leaf_bounds = leaf_bounds_containing(position);
            let target = resolve_layout_drop(DockDropResolverInput {
                root: Some(DockRootDropTarget {
                    root,
                    bounds: root_bounds(),
                }),
                leaves: &[DockLeafDropTarget {
                    root,
                    target_tabs: leaf_tabs,
                    bounds: leaf_bounds,
                    is_central: false,
                }],
                ..DockDropResolverInput::new(position, &policy())
            })
            .and_then(DockDropResolution::target)
            .unwrap_or_else(|| panic!("{zone:?} smaller leaf target should resolve"));

            assert_eq!(target.source, DockDropResolveSource::LeafBody, "{zone:?}");
            assert_eq!(
                target.kind,
                DockResolvedDropTargetKind::LeafCenter {
                    root,
                    target_tabs: leaf_tabs,
                },
                "{zone:?}"
            );
            assert_eq!(target.zone(), Some(DropZone::Center), "{zone:?}");
            assert_eq!(target.preview_bounds, Some(leaf_bounds), "{zone:?}");
        }
    }

    #[test]
    fn leaf_edge_inside_root_center_stays_inner_edge() {
        let (leaf_tabs, sibling) = two_node_ids();
        let mut graph = DockGraph::new();
        let root = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Horizontal,
            children: vec![leaf_tabs, sibling],
            fractions: vec![0.5, 0.5],
        });
        let leaf_bounds = Bounds::new(point(px(240.0), px(60.0)), size(px(120.0), px(280.0)));
        let position = drop_box_center(
            leaf_bounds,
            DockDropBoxSet::Inner,
            DockDropBoxKind::InnerEdge(DropZone::Left),
        );
        let target = resolve_layout_drop(DockDropResolverInput {
            root: Some(DockRootDropTarget {
                root,
                bounds: root_bounds(),
            }),
            leaves: &[DockLeafDropTarget {
                root,
                target_tabs: leaf_tabs,
                bounds: leaf_bounds,
                is_central: false,
            }],
            ..DockDropResolverInput::new(position, &policy())
        })
        .and_then(DockDropResolution::target)
        .expect("leaf edge inside the root center should resolve");

        assert_eq!(target.source, DockDropResolveSource::InnerEdge);
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::InnerEdge {
                root,
                target_tabs: leaf_tabs,
                zone: DropZone::Left,
            }
        );
    }

    #[test]
    fn root_outer_edge_resolves_without_leaf_hit() {
        let mut graph = DockGraph::new();
        let left = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("a")],
            selected: Some(DockItemId::from("a")),
        });
        let right = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("b")],
            selected: Some(DockItemId::from("b")),
        });
        let root = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Horizontal,
            children: vec![left, right],
            fractions: vec![0.5, 0.5],
        });
        let target = resolve_layout_drop(DockDropResolverInput {
            root: Some(DockRootDropTarget {
                root,
                bounds: root_bounds(),
            }),
            ..DockDropResolverInput::new(root_edge_position(DropZone::Right), &policy())
        })
        .and_then(DockDropResolution::target)
        .expect("root edge should resolve without a leaf hit");

        assert_eq!(target.source, DockDropResolveSource::RootEdge);
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::RootEdge {
                root,
                leaf_tabs: None,
                zone: DropZone::Right,
            }
        );
    }

    #[test]
    fn root_that_is_a_leaf_still_supports_outer_edge_docking() {
        let root = tabs();
        let target = resolve_layout_drop(DockDropResolverInput {
            root: Some(DockRootDropTarget {
                root,
                bounds: root_bounds(),
            }),
            leaves: &[DockLeafDropTarget {
                root,
                target_tabs: root,
                bounds: root_bounds(),
                is_central: false,
            }],
            ..DockDropResolverInput::new(root_edge_position(DropZone::Left), &policy())
        })
        .and_then(DockDropResolution::target)
        .expect("root leaf edge should resolve as a root edge");

        assert_eq!(target.source, DockDropResolveSource::RootEdge);
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::RootEdge {
                root,
                leaf_tabs: Some(root),
                zone: DropZone::Left,
            }
        );
    }

    #[test]
    fn leaf_from_different_root_does_not_promote_to_root_edge() {
        let mut graph = DockGraph::new();
        let floating_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("floating")],
            selected: Some(DockItemId::from("floating")),
        });
        let primary_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("primary")],
            selected: Some(DockItemId::from("primary")),
        });
        let primary_root = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Horizontal,
            children: vec![primary_tabs],
            fractions: vec![1.0],
        });
        let leaf_bounds = bounds(300.0, 200.0);
        let position = drop_box_center(
            leaf_bounds,
            DockDropBoxSet::Inner,
            DockDropBoxKind::InnerEdge(DropZone::Right),
        );
        let target = resolve_layout_drop(DockDropResolverInput {
            root: Some(DockRootDropTarget {
                root: primary_root,
                bounds: root_bounds(),
            }),
            leaves: &[DockLeafDropTarget {
                root: floating_tabs,
                target_tabs: floating_tabs,
                bounds: leaf_bounds,
                is_central: false,
            }],
            ..DockDropResolverInput::new(position, &policy())
        })
        .and_then(DockDropResolution::target)
        .expect("foreign-root leaf should still resolve its own inner edge");

        assert_eq!(target.source, DockDropResolveSource::InnerEdge);
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::InnerEdge {
                root: floating_tabs,
                target_tabs: floating_tabs,
                zone: DropZone::Right,
            }
        );
    }

    #[test]
    fn smaller_foreign_leaf_hit_beats_root_edge_candidate() {
        let mut graph = DockGraph::new();
        let leaf_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("leaf")],
            selected: Some(DockItemId::from("leaf")),
        });
        let sibling = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("sibling")],
            selected: Some(DockItemId::from("sibling")),
        });
        let foreign_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("foreign")],
            selected: Some(DockItemId::from("foreign")),
        });
        let root = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Horizontal,
            children: vec![leaf_tabs, sibling],
            fractions: vec![0.5, 0.5],
        });
        let position = root_edge_position(DropZone::Left);
        let same_root_bounds = leaf_bounds_containing(position);
        let foreign_bounds = Bounds::new(
            point(position.x - px(20.0), position.y - px(20.0)),
            size(px(40.0), px(40.0)),
        );

        let target = resolve_layout_drop(DockDropResolverInput {
            root: Some(DockRootDropTarget {
                root,
                bounds: root_bounds(),
            }),
            leaves: &[
                DockLeafDropTarget {
                    root,
                    target_tabs: leaf_tabs,
                    bounds: same_root_bounds,
                    is_central: false,
                },
                DockLeafDropTarget {
                    root: foreign_tabs,
                    target_tabs: foreign_tabs,
                    bounds: foreign_bounds,
                    is_central: false,
                },
            ],
            ..DockDropResolverInput::new(position, &policy())
        })
        .and_then(DockDropResolution::target)
        .expect("smaller foreign leaf should resolve");

        assert_eq!(target.source, DockDropResolveSource::LeafBody);
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::LeafCenter {
                root: foreign_tabs,
                target_tabs: foreign_tabs,
            }
        );
    }

    #[test]
    fn rejected_root_edge_falls_back_to_valid_leaf_candidate() {
        let mut graph = DockGraph::new();
        let leaf_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("leaf")],
            selected: Some(DockItemId::from("leaf")),
        });
        let sibling = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("sibling")],
            selected: Some(DockItemId::from("sibling")),
        });
        let root = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Horizontal,
            children: vec![leaf_tabs, sibling],
            fractions: vec![0.5, 0.5],
        });
        let position = root_edge_position(DropZone::Left);
        let validator = |target: &DockResolvedDropTarget| {
            if target.source == DockDropResolveSource::RootEdge {
                Err(DockPolicyError::DockClassRejected {
                    space: DockSpaceId::from("main"),
                    item: DockItemId::from("a"),
                    dock_class: None,
                })
            } else {
                Ok(())
            }
        };

        let target = resolve_layout_drop(DockDropResolverInput {
            root: Some(DockRootDropTarget {
                root,
                bounds: root_bounds(),
            }),
            leaves: &[DockLeafDropTarget {
                root,
                target_tabs: leaf_tabs,
                bounds: leaf_bounds_containing(position),
                is_central: false,
            }],
            target_validator: Some(&validator),
            ..DockDropResolverInput::new(position, &policy())
        })
        .and_then(DockDropResolution::target)
        .expect("valid leaf fallback should resolve");

        assert_eq!(target.source, DockDropResolveSource::LeafBody);
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::LeafCenter {
                root,
                target_tabs: leaf_tabs,
            }
        );
    }

    #[test]
    fn empty_dock_space_resolves_without_tabs_node() {
        let space = DockSpaceId::from("empty");
        let target = resolve_layout_drop(DockDropResolverInput {
            empty_spaces: &[DockEmptySpaceDropTarget {
                space: space.clone(),
                bounds: bounds(300.0, 200.0),
                is_central: false,
            }],
            ..DockDropResolverInput::new(point(px(160.0), px(120.0)), &policy())
        })
        .and_then(DockDropResolution::target)
        .expect("empty dock space should resolve");

        assert_eq!(target.source, DockDropResolveSource::EmptyDockSpace);
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::EmptyDockSpace { space }
        );
        assert_eq!(
            target.preview_bounds,
            Some(bounds(300.0, 200.0)),
            "empty dock spaces now carry host-overlay preview bounds"
        );
    }

    #[test]
    fn empty_dock_space_respects_target_validator() {
        let space = DockSpaceId::from("restricted");
        let target_validator = |_: &DockResolvedDropTarget| {
            Err(DockPolicyError::DockClassRejected {
                space: space.clone(),
                item: DockItemId::from("editor"),
                dock_class: None,
            })
        };
        let resolution = resolve_layout_drop(DockDropResolverInput {
            empty_spaces: &[DockEmptySpaceDropTarget {
                space: space.clone(),
                bounds: bounds(300.0, 200.0),
                is_central: false,
            }],
            target_validator: Some(&target_validator),
            ..DockDropResolverInput::new(point(px(160.0), px(120.0)), &policy())
        })
        .expect("empty dock space should resolve to a policy decision");

        let DockDropResolution::Rejected(rejection) = resolution else {
            panic!("empty dock space target validator should reject");
        };
        assert_eq!(
            rejection.target.source,
            DockDropResolveSource::EmptyDockSpace
        );
        assert_eq!(
            rejection.target.kind,
            DockResolvedDropTargetKind::EmptyDockSpace {
                space: space.clone(),
            }
        );
        assert_eq!(
            rejection.reason,
            DockPolicyError::DockClassRejected {
                space,
                item: DockItemId::from("editor"),
                dock_class: None,
            }
        );
    }

    #[test]
    fn empty_central_space_respects_central_dock_over_policy() {
        let space = DockSpaceId::from("central");
        let mut policy = DockPolicy::default();
        policy.set_allow_central_region_dock_over(false);
        let resolution = resolve_layout_drop(DockDropResolverInput {
            empty_spaces: &[DockEmptySpaceDropTarget {
                space: space.clone(),
                bounds: bounds(300.0, 200.0),
                is_central: true,
            }],
            ..DockDropResolverInput::new(point(px(160.0), px(120.0)), &policy)
        })
        .expect("empty central space should resolve to a policy decision");

        let DockDropResolution::Rejected(rejection) = resolution else {
            panic!("central dock-over should be rejected");
        };
        assert_eq!(
            rejection.target.source,
            DockDropResolveSource::EmptyDockSpace
        );
        assert_eq!(
            rejection.target.kind,
            DockResolvedDropTargetKind::EmptyDockSpace { space }
        );
        assert_eq!(
            rejection.reason,
            DockPolicyError::CentralRegionDockOverDisabled
        );
    }

    #[test]
    fn floating_title_bar_resolves_against_floating_child_layout() {
        let (floating, target_tabs) = two_node_ids();
        let target = resolve_layout_drop(DockDropResolverInput {
            floating_title_bars: &[DockFloatingTitleBarDropTarget {
                floating,
                target_tabs,
                title_bounds: bounds(220.0, 24.0),
                preview_bounds: bounds(220.0, 140.0),
            }],
            ..DockDropResolverInput::new(point(px(40.0), px(28.0)), &policy())
        })
        .and_then(DockDropResolution::target)
        .expect("floating title should resolve");

        assert_eq!(target.source, DockDropResolveSource::FloatingTitleBar);
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::FloatingTitleBar {
                floating,
                target_tabs,
            }
        );
        assert_eq!(target.zone(), Some(DropZone::Center));
        assert_eq!(target.preview_bounds, Some(bounds(220.0, 140.0)));
    }

    #[test]
    fn policy_disabled_center_merge_rejects_rich_target() {
        let root = tabs();
        let mut policy = DockPolicy::default();
        policy.set_allow_center_merge(false);
        let resolution = resolve_layout_drop(DockDropResolverInput {
            leaves: &[leaf(root, root)],
            ..DockDropResolverInput::new(point(px(160.0), px(120.0)), &policy)
        })
        .expect("leaf center should resolve to a policy decision");

        let DockDropResolution::Rejected(rejection) = resolution else {
            panic!("center merge should be rejected");
        };
        assert_eq!(rejection.target.source, DockDropResolveSource::LeafBody);
        assert_eq!(rejection.target.zone(), Some(DropZone::Center));
        assert_eq!(rejection.reason, DockPolicyError::CenterMergeDisabled);
    }

    #[test]
    fn central_leaf_center_respects_central_dock_over_policy() {
        let root = tabs();
        let mut policy = DockPolicy::default();
        policy.set_allow_central_region_dock_over(false);
        let resolution = resolve_layout_drop(DockDropResolverInput {
            leaves: &[DockLeafDropTarget {
                is_central: true,
                ..leaf(root, root)
            }],
            ..DockDropResolverInput::new(point(px(160.0), px(120.0)), &policy)
        })
        .expect("central leaf center should resolve to a policy decision");

        let DockDropResolution::Rejected(rejection) = resolution else {
            panic!("central dock-over should be rejected");
        };
        assert_eq!(rejection.target.source, DockDropResolveSource::LeafBody);
        assert_eq!(rejection.target.zone(), Some(DropZone::Center));
        assert_eq!(
            rejection.reason,
            DockPolicyError::CentralRegionDockOverDisabled
        );
    }

    #[test]
    fn central_tab_bar_reorder_respects_central_dock_over_policy() {
        let root = tabs();
        let mut policy = DockPolicy::default();
        policy.set_allow_central_region_dock_over(false);
        let resolution = resolve_tab_reorder_drop_with_central(
            root,
            0,
            bounds(100.0, 24.0),
            point(px(24.0), px(28.0)),
            true,
            &policy,
        )
        .expect("central tab bar should resolve to a policy decision");

        let DockDropResolution::Rejected(rejection) = resolution else {
            panic!("central tab-bar dock-over should be rejected");
        };
        assert_eq!(rejection.target.source, DockDropResolveSource::TabBar);
        assert_eq!(rejection.target.zone(), Some(DropZone::Center));
        assert_eq!(
            rejection.reason,
            DockPolicyError::CentralRegionDockOverDisabled
        );
    }

    #[test]
    fn central_leaf_side_hit_does_not_create_inner_edge_target() {
        let root = tabs();
        let leaf_bounds = bounds(300.0, 200.0);
        let position = drop_box_center(
            leaf_bounds,
            DockDropBoxSet::Inner,
            DockDropBoxKind::InnerEdge(DropZone::Left),
        );

        let target = resolve_layout_drop(DockDropResolverInput {
            leaves: &[DockLeafDropTarget {
                is_central: true,
                bounds: leaf_bounds,
                ..leaf(root, root)
            }],
            ..DockDropResolverInput::new(position, &policy())
        })
        .and_then(DockDropResolution::target);

        assert_eq!(
            target, None,
            "central side hits should be represented by root outer docking, not hidden inner-edge targets"
        );
    }

    #[test]
    fn central_leaf_side_hit_prefers_smaller_leaf_body_over_root_outer_edge() {
        let (central_tabs, sibling) = two_node_ids();
        let mut graph = DockGraph::new();
        let root = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Horizontal,
            children: vec![central_tabs, sibling],
            fractions: vec![0.5, 0.5],
        });
        let zone = DropZone::Left;
        let position = root_edge_position(zone);
        let target = resolve_layout_drop(DockDropResolverInput {
            root: Some(DockRootDropTarget {
                root,
                bounds: root_bounds(),
            }),
            leaves: &[DockLeafDropTarget {
                root,
                target_tabs: central_tabs,
                bounds: leaf_bounds_containing(position),
                is_central: true,
            }],
            ..DockDropResolverInput::new(position, &policy())
        })
        .and_then(DockDropResolution::target)
        .expect("central side hit should resolve to the smaller leaf body");

        assert_eq!(target.source, DockDropResolveSource::LeafBody);
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::LeafCenter {
                root,
                target_tabs: central_tabs,
            }
        );
    }
}
