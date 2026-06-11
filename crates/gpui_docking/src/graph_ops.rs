use crate::DockOp;
#[cfg(test)]
use crate::{DockNodeId, SplitFractionsUpdate};

use super::DockGraph;

impl DockGraph {
    /// Applies an operation and returns whether it changed or preserved a valid graph state.
    pub(crate) fn apply_op(&mut self, op: &DockOp) -> bool {
        match op {
            DockOp::SetActiveTab { tabs, active } => self.set_active_tab(*tabs, *active),
            DockOp::CloseItem { space, item } => self.close_item(space, item.clone()),
            DockOp::OpenItem {
                space,
                target_tabs,
                item,
                insert_index,
            } => self.open_item(space, *target_tabs, item.clone(), *insert_index),
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
                if !self.target_space_is_empty_for_item_move(source_space, item, target_space) {
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
                if !self.target_space_is_empty_for_tabs_move(
                    source_space,
                    *source_tabs,
                    target_space,
                ) {
                    return false;
                }
                self.move_tabs_to_empty_space(source_space, *source_tabs, target_space)
            }
            DockOp::MoveFloating {
                source_space,
                floating,
                target_space,
                target,
                zone,
            } => self.move_floating_between_spaces(
                source_space,
                *floating,
                target_space,
                *target,
                *zone,
            ),
            DockOp::MoveFloatingToEmptyDockSpace {
                source_space,
                floating,
                target_space,
            } => self.move_floating_to_empty_space(source_space, *floating, target_space),
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
            #[cfg(test)]
            DockOp::SetSplitFractionsMany { updates } => {
                let mut changed = false;
                for update in updates {
                    changed |= self.update_split_fractions(update.split, update.fractions.clone());
                }
                changed
            }
            #[cfg(test)]
            DockOp::SetSplitFractionTwo {
                split,
                first_fraction,
            } => self.update_split_two(*split, *first_fraction),
        }
    }
}

#[cfg(test)]
impl From<SplitFractionsUpdate> for (DockNodeId, Vec<f32>) {
    fn from(update: SplitFractionsUpdate) -> Self {
        (update.split, update.fractions)
    }
}
