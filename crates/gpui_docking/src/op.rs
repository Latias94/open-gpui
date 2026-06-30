use crate::{DockEdgeDockPlan, DockItemId, DockNodeId, DockSpaceId, DockSplitResize, DropZone};
use thiserror::Error;

/// Graph-level target consumed by dock tree drop mutations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum DockGraphDropTarget {
    /// Merge into an existing tab stack through a center dock-over target.
    Center {
        /// Target tabs node.
        tabs: DockNodeId,
    },
    /// Insert into an existing tab bar at a concrete tab index.
    TabBar {
        /// Target tabs node.
        tabs: DockNodeId,
        /// Insertion index in the target tabs node.
        insert_index: usize,
    },
    /// Dock against an edge anchor.
    Edge {
        /// Precomputed topology plan for the edge drop.
        plan: DockEdgeDockPlan,
    },
    /// Promote the payload subtree as the root of an empty dock space.
    EmptySpace,
}

impl DockGraphDropTarget {
    /// Builds a center merge target.
    pub(crate) fn center(tabs: DockNodeId) -> Self {
        Self::Center { tabs }
    }

    /// Builds a tab-bar insertion target.
    pub(crate) fn tab_bar(tabs: DockNodeId, insert_index: usize) -> Self {
        Self::TabBar { tabs, insert_index }
    }

    /// Builds an edge target from a precomputed dock topology plan.
    pub(crate) fn edge(plan: DockEdgeDockPlan) -> Self {
        Self::Edge { plan }
    }

    /// Builds an empty-space promotion target.
    pub(crate) fn empty_space() -> Self {
        Self::EmptySpace
    }

    pub(crate) fn existing_node(self) -> Option<DockNodeId> {
        match self {
            Self::Center { tabs } | Self::TabBar { tabs, .. } => Some(tabs),
            Self::Edge { plan } => Some(plan.target_node()),
            Self::EmptySpace => None,
        }
    }

    pub(crate) fn drop_zone(self) -> Option<DropZone> {
        match self {
            Self::Center { .. } | Self::TabBar { .. } => Some(DropZone::Center),
            Self::Edge { plan } => Some(plan.drop_zone()),
            Self::EmptySpace => None,
        }
    }

    pub(crate) fn center_tabs(self) -> Option<DockNodeId> {
        match self {
            Self::Center { tabs } => Some(tabs),
            Self::TabBar { .. } | Self::Edge { .. } | Self::EmptySpace => None,
        }
    }
}

/// High-level graph mutation emitted by docking UI or application code.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockOp {
    /// Selects an item within a tabs node.
    SelectTab {
        /// The tabs node to update.
        tabs: DockNodeId,
        /// The item to select.
        item: DockItemId,
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

    /// Moves one item into another dock target.
    MoveItem {
        /// The source dock space.
        source_space: DockSpaceId,
        /// The item to move.
        item: DockItemId,
        /// The target dock space.
        target_space: DockSpaceId,
        /// Graph drop target.
        target: DockGraphDropTarget,
    },

    /// Moves an entire tabs node as a group.
    MoveTabs {
        /// The source dock space.
        source_space: DockSpaceId,
        /// The tabs node to move.
        source_tabs: DockNodeId,
        /// The target dock space.
        target_space: DockSpaceId,
        /// Graph drop target.
        target: DockGraphDropTarget,
    },

    /// Moves an in-window floating subtree into another dock target.
    MoveFloating {
        /// The source dock space containing the floating container.
        source_space: DockSpaceId,
        /// The floating container node to move.
        floating: DockNodeId,
        /// The target dock space.
        target_space: DockSpaceId,
        /// Graph drop target.
        target: DockGraphDropTarget,
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
    SetSplitFractionsMany {
        /// The split fraction updates.
        updates: Vec<DockSplitResize>,
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

    /// The requested item is not present in the target tabs node.
    #[error("dock item {item} not found in tabs node {tabs:?}")]
    ItemNotInTabs {
        /// The tabs node.
        tabs: DockNodeId,
        /// Missing item.
        item: DockItemId,
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

    /// A visibly split payload cannot be center-merged into a non-empty target.
    #[error(
        "visible split payload {payload:?} cannot be center-merged into non-empty target {target:?}"
    )]
    VisibleSplitPayloadCannotDockOverNonEmptyTarget {
        /// The split payload root.
        payload: DockNodeId,
        /// The non-empty target node.
        target: DockNodeId,
    },

    /// A whole-space merge needs exactly one concrete target tab stack.
    #[error("dock space {space} has {tabs_len} root tab stacks; merge target is not unique")]
    MergeTargetTabsNotUnique {
        /// The target dock space.
        space: DockSpaceId,
        /// Number of target tab stacks reachable from the root.
        tabs_len: usize,
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
