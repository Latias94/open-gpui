use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockItemId, DockNode, DockNodeId, DockOp,
    DockOpApplyError, DockWorkspace,
};

impl DockWorkspace {
    /// Applies a docking interaction action.
    pub fn apply_action(
        &mut self,
        action: &DockAction,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        match action {
            DockAction::SelectTab { tabs, item } => self.select_tab(*tabs, item),
            DockAction::CloseItem { space, item } => self.close_item_action(space, item),
            DockAction::OpenItem {
                space,
                target_tabs,
                item,
                insert_index,
            } => self.open_item_action(space, *target_tabs, item, *insert_index),
            DockAction::FloatItemInWindow {
                source_space,
                item,
                target_space,
                bounds,
            } => self.float_item_in_window_action(source_space, item, target_space, *bounds),
            DockAction::FloatTabsInWindow {
                source_space,
                source_tabs,
                target_space,
                bounds,
            } => {
                self.float_tabs_in_window_action(source_space, *source_tabs, target_space, *bounds)
            }
            DockAction::SetFloatingBounds {
                space,
                floating,
                bounds,
            } => self.set_floating_bounds_action(space, *floating, *bounds),
            DockAction::RaiseFloating { space, floating } => {
                self.raise_floating_action(space, *floating)
            }
            DockAction::MergeFloatingInto {
                space,
                floating,
                target_tabs,
            } => self.merge_floating_into_action(space, *floating, *target_tabs),
            DockAction::ResizeSplit { split, fractions } => {
                self.resize_split_action(*split, fractions)
            }
        }
    }

    pub(crate) fn commit_graph_op(
        &mut self,
        op: DockOp,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.apply_op_checked(&op)
            .map(DockActionOutcome::from_changed)
            .map_err(Into::into)
    }

    fn select_tab(
        &mut self,
        tabs: DockNodeId,
        item: &DockItemId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        let Some(node) = self.graph().node(tabs) else {
            return Err(DockOpApplyError::TabsNodeNotFound { tabs }.into());
        };
        let DockNode::Tabs { items, active } = node else {
            return Err(DockOpApplyError::NodeIsNotTabs { node: tabs }.into());
        };
        let Some(next_active) = items.iter().position(|candidate| candidate == item) else {
            return Err(DockActionApplyError::ItemNotInTabs {
                tabs,
                item: item.clone(),
            });
        };
        if *active == next_active {
            return Ok(DockActionOutcome::Unchanged);
        }

        self.commit_graph_op(DockOp::SetActiveTab {
            tabs,
            active: next_active,
        })
    }
}
