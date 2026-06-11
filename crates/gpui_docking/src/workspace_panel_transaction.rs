use crate::{
    DockActionApplyError, DockActionOutcome, DockItemId, DockNodeId, DockOp, DockSpaceId,
    DockWorkspace,
};

impl DockWorkspace {
    pub(crate) fn commit_close_item(
        &mut self,
        space: &DockSpaceId,
        item: &DockItemId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.panel_lifecycle().validate_close(item)?;

        self.commit_graph_op(DockOp::CloseItem {
            space: space.clone(),
            item: item.clone(),
        })
    }

    pub(crate) fn commit_open_item(
        &mut self,
        space: &DockSpaceId,
        target_tabs: Option<DockNodeId>,
        item: &DockItemId,
        insert_index: Option<usize>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.panel_lifecycle().validate_open(item)?;
        self.move_validation()
            .validate_item_target_space(space, item)?;

        self.commit_graph_op(DockOp::OpenItem {
            space: space.clone(),
            target_tabs,
            item: item.clone(),
            insert_index,
        })
    }
}
