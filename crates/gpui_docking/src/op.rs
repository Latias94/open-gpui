use crate::{DockItemId, DockNodeId, DockSpaceId, DropZone};
use thiserror::Error;

/// High-level graph mutation emitted by docking UI or application code.
#[derive(Debug, Clone, PartialEq)]
pub enum DockOp {
    /// Selects the active item within a tabs node.
    SetActiveTab {
        /// The tabs node to update.
        tabs: DockNodeId,
        /// The active tab index.
        active: usize,
    },

    /// Removes an item from a dock space.
    CloseItem {
        /// The dock space containing the item.
        space: DockSpaceId,
        /// The item to close.
        item: DockItemId,
    },

    /// Moves one item into an existing tabs node or split target.
    MoveItem {
        /// The source dock space.
        source_space: DockSpaceId,
        /// The item to move.
        item: DockItemId,
        /// The target dock space.
        target_space: DockSpaceId,
        /// The target tabs or split node.
        target_tabs: DockNodeId,
        /// The drop zone.
        zone: DropZone,
        /// Optional tab insertion index for center drops.
        insert_index: Option<usize>,
    },

    /// Moves one item into an empty dock space, creating its root tabs node.
    MoveItemToEmptyDockSpace {
        /// The source dock space.
        source_space: DockSpaceId,
        /// The item to move.
        item: DockItemId,
        /// The target dock space.
        target_space: DockSpaceId,
    },

    /// Moves an entire tabs node as a group.
    MoveTabs {
        /// The source dock space.
        source_space: DockSpaceId,
        /// The tabs node to move.
        source_tabs: DockNodeId,
        /// The target dock space.
        target_space: DockSpaceId,
        /// The target tabs or split node.
        target_tabs: DockNodeId,
        /// The drop zone.
        zone: DropZone,
        /// Optional tab insertion index for center drops.
        insert_index: Option<usize>,
    },

    /// Moves an entire tabs node into an empty dock space.
    MoveTabsToEmptyDockSpace {
        /// The source dock space.
        source_space: DockSpaceId,
        /// The tabs node to move.
        source_tabs: DockNodeId,
        /// The target dock space.
        target_space: DockSpaceId,
    },

    /// Floats one item inside a dock space without creating a platform window.
    FloatItemInWindow {
        /// The source dock space.
        source_space: DockSpaceId,
        /// The item to float.
        item: DockItemId,
        /// The target dock space that will own the floating container.
        target_space: DockSpaceId,
        /// The floating container bounds.
        bounds: open_gpui::Bounds<open_gpui::Pixels>,
    },

    /// Floats a tabs node inside a dock space without creating a platform window.
    FloatTabsInWindow {
        /// The source dock space.
        source_space: DockSpaceId,
        /// The tabs node to float.
        source_tabs: DockNodeId,
        /// The target dock space that will own the floating container.
        target_space: DockSpaceId,
        /// The floating container bounds.
        bounds: open_gpui::Bounds<open_gpui::Pixels>,
    },

    /// Updates the bounds of an in-window floating container.
    SetFloatingBounds {
        /// The dock space containing the floating container.
        space: DockSpaceId,
        /// The floating container node.
        floating: DockNodeId,
        /// The new bounds.
        bounds: open_gpui::Bounds<open_gpui::Pixels>,
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
        /// The dock space containing the floating container and target tabs.
        space: DockSpaceId,
        /// The floating container node.
        floating: DockNodeId,
        /// The target tabs node.
        target_tabs: DockNodeId,
    },

    /// Replaces every fraction in one split node.
    SetSplitFractions {
        /// The split node to update.
        split: DockNodeId,
        /// The normalized fractions to store.
        fractions: Vec<f32>,
    },

    /// Replaces fractions for multiple split nodes.
    SetSplitFractionsMany {
        /// The split fraction updates.
        updates: Vec<SplitFractionsUpdate>,
    },

