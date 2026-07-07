use open_gpui::{Bounds, Pixels, point, px, size};

mod availability;
mod candidate;
mod edge;
mod model;

pub(crate) use availability::validate_resolved_drop_target;
use candidate::{
    DockDropCandidate, choose_drop_candidate, push_drop_candidate, push_prioritized_drop_candidate,
};
use edge::{
    best_leaf_containing, best_leaf_for_root_containing, leaf_bounds_for_tabs, leaf_guide_target,
    resolve_leaf_drop, resolve_root_edge_drop, root_guide_target,
};
pub(crate) use model::{
    DockDropRejection, DockDropResolution, DockDropResolveSource, DockDropResolverInput,
    DockDropTargetKey, DockDropTargetValidator, DockEdgePlanResolver, DockEmptySpaceDropTarget,
    DockFloatingTitleBarDropTarget, DockLeafDropTarget, DockResolvedDropTarget,
    DockResolvedDropTargetAvailability, DockResolvedDropTargetKind, DockRootDropTarget,
    DockTabBarDropTarget, DockTabLabelDropTarget,
};

pub(crate) fn resolve_layout_drop(input: DockDropResolverInput<'_>) -> Option<DockDropResolution> {
    let candidates = collect_drop_candidates(&input);
    choose_drop_candidate(candidates, input.policy, input.target_validator)
}

