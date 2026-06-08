use crate::{
    DockItemId, DockNode, DockNodeId, DockOp, DockOpApplyError, DockPolicyError, DockSpaceId,
    DockWorkspace, DropZone,
};
use open_gpui::{Bounds, Pixels};
use thiserror::Error;

/// Docking interaction emitted by GPUI render adapters.
#[derive(Debug, Clone, PartialEq)]
pub enum DockAction {
    /// Selects a tab within one tabs node.
    SelectTab {
        /// The tabs node containing the item.
        tabs: DockNodeId,
        /// The item to select.
        item: DockItemId,
    },
    /// Moves one tab into another tabs node or edge split target.
    MoveTab {
        /// The source dock space containing the item.
        source_space: DockSpaceId,
        /// The source tabs node where the drag started.
        source_tabs: DockNodeId,
        /// The item being moved.
        item: DockItemId,
        /// The target dock space receiving the item.
        target_space: DockSpaceId,
        /// The target tabs node or split target.
        target_tabs: DockNodeId,
        /// The resolved drop zone.
        zone: DropZone,
    },
    /// Floats one tab inside a dock space without creating a platform window.
    FloatItemInWindow {
        /// The source dock space containing the item.
        source_space: DockSpaceId,
        /// The item to float.
        item: DockItemId,
        /// The target dock space that will own the floating container.
        target_space: DockSpaceId,
        /// Bounds for the floating container, relative to the host.
        bounds: Bounds<Pixels>,
    },
    /// Floats an entire tabs node inside a dock space without creating a platform window.
    FloatTabsInWindow {
        /// The source dock space containing the tabs node.
        source_space: DockSpaceId,
        /// The tabs node to float.
        source_tabs: DockNodeId,
        /// The target dock space that will own the floating container.
        target_space: DockSpaceId,
        /// Bounds for the floating container, relative to the host.
        bounds: Bounds<Pixels>,
    },
    /// Updates the bounds of an in-window floating container.
    SetFloatingBounds {
        /// The dock space containing the floating container.
        space: DockSpaceId,
        /// The floating container node.
        floating: DockNodeId,
        /// Bounds for the floating container, relative to the host.
        bounds: Bounds<Pixels>,
    },
    /// Raises an in-window floating container above other floating containers.
    RaiseFloating {
        /// The dock space containing the floating container.
        space: DockSpaceId,
        /// The floating container node.
        floating: DockNodeId,
    },
    /// Merges an in-window floating container into an existing tabs node.
    MergeFloatingInto {
        /// The dock space containing the floating container.
        space: DockSpaceId,
        /// The floating container node.
        floating: DockNodeId,
        /// The target tabs node.
        target_tabs: DockNodeId,
    },
    /// Resizes one split node by replacing its normalized fractions.
    ResizeSplit {
        /// The split node to update.
        split: DockNodeId,
        /// The next normalized split fractions.
        fractions: Vec<f32>,
    },
}

/// Outcome of applying a docking action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockActionOutcome {
    /// The action changed docking state.
    Changed,
    /// The action was valid but did not change state.
    Unchanged,
}

impl DockActionOutcome {
    /// Returns true when the action changed docking state.
    pub fn changed(self) -> bool {
        matches!(self, Self::Changed)
    }

    pub(crate) fn from_changed(changed: bool) -> Self {
        if changed {
            Self::Changed
        } else {
            Self::Unchanged
        }
    }
}

/// Error returned when a docking action cannot be applied.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DockActionApplyError {
    /// The action was sent directly to a controller-backed host.
    #[error("controller-backed hosts apply actions through DockController")]
    ControllerBackedHost,
    /// The selected item was not found in the target tabs node.
    #[error("dock item {item} not found in tabs node {tabs:?}")]
    ItemNotInTabs {
        /// The tabs node that was targeted.
        tabs: DockNodeId,
        /// The item that was requested.
        item: DockItemId,
    },
    /// The underlying graph operation failed.
    #[error(transparent)]
    Graph(#[from] DockOpApplyError),
    /// The action was rejected by workspace policy.
    #[error(transparent)]
    Policy(#[from] DockPolicyError),
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
            } => self.move_tab(
                source_space,
                *source_tabs,
                item,
                target_space,
                *target_tabs,
                *zone,
            ),
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

        self.apply_op_checked(&DockOp::SetActiveTab {
            tabs,
            active: next_active,
        })
        .map(DockActionOutcome::from_changed)
        .map_err(Into::into)
    }

    fn move_tab(
        &mut self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        item: &DockItemId,
        target_space: &DockSpaceId,
        target_tabs: DockNodeId,
        zone: DropZone,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_drop_zone(zone)?;
        if source_space == target_space && source_tabs == target_tabs && zone == DropZone::Center {
            self.policy().validate_same_stack_center_drop()?;
            return Ok(DockActionOutcome::Unchanged);
        }

        self.apply_op_checked(&DockOp::MoveItem {
            source_space: source_space.clone(),
            item: item.clone(),
            target_space: target_space.clone(),
            target_tabs,
            zone,
            insert_index: None,
        })
        .map(DockActionOutcome::from_changed)
        .map_err(Into::into)
    }

    fn float_item_in_window(
        &mut self,
        source_space: &DockSpaceId,
        item: &DockItemId,
        target_space: &DockSpaceId,
        bounds: Bounds<Pixels>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_floating()?;
        self.apply_op_checked(&DockOp::FloatItemInWindow {
            source_space: source_space.clone(),
            item: item.clone(),
            target_space: target_space.clone(),
            bounds,
        })
        .map(DockActionOutcome::from_changed)
        .map_err(Into::into)
    }

    fn float_tabs_in_window(
        &mut self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        target_space: &DockSpaceId,
        bounds: Bounds<Pixels>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_floating()?;
        self.apply_op_checked(&DockOp::FloatTabsInWindow {
            source_space: source_space.clone(),
            source_tabs,
            target_space: target_space.clone(),
            bounds,
        })
        .map(DockActionOutcome::from_changed)
        .map_err(Into::into)
    }

    fn set_floating_bounds(
        &mut self,
        space: &DockSpaceId,
        floating: DockNodeId,
        bounds: Bounds<Pixels>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_floating()?;
        self.apply_op_checked(&DockOp::SetFloatingBounds {
            space: space.clone(),
            floating,
            bounds,
        })
        .map(DockActionOutcome::from_changed)
        .map_err(Into::into)
    }

    fn raise_floating(
        &mut self,
        space: &DockSpaceId,
        floating: DockNodeId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_floating()?;
        self.apply_op_checked(&DockOp::RaiseFloating {
            space: space.clone(),
            floating,
        })
        .map(DockActionOutcome::from_changed)
        .map_err(Into::into)
    }

    fn merge_floating_into(
        &mut self,
        space: &DockSpaceId,
        floating: DockNodeId,
        target_tabs: DockNodeId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_floating()?;
        self.apply_op_checked(&DockOp::MergeFloatingInto {
            space: space.clone(),
            floating,
            target_tabs,
        })
        .map(DockActionOutcome::from_changed)
        .map_err(Into::into)
    }

    fn resize_split(
        &mut self,
        split: DockNodeId,
        fractions: &[f32],
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_splitter_resize()?;
        self.apply_op_checked(&DockOp::SetSplitFractions {
            split,
            fractions: fractions.to_vec(),
        })
        .map(DockActionOutcome::from_changed)
        .map_err(Into::into)
    }
}