    /// Updates a two-child split using the first child's fraction.
    SetSplitFractionTwo {
        /// The split node to update.
        split: DockNodeId,
        /// The first child's fraction.
        first_fraction: f32,
    },
}

/// Fraction update for one split node.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitFractionsUpdate {
    /// The split node to update.
    pub split: DockNodeId,
    /// The normalized fractions to store.
    pub fractions: Vec<f32>,
}

/// Error returned by checked dock operation application.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DockOpApplyError {
    /// The requested tabs node does not exist.
    #[error("tabs node not found: {tabs:?}")]
    TabsNodeNotFound {
        /// Missing tabs node.
        tabs: DockNodeId,
    },

    /// The requested node exists but is not a tabs node.
    #[error("node is not a tabs node: {node:?}")]
    NodeIsNotTabs {
        /// Node with the wrong kind.
        node: DockNodeId,
    },

    /// The requested split node does not exist.
    #[error("split node not found: {split:?}")]
    SplitNodeNotFound {
        /// Missing split node.
        split: DockNodeId,
    },

    /// The requested node exists but is not a split node.
    #[error("node is not a split node: {node:?}")]
    NodeIsNotSplit {
        /// Node with the wrong kind.
        node: DockNodeId,
    },

    /// The requested active tab index is out of bounds.
    #[error("active tab index {active} out of bounds for {tabs:?} with length {len}")]
    ActiveOutOfBounds {
        /// The tabs node.
        tabs: DockNodeId,
        /// Requested active index.
        active: usize,
        /// Current tab count.
        len: usize,
    },

    /// The requested item was not found in the source space.
    #[error("dock item {item} not found in space {space}")]
    ItemNotFound {
        /// The source dock space.
        space: DockSpaceId,
        /// The missing item.
        item: DockItemId,
    },

    /// The target node is not contained by the target dock space.
    #[error("target node {target:?} not found in dock space {space}")]
    TargetNodeNotInSpace {
        /// The target dock space.
        space: DockSpaceId,
        /// The target node.
        target: DockNodeId,
    },

    /// The source node is not contained by the source dock space.
    #[error("source node {node:?} not found in dock space {space}")]
    SourceNodeNotInSpace {
        /// The source dock space.
        space: DockSpaceId,
        /// The source node.
        node: DockNodeId,
    },

    /// The target dock space already has a root node.
    #[error("target dock space {space} is not empty")]
    TargetSpaceNotEmpty {
        /// The target dock space.
        space: DockSpaceId,
    },

    /// The requested floating container is not registered in the dock space.
    #[error("floating container {floating:?} not found in dock space {space}")]
    FloatingContainerNotFound {
        /// Dock space containing the floating container.
        space: DockSpaceId,
        /// Missing floating container node.
        floating: DockNodeId,
    },

    /// A split fraction update has the wrong number of fractions.
    #[error(
        "split node {split:?} has {children_len} children but received {fractions_len} fractions"
    )]
    SplitFractionsLenMismatch {
        /// The split node.
        split: DockNodeId,
        /// Current split child count.
        children_len: usize,
        /// Provided fraction count.
        fractions_len: usize,
    },

    /// A split fraction update targeted a split with too few children.
    #[error("split node {split:?} has too few children: {children_len}")]
    SplitTooFewChildren {
        /// The split node.
        split: DockNodeId,
        /// Current split child count.
        children_len: usize,
    },

    /// A split fraction is non-finite or negative.
    #[error("split node {split:?} fraction {index} is invalid")]
    SplitFractionInvalid {
        /// The split node.
        split: DockNodeId,
        /// Invalid fraction index.
        index: usize,
    },

    /// A batch split fraction operation contains more than one update for the same split.
    #[error("duplicate split fraction update for split node {split:?}")]
    DuplicateSplitFractionUpdate {
        /// The split node that appears more than once.
        split: DockNodeId,
    },

    /// The requested operation could not be applied.
    #[error("dock operation failed")]
    OperationFailed,
}
