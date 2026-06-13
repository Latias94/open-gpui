use crate::{
    DockNodeId, DockPolicy,
    drop_target::{
        self, DockDropResolution, DockDropResolverInput, DockDropTargetValidator,
        DockEmptySpaceDropTarget, DockFloatingTitleBarDropTarget, DockLeafDropTarget,
        DockResolvedDropTarget, DockRootDropTarget, DockTabBarDropTarget, DockTabLabelDropTarget,
    },
};
use open_gpui::{Bounds, Pixels, Point};

#[derive(Debug, Clone, Copy, PartialEq)]
struct DockTabReorderHold {
    target_tabs: DockNodeId,
    bounds: Bounds<Pixels>,
}

#[derive(Debug, Default)]
pub(crate) struct DockDropRuntime {
    resolution: Option<DockDropResolution>,
    scene: Option<DockHostDropScene>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockHostDropScene {
    pub(crate) position: Point<Pixels>,
    excluded_node: Option<DockNodeId>,
    pub(crate) tab_labels: Vec<DockTabLabelDropTarget>,
    pub(crate) tab_bars: Vec<DockTabBarDropTarget>,
    pub(crate) leaves: Vec<DockLeafDropTarget>,
    pub(crate) root: Option<DockRootDropTarget>,
    pub(crate) floating_title_bars: Vec<DockFloatingTitleBarDropTarget>,
    pub(crate) empty_spaces: Vec<DockEmptySpaceDropTarget>,
    pub(crate) clear_on_miss: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockHostDropSceneFact {
    TabLabel(DockTabLabelDropTarget),
    TabBar(DockTabBarDropTarget),
    Leaf(DockLeafDropTarget),
    Root(DockRootDropTarget),
    FloatingTitleBar(DockFloatingTitleBarDropTarget),
    EmptySpace(DockEmptySpaceDropTarget),
}

impl DockHostDropScene {
    pub(crate) fn new(position: Point<Pixels>) -> Self {
        Self {
            position,
            excluded_node: None,
            tab_labels: Vec::new(),
            tab_bars: Vec::new(),
            leaves: Vec::new(),
            root: None,
            floating_title_bars: Vec::new(),
            empty_spaces: Vec::new(),
            clear_on_miss: true,
        }
    }

    pub(crate) fn excluding_node(mut self, node: Option<DockNodeId>) -> Self {
        self.excluded_node = node;
        self
    }

    pub(crate) fn push_fact(&mut self, fact: DockHostDropSceneFact) {
        if self
            .excluded_node
            .is_some_and(|node| fact.targets_node(node))
        {
            return;
        }

        match fact {
            DockHostDropSceneFact::TabLabel(target) => self.tab_labels.push(target),
            DockHostDropSceneFact::TabBar(target) => self.tab_bars.push(target),
            DockHostDropSceneFact::Leaf(target) => self.leaves.push(target),
            DockHostDropSceneFact::Root(target) => self.root = Some(target),
            DockHostDropSceneFact::FloatingTitleBar(target) => {
                self.floating_title_bars.push(target);
            }
            DockHostDropSceneFact::EmptySpace(target) => self.empty_spaces.push(target),
        }
    }

    #[cfg(test)]
    pub(crate) fn preserve_on_miss(mut self) -> Self {
        self.clear_on_miss = false;
        self
    }

    pub(crate) fn resolve_drop_with_validator(
        &self,
        policy: &DockPolicy,
        target_validator: Option<&DockDropTargetValidator<'_>>,
    ) -> Option<DockDropResolution> {
        drop_target::resolve_layout_drop(DockDropResolverInput {
            position: self.position,
            policy,
            target_validator,
            tab_labels: &self.tab_labels,
            tab_bars: &self.tab_bars,
            leaves: &self.leaves,
            root: self.root,
            floating_title_bars: &self.floating_title_bars,
            empty_spaces: &self.empty_spaces,
        })
    }
}

impl DockHostDropSceneFact {
    fn targets_node(&self, node: DockNodeId) -> bool {
        match self {
            DockHostDropSceneFact::TabLabel(target) => target.target_tabs == node,
            DockHostDropSceneFact::TabBar(target) => target.target_tabs == node,
            DockHostDropSceneFact::Leaf(target) => {
                target.root == node || target.target_tabs == node
            }
            DockHostDropSceneFact::Root(target) => target.root == node,
            DockHostDropSceneFact::FloatingTitleBar(target) => {
                target.floating == node || target.target_tabs == node
            }
            DockHostDropSceneFact::EmptySpace(_) => false,
        }
    }
}

impl DockDropRuntime {
    #[cfg(test)]
    pub(crate) fn begin_scene(&mut self, scene: DockHostDropScene, policy: &DockPolicy) -> bool {
        self.begin_scene_with_validator(scene, policy, None)
    }

    pub(crate) fn begin_scene_with_validator(
        &mut self,
        scene: DockHostDropScene,
        policy: &DockPolicy,
        target_validator: Option<&DockDropTargetValidator<'_>>,
    ) -> bool {
        let changed = self.resolve_scene(&scene, policy, target_validator);
        self.scene = Some(scene);
        changed
    }

    #[cfg(test)]
    pub(crate) fn push_scene_fact(
        &mut self,
        position: Point<Pixels>,
        excluded_node: Option<DockNodeId>,
        fact: DockHostDropSceneFact,
        policy: &DockPolicy,
    ) -> bool {
        self.push_scene_fact_with_validator(position, excluded_node, fact, policy, None)
    }

    pub(crate) fn push_scene_fact_with_validator(
        &mut self,
        position: Point<Pixels>,
        excluded_node: Option<DockNodeId>,
        fact: DockHostDropSceneFact,
        policy: &DockPolicy,
        target_validator: Option<&DockDropTargetValidator<'_>>,
    ) -> bool {
        let scene = self.scene_for_position(position, excluded_node);
        scene.push_fact(fact);
        let scene = scene.clone();
        self.resolve_scene(&scene, policy, target_validator)
    }

    fn resolve_scene(
        &mut self,
        scene: &DockHostDropScene,
        policy: &DockPolicy,
        target_validator: Option<&DockDropTargetValidator<'_>>,
    ) -> bool {
        let mut resolution = match scene.resolve_drop_with_validator(policy, target_validator) {
            Some(resolution) => Some(resolution),
            None if scene.clear_on_miss => None,
            None => return false,
        };
        if let Some(existing) = self.resolution.as_ref().and_then(valid_target)
            && let Some(reorder_hold) = tab_reorder_hold(existing)
            && reorder_hold.bounds.contains(&scene.position)
            && resolution
                .as_ref()
                .and_then(valid_target)
                .and_then(DockResolvedDropTarget::center_target_tabs)
                .is_some_and(|target_tabs| target_tabs == reorder_hold.target_tabs)
        {
            resolution = Some(DockDropResolution::Valid(existing.clone()));
        }
        self.replace_resolution(resolution)
    }

    pub(crate) fn take_resolved_target(&mut self) -> Option<DockResolvedDropTarget> {
        self.scene = None;
        self.resolution.take().and_then(DockDropResolution::target)
    }

    pub(crate) fn drop_resolution(&self) -> Option<&DockDropResolution> {
        self.resolution.as_ref()
    }

    fn scene_for_position(
        &mut self,
        position: Point<Pixels>,
        excluded_node: Option<DockNodeId>,
    ) -> &mut DockHostDropScene {
        let should_reset = self
            .scene
            .as_ref()
            .is_none_or(|scene| scene.position != position || scene.excluded_node != excluded_node);
        if should_reset {
            self.scene = Some(DockHostDropScene::new(position).excluding_node(excluded_node));
        }
        self.scene.as_mut().expect("scene should be initialized")
    }

    fn replace_resolution(&mut self, resolution: Option<DockDropResolution>) -> bool {
        if self.resolution == resolution {
            return false;
        }
        self.resolution = resolution;
        true
    }

    #[cfg(test)]
    pub(crate) fn resolved_target(&self) -> Option<&DockResolvedDropTarget> {
        self.resolution.as_ref().and_then(resolution_target)
    }
}

pub(crate) fn resolution_target(
    resolution: &DockDropResolution,
) -> Option<&DockResolvedDropTarget> {
    match resolution {
        DockDropResolution::Valid(target) => Some(target),
        DockDropResolution::Rejected(rejection) => Some(&rejection.target),
    }
}

fn valid_target(resolution: &DockDropResolution) -> Option<&DockResolvedDropTarget> {
    match resolution {
        DockDropResolution::Valid(target) => Some(target),
        DockDropResolution::Rejected(_) => None,
    }
}

fn tab_reorder_hold(target: &DockResolvedDropTarget) -> Option<DockTabReorderHold> {
    let drop_target::DockResolvedDropTargetKind::TabBar {
        target_tabs,
        insert_index: _,
    } = target.kind
    else {
        return None;
    };

    Some(DockTabReorderHold {
        target_tabs,
        bounds: target.preview_bounds?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockPolicyError, DropZone,
        drop_target::{DockDropResolveSource, DockResolvedDropTargetKind},
        geometry::{self, DockDropBoxKind, DockDropBoxSet},
    };
    use open_gpui::{point, px, size};
    use slotmap::Key;

    fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
    }

    fn resolved_tab_insert_index(runtime: &DockDropRuntime) -> Option<usize> {
        let DockResolvedDropTargetKind::TabBar { insert_index, .. } =
            runtime.resolved_target()?.kind
        else {
            return None;
        };
        Some(insert_index)
    }

    fn root_edge_leaf_bounds_and_position(zone: DropZone) -> (Bounds<Pixels>, Point<Pixels>) {
        let root_bounds = bounds(0.0, 0.0, 400.0, 240.0);
        let position = geometry::drop_boxes(root_bounds, DockDropBoxSet::Outer)
            .into_iter()
            .find(|drop_box| drop_box.kind == DockDropBoxKind::OuterEdge(zone))
            .map(|drop_box| drop_box.hit_bounds.center())
            .unwrap_or_else(|| panic!("{zone:?} outer box should exist"));
        let leaf_bounds = Bounds::new(
            point(position.x - px(60.0), position.y - px(60.0)),
            size(px(120.0), px(120.0)),
        );
        (leaf_bounds, position)
    }

    fn leaf_center_position(bounds: Bounds<Pixels>) -> Point<Pixels> {
        geometry::drop_boxes(bounds, DockDropBoxSet::Inner)
            .into_iter()
            .find(|drop_box| drop_box.kind == DockDropBoxKind::Center)
            .map(|drop_box| drop_box.hit_bounds.center())
            .expect("center box should exist")
    }

    #[test]
    fn tab_reorder_drop_updates_target_only_inside_tab_bounds() {
        let tabs = DockNodeId::null();
        let mut runtime = DockDropRuntime::default();
        let position = point(px(95.0), px(28.0));

        runtime.begin_scene(DockHostDropScene::new(position), &DockPolicy::default());
        assert!(runtime.push_scene_fact(
            position,
            None,
            DockHostDropSceneFact::TabLabel(DockTabLabelDropTarget {
                target_tabs: tabs,
                target_index: 2,
                bounds: bounds(10.0, 20.0, 100.0, 24.0),
                is_central: false,
            }),
            &DockPolicy::default()
        ));
        assert_eq!(resolved_tab_insert_index(&runtime), Some(3));

        assert!(!runtime.begin_scene(
            DockHostDropScene::new(point(px(200.0), px(28.0))).preserve_on_miss(),
            &DockPolicy::default()
        ));
        assert_eq!(resolved_tab_insert_index(&runtime), Some(3));
    }

    #[test]
    fn tabs_drop_preserves_reorder_target_while_pointer_stays_inside_tab() {
        let tabs = DockNodeId::null();
        let mut runtime = DockDropRuntime::default();
        let position = point(px(95.0), px(108.0));

        runtime.begin_scene(DockHostDropScene::new(position), &DockPolicy::default());
        assert!(runtime.push_scene_fact(
            position,
            None,
            DockHostDropSceneFact::TabLabel(DockTabLabelDropTarget {
                target_tabs: tabs,
                target_index: 2,
                bounds: bounds(10.0, 100.0, 100.0, 24.0),
                is_central: false,
            }),
            &DockPolicy::default()
        ));
        assert!(!runtime.push_scene_fact(
            position,
            None,
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: tabs,
                target_tabs: tabs,
                bounds: bounds(0.0, 0.0, 400.0, 400.0),
                is_central: false,
            }),
            &DockPolicy::default()
        ));

        let target = runtime
            .take_resolved_target()
            .expect("reorder target should remain available");
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::TabBar {
                target_tabs: tabs,
                insert_index: 3,
            }
        );
        assert!(runtime.resolved_target().is_none());
    }

    #[test]
    fn reorder_target_keeps_insert_index_in_resolved_target() {
        let tabs = DockNodeId::null();
        let mut runtime = DockDropRuntime::default();
        let position = point(px(20.0), px(28.0));

        runtime.begin_scene(DockHostDropScene::new(position), &DockPolicy::default());
        assert!(runtime.push_scene_fact(
            position,
            None,
            DockHostDropSceneFact::TabLabel(DockTabLabelDropTarget {
                target_tabs: tabs,
                target_index: 0,
                bounds: bounds(10.0, 20.0, 100.0, 24.0),
                is_central: false,
            }),
            &DockPolicy::default()
        ));

        assert_eq!(resolved_tab_insert_index(&runtime), Some(0));
    }

    #[test]
    fn runtime_resolves_multi_fact_update_through_layout_resolver() {
        let tabs = DockNodeId::null();
        let mut runtime = DockDropRuntime::default();
        let leaf_bounds = bounds(0.0, 0.0, 400.0, 400.0);
        let position = leaf_center_position(leaf_bounds);

        runtime.begin_scene(DockHostDropScene::new(position), &DockPolicy::default());
        assert!(runtime.push_scene_fact(
            position,
            None,
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: tabs,
                target_tabs: tabs,
                bounds: leaf_bounds,
                is_central: false,
            }),
            &DockPolicy::default()
        ));
        assert!(runtime.push_scene_fact(
            position,
            None,
            DockHostDropSceneFact::TabLabel(DockTabLabelDropTarget {
                target_tabs: tabs,
                target_index: 2,
                bounds: bounds(140.0, 188.0, 100.0, 24.0),
                is_central: false,
            }),
            &DockPolicy::default()
        ));

        let target = runtime
            .take_resolved_target()
            .expect("multi-fact update should resolve");
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::TabBar {
                target_tabs: tabs,
                insert_index: 3,
            }
        );
    }

    #[test]
    fn stack_drop_excludes_source_tabs_during_scene_resolution() {
        let mut graph = crate::DockGraph::new();
        let source_tabs = graph.insert_node(crate::DockNode::Tabs {
            items: vec![crate::DockItemId::from("source")],
            selected: Some(crate::DockItemId::from("source")),
        });
        let target_tabs = graph.insert_node(crate::DockNode::Tabs {
            items: vec![crate::DockItemId::from("target")],
            selected: Some(crate::DockItemId::from("target")),
        });
        let mut runtime = DockDropRuntime::default();
        let leaf_bounds = bounds(0.0, 0.0, 400.0, 240.0);
        let position = geometry::drop_boxes(leaf_bounds, DockDropBoxSet::Inner)
            .into_iter()
            .find(|drop_box| drop_box.kind == DockDropBoxKind::Center)
            .map(|drop_box| drop_box.hit_bounds.center())
            .expect("center box should exist");

        runtime.begin_scene(
            DockHostDropScene::new(position).excluding_node(Some(source_tabs)),
            &DockPolicy::default(),
        );
        assert!(runtime.push_scene_fact(
            position,
            Some(source_tabs),
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: target_tabs,
                target_tabs,
                bounds: leaf_bounds,
                is_central: false,
            }),
            &DockPolicy::default()
        ));
        assert!(!runtime.push_scene_fact(
            position,
            Some(source_tabs),
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: source_tabs,
                target_tabs: source_tabs,
                bounds: bounds(80.0, 40.0, 220.0, 140.0),
                is_central: false,
            }),
            &DockPolicy::default()
        ));

        let target = runtime
            .take_resolved_target()
            .expect("underlying target should remain after excluding source tabs");
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::LeafCenter {
                root: target_tabs,
                target_tabs,
            }
        );
    }

    #[test]
    fn floating_drop_excludes_source_floating_during_scene_resolution() {
        let mut graph = crate::DockGraph::new();
        let floating_tabs = graph.insert_node(crate::DockNode::Tabs {
            items: vec![crate::DockItemId::from("floating")],
            selected: Some(crate::DockItemId::from("floating")),
        });
        let floating = graph.insert_node(crate::DockNode::Floating {
            child: floating_tabs,
        });
        let target_tabs = graph.insert_node(crate::DockNode::Tabs {
            items: vec![crate::DockItemId::from("target")],
            selected: Some(crate::DockItemId::from("target")),
        });
        let mut runtime = DockDropRuntime::default();
        let leaf_bounds = bounds(0.0, 0.0, 400.0, 240.0);
        let position = geometry::drop_boxes(leaf_bounds, DockDropBoxSet::Inner)
            .into_iter()
            .find(|drop_box| drop_box.kind == DockDropBoxKind::Center)
            .map(|drop_box| drop_box.hit_bounds.center())
            .expect("center box should exist");

        runtime.begin_scene(
            DockHostDropScene::new(position).excluding_node(Some(floating)),
            &DockPolicy::default(),
        );
        assert!(!runtime.push_scene_fact(
            position,
            Some(floating),
            DockHostDropSceneFact::FloatingTitleBar(DockFloatingTitleBarDropTarget {
                floating,
                target_tabs: floating_tabs,
                title_bounds: bounds(0.0, 0.0, 400.0, 240.0),
                preview_bounds: bounds(0.0, 0.0, 400.0, 240.0),
            }),
            &DockPolicy::default()
        ));
        runtime.push_scene_fact(
            position,
            Some(floating),
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: target_tabs,
                target_tabs,
                bounds: leaf_bounds,
                is_central: false,
            }),
            &DockPolicy::default(),
        );

        let target = runtime
            .take_resolved_target()
            .expect("target leaf should resolve");
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::LeafCenter {
                root: target_tabs,
                target_tabs,
            }
        );
    }

    #[test]
    fn root_edge_targets_can_be_taken_without_receiver_bounds() {
        let root = DockNodeId::null();
        let mut graph = crate::DockGraph::new();
        let leaf = graph.insert_node(crate::DockNode::Tabs {
            items: vec![crate::DockItemId::from("a")],
            selected: Some(crate::DockItemId::from("a")),
        });

        for zone in [
            DropZone::Left,
            DropZone::Right,
            DropZone::Top,
            DropZone::Bottom,
        ] {
            let mut runtime = DockDropRuntime::default();
            let (leaf_bounds, position) = root_edge_leaf_bounds_and_position(zone);

            runtime.begin_scene(DockHostDropScene::new(position), &DockPolicy::default());
            assert!(runtime.push_scene_fact(
                position,
                None,
                DockHostDropSceneFact::Root(crate::drop_target::DockRootDropTarget {
                    root,
                    bounds: bounds(0.0, 0.0, 400.0, 240.0),
                }),
                &DockPolicy::default()
            ));
            let root_only_target = runtime
                .resolved_target()
                .unwrap_or_else(|| panic!("{zone:?} root-only target should resolve"));
            assert!(
                matches!(
                    root_only_target.kind,
                    DockResolvedDropTargetKind::RootEdge {
                        root: matched_root,
                        leaf_tabs: None,
                        zone: matched_zone,
                    } if matched_root == root && matched_zone == zone
                ),
                "{zone:?}: unexpected root-only target {:?}",
                root_only_target
            );

            assert!(runtime.push_scene_fact(
                position,
                None,
                DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                    root,
                    target_tabs: leaf,
                    bounds: leaf_bounds,
                    is_central: false,
                }),
                &DockPolicy::default()
            ));

            let target = runtime
                .take_resolved_target()
                .unwrap_or_else(|| panic!("{zone:?} root-edge target should resolve"));
            assert_eq!(target.source, DockDropResolveSource::RootEdge, "{zone:?}");
            assert!(
                matches!(
                    target.kind,
                    DockResolvedDropTargetKind::RootEdge {
                        root: matched_root,
                        leaf_tabs: Some(leaf_tabs),
                        zone: matched_zone,
                    } if matched_root == root && leaf_tabs == leaf && matched_zone == zone
                ),
                "{zone:?}: unexpected target {:?}",
                target
            );
        }
    }

    #[test]
    fn empty_space_target_can_be_taken_without_receiver_bounds() {
        let space = crate::DockSpaceId::from("empty");
        let mut runtime = DockDropRuntime::default();
        let position = point(px(40.0), px(40.0));

        runtime.begin_scene(DockHostDropScene::new(position), &DockPolicy::default());
        assert!(runtime.push_scene_fact(
            position,
            None,
            DockHostDropSceneFact::EmptySpace(DockEmptySpaceDropTarget {
                space: space.clone(),
                bounds: bounds(0.0, 0.0, 400.0, 240.0),
                is_central: false,
            }),
            &DockPolicy::default()
        ));

        let target = runtime
            .take_resolved_target()
            .expect("empty-space target should resolve without receiver bounds");
        assert!(matches!(
            target.kind,
            DockResolvedDropTargetKind::EmptyDockSpace { space: target_space, .. } if target_space == space
        ));
    }

    #[test]
    fn central_empty_space_target_records_rejected_resolution() {
        let space = crate::DockSpaceId::from("central");
        let mut runtime = DockDropRuntime::default();
        let position = point(px(40.0), px(40.0));
        let mut policy = DockPolicy::default();
        policy.set_allow_central_region_dock_over(false);

        runtime.begin_scene(DockHostDropScene::new(position), &policy);
        assert!(runtime.push_scene_fact(
            position,
            None,
            DockHostDropSceneFact::EmptySpace(DockEmptySpaceDropTarget {
                space: space.clone(),
                bounds: bounds(0.0, 0.0, 400.0, 240.0),
                is_central: true,
            }),
            &policy
        ));

        let DockDropResolution::Rejected(rejection) = runtime
            .drop_resolution()
            .expect("central empty-space target should resolve to a policy decision")
        else {
            panic!("central empty-space dock-over should be rejected");
        };
        assert_eq!(
            rejection.reason,
            DockPolicyError::CentralRegionDockOverDisabled
        );
        assert!(matches!(
            rejection.target.kind,
            DockResolvedDropTargetKind::EmptyDockSpace {
                space: ref target_space,
                is_central: true,
            } if target_space == &space
        ));
    }
}
