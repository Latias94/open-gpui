use crate::{DockItemId, DockNodeId, DropZone, drop_preview::DockDropRoutePreviewKind};
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
    /// The active drag/drop preview overlay for the host.
    DropPreview,
    /// Visible guide for one local dock drop zone during a drag.
    DropGuide {
        /// Runtime node id that owns the guide, or None for host-level empty/root guides.
        node: Option<DockNodeId>,
        /// Zone advertised by the guide.
        zone: DropZone,
    },
    /// One payload tab label rendered inside a center/tab drop preview.
    DropPayloadTabPreview {
        /// Payload tab preview index in drag payload order.
        index: usize,
    },
    /// The body rectangle rendered below a center/tab drop preview tab label.
    DropPreviewBody,
    /// A viewport route or tear-off preview before host-local target resolution.
    DropRoutePreview {
        /// Preview route category.
        kind: DockDropRoutePreviewKind,
    },
    /// A tab label for one dock item.
    Tab {
        /// Runtime tabs node id containing the item.
        tabs: DockNodeId,
        /// Dock item id.
        item: DockItemId,
    },
    /// Close control for one tab label.
    TabClose {
        /// Runtime tabs node id containing the item.
        tabs: DockNodeId,
        /// Dock item id.
        item: DockItemId,
    },
    /// The selected panel body for one dock item.
    Panel {
        /// Dock item id.
        item: DockItemId,
    },
    /// The missing-panel placeholder for one dock item.
    MissingPanel {
        /// Dock item id.
        item: DockItemId,
    },
    /// An in-window floating container frame.
    Floating {
        /// Runtime floating node id.
        node: DockNodeId,
    },
    /// The drag handle for an in-window floating container.
    FloatingHandle {
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
