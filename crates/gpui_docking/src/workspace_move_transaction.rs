use crate::{
    DockActionApplyError, DockActionOutcome, DockGraphMutationError, DockItemId, DockMoveTarget,
    DockNode, DockNodeId, DockOp, DockPolicy, DockSpaceId, DockWorkspace,
};

pub(crate) enum DockWorkspaceMove<'a> {
    Item {
        source_space: &'a DockSpaceId,
        source_tabs: DockNodeId,
        item: &'a DockItemId,
        target_space: &'a DockSpaceId,
        target: DockMoveTarget,
    },
    Tabs {
        source_space: &'a DockSpaceId,
        source_tabs: DockNodeId,
        target_space: &'a DockSpaceId,
        target: DockMoveTarget,
    },
    Floating {
        source_space: &'a DockSpaceId,
        floating: DockNodeId,
        target_space: &'a DockSpaceId,
        target: DockMoveTarget,
    },
}

impl DockWorkspace {
    pub(crate) fn validate_merge_space_into(
        &self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
    ) -> Result<(), DockActionApplyError> {
        if source_space == target_space {
            return Ok(());
        }

        let source_root = self.graph().root(source_space);
        let target_root = self.graph().root(target_space);

        if source_root.is_some() && target_root.is_none() {
            return self.validate_merge_root_into_empty_space(source_space, target_space);
        }

        if source_root.is_some() {
            for source_tabs in self.graph().root_tabs_in_space(source_space) {
                if self
                    .graph()
                    .root_for_node_in_space(source_space, source_tabs)
                    .is_none()
                {
                    continue;
                }

                self.require_non_empty_tabs_for_merge(source_tabs)?;
                self.move_validation()
                    .validate_tabs_target_space(target_space, source_tabs)?;
            }
            self.move_validation()
                .validate_space_floating_forest_target_space(source_space, target_space)?;
            return Ok(());
        }

        self.move_validation()
            .validate_space_floating_forest_target_space(source_space, target_space)?;

        Ok(())
    }

    fn validate_merge_root_into_empty_space(
        &self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
    ) -> Result<(), DockActionApplyError> {
        if !self.graph().floating_containers(target_space).is_empty() {
            return Err(DockGraphMutationError::TargetSpaceNotEmpty {
                space: target_space.clone(),
            }
            .into());
        }

        self.policy().validate_platform_viewports()?;

        for source_tabs in self.graph().root_tabs_in_space(source_space) {
            self.require_non_empty_tabs_for_merge(source_tabs)?;
            self.move_validation()
                .validate_tabs_target_space(target_space, source_tabs)?;
        }

        self.move_validation()
            .validate_space_floating_forest_target_space(source_space, target_space)?;

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
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        item: &DockItemId,
        target_space: &DockSpaceId,
        target: DockMoveTarget,
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
        target: DockMoveTarget,
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

        let source_tabs = self.graph().root_tabs_in_space(source_space);
        if source_tabs.is_empty() {
            return self.commit_merge_space_floating_forest_into(source_space, target_space);
        }

        if self.graph().root(target_space).is_none() {
            let root_outcome =
                self.commit_merge_space_root_into_empty_space(source_space, target_space)?;
            let mut changed = root_outcome.changed();
            changed |= self
                .commit_merge_space_floating_forest_into(source_space, target_space)?
                .changed();
            return Ok(if changed {
                DockActionOutcome::Changed
            } else {
                DockActionOutcome::Unchanged
            });
        }

        let mut next = self.graph().clone();
        let changed = next.move_root_to_non_empty_space(source_space, target_space);
        if !changed {
            return self.commit_merge_space_floating_forest_into(source_space, target_space);
        }

        next.validate().map_err(|error| {
            DockActionApplyError::Graph(DockGraphMutationError::MutationInvariantViolation {
                op: "MergeSpaceIntoNonEmptySpace",
                reason: error.to_string(),
            })
        })?;
        self.set_graph(next);
        let mut changed = true;
        changed |= self
            .commit_merge_space_floating_forest_into(source_space, target_space)?
            .changed();

        Ok(if changed {
            DockActionOutcome::Changed
        } else {
            DockActionOutcome::Unchanged
        })
    }

    fn commit_merge_space_root_into_empty_space(
        &mut self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.validate_merge_root_into_empty_space(source_space, target_space)?;

        let mut next = self.graph().clone();
        let changed = next.move_root_to_empty_space(source_space, target_space);
        if !changed {
            return Ok(DockActionOutcome::Unchanged);
        }
        next.validate().map_err(|error| {
            DockActionApplyError::Graph(DockGraphMutationError::MutationInvariantViolation {
                op: "MergeSpaceRootIntoEmptySpace",
                reason: error.to_string(),
            })
        })?;
        self.set_graph(next);
        Ok(DockActionOutcome::Changed)
    }

    fn commit_merge_space_floating_forest_into(
        &mut self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.move_validation()
            .validate_space_floating_forest_target_space(source_space, target_space)?;
        let mut next = self.graph().clone();
        let changed = next.merge_space_floating_forest_into(source_space, target_space);
        if !changed {
            return Ok(DockActionOutcome::Unchanged);
        }
        next.validate().map_err(|error| {
            DockActionApplyError::Graph(DockGraphMutationError::MutationInvariantViolation {
                op: "MergeSpaceFloatingForest",
                reason: error.to_string(),
            })
        })?;
        self.set_graph(next);
        Ok(DockActionOutcome::Changed)
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
        target: DockMoveTarget,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
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

    fn commit_tabs_move_impl(
        &mut self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        target_space: &DockSpaceId,
        target: DockMoveTarget,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
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

    fn commit_floating_move_impl(
        &mut self,
        source_space: &DockSpaceId,
        floating: DockNodeId,
        target_space: &DockSpaceId,
        target: DockMoveTarget,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.move_validation()
            .validate_floating_target_space(target_space, floating)?;
        validate_move_target_policy(self.policy(), target)?;
        self.commit_graph_op(DockOp::MoveFloating {
            source_space: source_space.clone(),
            floating,
            target_space: target_space.clone(),
            target,
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
