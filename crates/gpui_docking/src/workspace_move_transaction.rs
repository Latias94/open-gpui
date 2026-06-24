use crate::{
    DockActionApplyError, DockActionOutcome, DockGraphDropTarget, DockItemId, DockNodeId, DockOp,
    DockPolicy, DockSpaceId, DockWorkspace,
};

pub(crate) enum DockWorkspaceMove<'a> {
    Item {
        source_space: &'a DockSpaceId,
        source_tabs: DockNodeId,
        item: &'a DockItemId,
        target_space: &'a DockSpaceId,
        target: DockGraphDropTarget,
    },
    Tabs {
        source_space: &'a DockSpaceId,
        source_tabs: DockNodeId,
        target_space: &'a DockSpaceId,
        target: DockGraphDropTarget,
    },
    Floating {
        source_space: &'a DockSpaceId,
        floating: DockNodeId,
        target_space: &'a DockSpaceId,
        target: DockGraphDropTarget,
    },
}

impl DockWorkspace {
    pub(crate) fn commit_tab_move(
        &mut self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        item: &DockItemId,
        target_space: &DockSpaceId,
        target: DockGraphDropTarget,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.commit_move(DockWorkspaceMove::Item {
            source_space,
            source_tabs,
            item,
            target_space,
            target,
        })
    }

    pub(crate) fn commit_tabs_move(
        &mut self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        target_space: &DockSpaceId,
        target: DockGraphDropTarget,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.commit_move(DockWorkspaceMove::Tabs {
            source_space,
            source_tabs,
            target_space,
            target,
        })
    }

    pub(crate) fn commit_item_to_empty_dock_space(
        &mut self,
        source_space: &DockSpaceId,
        item: &DockItemId,
        target_space: &DockSpaceId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_platform_viewports()?;
        self.move_validation()
            .validate_item_target_space(target_space, item)?;
        self.commit_graph_op(DockOp::MoveItem {
            source_space: source_space.clone(),
            item: item.clone(),
            target_space: target_space.clone(),
            target: DockGraphDropTarget::empty_space(),
        })
    }

    pub(crate) fn commit_tabs_to_empty_dock_space(
        &mut self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        target_space: &DockSpaceId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_platform_viewports()?;
        self.move_validation()
            .validate_tabs_target_space(target_space, source_tabs)?;
        self.commit_graph_op(DockOp::MoveTabs {
            source_space: source_space.clone(),
            source_tabs,
            target_space: target_space.clone(),
            target: DockGraphDropTarget::empty_space(),
        })
    }

    pub(crate) fn commit_move(
        &mut self,
        request: DockWorkspaceMove<'_>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        match request {
            DockWorkspaceMove::Item {
                source_space,
                source_tabs,
                item,
                target_space,
                target,
            } => self.commit_item_move(source_space, source_tabs, item, target_space, target),
            DockWorkspaceMove::Tabs {
                source_space,
                source_tabs,
                target_space,
                target,
            } => self.commit_tabs_move_impl(source_space, source_tabs, target_space, target),
            DockWorkspaceMove::Floating {
                source_space,
                floating,
                target_space,
                target,
            } => self.commit_floating_move_impl(source_space, floating, target_space, target),
        }
    }

    fn commit_item_move(
        &mut self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        item: &DockItemId,
        target_space: &DockSpaceId,
        target: DockGraphDropTarget,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.move_validation()
            .validate_move_tab_source(source_space, source_tabs, item)?;
        self.move_validation()
            .validate_item_target_space(target_space, item)?;
        validate_graph_drop_target_policy(self.policy(), target)?;
        if source_space == target_space && target.center_tabs() == Some(source_tabs) {
            self.policy().validate_same_stack_center_drop()?;
        }

        self.commit_graph_op(DockOp::MoveItem {
            source_space: source_space.clone(),
            item: item.clone(),
            target_space: target_space.clone(),
            target,
        })
    }

    fn commit_tabs_move_impl(
        &mut self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        target_space: &DockSpaceId,
        target: DockGraphDropTarget,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.move_validation()
            .validate_tabs_target_space(target_space, source_tabs)?;
        validate_graph_drop_target_policy(self.policy(), target)?;
        if source_space == target_space && target.center_tabs() == Some(source_tabs) {
            self.policy().validate_same_stack_center_drop()?;
        }

        self.commit_graph_op(DockOp::MoveTabs {
            source_space: source_space.clone(),
            source_tabs,
            target_space: target_space.clone(),
            target,
        })
    }

    fn commit_floating_move_impl(
        &mut self,
        source_space: &DockSpaceId,
        floating: DockNodeId,
        target_space: &DockSpaceId,
        target: DockGraphDropTarget,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.move_validation()
            .validate_floating_target_space(target_space, floating)?;
        validate_graph_drop_target_policy(self.policy(), target)?;
        self.commit_graph_op(DockOp::MoveFloating {
            source_space: source_space.clone(),
            floating,
            target_space: target_space.clone(),
            target,
        })
    }
}

pub(crate) fn validate_graph_drop_target_policy(
    policy: &DockPolicy,
    target: DockGraphDropTarget,
) -> Result<(), DockActionApplyError> {
    if let Some(zone) = target.drop_zone() {
        policy.validate_drop_zone(zone)?;
    } else {
        policy.validate_platform_viewports()?;
    }
    Ok(())
}
