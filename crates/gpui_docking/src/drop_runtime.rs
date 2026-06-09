use crate::{
    DockNodeId, DockPolicy,
    drop_target::{
        self, DockDropResolution, DockDropResolverInput, DockEmptySpaceDropTarget,
        DockFloatingTitleBarDropTarget, DockLeafDropTarget, DockResolvedDropTarget,
        DockRootDropTarget, DockTabLabelDropTarget, DockTearOffCandidateDropTarget,
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
    pub(crate) tab_labels: Vec<DockTabLabelDropTarget>,
    pub(crate) leaves: Vec<DockLeafDropTarget>,
    pub(crate) root: Option<DockRootDropTarget>,
    pub(crate) floating_title_bars: Vec<DockFloatingTitleBarDropTarget>,
    pub(crate) empty_spaces: Vec<DockEmptySpaceDropTarget>,
    pub(crate) known_viewport: Option<crate::DockViewportHit>,
    pub(crate) tear_off_candidate: Option<DockTearOffCandidateDropTarget>,
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
            tab_labels: Vec::new(),
            leaves: Vec::new(),
            root: None,
            floating_title_bars: Vec::new(),
            empty_spaces: Vec::new(),
            known_viewport: None,
            tear_off_candidate: None,
            clear_on_miss: true,
        }
    }

    pub(crate) fn push_fact(&mut self, fact: DockHostDropSceneFact) {
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
        drop_target::resolve_layout_drop(DockDropResolverInput {
            position: self.position,
            policy,
            tab_labels: &self.tab_labels,
            leaves: &self.leaves,
            root: self.root,
            floating_title_bars: &self.floating_title_bars,
            empty_spaces: &self.empty_spaces,
            known_viewport: self.known_viewport.clone(),
            tear_off_candidate: self.tear_off_candidate.clone(),
        })
    }

    pub(crate) fn resolved_target(&self, policy: &DockPolicy) -> Option<DockResolvedDropTarget> {
        self.resolve_drop(policy)
            .and_then(DockDropResolution::target)
    }

    fn without_tabs_targets(&self, tabs: DockNodeId) -> Self {
        let mut scene = self.clone();
        scene.tab_labels.retain(|target| target.target_tabs != tabs);
        scene
            .leaves
            .retain(|target| target.root != tabs && target.target_tabs != tabs);
        if scene.root.is_some_and(|target| target.root == tabs) {
            scene.root = None;
        }
        scene
            .floating_title_bars
            .retain(|target| target.target_tabs != tabs);
        scene
    }
}

impl DockDropRuntime {
    pub(crate) fn begin_scene(&mut self, scene: DockHostDropScene, policy: &DockPolicy) -> bool {
        let changed = self.resolve_scene(&scene, policy);
        self.scene = Some(scene);
        changed
    }

    pub(crate) fn push_scene_fact(
        &mut self,
        position: Point<Pixels>,
        fact: DockHostDropSceneFact,
        policy: &DockPolicy,
    ) -> bool {
        let scene = self.scene_for_position(position);
        scene.push_fact(fact);
        let scene = scene.clone();
        self.resolve_scene(&scene, policy)
    }

    fn resolve_scene(&mut self, scene: &DockHostDropScene, policy: &DockPolicy) -> bool {
        let mut resolution = match scene.resolve_drop(policy) {
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

    pub(crate) fn take_resolved_target_excluding_tabs(
        &mut self,
        source_tabs: DockNodeId,
        policy: &DockPolicy,
    ) -> Option<DockResolvedDropTarget> {
        let target = self
            .scene
            .as_ref()
            .and_then(|scene| {
                scene
                    .without_tabs_targets(source_tabs)
                    .resolved_target(policy)
            })
            .or_else(|| self.resolution.take().and_then(DockDropResolution::target));
        self.scene = None;
        self.resolution = None;
        target
    }

    fn scene_for_position(&mut self, position: Point<Pixels>) -> &mut DockHostDropScene {
        let should_reset = self
            .scene
            .as_ref()
            .is_none_or(|scene| scene.position != position);
        if should_reset {
            self.scene = Some(DockHostDropScene::new(position));
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
    fn resolved_target(&self) -> Option<&DockResolvedDropTarget> {
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
    use crate::{DropZone, drop_target::DockResolvedDropTargetKind};
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

    #[test]
    fn tab_reorder_drop_updates_target_only_inside_tab_bounds() {
        let tabs = DockNodeId::null();
        let mut runtime = DockDropRuntime::default();
        let position = point(px(95.0), px(28.0));

        runtime.begin_scene(DockHostDropScene::new(position), &DockPolicy::default());
        assert!(runtime.push_scene_fact(
            position,
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
    fn stack_drop_can_exclude_source_tabs_when_resolving_scene() {
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

        runtime.begin_scene(DockHostDropScene::new(position), &DockPolicy::default());
        assert!(runtime.push_scene_fact(
            position,
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: target_tabs,
                target_tabs,
                bounds: bounds(0.0, 0.0, 400.0, 240.0),
                is_central: false,
            }),
            &DockPolicy::default()
        ));
        assert!(runtime.push_scene_fact(
            position,
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: source_tabs,
                target_tabs: source_tabs,
                bounds: bounds(80.0, 40.0, 220.0, 140.0),
                is_central: false,
            }),
            &DockPolicy::default()
        ));

        let target = runtime
            .take_resolved_target_excluding_tabs(source_tabs, &DockPolicy::default())
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
    fn resolved_target_take_does_not_require_tab_receiver() {
        let root = DockNodeId::null();
        let mut graph = crate::DockGraph::new();
        let leaf = graph.insert_node(crate::DockNode::Tabs {
            items: vec![crate::DockItemId::from("a")],
            active: 0,
        });
        let mut runtime = DockDropRuntime::default();
        let position = point(px(2.0), px(100.0));

        runtime.begin_scene(DockHostDropScene::new(position), &DockPolicy::default());
        assert!(!runtime.push_scene_fact(
            position,
            DockHostDropSceneFact::Root(crate::drop_target::DockRootDropTarget {
                root,
                bounds: bounds(0.0, 0.0, 400.0, 240.0),
            }),
            &DockPolicy::default()
        ));
        assert!(runtime.push_scene_fact(
            position,
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root,
                target_tabs: leaf,
                bounds: bounds(0.0, 20.0, 160.0, 180.0),
                is_central: false,
            }),
            &DockPolicy::default()
        ));

        let target = runtime
            .take_resolved_target()
            .expect("root-edge target should not require a tab receiver");
        assert!(matches!(
            target.kind,
            DockResolvedDropTargetKind::RootEdge {
                root: matched_root,
                leaf_tabs,
                zone: DropZone::Left,
            } if matched_root == root && leaf_tabs == leaf
        ));
    }

    #[test]
    fn empty_space_target_can_be_taken_without_tab_receiver() {
        let space = crate::DockSpaceId::from("empty");
        let mut runtime = DockDropRuntime::default();
        let position = point(px(40.0), px(40.0));

        runtime.begin_scene(DockHostDropScene::new(position), &DockPolicy::default());
        assert!(runtime.push_scene_fact(
            position,
            DockHostDropSceneFact::EmptySpace(DockEmptySpaceDropTarget {
                space: space.clone(),
                bounds: bounds(0.0, 0.0, 400.0, 240.0),
            }),
            &DockPolicy::default()
        ));

        let target = runtime
            .take_resolved_target()
            .expect("empty-space target should not require a tab receiver");
        assert!(matches!(
            target.kind,
            DockResolvedDropTargetKind::EmptyDockSpace { space: target_space } if target_space == space
        ));
    }
}
