use crate::{DockItemId, DockNodeId, DockOpApplyError, DockPolicyError, DockSpaceId, DropZone};
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
    /// Opens one registered dock item into an existing tabs node or empty dock space.
    OpenItem {
        /// The dock space receiving the item.
        space: DockSpaceId,
        /// Existing tabs node to receive the item, or `None` to create a root in an empty space.
        target_tabs: Option<DockNodeId>,
        /// The item to open.
        item: DockItemId,
        /// Optional insertion index when opening into existing tabs.
        insert_index: Option<usize>,
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
pub enum DockTransactionError {
    /// A viewport target must first be resolved in that viewport's local layout.
    #[error("viewport target for dock space {space} requires host-local resolution")]
    ViewportTargetRequiresLocalResolution {
        /// The viewport space that was hit.
        space: DockSpaceId,
    },
    /// Tear-off drops require the viewport runtime state machine.
    #[error("tear-off target requires viewport runtime transaction")]
    TearOffRequiresViewportRuntime,
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
    /// The higher-level docking transaction could not be completed.
    #[error(transparent)]
    Transaction(#[from] DockTransactionError),
}
