use crate::{
    DockGraph, DockItemId, DockOp, DockOpApplyError, DockPanel, DockPanelRegistry, DockSpaceId,
    host::DockHostOptions,
};
use open_gpui::AnyView;

/// Owner for one logical docking workspace.
///
/// A workspace coordinates the pure graph, selected dock space, panel registry, host options, and
/// future interaction state. GPUI render adapters such as [`DockHost`](crate::DockHost) should
/// render a workspace instead of owning every piece of docking state directly.
#[derive(Debug)]
pub struct DockWorkspace {
    graph: DockGraph,
    space: DockSpaceId,
    panels: DockPanelRegistry,
    options: DockHostOptions,
}

impl DockWorkspace {
    /// Creates a workspace for one dock space and graph.
    pub fn new(space: impl Into<DockSpaceId>, graph: DockGraph) -> Self {
        Self::with_options(space, graph, DockHostOptions::default())
    }

    /// Creates a workspace with explicit static rendering options.
    pub fn with_options(
        space: impl Into<DockSpaceId>,
        graph: DockGraph,
        options: DockHostOptions,
    ) -> Self {
        Self {
            graph,
            space: space.into(),
            panels: DockPanelRegistry::new(),
            options,
        }
    }

    /// Returns the logical dock space owned by this workspace.
    pub fn space(&self) -> &DockSpaceId {
        &self.space
    }

    /// Returns the workspace graph.
    pub fn graph(&self) -> &DockGraph {
        &self.graph
    }

    /// Replaces the workspace graph.
    pub fn set_graph(&mut self, graph: DockGraph) {
        self.graph = graph;
    }

    /// Applies a docking operation with checked failure reporting.
    pub fn apply_op_checked(&mut self, op: &DockOp) -> Result<bool, DockOpApplyError> {
        self.graph.apply_op_checked(op)
    }

    /// Applies a docking operation and returns whether it changed or preserved valid state.
    pub fn apply_op(&mut self, op: &DockOp) -> bool {
        self.graph.apply_op(op)
    }

    /// Returns the panel registry.
    pub fn panels(&self) -> &DockPanelRegistry {
        &self.panels
    }

    /// Registers a panel for a dock item, returning any previous registration.
    pub fn register_panel(
        &mut self,
        item: impl Into<DockItemId>,
        panel: DockPanel,
    ) -> Option<DockPanel> {
        self.panels.register(item, panel)
    }

    /// Registers a GPUI view as panel content for a dock item.
    pub fn register_panel_view(
        &mut self,
        item: impl Into<DockItemId>,
        title: impl Into<String>,
        view: impl Into<AnyView>,
    ) -> Option<DockPanel> {
        self.panels.register_view(item, title, view)
    }

    /// Returns the workspace rendering options.
    pub fn options(&self) -> &DockHostOptions {
        &self.options
    }

    /// Returns mutable workspace rendering options.
    pub fn options_mut(&mut self) -> &mut DockHostOptions {
        &mut self.options
    }

    pub(crate) fn graph_mut(&mut self) -> &mut DockGraph {
        &mut self.graph
    }

    pub(crate) fn panels_mut(&mut self) -> &mut DockPanelRegistry {
        &mut self.panels
    }
}
