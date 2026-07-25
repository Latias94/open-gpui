use crate::{
    DockNodeId, DockPolicy,
    drop_target::{
        self, DockDropResolution, DockDropResolverInput, DockDropTargetValidator,
        DockEdgePlanResolver, DockEmptySpaceDropTarget, DockFloatingTitleBarDropTarget,
        DockLeafDropTarget, DockResolvedDropTarget, DockRootDropTarget, DockTabBarDropTarget,
        DockTabLabelDropTarget,
    },
    geometry::DockDropGuideMetrics,
};
use open_gpui::{Bounds, Pixels, Point, Size};

const TAB_REORDER_HOLD_DEAD_ZONE_PX: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct DockTabReorderHold {
    target_tabs: DockNodeId,
    insert_index: usize,
    bounds: Bounds<Pixels>,
}

#[derive(Debug, Default)]
pub(crate) struct DockDropRuntime {
    resolution: Option<DockDropResolution>,
    guide_target: Option<DockResolvedDropTarget>,
    reorder_hold_resolution: Option<DockDropResolution>,
    scene: Option<DockHostDropScene>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockHostDropScene {
    pub(crate) position: Point<Pixels>,
    payload_size: Option<Size<Pixels>>,
    drop_guide_metrics: DockDropGuideMetrics,
    excluded_nodes: Vec<DockNodeId>,
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
            payload_size: None,
            drop_guide_metrics: DockDropGuideMetrics::default(),
            excluded_nodes: Vec::new(),
            tab_labels: Vec::new(),
            tab_bars: Vec::new(),
            leaves: Vec::new(),
            root: None,
            floating_title_bars: Vec::new(),
            empty_spaces: Vec::new(),
            clear_on_miss: true,
        }
    }

    pub(crate) fn excluding_nodes(mut self, nodes: Vec<DockNodeId>) -> Self {
        let is_excluded = |node| nodes.contains(&node);
        self.tab_labels
            .retain(|target| !is_excluded(target.target_tabs));
        self.tab_bars
            .retain(|target| !is_excluded(target.target_tabs));
        self.leaves
            .retain(|target| !is_excluded(target.root) && !is_excluded(target.target_tabs));
        if self.root.is_some_and(|target| is_excluded(target.root)) {
            self.root = None;
        }
        self.floating_title_bars
            .retain(|target| !is_excluded(target.floating) && !is_excluded(target.target_tabs));
        self.excluded_nodes = nodes;
        self
    }

    pub(crate) fn with_payload_size(mut self, payload_size: Option<Size<Pixels>>) -> Self {
        self.payload_size = payload_size;
        self
    }

    pub(crate) fn with_drop_guide_metrics(mut self, metrics: DockDropGuideMetrics) -> Self {
        self.drop_guide_metrics = metrics;
        self
    }

    pub(crate) fn drop_guide_metrics(&self) -> DockDropGuideMetrics {
        self.drop_guide_metrics
    }

    pub(crate) fn push_fact(&mut self, fact: DockHostDropSceneFact) -> bool {
        if self.fact_is_excluded(&fact) {
            return false;
        }

        match fact {
            DockHostDropSceneFact::TabLabel(target) => {
                if let Some(existing) = self.tab_labels.iter_mut().find(|label| {
                    label.target_tabs == target.target_tabs
                        && label.target_index == target.target_index
                }) {
                    if *existing == target {
                        false
                    } else {
                        *existing = target;
                        true
                    }
                } else {
                    self.tab_labels.push(target);
                    true
                }
            }
            DockHostDropSceneFact::TabBar(target) => {
                self.tab_bars.push(target);
                true
            }
            DockHostDropSceneFact::Leaf(target) => {
                self.leaves.push(target);
                true
            }
            DockHostDropSceneFact::Root(target) => {
                if self.root == Some(target) {
                    false
                } else {
                    self.root = Some(target);
                    true
                }
            }
            DockHostDropSceneFact::FloatingTitleBar(target) => {
                self.floating_title_bars.push(target);
                true
            }
            DockHostDropSceneFact::EmptySpace(target) => {
                self.empty_spaces.push(target);
                true
            }
        }
    }

    pub(crate) fn preserve_measured_tab_labels_from(&mut self, previous: &Self) {
        for label in previous.tab_labels.iter().copied() {
            let should_preserve = self.tab_bars.iter().any(|tab_bar| {
                tab_bar.target_tabs == label.target_tabs
                    && label.target_index < tab_bar.insert_index
                    && tab_bar.is_central == label.is_central
            });
            if should_preserve {
                self.push_fact(DockHostDropSceneFact::TabLabel(label));
            }
        }
    }

    fn fact_is_excluded(&self, fact: &DockHostDropSceneFact) -> bool {
        self.excluded_nodes
            .iter()
            .copied()
            .any(|node| fact.targets_node(node))
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
        edge_plan_resolver: Option<&DockEdgePlanResolver<'_>>,
    ) -> Option<DockDropResolution> {
        drop_target::resolve_layout_drop(DockDropResolverInput {
            position: self.position,
            payload_size: self.payload_size,
            drop_guide_metrics: self.drop_guide_metrics,
            policy,
            target_validator,
            edge_plan_resolver,
            tab_labels: &self.tab_labels,
            tab_bars: &self.tab_bars,
            leaves: &self.leaves,
            root: self.root,
            floating_title_bars: &self.floating_title_bars,
            empty_spaces: &self.empty_spaces,
        })
    }

    pub(crate) fn resolve_guide_target_with_validator(
        &self,
        policy: &DockPolicy,
        target_validator: Option<&DockDropTargetValidator<'_>>,
        edge_plan_resolver: Option<&DockEdgePlanResolver<'_>>,
    ) -> Option<DockResolvedDropTarget> {
        drop_target::resolve_layout_drop_guide(DockDropResolverInput {
            position: self.position,
            payload_size: self.payload_size,
            drop_guide_metrics: self.drop_guide_metrics,
            policy,
            target_validator,
            edge_plan_resolver,
            tab_labels: &self.tab_labels,
            tab_bars: &self.tab_bars,
            leaves: &self.leaves,
            root: self.root,
            floating_title_bars: &self.floating_title_bars,
            empty_spaces: &self.empty_spaces,
        })
    }

    fn for_release_position(&self, position: Point<Pixels>) -> Self {
        let mut scene = self.clone();
        scene.position = position;
        scene.clear_on_miss = true;
        scene
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
        self.begin_scene_with_validator(scene, policy, None, None)
    }

    pub(crate) fn begin_scene_with_validator(
        &mut self,
        scene: DockHostDropScene,
        policy: &DockPolicy,
        target_validator: Option<&DockDropTargetValidator<'_>>,
        edge_plan_resolver: Option<&DockEdgePlanResolver<'_>>,
    ) -> bool {
        let changed = self.resolve_scene(&scene, policy, target_validator, edge_plan_resolver);
        self.scene = Some(scene);
        changed
    }

    #[cfg(test)]
    pub(crate) fn push_scene_fact(
        &mut self,
        position: Point<Pixels>,
        excluded_nodes: Vec<DockNodeId>,
        fact: DockHostDropSceneFact,
        policy: &DockPolicy,
    ) -> bool {
        self.push_scene_fact_with_validator(
            position,
            None,
            DockDropGuideMetrics::default(),
            excluded_nodes,
            fact,
            policy,
            None,
            None,
        )
    }

    pub(crate) fn push_scene_fact_with_validator(
        &mut self,
        position: Point<Pixels>,
        payload_size: Option<Size<Pixels>>,
        drop_guide_metrics: DockDropGuideMetrics,
        excluded_nodes: Vec<DockNodeId>,
        fact: DockHostDropSceneFact,
        policy: &DockPolicy,
        target_validator: Option<&DockDropTargetValidator<'_>>,
        edge_plan_resolver: Option<&DockEdgePlanResolver<'_>>,
    ) -> bool {
        let scene =
            self.scene_for_position(position, payload_size, drop_guide_metrics, excluded_nodes);
        scene.push_fact(fact);
        let scene = scene.clone();
        self.resolve_scene(&scene, policy, target_validator, edge_plan_resolver)
    }

    fn resolve_scene(
        &mut self,
        scene: &DockHostDropScene,
        policy: &DockPolicy,
        target_validator: Option<&DockDropTargetValidator<'_>>,
        edge_plan_resolver: Option<&DockEdgePlanResolver<'_>>,
    ) -> bool {
        let (resolution, guide_target) = match self.resolve_scene_resolution(
            scene,
            policy,
            target_validator,
            edge_plan_resolver,
        ) {
            Some(resolved) => resolved,
            None => return false,
        };
        self.replace_resolution(resolution, guide_target)
    }

    fn resolve_scene_resolution(
        &self,
        scene: &DockHostDropScene,
        policy: &DockPolicy,
        target_validator: Option<&DockDropTargetValidator<'_>>,
        edge_plan_resolver: Option<&DockEdgePlanResolver<'_>>,
    ) -> Option<(Option<DockDropResolution>, Option<DockResolvedDropTarget>)> {
        let mut resolution =
            match scene.resolve_drop_with_validator(policy, target_validator, edge_plan_resolver) {
                Some(resolution) => Some(resolution),
                None if scene.clear_on_miss => None,
                None => return None,
            };
        let mut guide_target = if resolution.is_none() {
            scene.resolve_guide_target_with_validator(policy, target_validator, edge_plan_resolver)
        } else {
            None
        };
        if let Some(existing_resolution) = self.reorder_hold_resolution.as_ref()
            && let Some(existing) = resolution_target(existing_resolution)
            && let Some(reorder_hold) = tab_reorder_hold(existing)
            && should_hold_tab_reorder_target(
                resolution.as_ref().and_then(resolution_target),
                reorder_hold,
                scene.position,
            )
        {
            resolution = Some(existing_resolution.clone());
            guide_target = None;
        }
        Some((resolution, guide_target))
    }

    pub(crate) fn take_release_target_at(
        &mut self,
        release_position: Point<Pixels>,
        policy: &DockPolicy,
        target_validator: Option<&DockDropTargetValidator<'_>>,
        edge_plan_resolver: Option<&DockEdgePlanResolver<'_>>,
    ) -> Option<DockResolvedDropTarget> {
        let Some(scene) = self.scene.as_ref() else {
            self.resolution = None;
            return None;
        };
        let release_scene = scene.for_release_position(release_position);
        let resolution = self.resolve_scene_resolution(
            &release_scene,
            policy,
            target_validator,
            edge_plan_resolver,
        );
        self.scene = None;
        self.reorder_hold_resolution = None;
        match resolution {
            Some((Some(DockDropResolution::Valid(target)), _)) => {
                self.resolution = None;
                self.guide_target = None;
                Some(target)
            }
            Some((Some(DockDropResolution::Rejected(rejection)), _)) => {
                self.resolution = Some(DockDropResolution::Rejected(rejection));
                self.guide_target = None;
                None
            }
            Some((None, guide_target)) => {
                self.resolution = None;
                self.guide_target = guide_target;
                None
            }
            None => {
                self.resolution = None;
                self.guide_target = None;
                None
            }
        }
    }

    #[cfg(test)]
    fn take_release_target(&mut self) -> Option<DockResolvedDropTarget> {
        let position = self.scene.as_ref()?.position;
        self.take_release_target_at(position, &DockPolicy::default(), None, None)
    }

    pub(crate) fn clear(&mut self) -> bool {
        let changed = self.resolution.take().is_some()
            || self.guide_target.take().is_some()
            || self.reorder_hold_resolution.take().is_some()
            || self.scene.take().is_some();
        changed
    }

    pub(crate) fn drop_resolution(&self) -> Option<&DockDropResolution> {
        self.resolution.as_ref()
    }

    pub(crate) fn guide_target(&self) -> Option<&DockResolvedDropTarget> {
        self.guide_target.as_ref()
    }

    pub(crate) fn scene_position(&self) -> Option<Point<Pixels>> {
        self.scene.as_ref().map(|scene| scene.position)
    }

    pub(crate) fn drop_guide_metrics(&self) -> DockDropGuideMetrics {
        self.scene
            .as_ref()
            .map(|scene| scene.drop_guide_metrics)
            .unwrap_or_default()
    }

    fn scene_for_position(
        &mut self,
        position: Point<Pixels>,
        payload_size: Option<Size<Pixels>>,
        drop_guide_metrics: DockDropGuideMetrics,
        excluded_nodes: Vec<DockNodeId>,
    ) -> &mut DockHostDropScene {
        let should_reset = self.scene.as_ref().is_none_or(|scene| {
            scene.position != position
                || scene.payload_size != payload_size
                || scene.drop_guide_metrics != drop_guide_metrics
                || scene.excluded_nodes != excluded_nodes
        });
        if should_reset {
            self.scene = Some(
                DockHostDropScene::new(position)
                    .with_payload_size(payload_size)
                    .with_drop_guide_metrics(drop_guide_metrics)
                    .excluding_nodes(excluded_nodes),
            );
        }
        self.scene.as_mut().expect("scene should be initialized")
    }

    fn replace_resolution(
        &mut self,
        resolution: Option<DockDropResolution>,
        guide_target: Option<DockResolvedDropTarget>,
    ) -> bool {
        self.reorder_hold_resolution = resolution
            .as_ref()
            .filter(|resolution| {
                resolution_target(resolution)
                    .is_some_and(|target| tab_reorder_hold(target).is_some())
            })
            .cloned();
        if self.resolution == resolution && self.guide_target == guide_target {
            return false;
        }
        self.resolution = resolution;
        self.guide_target = guide_target;
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

fn tab_reorder_hold(target: &DockResolvedDropTarget) -> Option<DockTabReorderHold> {
    let drop_target::DockResolvedDropTargetKind::TabBar {
        target_tabs,
        insert_index,
    } = target.kind
    else {
        return None;
    };

    Some(DockTabReorderHold {
        target_tabs,
        insert_index,
        bounds: target.hit_bounds.or(target.preview_bounds)?,
    })
}

fn should_hold_tab_reorder_target(
    target: Option<&DockResolvedDropTarget>,
    hold: DockTabReorderHold,
    position: Point<Pixels>,
) -> bool {
    let Some(target) = target else {
        return hold.bounds.contains(&position);
    };
    match target.kind {
        drop_target::DockResolvedDropTargetKind::LeafCenter { .. } => {
            target.center_target_tabs() == Some(hold.target_tabs) && hold.bounds.contains(&position)
        }
        drop_target::DockResolvedDropTargetKind::TabBar {
            target_tabs,
            insert_index,
        } => {
            target_tabs == hold.target_tabs
                && (insert_index == hold.insert_index
                    || (insert_index.abs_diff(hold.insert_index) == 1
                        && tab_reorder_hold_dead_zone_contains(hold.bounds, position)))
        }
        drop_target::DockResolvedDropTargetKind::InnerEdge { .. }
        | drop_target::DockResolvedDropTargetKind::RootEdge { .. }
        | drop_target::DockResolvedDropTargetKind::FloatingTitleBar { .. }
        | drop_target::DockResolvedDropTargetKind::EmptyDockSpace { .. } => false,
    }
}

fn tab_reorder_hold_dead_zone_contains(bounds: Bounds<Pixels>, position: Point<Pixels>) -> bool {
    if !bounds.contains(&position) {
        return false;
    }
    let center_x = f32::from(bounds.center().x);
    let position_x = f32::from(position.x);
    (position_x - center_x).abs() <= TAB_REORDER_HOLD_DEAD_ZONE_PX
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

    fn root_edge_passive_leaf_bounds_and_position(
        zone: DropZone,
    ) -> (Bounds<Pixels>, Point<Pixels>) {
        let root_bounds = bounds(0.0, 0.0, 400.0, 240.0);
        let position = geometry::drop_boxes(root_bounds, DockDropBoxSet::Outer)
            .into_iter()
            .find(|drop_box| drop_box.kind == DockDropBoxKind::OuterEdge(zone))
            .map(|drop_box| drop_box.hit_bounds.center())
            .unwrap_or_else(|| panic!("{zone:?} outer box should exist"));
        let leaf_bounds = Bounds::new(
            point(position.x - px(2.0), position.y - px(100.0)),
            size(px(300.0), px(200.0)),
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

    fn resolve_leaf_center(
        runtime: &mut DockDropRuntime,
        tabs: DockNodeId,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
    ) {
        runtime.begin_scene(DockHostDropScene::new(position), &DockPolicy::default());
        runtime.push_scene_fact(
            position,
            Vec::new(),
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: tabs,
                target_tabs: tabs,
                bounds,
                is_central: false,
            }),
            &DockPolicy::default(),
        );
    }

    #[test]
    fn first_resolved_target_is_deliverable_at_release() {
        let tabs = DockNodeId::null();
        let mut runtime = DockDropRuntime::default();
        let leaf_bounds = bounds(0.0, 0.0, 400.0, 240.0);
        let position = leaf_center_position(leaf_bounds);

        resolve_leaf_center(&mut runtime, tabs, position, leaf_bounds);
        let target = runtime
            .take_release_target()
            .expect("release should deliver the current resolved target");
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::LeafCenter {
                root: tabs,
                target_tabs: tabs,
            }
        );
    }

    #[test]
    fn release_revalidation_delivers_current_target_without_render_gate() {
        let tabs = DockNodeId::null();
        let mut runtime = DockDropRuntime::default();
        let leaf_bounds = bounds(0.0, 0.0, 400.0, 240.0);
        let position = leaf_center_position(leaf_bounds);

        resolve_leaf_center(&mut runtime, tabs, position, leaf_bounds);
        let target = runtime
            .take_release_target_at(position, &DockPolicy::default(), None, None)
            .expect("release should deliver a current target after fresh revalidation");
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::LeafCenter {
                root: tabs,
                target_tabs: tabs,
            }
        );
    }

    #[test]
    fn target_change_delivers_the_current_target() {
        let mut graph = crate::DockGraph::new();
        let second = graph.insert_node(crate::DockNode::Tabs {
            items: vec![crate::DockItemId::from("target")],
            selected: Some(crate::DockItemId::from("target")),
        });
        let mut runtime = DockDropRuntime::default();
        let second_bounds = bounds(180.0, 0.0, 160.0, 160.0);
        let second_position = leaf_center_position(second_bounds);

        resolve_leaf_center(&mut runtime, second, second_position, second_bounds);
        let target = runtime
            .take_release_target()
            .expect("release should deliver the current target after it changes");
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::LeafCenter {
                root: second,
                target_tabs: second,
            }
        );
    }

    #[test]
    fn restarting_scene_at_same_position_discards_previous_facts() {
        let leaf_tabs = DockNodeId::null();
        let replacement_root = DockNodeId::from(slotmap::KeyData::from_ffi(2));
        let replacement_tabs = DockNodeId::from(slotmap::KeyData::from_ffi(3));
        let mut runtime = DockDropRuntime::default();
        let original_bounds = bounds(0.0, 0.0, 420.0, 260.0);
        let replacement_bounds = bounds(160.0, 80.0, 120.0, 120.0);
        let position = leaf_center_position(original_bounds);

        runtime.begin_scene(DockHostDropScene::new(position), &DockPolicy::default());
        assert!(runtime.push_scene_fact(
            position,
            Vec::new(),
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: leaf_tabs,
                target_tabs: leaf_tabs,
                bounds: original_bounds,
                is_central: false,
            }),
            &DockPolicy::default()
        ));
        assert_eq!(
            runtime.resolved_target().map(|target| target.kind.clone()),
            Some(DockResolvedDropTargetKind::LeafCenter {
                root: leaf_tabs,
                target_tabs: leaf_tabs,
            })
        );

        assert!(runtime.begin_scene(DockHostDropScene::new(position), &DockPolicy::default()));
        assert!(runtime.push_scene_fact(
            position,
            Vec::new(),
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: replacement_root,
                target_tabs: replacement_tabs,
                bounds: replacement_bounds,
                is_central: false,
            }),
            &DockPolicy::default()
        ));

        let target = runtime
            .take_release_target()
            .expect("current scene should resolve after restart");
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::LeafCenter {
                root: replacement_root,
                target_tabs: replacement_tabs,
            },
            "restarting the scene at the same pointer position must drop stale facts from the prior drag-move pass"
        );
    }

    #[test]
    fn release_revalidation_requires_pointer_to_still_hit_current_target() {
        let tabs = DockNodeId::null();
        let mut runtime = DockDropRuntime::default();
        let leaf_bounds = bounds(0.0, 0.0, 400.0, 240.0);
        let position = leaf_center_position(leaf_bounds);

        resolve_leaf_center(&mut runtime, tabs, position, leaf_bounds);
        assert!(!runtime.begin_scene(
            DockHostDropScene::new(point(px(900.0), px(900.0))).preserve_on_miss(),
            &DockPolicy::default()
        ));

        assert!(
            runtime
                .take_release_target_at(
                    point(px(900.0), px(900.0)),
                    &DockPolicy::default(),
                    None,
                    None,
                )
                .is_none(),
            "release outside the current scene must not deliver a stale resolved target"
        );
        assert!(runtime.resolved_target().is_none());
    }

    #[test]
    fn tab_reorder_drop_updates_target_only_inside_tab_bounds() {
        let tabs = DockNodeId::null();
        let mut runtime = DockDropRuntime::default();
        let position = point(px(95.0), px(28.0));

        runtime.begin_scene(DockHostDropScene::new(position), &DockPolicy::default());
        assert!(runtime.push_scene_fact(
            position,
            Vec::new(),
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
            Vec::new(),
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
            Vec::new(),
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: tabs,
                target_tabs: tabs,
                bounds: bounds(0.0, 0.0, 400.0, 400.0),
                is_central: false,
            }),
            &DockPolicy::default()
        ));
        let target = runtime
            .take_release_target()
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
            Vec::new(),
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
    fn reorder_target_updates_insert_index_within_same_tab_stack() {
        let tabs = DockNodeId::null();
        let mut runtime = DockDropRuntime::default();
        let first_position = point(px(20.0), px(28.0));
        let second_position = point(px(120.0), px(28.0));

        runtime.begin_scene(
            DockHostDropScene::new(first_position),
            &DockPolicy::default(),
        );
        assert!(runtime.push_scene_fact(
            first_position,
            Vec::new(),
            DockHostDropSceneFact::TabLabel(DockTabLabelDropTarget {
                target_tabs: tabs,
                target_index: 0,
                bounds: bounds(10.0, 20.0, 80.0, 24.0),
                is_central: false,
            }),
            &DockPolicy::default()
        ));
        assert_eq!(resolved_tab_insert_index(&runtime), Some(0));

        assert!(!runtime.begin_scene(
            DockHostDropScene::new(second_position).preserve_on_miss(),
            &DockPolicy::default()
        ));
        assert!(runtime.push_scene_fact(
            second_position,
            Vec::new(),
            DockHostDropSceneFact::TabLabel(DockTabLabelDropTarget {
                target_tabs: tabs,
                target_index: 1,
                bounds: bounds(100.0, 20.0, 80.0, 24.0),
                is_central: false,
            }),
            &DockPolicy::default()
        ));

        assert_eq!(
            resolved_tab_insert_index(&runtime),
            Some(1),
            "same-stack reorder hold must not freeze the old tab insertion slot"
        );
    }

    #[test]
    fn tab_reorder_hold_dampens_adjacent_slot_jitter_near_tab_center() {
        let tabs = DockNodeId::null();
        let mut runtime = DockDropRuntime::default();
        let tab_bounds = bounds(10.0, 20.0, 100.0, 24.0);
        let left_of_center = point(px(56.0), px(28.0));
        let right_of_center = point(px(64.0), px(28.0));
        let far_right = point(px(96.0), px(28.0));

        runtime.begin_scene(
            DockHostDropScene::new(left_of_center),
            &DockPolicy::default(),
        );
        assert!(runtime.push_scene_fact(
            left_of_center,
            Vec::new(),
            DockHostDropSceneFact::TabLabel(DockTabLabelDropTarget {
                target_tabs: tabs,
                target_index: 0,
                bounds: tab_bounds,
                is_central: false,
            }),
            &DockPolicy::default()
        ));
        assert_eq!(resolved_tab_insert_index(&runtime), Some(0));

        assert!(!runtime.begin_scene(
            DockHostDropScene::new(right_of_center).preserve_on_miss(),
            &DockPolicy::default()
        ));
        assert!(!runtime.push_scene_fact(
            right_of_center,
            Vec::new(),
            DockHostDropSceneFact::TabLabel(DockTabLabelDropTarget {
                target_tabs: tabs,
                target_index: 0,
                bounds: tab_bounds,
                is_central: false,
            }),
            &DockPolicy::default()
        ));
        assert_eq!(
            resolved_tab_insert_index(&runtime),
            Some(0),
            "adjacent insert slots around the tab center should not flicker while the pointer is effectively stationary"
        );

        assert!(!runtime.begin_scene(
            DockHostDropScene::new(far_right).preserve_on_miss(),
            &DockPolicy::default()
        ));
        assert!(runtime.push_scene_fact(
            far_right,
            Vec::new(),
            DockHostDropSceneFact::TabLabel(DockTabLabelDropTarget {
                target_tabs: tabs,
                target_index: 0,
                bounds: tab_bounds,
                is_central: false,
            }),
            &DockPolicy::default()
        ));
        assert_eq!(
            resolved_tab_insert_index(&runtime),
            Some(1),
            "intentional movement out of the center dead zone should still update the insertion slot"
        );
    }

    #[test]
    fn tab_reorder_hold_survives_partial_scene_while_pointer_stays_inside_held_tab() {
        let tabs = DockNodeId::null();
        let mut runtime = DockDropRuntime::default();
        let tab_bounds = bounds(10.0, 20.0, 100.0, 24.0);
        let position = point(px(64.0), px(28.0));

        runtime.begin_scene(DockHostDropScene::new(position), &DockPolicy::default());
        assert!(runtime.push_scene_fact(
            position,
            Vec::new(),
            DockHostDropSceneFact::TabLabel(DockTabLabelDropTarget {
                target_tabs: tabs,
                target_index: 0,
                bounds: tab_bounds,
                is_central: false,
            }),
            &DockPolicy::default()
        ));
        assert_eq!(resolved_tab_insert_index(&runtime), Some(1));

        assert!(
            !runtime.begin_scene(DockHostDropScene::new(position), &DockPolicy::default()),
            "partial render scenes with no facts should keep the held tab target stable"
        );
        assert_eq!(resolved_tab_insert_index(&runtime), Some(1));

        let outside = point(px(140.0), px(28.0));
        assert!(
            runtime.begin_scene(DockHostDropScene::new(outside), &DockPolicy::default()),
            "moving outside the held tab bounds should still clear the stale tab target"
        );
        assert_eq!(runtime.resolved_target(), None);
        assert!(runtime.reorder_hold_resolution.is_none());
    }

    #[test]
    fn tab_reorder_hold_suppresses_partial_guide_target_churn() {
        let tabs = DockNodeId::null();
        let mut runtime = DockDropRuntime::default();
        let tab_bounds = bounds(10.0, 20.0, 100.0, 24.0);
        let root_bounds = bounds(0.0, 0.0, 400.0, 240.0);
        let position = point(px(64.0), px(28.0));

        runtime.begin_scene(DockHostDropScene::new(position), &DockPolicy::default());
        assert!(runtime.push_scene_fact(
            position,
            Vec::new(),
            DockHostDropSceneFact::TabLabel(DockTabLabelDropTarget {
                target_tabs: tabs,
                target_index: 0,
                bounds: tab_bounds,
                is_central: false,
            }),
            &DockPolicy::default()
        ));
        assert_eq!(resolved_tab_insert_index(&runtime), Some(1));
        assert!(runtime.guide_target().is_none());

        assert!(
            !runtime.begin_scene(DockHostDropScene::new(position), &DockPolicy::default()),
            "a partial scene with the same held tab target should not report a state change"
        );
        assert!(runtime.guide_target().is_none());

        assert!(
            !runtime.push_scene_fact(
                position,
                Vec::new(),
                DockHostDropSceneFact::Root(crate::drop_target::DockRootDropTarget {
                    root: tabs,
                    bounds: root_bounds,
                }),
                &DockPolicy::default()
            ),
            "guide-only partial facts must not churn state while a held tab target is active"
        );
        assert_eq!(resolved_tab_insert_index(&runtime), Some(1));
        assert!(runtime.guide_target().is_none());
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
            Vec::new(),
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
            Vec::new(),
            DockHostDropSceneFact::TabLabel(DockTabLabelDropTarget {
                target_tabs: tabs,
                target_index: 2,
                bounds: bounds(180.0, 188.0, 40.0, 24.0),
                is_central: false,
            }),
            &DockPolicy::default()
        ));
        let target = runtime
            .take_release_target()
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
    fn edge_payload_size_change_recomputes_delivery_from_current_scene() {
        let tabs = DockNodeId::null();
        let mut runtime = DockDropRuntime::default();
        let leaf_bounds = bounds(0.0, 0.0, 1000.0, 600.0);
        let position = geometry::drop_boxes(leaf_bounds, DockDropBoxSet::Inner)
            .into_iter()
            .find(|drop_box| drop_box.kind == DockDropBoxKind::InnerEdge(DropZone::Right))
            .map(|drop_box| drop_box.hit_bounds.center())
            .expect("right edge box should exist");

        runtime.begin_scene(
            DockHostDropScene::new(position).with_payload_size(Some(size(px(240.0), px(200.0)))),
            &DockPolicy::default(),
        );
        assert!(runtime.push_scene_fact(
            position,
            Vec::new(),
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: tabs,
                target_tabs: tabs,
                bounds: leaf_bounds,
                is_central: false,
            }),
            &DockPolicy::default()
        ));

        assert!(runtime.begin_scene(
            DockHostDropScene::new(position).with_payload_size(Some(size(px(360.0), px(200.0)))),
            &DockPolicy::default(),
        ));
        assert!(runtime.push_scene_fact(
            position,
            Vec::new(),
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: tabs,
                target_tabs: tabs,
                bounds: leaf_bounds,
                is_central: false,
            }),
            &DockPolicy::default()
        ));

        assert!(
            runtime.take_release_target().is_some(),
            "release should recompute the current edge target from the latest scene facts"
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
            DockHostDropScene::new(position).excluding_nodes(vec![source_tabs]),
            &DockPolicy::default(),
        );
        assert!(runtime.push_scene_fact(
            position,
            vec![source_tabs],
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
            vec![source_tabs],
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: source_tabs,
                target_tabs: source_tabs,
                bounds: bounds(80.0, 40.0, 220.0, 140.0),
                is_central: false,
            }),
            &DockPolicy::default()
        ));
        let target = runtime
            .take_release_target()
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
            DockHostDropScene::new(position).excluding_nodes(vec![floating, floating_tabs]),
            &DockPolicy::default(),
        );
        assert!(!runtime.push_scene_fact(
            position,
            vec![floating, floating_tabs],
            DockHostDropSceneFact::FloatingTitleBar(DockFloatingTitleBarDropTarget {
                floating,
                target_tabs: floating_tabs,
                title_bounds: bounds(0.0, 0.0, 400.0, 240.0),
                preview_bounds: bounds(0.0, 0.0, 400.0, 240.0),
            }),
            &DockPolicy::default()
        ));
        assert!(!runtime.push_scene_fact(
            position,
            vec![floating, floating_tabs],
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: floating,
                target_tabs: floating_tabs,
                bounds: leaf_bounds,
                is_central: false,
            }),
            &DockPolicy::default()
        ));
        runtime.push_scene_fact(
            position,
            vec![floating, floating_tabs],
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: target_tabs,
                target_tabs,
                bounds: leaf_bounds,
                is_central: false,
            }),
            &DockPolicy::default(),
        );
        let target = runtime
            .take_release_target()
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
            let (leaf_bounds, position) = root_edge_passive_leaf_bounds_and_position(zone);

            runtime.begin_scene(DockHostDropScene::new(position), &DockPolicy::default());
            assert!(runtime.push_scene_fact(
                position,
                Vec::new(),
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
                Vec::new(),
                DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                    root,
                    target_tabs: leaf,
                    bounds: leaf_bounds,
                    is_central: false,
                }),
                &DockPolicy::default()
            ));
            let target = runtime
                .take_release_target()
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
            Vec::new(),
            DockHostDropSceneFact::EmptySpace(DockEmptySpaceDropTarget {
                space: space.clone(),
                bounds: bounds(0.0, 0.0, 400.0, 240.0),
                is_central: false,
            }),
            &DockPolicy::default()
        ));
        let target = runtime
            .take_release_target()
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
            Vec::new(),
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
            } if target_space == &space
        ));
    }
}
