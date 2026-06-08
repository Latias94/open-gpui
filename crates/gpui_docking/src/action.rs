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
        /// Optional tab insertion index for center drops.
        insert_index: Option<usize>,
    },
    /// Moves one tab into a new empty logical dock space.
    MoveItemToEmptyDockSpace {
        /// The source dock space containing the item.
        source_space: DockSpaceId,
        /// The item being moved.
        item: DockItemId,
        /// The empty target dock space that will receive a root tabs node.
        target_space: DockSpaceId,
    },
    /// Moves an entire tabs node into a new empty logical dock space.
    MoveTabsToEmptyDockSpace {
        /// The source dock space containing the tabs node.
        source_space: DockSpaceId,
        /// The tabs node being moved.
        source_tabs: DockNodeId,
        /// The empty target dock space that will receive the tabs node contents.
        target_space: DockSpaceId,
    },
    /// Closes one dock item through panel lifecycle policy.
    CloseItem {
        /// The dock space containing the item.
        space: DockSpaceId,
        /// The item to close.
        item: DockItemId,
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
    /// The item has no registered panel metadata to drive close policy.
    #[error("dock item {item} has no registered panel")]
    PanelNotRegistered {
        /// The item that was requested.
        item: DockItemId,
    },
    /// The panel is registered but not closable.
    #[error("dock item {item} is not closable")]
    PanelNotClosable {
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
