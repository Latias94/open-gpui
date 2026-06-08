use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockGraph, DockPanel, DockPanelRegistry,
    DockPolicy, DockSpaceId, DockWorkspace, host::DockHostOptions,
};
use open_gpui::AnyView;

/// Shared owner for one logical docking workspace.
///
/// A controller is the preferred owner when multiple rendered hosts need to observe and mutate the
/// same docking state. `DockHost` can render a controller-backed dock space without owning a cloned
/// graph.
#[derive(Debug)]
pub struct DockController {
    workspace: DockWorkspace,
}

impl DockController {
    /// Creates a controller from a configured workspace.
    pub fn new(workspace: DockWorkspace) -> Self {
        Self { workspace }
    }

    /// Creates a controller for one dock space and graph.
    pub fn from_graph(space: impl Into<DockSpaceId>, graph: DockGraph) -> Self {
        Self::with_options(space, graph, DockHostOptions::default())
    }

    /// Creates a controller with explicit static rendering options.
    pub fn with_options(
        space: impl Into<DockSpaceId>,
        graph: DockGraph,
        options: DockHostOptions,
    ) -> Self {
        Self::new(DockWorkspace::with_options(space, graph, options))
    }

    /// Returns the owned workspace.
    pub fn workspace(&self) -> &DockWorkspace {
        &self.workspace
    }

    /// Returns mutable access to the owned workspace.
    pub fn workspace_mut(&mut self) -> &mut DockWorkspace {
        &mut self.workspace
    }

    /// Returns the controller's default logical dock space.
    pub fn space(&self) -> &DockSpaceId {
        self.workspace.space()
    }

    /// Returns the docking graph.
    pub fn graph(&self) -> &DockGraph {
        self.workspace.graph()
    }

    /// Returns the panel registry.
    pub fn panels(&self) -> &DockPanelRegistry {
        self.workspace.panels()
    }

    /// Registers a panel for a dock item, returning any previous registration.
    pub fn register_panel(
        &mut self,
        item: impl Into<crate::DockItemId>,
        panel: DockPanel,
    ) -> Option<DockPanel> {
        self.workspace.register_panel(item, panel)
    }

    /// Registers a GPUI view as panel content for a dock item.
    pub fn register_panel_view(
        &mut self,
        item: impl Into<crate::DockItemId>,
        title: impl Into<String>,
        view: impl Into<AnyView>,
    ) -> Option<DockPanel> {
        self.workspace.register_panel_view(item, title, view)
    }

    /// Registers a GPUI view factory as lazy panel content for a dock item.
    pub fn register_panel_factory(
        &mut self,
        item: impl Into<crate::DockItemId>,
        title: impl Into<String>,
        factory: impl Fn(&mut open_gpui::Context<crate::DockHost>) -> AnyView + 'static,
    ) -> Option<DockPanel> {
        self.workspace.register_panel_factory(item, title, factory)
    }

    /// Returns the workspace rendering options.
    pub fn options(&self) -> &DockHostOptions {
        self.workspace.options()
    }

    /// Returns mutable workspace rendering options.
    pub fn options_mut(&mut self) -> &mut DockHostOptions {
        self.workspace.options_mut()
    }

    /// Returns the docking interaction policy.
    pub fn policy(&self) -> &DockPolicy {
        self.workspace.policy()
    }

    /// Returns mutable docking interaction policy.
    pub fn policy_mut(&mut self) -> &mut DockPolicy {
        self.workspace.policy_mut()
    }

    /// Applies a docking action through the controller's workspace.
    pub fn apply_action(
        &mut self,
        action: &DockAction,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace.apply_action(action)
    }
}
