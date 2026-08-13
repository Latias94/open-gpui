use crate::{
    DockGraphDropTarget, DockGraphMutationError, DockItemId, DockNodeId, DockOp, DockSpaceId,
    DockSplitResize,
};
use std::collections::HashSet;

use super::{DockGraph, DockNode, DropZone};

impl DockGraph {
    pub(in crate::graph) fn target_space_is_empty_for_open(&self, space: &DockSpaceId) -> bool {
        self.root(space).is_none() && self.floating_containers(space).is_empty()
    }

    pub(in crate::graph) fn target_space_is_empty_for_item_move(
        &self,
        source_space: &DockSpaceId,
        item: &DockItemId,
        target_space: &DockSpaceId,
    ) -> bool {
        if self.root(target_space).is_some() {
            return false;
        }
        if source_space != target_space {
            return self.floating_containers(target_space).is_empty();
        }

        let target_items = self.collect_items_in_space(target_space);
        if target_items.is_empty() {
            return true;
        }
        matches!(target_items.as_slice(), [target_item] if target_item == item)
    }

    pub(in crate::graph) fn target_space_is_empty_for_tabs_move(
        &self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        target_space: &DockSpaceId,
    ) -> bool {
        if self.root(target_space).is_some() {
            return false;
        }
        if source_space != target_space {
            return self.floating_containers(target_space).is_empty();
        }

        let target_items = self.collect_items_in_space(target_space);
        if target_items.is_empty() {
            return true;
        }
        let source_items = self.collect_items_in_subtree(source_tabs);
        !source_items.is_empty() && target_items == source_items
    }

    pub(in crate::graph) fn target_space_is_empty_for_floating_move(
        &self,
        source_space: &DockSpaceId,
        floating: DockNodeId,
        target_space: &DockSpaceId,
    ) -> bool {
        if self.root(target_space).is_some() {
            return false;
        }
        if source_space != target_space {
            return self.floating_containers(target_space).is_empty();
        }

        let floatings = self.floating_containers(target_space);
        !floatings.is_empty() && floatings.iter().all(|entry| entry.node == floating)
    }

    pub(in crate::graph) fn require_tabs_node(
        &self,
        tabs: DockNodeId,
    ) -> Result<&[DockItemId], DockGraphMutationError> {
        match self.node(tabs) {
            Some(DockNode::Tabs { items, .. }) => Ok(items),
            Some(_) => Err(DockGraphMutationError::NodeIsNotTabs { node: tabs }),
            None => Err(DockGraphMutationError::TabsNodeNotFound { tabs }),
        }
    }

    pub(in crate::graph) fn require_non_empty_tabs_node(
        &self,
        tabs: DockNodeId,
    ) -> Result<(), DockGraphMutationError> {
        let items = self.require_tabs_node(tabs)?;
        if items.is_empty() {
            return Err(DockGraphMutationError::TabsNodeEmpty { tabs });
        }
        Ok(())
    }

    pub(in crate::graph) fn require_source_node_in_space(
        &self,
        space: &DockSpaceId,
        node: DockNodeId,
    ) -> Result<DockNodeId, DockGraphMutationError> {
        self.root_for_node_in_space(space, node).ok_or_else(|| {
            DockGraphMutationError::SourceNodeNotInSpace {
                space: space.clone(),
                node,
            }
        })
    }

    pub(in crate::graph) fn require_target_node_in_space(
        &self,
        space: &DockSpaceId,
        target: DockNodeId,
    ) -> Result<DockNodeId, DockGraphMutationError> {
        self.root_for_node_in_space(space, target).ok_or_else(|| {
            DockGraphMutationError::TargetNodeNotInSpace {
                space: space.clone(),
                target,
            }
        })
    }

