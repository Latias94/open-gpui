use crate::{
    DockActionApplyError, DockActionOutcome, DockItemId, DockNodeId, DockOp, DockPanelCloseOutcome,
    DockPanelOpenOutcome, DockPanelOpenPlacementSource, DockPanelPlacement,
    DockPanelPlacementTarget, DockPanelReopenPolicy, DockSpaceId, DockWorkspace,
};

impl DockWorkspace {
    pub(crate) fn commit_close_item(
        &mut self,
        space: &DockSpaceId,
        item: &DockItemId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.commit_close_item_with_product_outcome(space, item)
            .map(|outcome| outcome.action())
    }

    pub(crate) fn commit_close_panel(
        &mut self,
        space: &DockSpaceId,
        item: &DockItemId,
    ) -> Result<DockPanelCloseOutcome, DockActionApplyError> {
        self.commit_close_item_with_product_outcome(space, item)
    }

    fn commit_close_item_with_product_outcome(
        &mut self,
        space: &DockSpaceId,
        item: &DockItemId,
    ) -> Result<DockPanelCloseOutcome, DockActionApplyError> {
        let placement = self.graph().panel_placement_for_item(space, item);
        self.panel_lifecycle().validate_close(item)?;
        let action = self.commit_graph_op(DockOp::CloseItem {
            space: space.clone(),
            item: item.clone(),
        })?;
        if action.changed() {
            self.record_panel_product_placement(item, placement.as_ref());
        }
        Ok(DockPanelCloseOutcome::new(
            action,
            space.clone(),
            item.clone(),
            placement,
        ))
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

        let action = self.commit_graph_op(DockOp::OpenItem {
            space: space.clone(),
            target_tabs,
            item: item.clone(),
            insert_index,
        })?;
        if action.changed() {
            let placement = self.graph().panel_placement_for_item(space, item);
            self.record_panel_product_placement(item, placement.as_ref());
        }
        Ok(action)
    }

    pub(crate) fn commit_open_item_at_placement(
        &mut self,
        space: &DockSpaceId,
        placement: &DockPanelPlacement,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.commit_open_panel_at_placement_with_source(
            space,
            placement,
            DockPanelOpenPlacementSource::Explicit,
        )
        .map(|outcome| outcome.action())
    }

    pub(crate) fn commit_open_panel_at_placement(
        &mut self,
        space: &DockSpaceId,
        placement: &DockPanelPlacement,
    ) -> Result<DockPanelOpenOutcome, DockActionApplyError> {
        self.commit_open_panel_at_placement_with_source(
            space,
            placement,
            DockPanelOpenPlacementSource::Explicit,
        )
    }

    pub(crate) fn commit_reopen_panel(
        &mut self,
        space: &DockSpaceId,
        item: &DockItemId,
    ) -> Result<DockPanelOpenOutcome, DockActionApplyError> {
        self.panel_lifecycle().validate_open(item)?;
        let (placement, source) = self.reopen_placement_for_item(space, item)?;
        self.commit_open_panel_at_placement_with_source(space, &placement, source)
    }

    fn commit_open_panel_at_placement_with_source(
        &mut self,
        space: &DockSpaceId,
        placement: &DockPanelPlacement,
        placement_source: DockPanelOpenPlacementSource,
    ) -> Result<DockPanelOpenOutcome, DockActionApplyError> {
        let target_tabs = self
            .graph()
            .target_tabs_for_panel_placement(space, placement);
        let insert_index = placement.open_insert_index(self.graph(), space, target_tabs);
        let action = self.commit_open_item(space, target_tabs, placement.item(), insert_index)?;
        Ok(DockPanelOpenOutcome::new(
            action,
            space.clone(),
            placement.item().clone(),
            placement.clone(),
            placement_source,
        ))
    }

    fn reopen_placement_for_item(
        &self,
        space: &DockSpaceId,
        item: &DockItemId,
    ) -> Result<(DockPanelPlacement, DockPanelOpenPlacementSource), DockActionApplyError> {
        let descriptor = self
            .panels()
            .descriptor(item)
            .ok_or_else(|| DockActionApplyError::PanelNotRegistered { item: item.clone() })?;

        if descriptor.reopen_policy() == DockPanelReopenPolicy::RestoreLastKnown
            && let Some(target) = descriptor.last_known_placement()
            && self.product_target_is_valid_for_open(space, target)
        {
            return Ok((
                DockPanelPlacement::new(item.clone(), target.clone()),
                DockPanelOpenPlacementSource::LastKnown,
            ));
        }

        if let Some(target) = descriptor.default_placement() {
            return Ok((
                DockPanelPlacement::new(item.clone(), target.clone()),
                DockPanelOpenPlacementSource::DescriptorDefault,
            ));
        }

        Ok((
            DockPanelPlacement::center(item.clone()),
            DockPanelOpenPlacementSource::ImplicitCenter,
        ))
    }

    fn product_target_is_valid_for_open(
        &self,
        space: &DockSpaceId,
        target: &DockPanelPlacementTarget,
    ) -> bool {
        self.graph()
            .target_tabs_for_panel_placement_target(space, target)
            .is_some()
            || matches!(target, DockPanelPlacementTarget::Center)
                && self.graph().root(space).is_none()
                && self.graph().floating_containers(space).is_empty()
    }

    fn record_panel_product_placement(
        &mut self,
        item: &DockItemId,
        placement: Option<&DockPanelPlacement>,
    ) {
        if let Some(placement) = placement {
            self.panels_mut()
                .record_last_known_placement(item, placement.target().clone());
        }
    }
}
