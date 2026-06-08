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
                if self.root(target_space).is_some() {
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
                if self.root(target_space).is_some() {
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

    /// Applies an operation and returns whether it changed or preserved a valid graph state.
    pub(crate) fn apply_op(&mut self, op: &DockOp) -> bool {
        match op {
            DockOp::SetActiveTab { tabs, active } => self.set_active_tab(*tabs, *active),
            DockOp::CloseItem { space, item } => self.close_item(space, item.clone()),
            DockOp::MoveItem {
                source_space,
                item,
                target_space,
                target_tabs,
                zone,
                insert_index,
            } => self.move_item_between_spaces(
                source_space,
                item.clone(),
                target_space,
                *target_tabs,
                *zone,
                *insert_index,
            ),
            DockOp::MoveItemToEmptyDockSpace {
                source_space,
                item,
                target_space,
            } => {
                if self.root(target_space).is_some() {
                    return false;
                }
                self.move_item_to_empty_space(source_space, item.clone(), target_space)
            }
            DockOp::MoveTabs {
                source_space,
                source_tabs,
                target_space,
                target_tabs,
                zone,
                insert_index,
            } => self.move_tabs_between_spaces(
                source_space,
                *source_tabs,
                target_space,
                *target_tabs,
                *zone,
                *insert_index,
            ),
            DockOp::MoveTabsToEmptyDockSpace {
                source_space,
                source_tabs,
                target_space,
            } => {
                if self.root(target_space).is_some() {
                    return false;
                }
                self.move_tabs_to_empty_space(source_space, *source_tabs, target_space)
            }
            DockOp::FloatItemInWindow {
                source_space,
                item,
                target_space,
                bounds,
            } => self.float_item_in_window(source_space, item.clone(), target_space, *bounds),
            DockOp::FloatTabsInWindow {
                source_space,
                source_tabs,
                target_space,
                bounds,
            } => self.float_tabs_in_window(source_space, *source_tabs, target_space, *bounds),
            DockOp::SetFloatingBounds {
                space,
                floating,
                bounds,
            } => self.set_floating_bounds(space, *floating, *bounds),
            DockOp::RaiseFloating { space, floating } => self.raise_floating(space, *floating),
            DockOp::MergeFloatingInto {
                space,
                floating,
                target_tabs,
            } => self.merge_floating_into(space, *floating, *target_tabs),
            DockOp::SetSplitFractions { split, fractions } => {
                self.update_split_fractions(*split, fractions.clone())
            }
            DockOp::SetSplitFractionsMany { updates } => {
                let mut changed = false;
                for update in updates {
                    changed |= self.update_split_fractions(update.split, update.fractions.clone());
                }
                changed
            }
            DockOp::SetSplitFractionTwo {
                split,
                first_fraction,
            } => self.update_split_two(*split, *first_fraction),
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

impl From<SplitFractionsUpdate> for (DockNodeId, Vec<f32>) {
    fn from(update: SplitFractionsUpdate) -> Self {
        (update.split, update.fractions)
    }
}
