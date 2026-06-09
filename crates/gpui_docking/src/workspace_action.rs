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
            DockAction::SelectTab { tabs, item } => self.commit_select_tab(*tabs, item),
            DockAction::CloseItem { space, item } => self.commit_close_item(space, item),
            DockAction::OpenItem {
                space,
                target_tabs,
                item,
                insert_index,
            } => self.commit_open_item(space, *target_tabs, item, *insert_index),
            DockAction::FloatItemInWindow {
                source_space,
                item,
                target_space,
                bounds,
            } => self.commit_float_item_in_window(source_space, item, target_space, *bounds),
            DockAction::FloatTabsInWindow {
                source_space,
                source_tabs,
                target_space,
                bounds,
            } => {
                self.commit_float_tabs_in_window(source_space, *source_tabs, target_space, *bounds)
            }
            DockAction::SetFloatingBounds {
                space,
                floating,
                bounds,
            } => self.commit_set_floating_bounds(space, *floating, *bounds),
            DockAction::RaiseFloating { space, floating } => {
                self.commit_raise_floating(space, *floating)
            }
            DockAction::MergeFloatingInto {
                space,
                floating,
                target_tabs,
            } => self.commit_merge_floating_into(space, *floating, *target_tabs),
            DockAction::ResizeSplit { split, fractions } => {
                self.commit_resize_split(*split, fractions)
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

    pub(crate) fn commit_select_tab(
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
