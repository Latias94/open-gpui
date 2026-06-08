use crate::{DockItemId, DockNodeId};
#[cfg(test)]
use std::collections::HashMap;

/// Debug-test region emitted by a dock host render pass.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum DockDebugRegion {
    /// The whole dock host.
    Host,
    /// The empty dock-space placeholder.
    EmptySpace,
    /// A split container.
    Split {
        /// Runtime split node id.
        node: DockNodeId,
    },
    /// A child wrapper inside a split container.
    SplitChild {
        /// Runtime split node id.
        split: DockNodeId,
        /// Child index within the split.
        index: usize,
    },
    /// A splitter handle between adjacent split children.
    SplitterHandle {
        /// Runtime split node id.
        split: DockNodeId,
        /// Handle index between child `index` and child `index + 1`.
        index: usize,
    },
    /// A tabs container.
    Tabs {
        /// Runtime tabs node id.
        node: DockNodeId,
    },
    /// A tab drag/drop preview overlay for one tabs container.
    DropPreview {
        /// Runtime tabs node id.
        tabs: DockNodeId,
    },
    /// A tab label for one dock item.
    Tab {
        /// Runtime tabs node id containing the item.
        tabs: DockNodeId,
        /// Dock item id.
        item: DockItemId,
    },
    /// The active panel body for one dock item.
    Panel {
        /// Dock item id.
        item: DockItemId,
    },
    /// The missing-panel placeholder for one dock item.
    MissingPanel {
        /// Dock item id.
        item: DockItemId,
    },
    /// A placeholder for a floating node deferred by Phase 2.
    DeferredFloating {
        /// Runtime floating node id.
        node: DockNodeId,
    },
    /// A placeholder for a graph node that cannot be found.
    MissingNode {
        /// Runtime node id referenced by the graph.
        node: DockNodeId,
    },
}

/// Selector instrumentation used by crate-local visual tests.
#[cfg(test)]
#[derive(Debug, Default)]
pub(crate) struct DockDebugInstrumentation {
    selectors: HashMap<DockDebugRegion, String>,
}

#[cfg(test)]
impl DockDebugInstrumentation {
    /// Clears selectors from the previous render pass.
    pub(crate) fn clear(&mut self) {
        self.selectors.clear();
    }

    /// Records a selector for a region and returns the selector for element wiring.
    pub(crate) fn record(&mut self, region: DockDebugRegion, selector: String) -> String {
        self.selectors.insert(region, selector.clone());
        selector
    }

    /// Returns the selector emitted for a region in the most recent render pass.
    #[cfg(test)]
    pub(crate) fn selector(&self, region: &DockDebugRegion) -> Option<&str> {
        self.selectors.get(region).map(String::as_str)
    }
}