    pub(in crate::graph) fn validate_split_fractions(
        &self,
        split: DockNodeId,
        fractions: &[f32],
    ) -> Result<(), DockGraphMutationError> {
        let Some(node) = self.node(split) else {
            return Err(DockGraphMutationError::SplitNodeNotFound { split });
        };
        let DockNode::Split { children, .. } = node else {
            return Err(DockGraphMutationError::NodeIsNotSplit { node: split });
        };
        if children.len() < 2 {
            return Err(DockGraphMutationError::SplitTooFewChildren {
                split,
                children_len: children.len(),
            });
        }
        if fractions.len() != children.len() {
            return Err(DockGraphMutationError::SplitFractionsLenMismatch {
                split,
                children_len: children.len(),
                fractions_len: fractions.len(),
            });
        }
        for (index, fraction) in fractions.iter().copied().enumerate() {
            if !fraction.is_finite() || fraction < 0.0 {
                return Err(DockGraphMutationError::SplitFractionInvalid { split, index });
            }
        }
        Ok(())
    }

    pub(in crate::graph) fn validate_split_fraction_updates(
        &self,
        updates: &[DockSplitResize],
    ) -> Result<(), DockGraphMutationError> {
        let mut seen = HashSet::new();
        for update in updates {
            if !seen.insert(update.split) {
                return Err(DockGraphMutationError::DuplicateSplitFractionUpdate {
                    split: update.split,
                });
            }
            self.validate_split_fractions(update.split, &update.fractions)?;
        }
        Ok(())
    }
}

