use crate::{DockGraphMutationError, DockItemId, DockNodeId, DockPolicyError, DockSpaceId};
use open_gpui::{Bounds, Pixels};
use thiserror::Error;

/// Fraction update for one split node.
#[derive(Debug, Clone, PartialEq)]
pub struct DockSplitResize {
    /// The split node to update.
    pub split: DockNodeId,
    /// The normalized fractions to store.
    pub fractions: Vec<f32>,
}

impl DockSplitResize {
    /// Creates a split resize update from a split id and fractions.
    pub fn new(split: DockNodeId, fractions: impl AsRef<[f32]>) -> Self {
        Self {
            split,
            fractions: fractions.as_ref().to_vec(),
        }
    }
}

/// Programmatic docking command object applied by [`DockWorkspace`](crate::model::DockWorkspace).
///
/// Rendered drag/drop interactions resolve a full-layout target first and commit through the
/// workspace transaction path. Prefer the named
/// [`DockController`](crate::model::DockController) and
/// [`DockWorkspace`](crate::model::DockWorkspace) command methods for ordinary application flows;
/// use these command objects when an application needs to store, queue, or dispatch explicit
/// docking commands.
#[derive(Debug, Clone, PartialEq)]
pub enum DockAction {
    /// Selects a tab within one tabs node.
    SelectTab {
        /// The tabs node containing the item.
        tabs: DockNodeId,
        /// The item to select.
        item: DockItemId,
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
    /// Resizes multiple split nodes in one graph transaction.
    ResizeSplits {
        /// Split fraction updates to validate and apply together.
        updates: Vec<DockSplitResize>,
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
    Graph(#[from] DockGraphMutationError),
    /// The action was rejected by workspace policy.
    #[error(transparent)]
    Policy(#[from] DockPolicyError),
    /// The resolved drop target was no longer available at commit time.
    #[error("dock drop target is not currently available")]
    DropTargetUnavailable,
    /// The host belongs to a surface window-session generation that no longer admits mutations.
    #[error("dock surface window session is not currently active")]
    SurfaceSessionUnavailable,
    /// The routed drop was created by a drag session that is no longer active.
    #[error("dock drop drag session {session} is no longer active")]
    DropDragSessionStale {
        /// Runtime drag session id recorded by the routed drop.
        session: u64,
    },
    /// The routed drop did not come from an active runtime drag session.
    #[error("dock drop is missing an active runtime drag session")]
    DropDragSessionMissing,
    /// The viewport runtime could not open a platform window for a tear-off request.
    #[error("tear-off viewport open failed: {message}")]
    TearOffViewportOpenFailed {
        /// Platform or GPUI error message returned while opening the window.
        message: String,
    },
    /// The tear-off route did not carry an authoritative platform-window placement.
    #[error("tear-off viewport placement is unavailable")]
    TearOffViewportPlacementUnavailable,
    /// A routed drop payload no longer matched the recorded drag source.
    #[error("dock drop payload for dock space {space} and tabs node {tabs:?} did not match")]
    DropPayloadMismatch {
        /// Source dock space recorded for the payload.
        space: DockSpaceId,
        /// Source tabs node recorded for the payload.
        tabs: DockNodeId,
    },
}
