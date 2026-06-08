use crate::{
    DockGraph, DockItemId, DockNodeId, DockPanel, DockPanelRegistry, DockSpaceId,
    workspace::DockWorkspace,
};
use open_gpui::AnyView;
use std::collections::HashMap;

/// Debug-test region emitted by a [`DockHost`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DockDebugRegion {
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
    /// A tabs container.
    Tabs {
        /// Runtime tabs node id.
        node: DockNodeId,
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

/// Static host rendering options.
#[derive(Debug, Clone)]
pub struct DockHostOptions {
    /// Message rendered when the selected dock space has no root node.
    pub empty_message: String,
    /// Message prefix rendered when an active panel is missing from the registry.
    pub missing_panel_prefix: String,
    /// Message rendered for in-window floating nodes during Phase 2.
    pub deferred_floating_message: String,
}

impl Default for DockHostOptions {
    fn default() -> Self {
        Self {
            empty_message: "Empty dock space".to_string(),
            missing_panel_prefix: "Missing panel".to_string(),
            deferred_floating_message: "Floating panels render in a later phase".to_string(),
        }
    }
}

/// Retained GPUI host that renders one logical dock space from a [`DockGraph`].
#[derive(Debug)]
pub struct DockHost {
    workspace: DockWorkspace,
    debug_selectors: HashMap<DockDebugRegion, String>,
}

impl DockHost {
    /// Creates a host for one dock space and graph.
    pub fn new(space: impl Into<DockSpaceId>, graph: DockGraph) -> Self {
        Self::with_options(space, graph, DockHostOptions::default())
    }

    /// Creates a host with explicit static rendering options.
    pub fn with_options(
        space: impl Into<DockSpaceId>,
        graph: DockGraph,
        options: DockHostOptions,
    ) -> Self {
        Self {
            workspace: DockWorkspace::with_options(space, graph, options),
            debug_selectors: HashMap::new(),
        }
    }

    /// Returns the logical dock space rendered by this host.
    pub fn space(&self) -> &DockSpaceId {
        self.workspace.space()
    }

    /// Returns the host graph.
    pub fn graph(&self) -> &DockGraph {
        self.workspace.graph()
    }

    /// Returns the host graph for mutation by application code.
    pub fn graph_mut(&mut self) -> &mut DockGraph {
        self.workspace.graph_mut()
    }

    /// Replaces the host graph.
    pub fn set_graph(&mut self, graph: DockGraph) {
        self.workspace.set_graph(graph);
    }

    /// Returns the panel registry.
    pub fn panels(&self) -> &DockPanelRegistry {
        self.workspace.panels()
    }

    /// Returns the panel registry for mutation by application code.
    pub fn panels_mut(&mut self) -> &mut DockPanelRegistry {
        self.workspace.panels_mut()
    }

    /// Registers a panel for a dock item, returning any previous registration.
    pub fn register_panel(
        &mut self,
        item: impl Into<DockItemId>,
        panel: DockPanel,
    ) -> Option<DockPanel> {
        self.workspace.register_panel(item, panel)
    }

    /// Registers a GPUI view as panel content for a dock item.
    pub fn register_panel_view(
        &mut self,
        item: impl Into<DockItemId>,
        title: impl Into<String>,
        view: impl Into<AnyView>,
    ) -> Option<DockPanel> {
        self.workspace.register_panel_view(item, title, view)
    }

    /// Returns the static rendering options.
    pub fn options(&self) -> &DockHostOptions {
        self.workspace.options()
    }

    /// Returns mutable static rendering options.
    pub fn options_mut(&mut self) -> &mut DockHostOptions {
        self.workspace.options_mut()
    }

    /// Returns a debug selector emitted for a test region during the most recent render.
    pub fn debug_selector(&self, region: &DockDebugRegion) -> Option<&str> {
        self.debug_selectors.get(region).map(String::as_str)
    }

    pub(crate) fn clear_debug_selectors(&mut self) {
        self.debug_selectors.clear();
    }

    pub(crate) fn record_debug_selector(
        &mut self,
        region: DockDebugRegion,
        selector: String,
    ) -> String {
        self.debug_selectors.insert(region, selector.clone());
        selector
    }

    pub(crate) fn selector_prefix(&self) -> String {
        format!("dock:{}", self.space())
    }
}
