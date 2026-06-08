use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockItemId, DockNode, DockNodeId, DockOp,
    DockOpApplyError, DockSpaceId, DockWorkspace, DropZone,
};
use open_gpui::{Bounds, Pixels};

struct MoveTabRequest<'a> {
    source_space: &'a DockSpaceId,
    source_tabs: DockNodeId,
    item: &'a DockItemId,
    target_space: &'a DockSpaceId,
    target_tabs: DockNodeId,
    zone: DropZone,
    insert_index: Option<usize>,
}

impl DockWorkspace {
    /// Applies a docking interaction action.
    pub fn apply_action(
        &mut self,
        action: &DockAction,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        match action {
            DockAction::SelectTab { tabs, item } => self.select_tab(*tabs, item),
            DockAction::MoveTab {
                source_space,
                source_tabs,
                item,
                target_space,
                target_tabs,
                zone,
                insert_index,
            } => self.move_tab(MoveTabRequest {
                source_space,
                source_tabs: *source_tabs,
                item,
                target_space,
                target_tabs: *target_tabs,
                zone: *zone,
                insert_index: *insert_index,
            }),
            DockAction::MoveItemToEmptyDockSpace {
                source_space,
                item,
                target_space,
            } => self.move_item_to_empty_dock_space(source_space, item, target_space),
            DockAction::MoveTabsToEmptyDockSpace {
                source_space,
                source_tabs,
                target_space,
            } => self.move_tabs_to_empty_dock_space(source_space, *source_tabs, target_space),
            DockAction::CloseItem { space, item } => self.close_item(space, item),
            DockAction::FloatItemInWindow {
                source_space,
                item,
                target_space,
                bounds,
            } => self.float_item_in_window(source_space, item, target_space, *bounds),
            DockAction::FloatTabsInWindow {
                source_space,
                source_tabs,
                target_space,
                bounds,
            } => self.float_tabs_in_window(source_space, *source_tabs, target_space, *bounds),
            DockAction::SetFloatingBounds {
                space,
                floating,
                bounds,
            } => self.set_floating_bounds(space, *floating, *bounds),
            DockAction::RaiseFloating { space, floating } => self.raise_floating(space, *floating),
            DockAction::MergeFloatingInto {
                space,
                floating,
                target_tabs,
            } => self.merge_floating_into(space, *floating, *target_tabs),
            DockAction::ResizeSplit { split, fractions } => self.resize_split(*split, fractions),
        }
    }

    fn commit_graph_op(&mut self, op: DockOp) -> Result<DockActionOutcome, DockActionApplyError> {
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

    fn move_tab(
        &mut self,
        request: MoveTabRequest<'_>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        let MoveTabRequest {
            source_space,
            source_tabs,
            item,
            target_space,
            target_tabs,
            zone,
            insert_index,
        } = request;

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

    fn move_item_to_empty_dock_space(
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

    fn move_tabs_to_empty_dock_space(
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

    fn close_item(
        &mut self,
        space: &DockSpaceId,
        item: &DockItemId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        let Some(panel) = self.panels().descriptor(item) else {
            return Err(DockActionApplyError::PanelNotRegistered { item: item.clone() });
        };
        if !panel.is_closable() {
            return Err(DockActionApplyError::PanelNotClosable { item: item.clone() });
        }

        self.commit_graph_op(DockOp::CloseItem {
            space: space.clone(),
            item: item.clone(),
        })
    }

    fn float_item_in_window(
        &mut self,
        source_space: &DockSpaceId,
        item: &DockItemId,
        target_space: &DockSpaceId,
        bounds: Bounds<Pixels>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_floating()?;
        self.commit_graph_op(DockOp::FloatItemInWindow {
            source_space: source_space.clone(),
            item: item.clone(),
            target_space: target_space.clone(),
            bounds,
        })
    }

    fn float_tabs_in_window(
        &mut self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        target_space: &DockSpaceId,
        bounds: Bounds<Pixels>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_floating()?;
        self.commit_graph_op(DockOp::FloatTabsInWindow {
            source_space: source_space.clone(),
            source_tabs,
            target_space: target_space.clone(),
            bounds,
        })
    }

    fn set_floating_bounds(
        &mut self,
        space: &DockSpaceId,
        floating: DockNodeId,
        bounds: Bounds<Pixels>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_floating()?;
        self.commit_graph_op(DockOp::SetFloatingBounds {
            space: space.clone(),
            floating,
            bounds,
        })
    }

    fn raise_floating(
        &mut self,
        space: &DockSpaceId,
        floating: DockNodeId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_floating()?;
        self.commit_graph_op(DockOp::RaiseFloating {
            space: space.clone(),
            floating,
        })
    }

    fn merge_floating_into(
        &mut self,
        space: &DockSpaceId,
        floating: DockNodeId,
        target_tabs: DockNodeId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_floating()?;
        self.commit_graph_op(DockOp::MergeFloatingInto {
            space: space.clone(),
            floating,
            target_tabs,
        })
    }

    fn resize_split(
        &mut self,
        split: DockNodeId,
        fractions: &[f32],
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_splitter_resize()?;
        self.commit_graph_op(DockOp::SetSplitFractions {
            split,
            fractions: fractions.to_vec(),
        })
    }
}
