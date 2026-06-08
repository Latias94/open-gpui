use crate::{DockItemId, DockNodeId, DockOp, DockOpApplyError, DockSpaceId, SplitFractionsUpdate};
use std::collections::HashSet;

use super::{DockGraph, DockNode, DropZone};

impl DockGraph {
    /// Applies an operation with validation for the common error-prone cases.
    pub fn apply_op_checked(&mut self, op: &DockOp) -> Result<bool, DockOpApplyError> {
        match op {
            DockOp::SetActiveTab { tabs, active } => {
                let Some(node) = self.node(*tabs) else {
                    return Err(DockOpApplyError::TabsNodeNotFound { tabs: *tabs });
                };
                let DockNode::Tabs { items, .. } = node else {
                    return Err(DockOpApplyError::NodeIsNotTabs { node: *tabs });
                };
                if *active >= items.len() {
                    return Err(DockOpApplyError::ActiveOutOfBounds {
                        tabs: *tabs,
                        active: *active,
                        len: items.len(),
                    });
                }
                Ok(self.set_active_tab(*tabs, *active))
            }
            DockOp::CloseItem { space, item } => {
                if self.close_item(space, item.clone()) {
                    Ok(true)
                } else {
                    Err(DockOpApplyError::ItemNotFound {
                        space: space.clone(),
                        item: item.clone(),
                    })
                }
            }
            DockOp::OpenItem {
                space,
                target_tabs,
                item,
                ..
            } => {
                if self.contains_item(item) {
                    return Err(DockOpApplyError::ItemAlreadyOpen { item: item.clone() });
                }
                if let Some(target_tabs) = target_tabs {
                    self.validate_open_item_target(space, *target_tabs)?;
                } else if !self.target_space_is_empty_for_open(space) {
                    return Err(DockOpApplyError::TargetSpaceNotEmpty {
                        space: space.clone(),
                    });
                }
                Ok(self.apply_op(op))
            }
            DockOp::MoveItem {
                source_space,
                item,
                target_space,
                target_tabs,
                zone,
                insert_index,
            } => {
                self.validate_move_item(source_space, item, target_space, *target_tabs, *zone)?;
                if source_space == target_space
                    && self
                        .find_item_in_space(source_space, item)
                        .is_some_and(|(source_tabs, _)| source_tabs == *target_tabs)
                    && *zone == DropZone::Center
                    && insert_index.is_none()
                {
                    return Ok(false);
                }
                Ok(self.apply_op(op))
            }
            DockOp::MoveTabs {
                source_space,
                source_tabs,
                target_space,
                target_tabs,
                zone,
                ..
            } => {
                self.validate_move_tabs(
                    source_space,
                    *source_tabs,
                    target_space,
                    *target_tabs,
                    *zone,
                )?;
                if source_space == target_space
                    && *source_tabs == *target_tabs
                    && *zone == DropZone::Center
                {
                    return Ok(false);
                }
                Ok(self.apply_op(op))
            }
            DockOp::MoveItemToEmptyDockSpace {
                source_space,
                item,
                target_space,
            } => {
                if !self.target_space_is_empty_for_item_move(source_space, item, target_space) {
                    return Err(DockOpApplyError::TargetSpaceNotEmpty {
                        space: target_space.clone(),
                    });
                }
                if self.find_item_in_space(source_space, item).is_none() {
                    return Err(DockOpApplyError::ItemNotFound {
                        space: source_space.clone(),
                        item: item.clone(),
                    });
                }
                Ok(self.apply_op(op))
            }
            DockOp::MoveTabsToEmptyDockSpace {
                source_space,
                source_tabs,
                target_space,
            } => {
                if !self.target_space_is_empty_for_tabs_move(
                    source_space,
                    *source_tabs,
                    target_space,
                ) {
                    return Err(DockOpApplyError::TargetSpaceNotEmpty {
                        space: target_space.clone(),
                    });
                }
                let Some(node) = self.node(*source_tabs) else {
                    return Err(DockOpApplyError::TabsNodeNotFound { tabs: *source_tabs });
                };
                let DockNode::Tabs { items, .. } = node else {
                    return Err(DockOpApplyError::NodeIsNotTabs { node: *source_tabs });
                };
                if items.is_empty() {
                    return Err(DockOpApplyError::TabsNodeEmpty { tabs: *source_tabs });
                }
                if self
                    .root_for_node_in_space(source_space, *source_tabs)
                    .is_none()
                {
                    return Err(DockOpApplyError::SourceNodeNotInSpace {
                        space: source_space.clone(),
                        node: *source_tabs,
                    });
                }
                Ok(self.apply_op(op))
            }
            DockOp::FloatItemInWindow {
                source_space, item, ..
            } => {
                if self.find_item_in_space(source_space, item).is_none() {
                    return Err(DockOpApplyError::ItemNotFound {
                        space: source_space.clone(),
                        item: item.clone(),
                    });
                }
                Ok(self.apply_op(op))
            }
            DockOp::FloatTabsInWindow {
                source_space,
                source_tabs,
                ..
            } => {
                let Some(node) = self.node(*source_tabs) else {
                    return Err(DockOpApplyError::TabsNodeNotFound { tabs: *source_tabs });
                };
                match node {
                    DockNode::Tabs { items, .. } if items.is_empty() => {
                        return Err(DockOpApplyError::TabsNodeEmpty { tabs: *source_tabs });
                    }
                    DockNode::Tabs { .. } => {}
                    _ => return Err(DockOpApplyError::NodeIsNotTabs { node: *source_tabs }),
                }
                if self
                    .root_for_node_in_space(source_space, *source_tabs)
                    .is_none()
                {
                    return Err(DockOpApplyError::SourceNodeNotInSpace {
                        space: source_space.clone(),
                        node: *source_tabs,
                    });
                }
                Ok(self.apply_op(op))
            }
            DockOp::SetFloatingBounds {
                space, floating, ..
            }
            | DockOp::RaiseFloating { space, floating } => {
                if self.floating_container(space, *floating).is_none() {
                    return Err(DockOpApplyError::FloatingContainerNotFound {
                        space: space.clone(),
                        floating: *floating,
                    });
                }
                Ok(self.apply_op(op))
            }
            DockOp::MergeFloatingInto {
                space,
                floating,
                target_tabs,
            } => {
                if self.floating_container(space, *floating).is_none() {
                    return Err(DockOpApplyError::FloatingContainerNotFound {
                        space: space.clone(),
                        floating: *floating,
                    });
                }
                match self.node(*target_tabs) {
                    Some(DockNode::Tabs { .. }) => {}
                    Some(_) => return Err(DockOpApplyError::NodeIsNotTabs { node: *target_tabs }),
                    None => return Err(DockOpApplyError::TabsNodeNotFound { tabs: *target_tabs }),
                }
                let Some(target_root) = self.root_for_node_in_space(space, *target_tabs) else {
                    return Err(DockOpApplyError::TargetNodeNotInSpace {
                        space: space.clone(),
                        target: *target_tabs,
                    });
                };
                if target_root == *floating {
                    return Err(DockOpApplyError::CannotMergeFloatingIntoOwnSubtree {
                        floating: *floating,
                        target: *target_tabs,
                    });
                }
                Ok(self.apply_op(op))
            }
            DockOp::SetSplitFractions { split, fractions } => {
                self.validate_split_fractions(*split, fractions)?;
                Ok(self.apply_op(op))
            }
            DockOp::SetSplitFractionsMany { updates } => {
                self.validate_split_fraction_updates(updates)?;
                Ok(self.apply_op(op))
            }
            DockOp::SetSplitFractionTwo {
                split,
                first_fraction,
            } => {
                self.validate_split_fractions(*split, &[*first_fraction, 1.0 - *first_fraction])?;
                Ok(self.apply_op(op))
            }
        }
    }
    fn validate_move_item(
        &self,
        source_space: &DockSpaceId,
        item: &DockItemId,
        target_space: &DockSpaceId,
        target_tabs: DockNodeId,
        zone: DropZone,
    ) -> Result<(), DockOpApplyError> {
        if self.find_item_in_space(source_space, item).is_none() {
            return Err(DockOpApplyError::ItemNotFound {
                space: source_space.clone(),
                item: item.clone(),
            });
        }
        if self
            .root_for_node_in_space(target_space, target_tabs)
            .is_none()
        {
            return Err(DockOpApplyError::TargetNodeNotInSpace {
                space: target_space.clone(),
                target: target_tabs,
            });
        }
        if zone == DropZone::Center {
            match self.node(target_tabs) {
                Some(DockNode::Tabs { .. }) => {}
                Some(_) => return Err(DockOpApplyError::NodeIsNotTabs { node: target_tabs }),
                None => return Err(DockOpApplyError::TabsNodeNotFound { tabs: target_tabs }),
            }
        }
        Ok(())
    }

