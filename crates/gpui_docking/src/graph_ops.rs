use crate::DockOp;
#[cfg(test)]
use crate::{DockNodeId, SplitFractionsUpdate};

use super::DockGraph;

impl DockGraph {
    /// Applies an operation without validation.
    pub(in crate::graph) fn apply_op_unchecked(&mut self, op: &DockOp) -> bool {
        match op {
            DockOp::SelectTab { tabs, item } => self.select_tab(*tabs, item.clone()),
            DockOp::CloseItem {
                space,
                item,
                preferred_after_close,
            } => self.close_item(space, item.clone(), preferred_after_close.as_ref()),
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
                target,
            } => self.move_item_between_spaces(source_space, item.clone(), target_space, *target),
            DockOp::MoveTabs {
                source_space,
                source_tabs,
                target_space,
                target,
            } => self.move_tabs_between_spaces(source_space, *source_tabs, target_space, *target),
            DockOp::MoveFloating {
                source_space,
                floating,
                target_space,
                target,
            } => self.move_floating_between_spaces(source_space, *floating, target_space, *target),
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
