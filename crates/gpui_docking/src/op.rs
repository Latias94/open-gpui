use crate::{DockItemId, DockNodeId, DockSpaceId, DropZone};
use thiserror::Error;

/// Graph-level target for an existing dock tree move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockMoveTarget {
    /// Merge into an existing tab stack.
    Stack {
        /// Target tabs node.
        tabs: DockNodeId,
        /// Optional insertion index in the target tabs node.
        insert_index: Option<usize>,
    },
    /// Dock against an edge anchor.
    Edge {
        /// Edge anchor carrying whether this is a leaf or root edge.
        anchor: DockMoveTargetAnchor,
        /// Edge zone.
        zone: DropZone,
    },
}

/// Anchor for graph-level edge docking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockMoveTargetAnchor {
    /// Inner edge around a leaf, tied to its owning root.
    Leaf {
        /// Root containing the leaf.
        root: DockNodeId,
        /// Leaf tabs node.
        tabs: DockNodeId,
    },
    /// Outer edge around a dock root.
    Root {
        /// Root node.
        root: DockNodeId,
    },
}

impl DockMoveTarget {
    /// Builds a center merge target.
    pub(crate) fn center(tabs: DockNodeId) -> Self {
        Self::Stack {
            tabs,
            insert_index: None,
        }
    }

    /// Builds a tab-bar insertion target.
    pub(crate) fn tab_bar(tabs: DockNodeId, insert_index: usize) -> Self {
        Self::Stack {
            tabs,
            insert_index: Some(insert_index),
        }
    }

    /// Builds an inner-edge target around a leaf.
    pub(crate) fn inner_edge(root: DockNodeId, tabs: DockNodeId, zone: DropZone) -> Self {
        Self::Edge {
            anchor: DockMoveTargetAnchor::Leaf { root, tabs },
            zone,
        }
    }

    /// Builds an outer-edge target around a root.
    pub(crate) fn root_edge(root: DockNodeId, zone: DropZone) -> Self {
        Self::Edge {
            anchor: DockMoveTargetAnchor::Root { root },
            zone,
        }
    }

    pub(crate) fn node(self) -> DockNodeId {
        match self {
            Self::Stack { tabs, .. } => tabs,
            Self::Edge { anchor, .. } => anchor.node(),
        }
    }

    pub(crate) fn zone(self) -> DropZone {
        match self {
            Self::Stack { .. } => DropZone::Center,
            Self::Edge { zone, .. } => zone,
        }
    }

    pub(crate) fn insert_index(self) -> Option<usize> {
        match self {
            Self::Stack { insert_index, .. } => insert_index,
            Self::Edge { .. } => None,
        }
    }

    pub(crate) fn noop_tabs(self) -> Option<DockNodeId> {
        match self {
            Self::Stack { tabs, .. } => Some(tabs),
            Self::Edge { .. } => None,
        }
    }
}

impl DockMoveTargetAnchor {
    pub(crate) fn node(self) -> DockNodeId {
        match self {
            Self::Leaf { root: _, tabs } => tabs,
            Self::Root { root } => root,
        }
    }
}

/// High-level graph mutation emitted by docking UI or application code.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockOp {
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

    /// Opens a registered item into an existing tabs node or an empty dock space.
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

    /// Moves one item into an existing tabs node or split target.
    MoveItem {
        /// The source dock space.
        source_space: DockSpaceId,
        /// The item to move.
        item: DockItemId,
        /// The target dock space.
        target_space: DockSpaceId,
        /// Existing graph target.
        target: DockMoveTarget,
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
        /// Existing graph target.
        target: DockMoveTarget,
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

    /// Moves an in-window floating subtree into an existing target.
    MoveFloating {
        /// The source dock space containing the floating container.
        source_space: DockSpaceId,
        /// The floating container node to move.
        floating: DockNodeId,
        /// The target dock space.
        target_space: DockSpaceId,
        /// Existing graph target.
        target: DockMoveTarget,
    },

    /// Moves an in-window floating subtree into an empty dock space.
    MoveFloatingToEmptyDockSpace {
        /// The source dock space containing the floating container.
        source_space: DockSpaceId,
        /// The floating container node to move.
        floating: DockNodeId,
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

    /// Replaces every fraction in one split node.
    SetSplitFractions {
        /// The split node to update.
        split: DockNodeId,
        /// The normalized fractions to store.
        fractions: Vec<f32>,
    },

    /// Replaces fractions for multiple split nodes.
    #[cfg(test)]
    SetSplitFractionsMany {
        /// The split fraction updates.
        updates: Vec<SplitFractionsUpdate>,
    },

    /// Updates a two-child split using the first child's fraction.
    #[cfg(test)]
    SetSplitFractionTwo {
        /// The split node to update.
        split: DockNodeId,
        /// The first child's fraction.
        first_fraction: f32,
    },
}

/// Fraction update for one split node.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SplitFractionsUpdate {
    /// The split node to update.
    pub(crate) split: DockNodeId,
    /// The normalized fractions to store.
    pub(crate) fractions: Vec<f32>,
}

/// Error returned when a checked graph mutation cannot be applied.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DockGraphMutationError {
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

    /// The requested item is already reachable in the dock graph.
    #[error("dock item {item} is already open")]
    ItemAlreadyOpen {
        /// The already-open item.
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

    /// The requested tabs node has no items to move.
    #[error("tabs node {tabs:?} is empty")]
    TabsNodeEmpty {
        /// Empty tabs node.
        tabs: DockNodeId,
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

    /// A floating container cannot merge into a tabs node inside its own subtree.
    #[error("floating container {floating:?} cannot merge into its own target {target:?}")]
    CannotMergeFloatingIntoOwnSubtree {
        /// The floating container being merged.
        floating: DockNodeId,
        /// The target tabs node inside the floating container.
        target: DockNodeId,
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

    /// The graph mutation reported success or no-op incorrectly after partially changing state.
    #[error("dock graph mutation {op} violated transactional guarantees: {reason}")]
    MutationInvariantViolation {
        /// The mutation that failed its transactional contract.
        op: &'static str,
        /// The observed invariant failure.
        reason: String,
    },
}