pub(crate) fn resolve_layout_drop_guide(
    input: DockDropResolverInput<'_>,
) -> Option<DockResolvedDropTarget> {
    let target = resolve_layout_drop_guide_target(&input)?;
    let target = match validate_resolved_drop_target(target, input.policy, input.target_validator) {
        DockDropResolution::Valid(target) => target,
        DockDropResolution::Rejected(rejection) => rejection.target,
    };
    target.availability.any().then_some(target)
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
                target_bounds: Some(target.bounds),
                inner_target_bounds: None,
                availability: DockResolvedDropTargetAvailability::all(),
                drop_box: None,
                hit_bounds: Some(target.bounds),
                preview_bounds: Some(target.bounds),
                tab_insertion_bounds: None,
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
        let target_bounds =
            leaf_bounds_for_tabs(input.leaves, target.target_tabs).unwrap_or(target.bounds);
        push_drop_candidate(
            &mut candidates,
            &mut order,
            DockResolvedDropTarget {
                kind: DockResolvedDropTargetKind::TabBar {
                    target_tabs: target.target_tabs,
                    insert_index: target.insert_index,
                },
                source: DockDropResolveSource::TabBar,
                target_bounds: Some(target_bounds),
                inner_target_bounds: None,
                availability: DockResolvedDropTargetAvailability::all(),
                drop_box: None,
                hit_bounds: Some(target.bounds),
                preview_bounds: Some(target_bounds),
                tab_insertion_bounds: None,
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
        let target_bounds =
            leaf_bounds_for_tabs(input.leaves, target.target_tabs).unwrap_or(target.bounds);
        let insert_index = if input.position.x < target.bounds.center().x {
            target.target_index
        } else {
            target.target_index.saturating_add(1)
        };
        let tab_insertion_x = if insert_index == target.target_index {
            target.bounds.origin.x
        } else {
            target.bounds.right()
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
                target_bounds: Some(target_bounds),
                inner_target_bounds: None,
                availability: DockResolvedDropTargetAvailability::all(),
                drop_box: None,
                hit_bounds: Some(target.bounds),
                preview_bounds: Some(target_bounds),
                tab_insertion_bounds: Some(tab_insertion_slot_bounds(
                    tab_insertion_x,
                    target.bounds,
                )),
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
                target_bounds: Some(target.preview_bounds),
                inner_target_bounds: None,
                availability: DockResolvedDropTargetAvailability::all(),
                drop_box: None,
                hit_bounds: Some(target.title_bounds),
                preview_bounds: Some(target.preview_bounds),
                tab_insertion_bounds: None,
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
        push_prioritized_drop_candidate(&mut candidates, &mut order, target, hit_bounds, 1);
    }

    candidates
}

fn tab_insertion_slot_bounds(insertion_x: Pixels, target_bounds: Bounds<Pixels>) -> Bounds<Pixels> {
    let width = px(3.0);
    Bounds::new(
        point(insertion_x - width / 2.0, target_bounds.origin.y),
        size(width, target_bounds.size.height),
    )
}

fn resolve_layout_drop_guide_target(
    input: &DockDropResolverInput<'_>,
) -> Option<DockResolvedDropTarget> {
    let leaf = best_leaf_containing(input.leaves, input.position);
    if let Some(root) = input.root
        && root.bounds.contains(&input.position)
    {
        let leaf_for_root = leaf.filter(|leaf| leaf.root == root.root);
        if leaf_for_root.is_none_or(|leaf| leaf.is_central && leaf.root == leaf.target_tabs) {
            return Some(root_guide_target(root, leaf_for_root));
        }
    }
    leaf.map(leaf_guide_target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockEdgeDockPlan, DockGraph, DockItemId, DockNode, DockNodeId, DockPolicy, DockPolicyError,
        DockSpaceId, DropZone, SplitAxis,
        geometry::{self, DockDropBoxKind, DockDropBoxSet},
    };
    use open_gpui::{Point, point, px, size};
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
    fn leaf_interior_outside_drop_boxes_resolves_guide_target_only() {
        let mut graph = DockGraph::new();
        let root = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("root")],
            selected: Some(DockItemId::from("root")),
        });
        let leaf_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("leaf")],
            selected: Some(DockItemId::from("leaf")),
        });
        let position = point(px(754.9751), px(583.56213));
        let root_bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(920.0), px(640.0)));
        let leaf_bounds = Bounds::new(point(px(222.0), px(436.0)), size(px(697.0), px(203.0)));
        let leaves = [DockLeafDropTarget {
            root,
            target_tabs: leaf_tabs,
            bounds: leaf_bounds,
            is_central: false,
        }];
        let root_target = DockRootDropTarget {
            root,
            bounds: root_bounds,
        };

        assert!(
            resolve_layout_drop(DockDropResolverInput {
                leaves: &leaves,
                root: Some(root_target),
                ..DockDropResolverInput::new(position, &policy())
            })
            .is_none(),
            "the interior point should not mint a concrete delivery target"
        );

        let guide = resolve_layout_drop_guide(DockDropResolverInput {
            leaves: &leaves,
            root: Some(root_target),
            ..DockDropResolverInput::new(position, &policy())
        })
        .expect("the same point should still expose the containing leaf as a guide target");

        assert_eq!(
            guide.kind,
            DockResolvedDropTargetKind::LeafCenter {
                root,
                target_tabs: leaf_tabs,
            }
        );
        assert_eq!(guide.drop_box, None);
        assert_eq!(guide.preview_bounds, Some(leaf_bounds));
        assert!(guide.availability.center);
        assert!(guide.availability.sides);
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

    #[test]
    fn leaf_edge_plan_wraps_leaf_below_opposing_axis_parent() {
        let space = DockSpaceId::from("main");
        let mut graph = DockGraph::new();
        let left_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("left")],
            selected: Some(DockItemId::from("left")),
        });
        let top_right_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("top-right")],
            selected: Some(DockItemId::from("top-right")),
        });
        let bottom_right_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("bottom-right")],
            selected: Some(DockItemId::from("bottom-right")),
        });
        let right_split = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Vertical,
            children: vec![top_right_tabs, bottom_right_tabs],
            fractions: vec![0.5, 0.5],
        });
        let root = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Horizontal,
            children: vec![left_tabs, right_split],
            fractions: vec![0.5, 0.5],
        });
        graph.set_root(space.clone(), root);

        let bounds = bounds(300.0, 200.0);
        let target = resolve_layout_drop(DockDropResolverInput {
            leaves: &[DockLeafDropTarget {
                root,
                target_tabs: bottom_right_tabs,
                bounds,
                is_central: false,
            }],
            edge_plan_resolver: Some(&|target, zone, sizing| {
                graph.edge_dock_plan_with_sizing(&space, target, zone, sizing)
            }),
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

        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::InnerEdge {
                root,
                target_tabs: bottom_right_tabs,
                zone: DropZone::Left,
            }
        );
        assert_eq!(
            target.edge_plan,
            Some(DockEdgeDockPlan::WrapTarget {
                target: bottom_right_tabs,
                axis: SplitAxis::Horizontal,
                zone: DropZone::Left,
                sizing: target.edge_sizing.expect("edge target should carry sizing"),
            })
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
    fn explicit_root_outer_edges_override_smaller_leaf_targets() {
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
            .unwrap_or_else(|| panic!("{zone:?} root outer target should resolve"));

            assert_eq!(target.source, DockDropResolveSource::RootEdge, "{zone:?}");
            assert_eq!(
                target.kind,
                DockResolvedDropTargetKind::RootEdge {
                    root,
                    leaf_tabs: Some(leaf_tabs),
                    zone,
                },
                "{zone:?}"
            );
            assert_eq!(target.zone(), Some(zone), "{zone:?}");
            assert_eq!(target.target_bounds, Some(root_bounds()), "{zone:?}");
            assert_eq!(target.inner_target_bounds, Some(leaf_bounds), "{zone:?}");
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
    fn explicit_root_outer_hit_beats_smaller_foreign_leaf_candidate() {
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
        .expect("explicit root outer edge should resolve");

        assert_eq!(target.source, DockDropResolveSource::RootEdge);
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::RootEdge {
                root,
                leaf_tabs: Some(leaf_tabs),
                zone: DropZone::Left,
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
    fn root_central_leaf_side_hit_does_not_create_inner_edge_target() {
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
    fn nested_central_leaf_side_hit_creates_inner_edge_target() {
        let mut graph = DockGraph::new();
        let central_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("central")],
            selected: Some(DockItemId::from("central")),
        });
        let sibling = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("sibling")],
            selected: Some(DockItemId::from("sibling")),
        });
        let root = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Horizontal,
            children: vec![central_tabs, sibling],
            fractions: vec![0.5, 0.5],
        });
        let leaf_bounds = bounds(300.0, 200.0);
        let position = drop_box_center(
            leaf_bounds,
            DockDropBoxSet::Inner,
            DockDropBoxKind::InnerEdge(DropZone::Left),
        );

        let target = resolve_layout_drop(DockDropResolverInput {
            leaves: &[DockLeafDropTarget {
                root,
                target_tabs: central_tabs,
                bounds: leaf_bounds,
                is_central: true,
            }],
            ..DockDropResolverInput::new(position, &policy())
        })
        .and_then(DockDropResolution::target)
        .expect("nested central side hit should still resolve an inner-edge target");

        assert_eq!(target.source, DockDropResolveSource::InnerEdge);
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::InnerEdge {
                root,
                target_tabs: central_tabs,
                zone: DropZone::Left,
            }
        );
    }

    #[test]
    fn explicit_root_outer_hit_prefers_root_over_central_leaf_body() {
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
        .expect("central side hit should resolve to the root outer edge");

        assert_eq!(target.source, DockDropResolveSource::RootEdge);
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::RootEdge {
                root,
                leaf_tabs: Some(central_tabs),
                zone,
            }
        );
    }
}