impl DockGraph {
    /// Applies an operation with validation for the common error-prone cases.
    pub(crate) fn apply_op_checked(&mut self, op: &DockOp) -> Result<bool, DockGraphMutationError> {
        match op {
            DockOp::SelectTab { tabs, item } => {
                let items = self.require_tabs_node(*tabs)?;
                if !items.contains(item) {
                    return Err(DockGraphMutationError::ItemNotInTabs {
                        tabs: *tabs,
                        item: item.clone(),
                    });
                }
                Ok(self.select_tab(*tabs, item.clone()))
            }
            DockOp::CloseItem { space, item } => {
                if self.find_item_in_space(space, item).is_none() {
                    return Err(DockGraphMutationError::ItemNotFound {
                        space: space.clone(),
                        item: item.clone(),
                    });
                }
                self.apply_tree_mutation_plan(DockTreeMutationPlan::must_change(
                    op,
                    DockTreeMutationExpectation::ItemAbsent {
                        space: space.clone(),
                        item: item.clone(),
                    },
                ))
            }
            DockOp::OpenItem {
                space,
                target_tabs,
                item,
                ..
            } => {
                if self.contains_item(item) {
                    return Err(DockGraphMutationError::ItemAlreadyOpen { item: item.clone() });
                }
                if let Some(target_tabs) = target_tabs {
                    self.validate_open_item_target(space, *target_tabs)?;
                } else if !self.target_space_is_empty_for_open(space) {
                    return Err(DockGraphMutationError::TargetSpaceNotEmpty {
                        space: space.clone(),
                    });
                }
                self.apply_tree_mutation_plan(DockTreeMutationPlan::must_change(
                    op,
                    DockTreeMutationExpectation::ItemsReachable {
                        space: space.clone(),
                        items: vec![item.clone()],
                    },
                ))
            }
            DockOp::MoveItem {
                source_space,
                item,
                target_space,
                target,
            } => {
                self.validate_move_item(source_space, item, target_space, *target)?;
                if source_space == target_space
                    && self.find_item_in_space(source_space, item).is_some_and(
                        |(source_tabs, source_index)| match target {
                            DockGraphDropTarget::Center { tabs } => *tabs == source_tabs,
                            DockGraphDropTarget::TabBar { tabs, insert_index } => {
                                *tabs == source_tabs
                                    && (*insert_index == source_index
                                        || *insert_index == source_index.saturating_add(1))
                            }
                            DockGraphDropTarget::Edge { .. } | DockGraphDropTarget::EmptySpace => {
                                false
                            }
                        },
                    )
                {
                    return Ok(false);
                }
                self.apply_tree_mutation_plan(DockTreeMutationPlan::must_change(
                    op,
                    DockTreeMutationExpectation::ItemsReachable {
                        space: target_space.clone(),
                        items: vec![item.clone()],
                    },
                ))
            }
            DockOp::MoveTabs {
                source_space,
                source_tabs,
                target_space,
                target,
            } => {
                self.validate_move_tabs(source_space, *source_tabs, target_space, *target)?;
                if source_space == target_space
                    && matches!(target, DockGraphDropTarget::Center { tabs } if *tabs == *source_tabs)
                {
                    return Ok(false);
                }
                if source_space == target_space
                    && matches!(target, DockGraphDropTarget::TabBar { tabs, .. } if *tabs == *source_tabs)
                {
                    return Ok(false);
                }
                let items = self.collect_items_in_subtree(*source_tabs);
                self.apply_tree_mutation_plan(DockTreeMutationPlan::must_change(
                    op,
                    DockTreeMutationExpectation::ItemsReachable {
                        space: target_space.clone(),
                        items,
                    },
                ))
            }
            DockOp::MoveFloating {
                source_space,
                floating,
                target_space,
                target,
            } => {
                self.validate_move_floating(source_space, *floating, target_space, *target)?;
                let items = self.collect_items_in_subtree(*floating);
                self.apply_tree_mutation_plan(DockTreeMutationPlan::must_change(
                    op,
                    DockTreeMutationExpectation::ItemsReachable {
                        space: target_space.clone(),
                        items,
                    },
                ))
            }
            DockOp::FloatItemInWindow {
                source_space, item, ..
            } => {
                if self.find_item_in_space(source_space, item).is_none() {
                    return Err(DockGraphMutationError::ItemNotFound {
                        space: source_space.clone(),
                        item: item.clone(),
                    });
                }
                let target_space = match op {
                    DockOp::FloatItemInWindow { target_space, .. } => target_space.clone(),
                    _ => unreachable!(),
                };
                self.apply_tree_mutation_plan(DockTreeMutationPlan::must_change(
                    op,
                    DockTreeMutationExpectation::ItemsReachable {
                        space: target_space,
                        items: vec![item.clone()],
                    },
                ))
            }
            DockOp::FloatTabsInWindow {
                source_space,
                source_tabs,
                target_space,
                ..
            } => {
                self.require_non_empty_tabs_node(*source_tabs)?;
                self.require_source_node_in_space(source_space, *source_tabs)?;
                let items = self.collect_items_in_subtree(*source_tabs);
                self.apply_tree_mutation_plan(DockTreeMutationPlan::must_change(
                    op,
                    DockTreeMutationExpectation::ItemsReachable {
                        space: target_space.clone(),
                        items,
                    },
                ))
            }
            DockOp::SetFloatingBounds {
                space, floating, ..
            }
            | DockOp::RaiseFloating { space, floating } => {
                if self.floating_container(space, *floating).is_none() {
                    return Err(DockGraphMutationError::FloatingContainerNotFound {
                        space: space.clone(),
                        floating: *floating,
                    });
                }
                self.apply_tree_mutation_plan(DockTreeMutationPlan::allow_noop(
                    op,
                    DockTreeMutationExpectation::ValidateOnly,
                ))
            }
            DockOp::SetSplitFractions { split, fractions } => {
                self.validate_split_fractions(*split, fractions)?;
                self.apply_tree_mutation_plan(DockTreeMutationPlan::allow_noop(
                    op,
                    DockTreeMutationExpectation::ValidateOnly,
                ))
            }
            DockOp::SetSplitFractionsMany { updates } => {
                self.validate_split_fraction_updates(updates)?;
                self.apply_tree_mutation_plan(DockTreeMutationPlan::allow_noop(
                    op,
                    DockTreeMutationExpectation::ValidateOnly,
                ))
            }
            #[cfg(test)]
            DockOp::SetSplitFractionTwo {
                split,
                first_fraction,
            } => {
                self.validate_split_fractions(*split, &[*first_fraction, 1.0 - *first_fraction])?;
                self.apply_tree_mutation_plan(DockTreeMutationPlan::allow_noop(
                    op,
                    DockTreeMutationExpectation::ValidateOnly,
                ))
            }
        }
    }