    fn validate_open_item_target(
        &self,
        space: &DockSpaceId,
        target_tabs: DockNodeId,
    ) -> Result<(), DockOpApplyError> {
        if self.root_for_node_in_space(space, target_tabs).is_none() {
            return Err(DockOpApplyError::TargetNodeNotInSpace {
                space: space.clone(),
                target: target_tabs,
            });
        }
        match self.node(target_tabs) {
            Some(DockNode::Tabs { .. }) => Ok(()),
            Some(_) => Err(DockOpApplyError::NodeIsNotTabs { node: target_tabs }),
            None => Err(DockOpApplyError::TabsNodeNotFound { tabs: target_tabs }),
        }
    }

    fn target_space_is_empty_for_open(&self, space: &DockSpaceId) -> bool {
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

    fn validate_move_tabs(
        &self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        target_space: &DockSpaceId,
        target_tabs: DockNodeId,
        zone: DropZone,
    ) -> Result<(), DockOpApplyError> {
        let Some(source_node) = self.node(source_tabs) else {
            return Err(DockOpApplyError::TabsNodeNotFound { tabs: source_tabs });
        };
        match source_node {
            DockNode::Tabs { items, .. } if items.is_empty() => {
                return Err(DockOpApplyError::TabsNodeEmpty { tabs: source_tabs });
            }
            DockNode::Tabs { .. } => {}
            _ => return Err(DockOpApplyError::NodeIsNotTabs { node: source_tabs }),
        }
        if self
            .root_for_node_in_space(source_space, source_tabs)
            .is_none()
        {
            return Err(DockOpApplyError::SourceNodeNotInSpace {
                space: source_space.clone(),
                node: source_tabs,
            });
        }
        if self
            .root_for_node_in_space(target_space, target_tabs)
            .is_none()
        {
            return Err(DockOpApplyError::TargetNodeNotInSpace {
                space: target_space.clone(),
                target: target_tabs,
            });
        }
        if zone == DropZone::Center {
            match self.node(target_tabs) {
                Some(DockNode::Tabs { .. }) => {}
                Some(_) => return Err(DockOpApplyError::NodeIsNotTabs { node: target_tabs }),
                None => return Err(DockOpApplyError::TabsNodeNotFound { tabs: target_tabs }),
            }
        }
        Ok(())
    }

    fn validate_split_fractions(
        &self,
        split: DockNodeId,
        fractions: &[f32],
    ) -> Result<(), DockOpApplyError> {
        let Some(node) = self.node(split) else {
            return Err(DockOpApplyError::SplitNodeNotFound { split });
        };
        let DockNode::Split { children, .. } = node else {
            return Err(DockOpApplyError::NodeIsNotSplit { node: split });
        };
        if children.len() < 2 {
            return Err(DockOpApplyError::SplitTooFewChildren {
                split,
                children_len: children.len(),
            });
        }
        if fractions.len() != children.len() {
            return Err(DockOpApplyError::SplitFractionsLenMismatch {
                split,
                children_len: children.len(),
                fractions_len: fractions.len(),
            });
        }
        for (index, fraction) in fractions.iter().copied().enumerate() {
            if !fraction.is_finite() || fraction < 0.0 {
                return Err(DockOpApplyError::SplitFractionInvalid { split, index });
            }
        }
        Ok(())
    }

    fn validate_split_fraction_updates(
        &self,
        updates: &[SplitFractionsUpdate],
    ) -> Result<(), DockOpApplyError> {
        let mut seen = HashSet::new();
        for update in updates {
            if !seen.insert(update.split) {
                return Err(DockOpApplyError::DuplicateSplitFractionUpdate {
                    split: update.split,
                });
            }
            self.validate_split_fractions(update.split, &update.fractions)?;
        }
        Ok(())
    }
}
