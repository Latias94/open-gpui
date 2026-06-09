use crate::{
    DockActionApplyError, DockActionOutcome, DockItemId, DockNodeId, DockOp, DockSpaceId,
    DockWorkspace, DropZone,
};

pub(crate) struct DockWorkspaceMoveTabRequest<'a> {
    pub(crate) source_space: &'a DockSpaceId,
    pub(crate) source_tabs: DockNodeId,
    pub(crate) item: &'a DockItemId,
    pub(crate) target_space: &'a DockSpaceId,
    pub(crate) target_tabs: DockNodeId,
    pub(crate) zone: DropZone,
    pub(crate) insert_index: Option<usize>,
}

pub(crate) struct DockWorkspaceMoveTabsRequest<'a> {
    pub(crate) source_space: &'a DockSpaceId,
    pub(crate) source_tabs: DockNodeId,
    pub(crate) target_space: &'a DockSpaceId,
    pub(crate) target_tabs: DockNodeId,
    pub(crate) zone: DropZone,
    pub(crate) insert_index: Option<usize>,
}

impl DockWorkspace {
    pub(crate) fn commit_tab_move(
        &mut self,
        request: DockWorkspaceMoveTabRequest<'_>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        let DockWorkspaceMoveTabRequest {
            source_space,
            source_tabs,
            item,
            target_space,
            target_tabs,
            zone,
            insert_index,
        } = request;

        self.move_validation()
            .validate_move_tab_source(source_space, source_tabs, item)?;
        self.policy().validate_drop_zone(zone)?;
        if source_space == target_space && source_tabs == target_tabs && zone == DropZone::Center {
            self.policy().validate_same_stack_center_drop()?;
            if insert_index.is_none() {
                return Ok(DockActionOutcome::Unchanged);
            }
        }

        self.commit_graph_op(DockOp::MoveItem {
            source_space: source_space.clone(),
            item: item.clone(),
            target_space: target_space.clone(),
            target_tabs,
            zone,
            insert_index,
        })
    }

    pub(crate) fn commit_tabs_move(
        &mut self,
        request: DockWorkspaceMoveTabsRequest<'_>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        let DockWorkspaceMoveTabsRequest {
            source_space,
            source_tabs,
            target_space,
            target_tabs,
            zone,
            insert_index,
        } = request;

        self.policy().validate_drop_zone(zone)?;
        if source_space == target_space && source_tabs == target_tabs && zone == DropZone::Center {
            self.policy().validate_same_stack_center_drop()?;
            return Ok(DockActionOutcome::Unchanged);
        }

        self.commit_graph_op(DockOp::MoveTabs {
            source_space: source_space.clone(),
            source_tabs,
            target_space: target_space.clone(),
            target_tabs,
            zone,
            insert_index,
        })
    }

    pub(crate) fn commit_item_to_empty_dock_space(
        &mut self,
        source_space: &DockSpaceId,
        item: &DockItemId,
        target_space: &DockSpaceId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_platform_viewports()?;
        self.commit_graph_op(DockOp::MoveItemToEmptyDockSpace {
            source_space: source_space.clone(),
            item: item.clone(),
            target_space: target_space.clone(),
        })
    }

    pub(crate) fn commit_tabs_to_empty_dock_space(
        &mut self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        target_space: &DockSpaceId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_platform_viewports()?;
        self.commit_graph_op(DockOp::MoveTabsToEmptyDockSpace {
            source_space: source_space.clone(),
            source_tabs,
            target_space: target_space.clone(),
        })
    }
}
