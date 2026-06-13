use crate::{
    DockActionApplyError, DockActionOutcome, DockGraphMutationError, DockItemId, DockMoveTarget,
    DockNode, DockNodeId, DockOp, DockPolicy, DockSpaceId, DockWorkspace, DropZone,
};

pub(crate) struct DockWorkspaceMoveTabRequest<'a> {
    pub(crate) source_space: &'a DockSpaceId,
    pub(crate) source_tabs: DockNodeId,
    pub(crate) item: &'a DockItemId,
    pub(crate) target_space: &'a DockSpaceId,
    pub(crate) target: DockWorkspaceMoveTarget,
}

pub(crate) struct DockWorkspaceMoveTabsRequest<'a> {
    pub(crate) source_space: &'a DockSpaceId,
    pub(crate) source_tabs: DockNodeId,
    pub(crate) target_space: &'a DockSpaceId,
    pub(crate) target: DockWorkspaceMoveTarget,
}

pub(crate) type DockWorkspaceMoveTarget = DockMoveTarget;

impl DockWorkspace {
    pub(crate) fn validate_merge_space_into(
        &self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
    ) -> Result<(), DockActionApplyError> {
        if source_space == target_space {
            return Ok(());
        }

        let mut target_tabs = self
            .graph()
            .first_tabs_in_space(target_space)
            .map(|tabs| (tabs, true));
        for source_tabs in self.graph().tabs_in_space(source_space) {
            if self
                .graph()
                .root_for_node_in_space(source_space, source_tabs)
                .is_none()
            {
                continue;
            }

            self.require_non_empty_tabs_for_merge(source_tabs)?;
            if let Some((target_tabs, target_exists_now)) = target_tabs {
                self.validate_merge_tabs_target(target_space, target_tabs, target_exists_now)?;
                self.move_validation()
                    .validate_tabs_target_space(target_space, source_tabs)?;
                self.policy().validate_drop_zone(DropZone::Center)?;
                if source_space == target_space && source_tabs == target_tabs {
                    self.policy().validate_same_stack_center_drop()?;
                }
            } else {
                self.policy().validate_platform_viewports()?;
                self.move_validation()
                    .validate_tabs_target_space(target_space, source_tabs)?;
            }
            target_tabs = target_tabs.or(Some((source_tabs, false)));
        }

        Ok(())
    }

    fn validate_merge_tabs_target(
        &self,
        target_space: &DockSpaceId,
        target_tabs: DockNodeId,
        target_exists_now: bool,
    ) -> Result<(), DockActionApplyError> {
        self.require_non_empty_tabs_for_merge(target_tabs)?;
        if target_exists_now
            && self
                .graph()
                .root_for_node_in_space(target_space, target_tabs)
                .is_none()
        {
            return Err(DockGraphMutationError::TargetNodeNotInSpace {
                space: target_space.clone(),
                target: target_tabs,
            }
            .into());
        }
        Ok(())
    }

    fn require_non_empty_tabs_for_merge(
        &self,
        tabs: DockNodeId,
    ) -> Result<(), DockActionApplyError> {
        match self.graph().node(tabs) {
            Some(DockNode::Tabs { items, .. }) if !items.is_empty() => Ok(()),
            Some(DockNode::Tabs { .. }) => {
                Err(DockGraphMutationError::TabsNodeEmpty { tabs }.into())
            }
            Some(_) => Err(DockGraphMutationError::NodeIsNotTabs { node: tabs }.into()),
            None => Err(DockGraphMutationError::TabsNodeNotFound { tabs }.into()),
        }
    }

    pub(crate) fn commit_tab_move(
        &mut self,
        request: DockWorkspaceMoveTabRequest<'_>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        let DockWorkspaceMoveTabRequest {
            source_space,
            source_tabs,
            item,
            target_space,
            target,
        } = request;

        self.move_validation()
            .validate_move_tab_source(source_space, source_tabs, item)?;
        self.move_validation()
            .validate_item_target_space(target_space, item)?;
        validate_move_target_policy(self.policy(), target)?;
        if source_space == target_space && target.noop_tabs() == Some(source_tabs) {
            self.policy().validate_same_stack_center_drop()?;
            if target.insert_index().is_none() {
                return Ok(DockActionOutcome::Unchanged);
            }
        }

        self.commit_graph_op(DockOp::MoveItem {
            source_space: source_space.clone(),
            item: item.clone(),
            target_space: target_space.clone(),
            target,
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
            target,
        } = request;

        self.move_validation()
            .validate_tabs_target_space(target_space, source_tabs)?;
        validate_move_target_policy(self.policy(), target)?;
        if source_space == target_space && target.noop_tabs() == Some(source_tabs) {
            self.policy().validate_same_stack_center_drop()?;
            return Ok(DockActionOutcome::Unchanged);
        }

        self.commit_graph_op(DockOp::MoveTabs {
            source_space: source_space.clone(),
            source_tabs,
            target_space: target_space.clone(),
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
            target: DockMoveTarget::empty_space(),
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
            target: DockMoveTarget::empty_space(),
        })
    }

    pub(crate) fn commit_merge_space_into(
        &mut self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        if source_space == target_space {
            return Ok(DockActionOutcome::Unchanged);
        }

        let source_tabs = self.graph().tabs_in_space(source_space);
        if source_tabs.is_empty() {
            return Ok(DockActionOutcome::Unchanged);
        }

        let mut target_tabs = self.graph().first_tabs_in_space(target_space);
        let mut changed = false;
        for source_tabs in source_tabs {
            if self
                .graph()
                .root_for_node_in_space(source_space, source_tabs)
                .is_none()
            {
                continue;
            }

            let outcome = if let Some(target_tabs) = target_tabs {
                self.commit_tabs_move(DockWorkspaceMoveTabsRequest {
                    source_space,
                    source_tabs,
                    target_space,
                    target: DockWorkspaceMoveTarget::center(target_tabs),
                })?
            } else {
                self.commit_tabs_to_empty_dock_space(source_space, source_tabs, target_space)?
            };
            changed |= outcome.changed();
            target_tabs = self.graph().first_tabs_in_space(target_space);
        }

        Ok(if changed {
            DockActionOutcome::Changed
        } else {
            DockActionOutcome::Unchanged
        })
    }
}

pub(crate) fn validate_move_target_policy(
    policy: &DockPolicy,
    target: DockMoveTarget,
) -> Result<(), DockActionApplyError> {
    if let Some(zone) = target.drop_zone() {
        policy.validate_drop_zone(zone)?;
    } else {
        policy.validate_platform_viewports()?;
    }
    Ok(())
}