    fn apply_tree_mutation_plan(
        &mut self,
        plan: DockTreeMutationPlan<'_>,
    ) -> Result<bool, DockGraphMutationError> {
        let before = self.clone();
        let before_layout = before.export_layout();
        let retention = before.capture_mutation_retention();
        let mut next = before.clone();
        let changed = next.apply_op_unchecked(plan.op);
        if changed {
            next.finalize_checked_mutation(retention);
        }
        let next_layout = next.export_layout();

        if !changed {
            if next_layout != before_layout {
                return Err(plan
                    .invariant_violation("mutation reported no change after modifying the graph"));
            }
            if plan.allow_noop {
                return Ok(false);
            }
            return Err(
                plan.invariant_violation("validated mutation did not attach its planned subtree")
            );
        }

        if next_layout == before_layout {
            return Err(
                plan.invariant_violation("mutation reported a change without changing graph state")
            );
        }
        next.validate()
            .map_err(|error| plan.invariant_violation(error.to_string()))?;
        plan.expectation.verify(&next, plan.op_name())?;
        *self = next;
        Ok(true)
    }

    fn validate_move_item(
        &self,
        source_space: &DockSpaceId,
        item: &DockItemId,
        target_space: &DockSpaceId,
        target: DockGraphDropTarget,
    ) -> Result<(), DockGraphMutationError> {
        if matches!(target, DockGraphDropTarget::EmptySpace)
            && !self.target_space_is_empty_for_item_move(source_space, item, target_space)
        {
            return Err(DockGraphMutationError::TargetSpaceNotEmpty {
                space: target_space.clone(),
            });
        }
        if self.find_item_in_space(source_space, item).is_none() {
            return Err(DockGraphMutationError::ItemNotFound {
                space: source_space.clone(),
                item: item.clone(),
            });
        }
        self.validate_graph_drop_target(target_space, target)?;
        Ok(())
    }

    fn validate_open_item_target(
        &self,
        space: &DockSpaceId,
        target_tabs: DockNodeId,
    ) -> Result<(), DockGraphMutationError> {
        self.require_target_node_in_space(space, target_tabs)?;
        self.require_tabs_node(target_tabs)?;
        Ok(())
    }

    fn validate_move_tabs(
        &self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        target_space: &DockSpaceId,
        target: DockGraphDropTarget,
    ) -> Result<(), DockGraphMutationError> {
        if matches!(target, DockGraphDropTarget::EmptySpace)
            && !self.target_space_is_empty_for_tabs_move(source_space, source_tabs, target_space)
        {
            return Err(DockGraphMutationError::TargetSpaceNotEmpty {
                space: target_space.clone(),
            });
        }
        self.require_non_empty_tabs_node(source_tabs)?;
        self.require_source_node_in_space(source_space, source_tabs)?;
        self.validate_graph_drop_target(target_space, target)?;
        Ok(())
    }

    fn validate_move_floating(
        &self,
        source_space: &DockSpaceId,
        floating: DockNodeId,
        target_space: &DockSpaceId,
        target: DockGraphDropTarget,
    ) -> Result<(), DockGraphMutationError> {
        if self.floating_container(source_space, floating).is_none() {
            return Err(DockGraphMutationError::FloatingContainerNotFound {
                space: source_space.clone(),
                floating,
            });
        }
        if let DockGraphDropTarget::Center { tabs } | DockGraphDropTarget::TabBar { tabs, .. } =
            target
            && let Some(DockNode::Floating { child }) = self.node(floating)
            && self.is_visible_split_payload(*child)
        {
            return Err(
                DockGraphMutationError::VisibleSplitPayloadCannotDockOverNonEmptyTarget {
                    payload: *child,
                    target: tabs,
                },
            );
        }
        if matches!(target, DockGraphDropTarget::EmptySpace)
            && !self.target_space_is_empty_for_floating_move(source_space, floating, target_space)
        {
            return Err(DockGraphMutationError::TargetSpaceNotEmpty {
                space: target_space.clone(),
            });
        }
        self.validate_graph_drop_target(target_space, target)?;
        if let Some(target_node) = target.existing_node()
            && source_space == target_space
            && self.subtree_contains(floating, target_node)
        {
            return Err(DockGraphMutationError::CannotMergeFloatingIntoOwnSubtree {
                floating,
                target: target_node,
            });
        }
        Ok(())
    }

