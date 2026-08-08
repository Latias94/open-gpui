use crate::{
    DockActionApplyError, DockActionOutcome, DockGraphMutationError, DockMergeBackTarget, DockNode,
    DockNodeId, DockSpaceId, DockWorkspace,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DockMergePolicyValidation {
    CurrentPolicy,
    PrevalidatedPolicy,
}

impl DockWorkspace {
    pub(crate) fn validate_merge_space_into(
        &self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
    ) -> Result<(), DockActionApplyError> {
        self.validate_merge_space_into_target(
            source_space,
            target_space,
            None,
            DockMergePolicyValidation::CurrentPolicy,
        )
    }

    fn validate_merge_space_into_target(
        &self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        target_tabs: Option<DockNodeId>,
        policy_validation: DockMergePolicyValidation,
    ) -> Result<(), DockActionApplyError> {
        if source_space == target_space {
            return Ok(());
        }

        let source_root = self.graph().root(source_space);
        let target_root = self.graph().root(target_space);

        if source_root.is_some() && target_root.is_none() {
            return self.validate_merge_root_into_empty_space(
                source_space,
                target_space,
                policy_validation,
            );
        }

        if source_root.is_some() {
            self.validate_non_empty_merge_payload(source_space, target_space)?;
            self.merge_target_tabs(target_space, target_tabs)?;
            for source_tabs in self.graph().root_tabs_in_space(source_space) {
                if self
                    .graph()
                    .root_for_node_in_space(source_space, source_tabs)
                    .is_none()
                {
                    continue;
                }

                self.require_non_empty_tabs_for_merge(source_tabs)?;
                if policy_validation == DockMergePolicyValidation::CurrentPolicy {
                    self.move_validation()
                        .validate_tabs_target_space(target_space, source_tabs)?;
                }
            }
            if policy_validation == DockMergePolicyValidation::CurrentPolicy {
                self.move_validation()
                    .validate_space_floating_forest_target_space(source_space, target_space)?;
            }
            return Ok(());
        }

        if policy_validation == DockMergePolicyValidation::CurrentPolicy {
            self.move_validation()
                .validate_space_floating_forest_target_space(source_space, target_space)?;
        }

        Ok(())
    }

    fn merge_target_tabs(
        &self,
        target_space: &DockSpaceId,
        target_tabs: Option<DockNodeId>,
    ) -> Result<DockNodeId, DockActionApplyError> {
        match target_tabs {
            None => self.unique_merge_target_tabs(target_space),
            Some(tabs) => {
                self.validate_explicit_merge_target_tabs(target_space, tabs)?;
                Ok(tabs)
            }
        }
    }

    fn unique_merge_target_tabs(
        &self,
        target_space: &DockSpaceId,
    ) -> Result<DockNodeId, DockActionApplyError> {
        let tabs = self.graph().root_tabs_in_space(target_space);
        match tabs.as_slice() {
            [target_tabs] => Ok(*target_tabs),
            _ => Err(DockGraphMutationError::MergeTargetTabsNotUnique {
                space: target_space.clone(),
                tabs_len: tabs.len(),
            }
            .into()),
        }
    }

    pub(crate) fn resolve_merge_target(
        &self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
    ) -> Result<DockMergeBackTarget, DockActionApplyError> {
        self.validate_merge_space_into(source_space, target_space)?;
        if source_space == target_space
            || self.graph().root(source_space).is_none()
            || self.graph().root(target_space).is_none()
        {
            return Ok(DockMergeBackTarget::SpaceOnly);
        }
        self.unique_merge_target_tabs(target_space)
            .map(DockMergeBackTarget::Tabs)
    }

    fn validate_explicit_merge_target_tabs(
        &self,
        target_space: &DockSpaceId,
        target_tabs: DockNodeId,
    ) -> Result<(), DockActionApplyError> {
        match self.graph().node(target_tabs) {
            Some(DockNode::Tabs { .. }) => {}
            Some(_) => {
                return Err(DockGraphMutationError::NodeIsNotTabs { node: target_tabs }.into());
            }
            None => {
                return Err(DockGraphMutationError::TabsNodeNotFound { tabs: target_tabs }.into());
            }
        }
        if self
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

    fn validate_non_empty_merge_payload(
        &self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
    ) -> Result<(), DockActionApplyError> {
        let Some(source_root) = self.graph().root(source_space) else {
            return Ok(());
        };
        if self.graph().root(target_space).is_none() {
            return Ok(());
        }
        if self.graph().is_visible_split_payload(source_root) {
            let target_root = self
                .graph()
                .root(target_space)
                .expect("target root should exist after non-empty check");
            return Err(
                DockGraphMutationError::VisibleSplitPayloadCannotDockOverNonEmptyTarget {
                    payload: source_root,
                    target: target_root,
                }
                .into(),
            );
        }
        Ok(())
    }

    fn validate_merge_root_into_empty_space(
        &self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        policy_validation: DockMergePolicyValidation,
    ) -> Result<(), DockActionApplyError> {
        if !self.graph().floating_containers(target_space).is_empty() {
            return Err(DockGraphMutationError::TargetSpaceNotEmpty {
                space: target_space.clone(),
            }
            .into());
        }

        if policy_validation == DockMergePolicyValidation::CurrentPolicy {
            self.policy().validate_platform_viewports()?;
        }

        for source_tabs in self.graph().root_tabs_in_space(source_space) {
            self.require_non_empty_tabs_for_merge(source_tabs)?;
            if policy_validation == DockMergePolicyValidation::CurrentPolicy {
                self.move_validation()
                    .validate_tabs_target_space(target_space, source_tabs)?;
            }
        }

        if policy_validation == DockMergePolicyValidation::CurrentPolicy {
            self.move_validation()
                .validate_space_floating_forest_target_space(source_space, target_space)?;
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

    #[cfg(test)]
    pub(crate) fn commit_merge_space_into(
        &mut self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.validate_merge_space_into(source_space, target_space)?;
        if source_space == target_space {
            return Ok(DockActionOutcome::Unchanged);
        }
        if self.graph().root(source_space).is_some() && self.graph().root(target_space).is_some() {
            let target_tabs = self.unique_merge_target_tabs(target_space)?;
            return self.commit_merge_space_into_tabs(source_space, target_space, target_tabs);
        }
        self.commit_merge_space_into_target(source_space, target_space, None)
    }

    pub(crate) fn commit_prevalidated_merge_space_into_target(
        &mut self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        target: DockMergeBackTarget,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        if source_space == target_space {
            return Ok(DockActionOutcome::Unchanged);
        }
        if self.graph().root(source_space).is_some() && self.graph().root(target_space).is_some() {
            let DockMergeBackTarget::Tabs(target_tabs) = target else {
                return Err(DockGraphMutationError::MergeTargetTabsNotUnique {
                    space: target_space.clone(),
                    tabs_len: self.graph().root_tabs_in_space(target_space).len(),
                }
                .into());
            };
            return self.commit_merge_space_into_target_with_policy(
                source_space,
                target_space,
                Some(target_tabs),
                DockMergePolicyValidation::PrevalidatedPolicy,
            );
        }
        if let DockMergeBackTarget::Tabs(target) = target {
            return Err(DockGraphMutationError::TargetNodeNotInSpace {
                space: target_space.clone(),
                target,
            }
            .into());
        }
        self.commit_merge_space_into_target_with_policy(
            source_space,
            target_space,
            None,
            DockMergePolicyValidation::PrevalidatedPolicy,
        )
    }

    #[cfg(test)]
    pub(crate) fn commit_merge_space_into_tabs(
        &mut self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        target_tabs: DockNodeId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.commit_merge_space_into_target(source_space, target_space, Some(target_tabs))
    }

    #[cfg(test)]
    fn commit_merge_space_into_target(
        &mut self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        target_tabs: Option<DockNodeId>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.commit_merge_space_into_target_with_policy(
            source_space,
            target_space,
            target_tabs,
            DockMergePolicyValidation::CurrentPolicy,
        )
    }

    fn commit_merge_space_into_target_with_policy(
        &mut self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        target_tabs: Option<DockNodeId>,
        policy_validation: DockMergePolicyValidation,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.validate_merge_space_into_target(
            source_space,
            target_space,
            target_tabs,
            policy_validation,
        )?;

        if source_space == target_space {
            return Ok(DockActionOutcome::Unchanged);
        }

        let source_tabs = self.graph().root_tabs_in_space(source_space);
        if source_tabs.is_empty() {
            return self.commit_merge_space_floating_forest_into(
                source_space,
                target_space,
                policy_validation,
            );
        }

        if self.graph().root(target_space).is_none() {
            let root_outcome = self.commit_merge_space_root_into_empty_space(
                source_space,
                target_space,
                policy_validation,
            )?;
            let mut changed = root_outcome.changed();
            changed |= self
                .commit_merge_space_floating_forest_into(
                    source_space,
                    target_space,
                    policy_validation,
                )?
                .changed();
            return Ok(if changed {
                DockActionOutcome::Changed
            } else {
                DockActionOutcome::Unchanged
            });
        }

        self.validate_non_empty_merge_payload(source_space, target_space)?;
        let target_tabs = self.merge_target_tabs(target_space, target_tabs)?;
        let mut next = self.graph().clone();
        let changed = next.move_root_to_non_empty_space(source_space, target_space, target_tabs);
        if !changed {
            return self.commit_merge_space_floating_forest_into(
                source_space,
                target_space,
                policy_validation,
            );
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
            .commit_merge_space_floating_forest_into(source_space, target_space, policy_validation)?
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
        policy_validation: DockMergePolicyValidation,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.validate_merge_root_into_empty_space(source_space, target_space, policy_validation)?;

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
        policy_validation: DockMergePolicyValidation,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        if policy_validation == DockMergePolicyValidation::CurrentPolicy {
            self.move_validation()
                .validate_space_floating_forest_target_space(source_space, target_space)?;
        }
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
}
