use crate::{
    DockNodeId, DockPolicy,
    drop_target::{
        self, DockDropResolution, DockDropResolverInput, DockDropTargetValidator,
        DockEmptySpaceDropTarget, DockFloatingTitleBarDropTarget, DockLeafDropTarget,
        DockResolvedDropTarget, DockRootDropTarget, DockTabLabelDropTarget,
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
    excluded_tabs: Option<DockNodeId>,
    pub(crate) tab_labels: Vec<DockTabLabelDropTarget>,
    pub(crate) leaves: Vec<DockLeafDropTarget>,
    pub(crate) root: Option<DockRootDropTarget>,
    pub(crate) floating_title_bars: Vec<DockFloatingTitleBarDropTarget>,
    pub(crate) empty_spaces: Vec<DockEmptySpaceDropTarget>,
    pub(crate) clear_on_miss: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockHostDropSceneFact {
    TabLabel(DockTabLabelDropTarget),
    Leaf(DockLeafDropTarget),
    Root(DockRootDropTarget),
    FloatingTitleBar(DockFloatingTitleBarDropTarget),
    EmptySpace(DockEmptySpaceDropTarget),
}

impl DockHostDropScene {
    pub(crate) fn new(position: Point<Pixels>) -> Self {
        Self {
            position,
            excluded_tabs: None,
            tab_labels: Vec::new(),
            leaves: Vec::new(),
            root: None,
            floating_title_bars: Vec::new(),
            empty_spaces: Vec::new(),
            clear_on_miss: true,
        }
    }

    pub(crate) fn excluding_tabs(mut self, tabs: Option<DockNodeId>) -> Self {
        self.excluded_tabs = tabs;
        self
    }

    pub(crate) fn push_fact(&mut self, fact: DockHostDropSceneFact) {
        if self
            .excluded_tabs
            .is_some_and(|tabs| fact.targets_tabs(tabs))
        {
            return;
        }

        match fact {
            DockHostDropSceneFact::TabLabel(target) => self.tab_labels.push(target),
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

    pub(crate) fn resolve_drop(&self, policy: &DockPolicy) -> Option<DockDropResolution> {
        self.resolve_drop_with_validator(policy, None)
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
            leaves: &self.leaves,
            root: self.root,
            floating_title_bars: &self.floating_title_bars,
            empty_spaces: &self.empty_spaces,
        })
    }

    pub(crate) fn resolved_target(&self, policy: &DockPolicy) -> Option<DockResolvedDropTarget> {
        self.resolve_drop(policy)
            .and_then(DockDropResolution::target)
    }
}

impl DockHostDropSceneFact {
    fn targets_tabs(&self, tabs: DockNodeId) -> bool {
        match self {
            DockHostDropSceneFact::TabLabel(target) => target.target_tabs == tabs,
            DockHostDropSceneFact::Leaf(target) => {
                target.root == tabs || target.target_tabs == tabs
            }
            DockHostDropSceneFact::Root(target) => target.root == tabs,
            DockHostDropSceneFact::FloatingTitleBar(target) => target.target_tabs == tabs,
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
        excluded_tabs: Option<DockNodeId>,
        fact: DockHostDropSceneFact,
        policy: &DockPolicy,
    ) -> bool {
        self.push_scene_fact_with_validator(position, excluded_tabs, fact, policy, None)
    }

    pub(crate) fn push_scene_fact_with_validator(
        &mut self,
        position: Point<Pixels>,
        excluded_tabs: Option<DockNodeId>,
        fact: DockHostDropSceneFact,
        policy: &DockPolicy,
        target_validator: Option<&DockDropTargetValidator<'_>>,
    ) -> bool {
        let scene = self.scene_for_position(position, excluded_tabs);
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
                .and_then(center_target_tabs)
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
        excluded_tabs: Option<DockNodeId>,
    ) -> &mut DockHostDropScene {
        let should_reset = self
            .scene
            .as_ref()
            .is_none_or(|scene| scene.position != position || scene.excluded_tabs != excluded_tabs);
        if should_reset {
            self.scene = Some(DockHostDropScene::new(position).excluding_tabs(excluded_tabs));
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

fn center_target_tabs(target: &DockResolvedDropTarget) -> Option<DockNodeId> {
    match target.kind {
        drop_target::DockResolvedDropTargetKind::TabBar { target_tabs, .. }
        | drop_target::DockResolvedDropTargetKind::LeafCenter { target_tabs, .. }
        | drop_target::DockResolvedDropTargetKind::FloatingTitleBar { target_tabs, .. } => {
            Some(target_tabs)
        }
        drop_target::DockResolvedDropTargetKind::InnerEdge { .. }
        | drop_target::DockResolvedDropTargetKind::RootEdge { .. }
        | drop_target::DockResolvedDropTargetKind::EmptyDockSpace { .. }
        | drop_target::DockResolvedDropTargetKind::KnownViewport { .. }
        | drop_target::DockResolvedDropTargetKind::TearOffCandidate { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DropZone,
        drop_target::{DockDropResolveSource, DockResolvedDropTargetKind},
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
        match zone {
            DropZone::Left => (bounds(0.0, 20.0, 160.0, 180.0), point(px(2.0), px(100.0))),
            DropZone::Right => (
                bounds(240.0, 20.0, 160.0, 180.0),
                point(px(398.0), px(100.0)),
            ),
            DropZone::Top => (bounds(120.0, 0.0, 160.0, 120.0), point(px(200.0), px(2.0))),
            DropZone::Bottom => (
                bounds(120.0, 120.0, 160.0, 120.0),
                point(px(200.0), px(238.0)),
            ),
            DropZone::Center => unreachable!(),
        }
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
        let position = point(px(95.0), px(28.0));

        runtime.begin_scene(DockHostDropScene::new(position), &DockPolicy::default());
        assert!(runtime.push_scene_fact(
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
            active: 0,
        });
        let target_tabs = graph.insert_node(crate::DockNode::Tabs {
            items: vec![crate::DockItemId::from("target")],
            active: 0,
        });
        let mut runtime = DockDropRuntime::default();
        let position = point(px(120.0), px(80.0));

        runtime.begin_scene(
            DockHostDropScene::new(position).excluding_tabs(Some(source_tabs)),
            &DockPolicy::default(),
        );
        assert!(runtime.push_scene_fact(
            position,
            Some(source_tabs),
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: target_tabs,
                target_tabs,
                bounds: bounds(0.0, 0.0, 400.0, 240.0),
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
    fn root_edge_targets_can_be_taken_without_receiver_bounds() {
        let root = DockNodeId::null();
        let mut graph = crate::DockGraph::new();
        let leaf = graph.insert_node(crate::DockNode::Tabs {
            items: vec![crate::DockItemId::from("a")],
            active: 0,
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
            }),
            &DockPolicy::default()
        ));

        let target = runtime
            .take_resolved_target()
            .expect("empty-space target should resolve without receiver bounds");
        assert!(matches!(
            target.kind,
            DockResolvedDropTargetKind::EmptyDockSpace { space: target_space } if target_space == space
        ));
    }
}
