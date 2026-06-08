use crate::{
    DockItemId, DockNode, DockNodeId, DockOp, DockOpApplyError, DockSpaceId, DockWorkspace,
    DropZone,
};
use thiserror::Error;

/// Docking interaction emitted by GPUI render adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
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
        if source_space == target_space && source_tabs == target_tabs && zone == DropZone::Center {
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
}
