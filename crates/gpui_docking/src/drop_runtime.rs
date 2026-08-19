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

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockHostDropResolution {
    Drop(DockDropResolution),
    GuideOnly(DockResolvedDropTarget),
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

    pub(crate) fn resolve_pointer_move(
        &self,
        position: Point<Pixels>,
        payload_size: Option<Size<Pixels>>,
        excluded_nodes: Vec<DockNodeId>,
        policy: &DockPolicy,
        target_validator: Option<&DockDropTargetValidator<'_>>,
        edge_plan_resolver: Option<&DockEdgePlanResolver<'_>>,
    ) -> Option<DockHostDropResolution> {
        let mut scene = self.clone().excluding_nodes(excluded_nodes);
        scene.position = position;
        scene = scene.with_payload_size(payload_size);
        scene
            .resolve_drop_with_validator(policy, target_validator, edge_plan_resolver)
            .map(DockHostDropResolution::Drop)
            .or_else(|| {
                scene
                    .resolve_guide_target_with_validator(
                        policy,
                        target_validator,
                        edge_plan_resolver,
                    )
                    .map(DockHostDropResolution::GuideOnly)
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

pub(crate) fn resolution_target(
    resolution: &DockDropResolution,
) -> Option<&DockResolvedDropTarget> {
    match resolution {
        DockDropResolution::Valid(target) => Some(target),
        DockDropResolution::Rejected(rejection) => Some(&rejection.target),
    }
}

pub(crate) fn held_tab_reorder_resolution(
    previous: &DockDropResolution,
    current_target: Option<&DockResolvedDropTarget>,
    position: Point<Pixels>,
) -> Option<DockDropResolution> {
    let hold = resolution_target(previous).and_then(tab_reorder_hold)?;
    should_hold_tab_reorder_target(current_target, hold, position).then(|| previous.clone())
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
        return false;
    };
    match target.kind {
        drop_target::DockResolvedDropTargetKind::LeafCenter { .. } => {
            target.center_target_tabs() == Some(hold.target_tabs)
                && target
                    .target_bounds
                    .is_some_and(|bounds| bounds.contains(&hold.bounds.center()))
                && hold.bounds.contains(&position)
        }
        drop_target::DockResolvedDropTargetKind::TabBar {
            target_tabs,
            insert_index,
        } => {
            target_tabs == hold.target_tabs
                && target.hit_bounds.or(target.preview_bounds) == Some(hold.bounds)
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
    use crate::drop_target::{
        DockDropResolveSource, DockResolvedDropTargetAvailability, DockResolvedDropTargetKind,
    };
    use open_gpui::{point, px, size};
    use slotmap::Key;

    fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
    }

    fn tab_target(
        target_tabs: DockNodeId,
        insert_index: usize,
        bounds: Bounds<Pixels>,
    ) -> DockResolvedDropTarget {
        DockResolvedDropTarget {
            kind: DockResolvedDropTargetKind::TabBar {
                target_tabs,
                insert_index,
            },
            source: DockDropResolveSource::TabBar,
            target_bounds: Some(bounds),
            inner_target_bounds: None,
            availability: DockResolvedDropTargetAvailability::all(),
            drop_box: None,
            hit_bounds: Some(bounds),
            preview_bounds: Some(bounds),
            tab_insertion_bounds: Some(bounds),
            edge_sizing: None,
            edge_plan: None,
            is_central_region: false,
        }
    }

    #[test]
    fn tab_reorder_hold_dampens_only_adjacent_slot_jitter_near_tab_center() {
        let tabs = DockNodeId::null();
        let tab_bounds = bounds(10.0, 20.0, 100.0, 24.0);
        let previous = DockDropResolution::Valid(tab_target(tabs, 0, tab_bounds));
        let adjacent = tab_target(tabs, 1, tab_bounds);

        assert_eq!(
            held_tab_reorder_resolution(&previous, Some(&adjacent), point(px(64.0), px(28.0)),),
            Some(previous.clone())
        );
        assert_eq!(
            held_tab_reorder_resolution(&previous, Some(&adjacent), point(px(96.0), px(28.0)),),
            None
        );
    }

    #[test]
    fn excluded_nodes_reject_existing_and_future_scene_facts() {
        let tabs = DockNodeId::null();
        let leaf = DockHostDropSceneFact::Leaf(DockLeafDropTarget {
            root: tabs,
            target_tabs: tabs,
            bounds: bounds(0.0, 0.0, 200.0, 160.0),
            is_central: false,
        });
        let mut scene = DockHostDropScene::new(point(px(20.0), px(20.0)));
        assert!(scene.push_fact(leaf.clone()));

        let mut scene = scene.excluding_nodes(vec![tabs]);
        assert!(scene.leaves.is_empty());
        assert!(!scene.push_fact(leaf));
    }
}
