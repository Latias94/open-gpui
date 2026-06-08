use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockGraph, DockItemId, DockLayout,
    DockLayoutValidationError, DockPanel, DockPanelRegistry, DockPolicy, DockSpaceId,
    DockWorkspace, EditorDockLayoutSpec, host::DockHostOptions,
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
    /// Starts the recommended app-author setup path for a dock space.
    pub fn builder(space: impl Into<DockSpaceId>) -> DockControllerBuilder {
        DockControllerBuilder::new(space)
    }

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

/// Builder for the common controller setup path.
///
/// The builder keeps the user-facing path centered on stable application concepts: one logical
/// dock space, a graph or restored layout, panel registrations, options, and policy. Advanced
/// callers can still construct [`DockWorkspace`] or [`DockGraph`] directly when they need lower
/// level control.
#[derive(Debug)]
pub struct DockControllerBuilder {
    space: DockSpaceId,
    graph: DockGraph,
    panels: Vec<(DockItemId, DockPanel)>,
    options: DockHostOptions,
    policy: DockPolicy,
}

impl DockControllerBuilder {
    /// Creates a builder for a logical dock space.
    pub fn new(space: impl Into<DockSpaceId>) -> Self {
        Self {
            space: space.into(),
            graph: DockGraph::new(),
            panels: Vec::new(),
            options: DockHostOptions::default(),
            policy: DockPolicy::default(),
        }
    }

    /// Replaces the builder graph with an already constructed graph.
    pub fn graph(mut self, graph: DockGraph) -> Self {
        self.graph = graph;
        self
    }

    /// Restores the builder graph from serialized dock layout data.
    ///
    /// The builder's logical space is unchanged. Applications can restore a layout containing
    /// multiple logical spaces, then mount whichever space each [`DockHost`](crate::DockHost)
    /// should render.
    pub fn try_layout(mut self, layout: &DockLayout) -> Result<Self, DockLayoutValidationError> {
        self.graph = DockGraph::import_layout(layout)?;
        Ok(self)
    }

    /// Replaces the builder graph with the common editor-style layout.
    pub fn default_editor_layout(mut self, spec: EditorDockLayoutSpec) -> Self {
        self.graph = DockGraph::default_editor_layout(self.space.clone(), spec);
        self
    }

    /// Registers a prepared panel.
    pub fn panel(mut self, item: impl Into<DockItemId>, panel: DockPanel) -> Self {
        self.panels.push((item.into(), panel));
        self
    }

    /// Registers an eager GPUI view as panel content.
    pub fn panel_view(
        self,
        item: impl Into<DockItemId>,
        title: impl Into<String>,
        view: impl Into<AnyView>,
    ) -> Self {
        self.panel(item, DockPanel::new(title, view))
    }

    /// Registers a lazy GPUI view factory as panel content.
    pub fn panel_factory(
        self,
        item: impl Into<DockItemId>,
        title: impl Into<String>,
        factory: impl Fn(&mut open_gpui::Context<crate::DockHost>) -> AnyView + 'static,
    ) -> Self {
        self.panel(item, DockPanel::lazy(title, factory))
    }

    /// Replaces static host rendering options.
    pub fn options(mut self, options: DockHostOptions) -> Self {
        self.options = options;
        self
    }

    /// Replaces the docking interaction policy.
    pub fn policy(mut self, policy: DockPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Enables or disables in-window floating interactions.
    pub fn allow_floating(mut self, allowed: bool) -> Self {
        self.policy.set_allow_floating(allowed);
        self
    }

    /// Enables or disables platform viewport interactions.
    pub fn allow_platform_viewports(mut self, allowed: bool) -> Self {
        self.policy.set_allow_platform_viewports(allowed);
        self
    }

    /// Builds the controller.
    pub fn build(self) -> DockController {
        let mut workspace = DockWorkspace::with_options(self.space, self.graph, self.options);
        workspace.set_policy(self.policy);
        for (item, panel) in self.panels {
            workspace.register_panel(item, panel);
        }
        DockController::new(workspace)
    }
}
