use crate::{DockItemId, DockNodeId, DockOp, DockOpApplyError, DockSpaceId};

use super::{DockGraph, DropZone};

impl DockGraph {
    /// Applies an operation with validation for the common error-prone cases.
    pub fn apply_op_checked(&mut self, op: &DockOp) -> Result<bool, DockOpApplyError> {
        match op {
            DockOp::SetActiveTab { tabs, active } => {
                let items = self.require_tabs_node(*tabs)?;
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
                self.require_non_empty_tabs_node(*source_tabs)?;
                self.require_source_node_in_space(source_space, *source_tabs)?;
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
                self.require_non_empty_tabs_node(*source_tabs)?;
                self.require_source_node_in_space(source_space, *source_tabs)?;
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
                self.require_tabs_node(*target_tabs)?;
                let target_root = self.require_target_node_in_space(space, *target_tabs)?;
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
        self.require_target_node_in_space(target_space, target_tabs)?;
        if zone == DropZone::Center {
            self.require_tabs_node(target_tabs)?;
        }
        Ok(())
    }

    fn validate_open_item_target(
        &self,
        space: &DockSpaceId,
        target_tabs: DockNodeId,
    ) -> Result<(), DockOpApplyError> {
        self.require_target_node_in_space(space, target_tabs)?;
        self.require_tabs_node(target_tabs)?;
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
        self.require_non_empty_tabs_node(source_tabs)?;
        self.require_source_node_in_space(source_space, source_tabs)?;
        self.require_target_node_in_space(target_space, target_tabs)?;
        if zone == DropZone::Center {
            self.require_tabs_node(target_tabs)?;
        }
        Ok(())
    }
}