    fn validate_graph_drop_target(
        &self,
        space: &DockSpaceId,
        target: DockGraphDropTarget,
    ) -> Result<(), DockGraphMutationError> {
        match target {
            DockGraphDropTarget::Center { tabs } | DockGraphDropTarget::TabBar { tabs, .. } => {
                self.require_target_node_in_space(space, tabs)?;
                self.require_tabs_node(tabs)?;
            }
            DockGraphDropTarget::Edge { plan } => {
                if !self.edge_dock_plan_is_current(space, plan) {
                    return Err(DockGraphMutationError::MutationInvariantViolation {
                        op: "DockGraphDropTarget",
                        reason: "edge graph drop plan is no longer current".into(),
                    });
                }
                if plan.drop_zone() == DropZone::Center {
                    return Err(DockGraphMutationError::MutationInvariantViolation {
                        op: "DockGraphDropTarget",
                        reason: "edge graph drop target cannot use center drop zone".into(),
                    });
                }
            }
            DockGraphDropTarget::EmptySpace => {}
        }
        Ok(())
    }
}

struct DockTreeMutationPlan<'a> {
    op: &'a DockOp,
    expectation: DockTreeMutationExpectation,
    allow_noop: bool,
}

impl<'a> DockTreeMutationPlan<'a> {
    fn must_change(op: &'a DockOp, expectation: DockTreeMutationExpectation) -> Self {
        Self {
            op,
            expectation,
            allow_noop: false,
        }
    }

    fn allow_noop(op: &'a DockOp, expectation: DockTreeMutationExpectation) -> Self {
        Self {
            op,
            expectation,
            allow_noop: true,
        }
    }

    fn op_name(&self) -> &'static str {
        match self.op {
            DockOp::SelectTab { .. } => "SelectTab",
            DockOp::CloseItem { .. } => "CloseItem",
            DockOp::OpenItem { .. } => "OpenItem",
            DockOp::MoveItem { .. } => "MoveItem",
            DockOp::MoveTabs { .. } => "MoveTabs",
            DockOp::MoveFloating { .. } => "MoveFloating",
            DockOp::FloatItemInWindow { .. } => "FloatItemInWindow",
            DockOp::FloatTabsInWindow { .. } => "FloatTabsInWindow",
            DockOp::SetFloatingBounds { .. } => "SetFloatingBounds",
            DockOp::RaiseFloating { .. } => "RaiseFloating",
            DockOp::SetSplitFractions { .. } => "SetSplitFractions",
            DockOp::SetSplitFractionsMany { .. } => "SetSplitFractionsMany",
            #[cfg(test)]
            DockOp::SetSplitFractionTwo { .. } => "SetSplitFractionTwo",
        }
    }

    fn invariant_violation(&self, reason: impl Into<String>) -> DockGraphMutationError {
        DockGraphMutationError::MutationInvariantViolation {
            op: self.op_name(),
            reason: reason.into(),
        }
    }
}

enum DockTreeMutationExpectation {
    ValidateOnly,
    ItemsReachable {
        space: DockSpaceId,
        items: Vec<DockItemId>,
    },
    ItemAbsent {
        space: DockSpaceId,
        item: DockItemId,
    },
}

impl DockTreeMutationExpectation {
    fn verify(&self, graph: &DockGraph, op: &'static str) -> Result<(), DockGraphMutationError> {
        match self {
            Self::ValidateOnly => Ok(()),
            Self::ItemsReachable { space, items } => {
                let reachable = graph.collect_items_in_space(space);
                for item in items {
                    if !reachable.contains(item) {
                        return Err(DockGraphMutationError::MutationInvariantViolation {
                            op,
                            reason: format!(
                                "item {item} was not reachable in target space {space}"
                            ),
                        });
                    }
                }
                Ok(())
            }
            Self::ItemAbsent { space, item } => {
                if graph.find_item_in_space(space, item).is_some() {
                    return Err(DockGraphMutationError::MutationInvariantViolation {
                        op,
                        reason: format!("item {item} remained reachable in source space {space}"),
                    });
                }
                Ok(())
            }
        }
    }
}
