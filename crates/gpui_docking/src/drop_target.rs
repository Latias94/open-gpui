use crate::{
    DockNodeId, DockPolicy, DockPolicyError, DockSpaceId, DockViewportHit, DropZone,
    geometry::{self, DockDropGeometry},
};
use open_gpui::{Bounds, Pixels, Point, WindowBounds};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockDropPreviewIntent {
    pub(crate) target_tabs: DockNodeId,
    pub(crate) zone: DropZone,
    pub(crate) insert_index: Option<usize>,
    pub(crate) preview_bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockResolvedDropTarget {
    pub(crate) kind: DockResolvedDropTargetKind,
    pub(crate) source: DockDropResolveSource,
    pub(crate) preview_bounds: Option<Bounds<Pixels>>,
    pub(crate) is_central_region: bool,
}

impl DockResolvedDropTarget {
    pub(crate) fn matches_drop_receiver(&self, receiver_tabs: DockNodeId) -> bool {
        match self.kind {
            DockResolvedDropTargetKind::TabBar { target_tabs, .. }
            | DockResolvedDropTargetKind::LeafCenter { target_tabs, .. }
            | DockResolvedDropTargetKind::InnerEdge { target_tabs, .. }
            | DockResolvedDropTargetKind::FloatingTitleBar { target_tabs, .. } => {
                target_tabs == receiver_tabs
            }
            DockResolvedDropTargetKind::RootEdge {
                root, leaf_tabs, ..
            } => root == receiver_tabs || leaf_tabs == receiver_tabs,
            DockResolvedDropTargetKind::EmptyDockSpace { .. }
            | DockResolvedDropTargetKind::KnownViewport { .. }
            | DockResolvedDropTargetKind::TearOffCandidate { .. } => false,
        }
    }

    pub(crate) fn preview_intent(&self) -> Option<DockDropPreviewIntent> {
        let preview_bounds = self.preview_bounds?;
        let (target_tabs, zone, insert_index) = match self.kind {
            DockResolvedDropTargetKind::TabBar {
                target_tabs,
                insert_index,
            } => (target_tabs, DropZone::Center, Some(insert_index)),
            DockResolvedDropTargetKind::LeafCenter { target_tabs, .. } => {
                (target_tabs, DropZone::Center, None)
            }
            DockResolvedDropTargetKind::InnerEdge {
                target_tabs, zone, ..
            } => (target_tabs, zone, None),
            DockResolvedDropTargetKind::RootEdge { root, zone, .. } => (root, zone, None),
            DockResolvedDropTargetKind::FloatingTitleBar { target_tabs, .. } => {
                (target_tabs, DropZone::Center, None)
            }
            DockResolvedDropTargetKind::EmptyDockSpace { .. }
            | DockResolvedDropTargetKind::KnownViewport { .. }
            | DockResolvedDropTargetKind::TearOffCandidate { .. } => return None,
        };

        Some(DockDropPreviewIntent {
            target_tabs,
            zone,
            insert_index,
            preview_bounds,
        })
    }

    pub(crate) fn zone(&self) -> Option<DropZone> {
        match self.kind {
            DockResolvedDropTargetKind::TabBar { .. }
            | DockResolvedDropTargetKind::LeafCenter { .. }
            | DockResolvedDropTargetKind::FloatingTitleBar { .. } => Some(DropZone::Center),
            DockResolvedDropTargetKind::InnerEdge { zone, .. }
            | DockResolvedDropTargetKind::RootEdge { zone, .. } => Some(zone),
            DockResolvedDropTargetKind::EmptyDockSpace { .. }
            | DockResolvedDropTargetKind::KnownViewport { .. }
            | DockResolvedDropTargetKind::TearOffCandidate { .. } => None,
        }
    }
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
        leaf_tabs: DockNodeId,
        zone: DropZone,
    },
    FloatingTitleBar {
        floating: DockNodeId,
        target_tabs: DockNodeId,
    },
    EmptyDockSpace {
        space: DockSpaceId,
    },
    KnownViewport {
        hit: DockViewportHit,
    },
    TearOffCandidate {
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
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
    KnownViewport,
    TearOffCandidate,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockTabLabelDropTarget {
    pub(crate) target_tabs: DockNodeId,
    pub(crate) target_index: usize,
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
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockTearOffCandidateDropTarget {
    pub(crate) release_position: Point<Pixels>,
    pub(crate) suggested_window_bounds: Option<WindowBounds>,
}

pub(crate) struct DockDropResolverInput<'a> {
    pub(crate) position: Point<Pixels>,
    pub(crate) policy: &'a DockPolicy,
    pub(crate) tab_labels: &'a [DockTabLabelDropTarget],
    pub(crate) leaves: &'a [DockLeafDropTarget],
    pub(crate) root: Option<DockRootDropTarget>,
    pub(crate) floating_title_bars: &'a [DockFloatingTitleBarDropTarget],
    pub(crate) empty_spaces: &'a [DockEmptySpaceDropTarget],
    pub(crate) known_viewport: Option<DockViewportHit>,
    pub(crate) tear_off_candidate: Option<DockTearOffCandidateDropTarget>,
}

impl<'a> DockDropResolverInput<'a> {
    pub(crate) fn new(position: Point<Pixels>, policy: &'a DockPolicy) -> Self {
        Self {
            position,
            policy,
            tab_labels: &[],
            leaves: &[],
            root: None,
            floating_title_bars: &[],
            empty_spaces: &[],
            known_viewport: None,
            tear_off_candidate: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockDropResolution {
    Valid(DockResolvedDropTarget),
    Rejected(DockDropRejection),
}

impl DockDropResolution {
    pub(crate) fn target(self) -> Option<DockResolvedDropTarget> {
        match self {
            Self::Valid(target) => Some(target),
            Self::Rejected(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockDropRejection {
    pub(crate) target: DockResolvedDropTarget,
    pub(crate) reason: DockPolicyError,
}

pub(crate) fn resolve_layout_drop(input: DockDropResolverInput<'_>) -> Option<DockDropResolution> {
    if let Some(hit) = input.known_viewport {
        return Some(DockDropResolution::Valid(DockResolvedDropTarget {
            kind: DockResolvedDropTargetKind::KnownViewport { hit },
            source: DockDropResolveSource::KnownViewport,
            preview_bounds: None,
            is_central_region: false,
        }));
    }

    if let Some(target) = resolve_floating_title_bar_drop(&input) {
        return Some(validate_target(target, input.policy));
    }

    if let Some(target) = resolve_tab_bar_drop(&input) {
        return Some(validate_target(target, input.policy));
    }

    let leaf = smallest_leaf_containing(input.leaves, input.position);
    if let Some(target) = resolve_root_edge_drop(&input, leaf) {
        return Some(validate_target(target, input.policy));
    }

    if let Some(leaf) = leaf
        && let Some(target) = resolve_leaf_drop(leaf, input.position)
    {
        return Some(validate_target(target, input.policy));
    }

    if let Some(target) = resolve_empty_space_drop(&input) {
        return Some(DockDropResolution::Valid(target));
    }

    if let Some(candidate) = input.tear_off_candidate {
        let target = DockResolvedDropTarget {
            kind: DockResolvedDropTargetKind::TearOffCandidate {
                release_position: candidate.release_position,
                suggested_window_bounds: candidate.suggested_window_bounds,
            },
            source: DockDropResolveSource::TearOffCandidate,
            preview_bounds: None,
            is_central_region: false,
        };
        return Some(match input.policy.validate_platform_viewports() {
            Ok(()) => DockDropResolution::Valid(target),
            Err(reason) => DockDropResolution::Rejected(DockDropRejection { target, reason }),
        });
    }

    None
}

fn resolve_floating_title_bar_drop(
    input: &DockDropResolverInput<'_>,
) -> Option<DockResolvedDropTarget> {
    input
        .floating_title_bars
        .iter()
        .find(|target| target.title_bounds.contains(&input.position))
        .map(|target| DockResolvedDropTarget {
            kind: DockResolvedDropTargetKind::FloatingTitleBar {
                floating: target.floating,
                target_tabs: target.target_tabs,
            },
            source: DockDropResolveSource::FloatingTitleBar,
            preview_bounds: Some(target.preview_bounds),
            is_central_region: false,
        })
}

fn resolve_tab_bar_drop(input: &DockDropResolverInput<'_>) -> Option<DockResolvedDropTarget> {
    input
        .tab_labels
        .iter()
        .find(|target| target.bounds.contains(&input.position))
        .map(|target| {
            let insert_index = if input.position.x < target.bounds.center().x {
                target.target_index
            } else {
                target.target_index.saturating_add(1)
            };
            DockResolvedDropTarget {
                kind: DockResolvedDropTargetKind::TabBar {
                    target_tabs: target.target_tabs,
                    insert_index,
                },
                source: DockDropResolveSource::TabBar,
                preview_bounds: Some(target.bounds),
                is_central_region: target.is_central,
            }
        })
}

fn resolve_root_edge_drop(
    input: &DockDropResolverInput<'_>,
    leaf: Option<&DockLeafDropTarget>,
) -> Option<DockResolvedDropTarget> {
    let root = input.root?;
    let leaf = leaf?;
    if root.root == leaf.target_tabs || leaf.root != root.root {
        return None;
    }

    let geometry = geometry::resolve_drop_geometry(root.bounds, input.position)?;
    if geometry.zone == DropZone::Center {
        return None;
    }

    Some(DockResolvedDropTarget {
        kind: DockResolvedDropTargetKind::RootEdge {
            root: root.root,
            leaf_tabs: leaf.target_tabs,
            zone: geometry.zone,
        },
        source: DockDropResolveSource::RootEdge,
        preview_bounds: Some(geometry.preview_bounds),
        is_central_region: false,
    })
}

fn resolve_leaf_drop(
    leaf: &DockLeafDropTarget,
    position: Point<Pixels>,
) -> Option<DockResolvedDropTarget> {
    let geometry = geometry::resolve_drop_geometry(leaf.bounds, position)?;
    Some(target_from_leaf_geometry(leaf, geometry))
}

fn resolve_empty_space_drop(input: &DockDropResolverInput<'_>) -> Option<DockResolvedDropTarget> {
    input
        .empty_spaces
        .iter()
        .find(|target| target.bounds.contains(&input.position))
        .map(|target| DockResolvedDropTarget {
            kind: DockResolvedDropTargetKind::EmptyDockSpace {
                space: target.space.clone(),
            },
            source: DockDropResolveSource::EmptyDockSpace,
            preview_bounds: Some(target.bounds),
            is_central_region: false,
        })
}

fn validate_target(target: DockResolvedDropTarget, policy: &DockPolicy) -> DockDropResolution {
    if target.is_central_dock_over_target()
        && let Err(reason) = policy.validate_central_region_dock_over()
    {
        return DockDropResolution::Rejected(DockDropRejection { target, reason });
    }

    let Some(zone) = target.zone() else {
        return DockDropResolution::Valid(target);
    };
    match policy.validate_drop_zone(zone) {
        Ok(()) => DockDropResolution::Valid(target),
        Err(reason) => DockDropResolution::Rejected(DockDropRejection { target, reason }),
    }
}

impl DockResolvedDropTarget {
    fn is_central_dock_over_target(&self) -> bool {
        self.is_central_region
            && matches!(
                self.kind,
                DockResolvedDropTargetKind::TabBar { .. }
                    | DockResolvedDropTargetKind::LeafCenter { .. }
            )
    }
}

fn target_from_leaf_geometry(
    leaf: &DockLeafDropTarget,
    geometry: DockDropGeometry,
) -> DockResolvedDropTarget {
    let kind = if geometry.zone == DropZone::Center {
        DockResolvedDropTargetKind::LeafCenter {
            root: leaf.root,
            target_tabs: leaf.target_tabs,
        }
    } else {
        DockResolvedDropTargetKind::InnerEdge {
            root: leaf.root,
            target_tabs: leaf.target_tabs,
            zone: geometry.zone,
        }
    };
    let source = if geometry.zone == DropZone::Center {
        DockDropResolveSource::LeafBody
    } else {
        DockDropResolveSource::InnerEdge
    };

    DockResolvedDropTarget {
        kind,
        source,
        preview_bounds: Some(geometry.preview_bounds),
        is_central_region: leaf.is_central,
    }
}

fn smallest_leaf_containing(
    leaves: &[DockLeafDropTarget],
    position: Point<Pixels>,
) -> Option<&DockLeafDropTarget> {
    leaves
        .iter()
        .filter(|target| target.bounds.contains(&position))
        .min_by(|a, b| {
            area(a.bounds)
                .partial_cmp(&area(b.bounds))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn area(bounds: Bounds<Pixels>) -> f32 {
    f32::from(bounds.size.width) * f32::from(bounds.size.height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DockGraph, DockItemId, DockNode, SplitAxis};
    use open_gpui::{WindowBounds, point, px, size};
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
            active: 0,
        });
        let second = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("second")],
            active: 0,
        });
        (first, second)
    }

    #[test]
    fn center_point_resolves_to_center_zone() {
        let intent = resolve_tabs_drop_with_central(
            tabs(),
            bounds(300.0, 200.0),
            point(px(160.0), px(120.0)),
            false,
            &policy(),
        )
        .and_then(DockDropResolution::target)
        .and_then(|target| target.preview_intent())
        .expect("point should resolve");

        assert_eq!(intent.zone, DropZone::Center);
        assert_eq!(intent.preview_bounds.size, size(px(300.0), px(200.0)));
    }

    #[test]
    fn edge_points_resolve_to_matching_zones() {
        let bounds = bounds(300.0, 200.0);

        assert_eq!(
            resolve_tabs_drop_with_central(
                tabs(),
                bounds,
                point(px(12.0), px(120.0)),
                false,
                &policy()
            )
            .and_then(DockDropResolution::target)
            .and_then(|target| target.preview_intent())
            .map(|intent| intent.zone),
            Some(DropZone::Left)
        );
        assert_eq!(
            resolve_tabs_drop_with_central(
                tabs(),
                bounds,
                point(px(308.0), px(120.0)),
                false,
                &policy()
            )
            .and_then(DockDropResolution::target)
            .and_then(|target| target.preview_intent())
            .map(|intent| intent.zone),
            Some(DropZone::Right)
        );
        assert_eq!(
            resolve_tabs_drop_with_central(
                tabs(),
                bounds,
                point(px(160.0), px(22.0)),
                false,
                &policy()
            )
            .and_then(DockDropResolution::target)
            .and_then(|target| target.preview_intent())
            .map(|intent| intent.zone),
            Some(DropZone::Top)
        );
        assert_eq!(
            resolve_tabs_drop_with_central(
                tabs(),
                bounds,
                point(px(160.0), px(218.0)),
                false,
                &policy()
            )
            .and_then(DockDropResolution::target)
            .and_then(|target| target.preview_intent())
            .map(|intent| intent.zone),
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
            .and_then(|target| target.preview_intent())
            .map(|intent| intent.zone),
            Some(DropZone::Center)
        );
    }

    #[test]
    fn disabled_edge_split_returns_rejection_without_preview_projection() {
        let mut policy = DockPolicy::default();
        policy.set_allow_edge_split(false);
        let resolution = resolve_tabs_drop_with_central(
            tabs(),
            bounds(300.0, 200.0),
            point(px(12.0), px(120.0)),
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
        .and_then(|target| target.preview_intent())
        .expect("left half of the tab should resolve");
        assert_eq!(before.zone, DropZone::Center);
        assert_eq!(before.insert_index, Some(2));

        let after = resolve_tab_reorder_drop_with_central(
            tabs(),
            2,
            bounds,
            point(px(90.0), px(28.0)),
            false,
            &policy(),
        )
        .and_then(DockDropResolution::target)
        .and_then(|target| target.preview_intent())
        .expect("right half of the tab should resolve");
        assert_eq!(after.zone, DropZone::Center);
        assert_eq!(after.insert_index, Some(3));
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
    fn leaf_edge_resolves_to_inner_edge_split_target() {
        let root = tabs();
        let target = resolve_layout_drop(DockDropResolverInput {
            leaves: &[leaf(root, root)],
            ..DockDropResolverInput::new(point(px(12.0), px(120.0)), &policy())
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

    #[test]
    fn root_outer_edge_resolves_before_leaf_inner_edge_when_root_and_leaf_differ() {
        let mut graph = DockGraph::new();
        let leaf_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("a")],
            active: 0,
        });
        let sibling = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("b")],
            active: 0,
        });
        let root = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Horizontal,
            children: vec![leaf_tabs, sibling],
            fractions: vec![0.5, 0.5],
        });
        let root_bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(600.0), px(400.0)));
        let leaf_bounds = Bounds::new(point(px(10.0), px(20.0)), size(px(280.0), px(360.0)));
        let target = resolve_layout_drop(DockDropResolverInput {
            root: Some(DockRootDropTarget {
                root,
                bounds: root_bounds,
            }),
            leaves: &[DockLeafDropTarget {
                root,
                target_tabs: leaf_tabs,
                bounds: leaf_bounds,
                is_central: false,
            }],
            ..DockDropResolverInput::new(point(px(12.0), px(180.0)), &policy())
        })
        .and_then(DockDropResolution::target)
        .expect("root edge should resolve");

        assert_eq!(target.source, DockDropResolveSource::RootEdge);
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::RootEdge {
                root,
                leaf_tabs,
                zone: DropZone::Left,
            }
        );
        assert_eq!(
            target.preview_intent().map(|intent| intent.target_tabs),
            Some(root),
            "outer edge commits against the root node"
        );
    }

    #[test]
    fn empty_dock_space_resolves_without_tabs_node() {
        let space = DockSpaceId::from("empty");
        let target = resolve_layout_drop(DockDropResolverInput {
            empty_spaces: &[DockEmptySpaceDropTarget {
                space: space.clone(),
                bounds: bounds(300.0, 200.0),
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
        assert!(target.preview_intent().is_none());
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
        assert_eq!(
            target.preview_intent().map(|intent| intent.zone),
            Some(DropZone::Center)
        );
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
    fn central_edge_splits_do_not_use_central_dock_over_policy() {
        let root = tabs();
        let mut policy = DockPolicy::default();
        policy.set_allow_central_region_dock_over(false);
        let target = resolve_layout_drop(DockDropResolverInput {
            leaves: &[DockLeafDropTarget {
                is_central: true,
                ..leaf(root, root)
            }],
            ..DockDropResolverInput::new(point(px(12.0), px(120.0)), &policy)
        })
        .and_then(DockDropResolution::target)
        .expect("central inner edge should still be accepted");

        assert_eq!(target.source, DockDropResolveSource::InnerEdge);
        assert_eq!(target.zone(), Some(DropZone::Left));

        policy.set_allow_edge_split(false);
        let DockDropResolution::Rejected(rejection) = resolve_layout_drop(DockDropResolverInput {
            leaves: &[DockLeafDropTarget {
                is_central: true,
                ..leaf(root, root)
            }],
            ..DockDropResolverInput::new(point(px(12.0), px(120.0)), &policy)
        })
        .expect("central inner edge should still resolve to edge policy") else {
            panic!("disabled edge split should reject central inner edge");
        };
        assert_eq!(rejection.reason, DockPolicyError::EdgeSplitDisabled);
    }

    #[test]
    fn viewport_hit_resolves_to_known_viewport_with_host_position() {
        let hit = DockViewportHit {
            space: DockSpaceId::from("secondary"),
            host_position: point(px(12.0), px(18.0)),
        };
        let target = resolve_layout_drop(DockDropResolverInput {
            known_viewport: Some(hit.clone()),
            ..DockDropResolverInput::new(point(px(900.0), px(700.0)), &policy())
        })
        .and_then(DockDropResolution::target)
        .expect("known viewport should resolve");

        assert_eq!(target.source, DockDropResolveSource::KnownViewport);
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::KnownViewport { hit }
        );
    }

    #[test]
    fn tear_off_candidate_respects_platform_viewport_policy() {
        let release_position = point(px(900.0), px(700.0));
        let suggested_window_bounds = WindowBounds::Windowed(Bounds::new(
            point(px(880.0), px(680.0)),
            size(px(360.0), px(240.0)),
        ));
        let candidate = DockTearOffCandidateDropTarget {
            release_position,
            suggested_window_bounds: Some(suggested_window_bounds),
        };

        let disabled = resolve_layout_drop(DockDropResolverInput {
            tear_off_candidate: Some(candidate.clone()),
            ..DockDropResolverInput::new(release_position, &policy())
        })
        .expect("tear-off candidate should resolve to a policy decision");
        assert!(matches!(
            disabled,
            DockDropResolution::Rejected(DockDropRejection {
                reason: DockPolicyError::PlatformViewportsDisabled,
                ..
            })
        ));

        let mut policy = DockPolicy::default();
        policy.set_allow_platform_viewports(true);
        let target = resolve_layout_drop(DockDropResolverInput {
            tear_off_candidate: Some(candidate),
            ..DockDropResolverInput::new(release_position, &policy)
        })
        .and_then(DockDropResolution::target)
        .expect("enabled tear-off should resolve");

        assert_eq!(target.source, DockDropResolveSource::TearOffCandidate);
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::TearOffCandidate {
                release_position,
                suggested_window_bounds: Some(suggested_window_bounds),
            }
        );
    }
}
