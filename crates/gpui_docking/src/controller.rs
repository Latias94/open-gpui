use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockClassId, DockGraph,
    DockGraphValidationError, DockItemId, DockLayout, DockLayoutValidationError, DockNodeId,
    DockPanel, DockPanelAttachError, DockPanelDescriptor, DockPanelRegistration, DockPanelRegistry,
    DockPolicy, DockSpaceId, DockSplitResize, DockWorkspace, EditorDockLayoutSpec,
    host::DockHostOptions,
};
use open_gpui::{AnyView, Bounds, Pixels};

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

    /// Returns the owned workspace.
    pub fn workspace(&self) -> &DockWorkspace {
        &self.workspace
    }

    pub(crate) fn workspace_mut(&mut self) -> &mut DockWorkspace {
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

    /// Attaches GPUI view content to existing panel metadata.
    pub fn attach_panel_view(
        &mut self,
        item: impl Into<DockItemId>,
        view: impl Into<AnyView>,
    ) -> Result<Option<DockPanelRegistration>, DockPanelAttachError> {
        self.workspace.panels_mut().attach_view_handle(
            item.into(),
            crate::panel_view::DockPanelViewHandle::from_view(view),
        )
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

    /// Selects a tab within one tabs node.
    pub fn select_tab(
        &mut self,
        tabs: DockNodeId,
        item: impl Into<DockItemId>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace.select_tab(tabs, item)
    }

    /// Selects the tab containing one item within the controller's logical dock space.
    pub fn select_item_in_space(
        &mut self,
        item: impl Into<DockItemId>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace
            .select_item_in_space(self.space().clone(), item)
    }

    /// Closes one registered dock item through panel lifecycle policy.
    pub fn close_item(
        &mut self,
        space: impl Into<DockSpaceId>,
        item: impl Into<DockItemId>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace.close_item(space, item)
    }

    /// Opens one registered dock item into an existing tabs node or empty dock space.
    pub fn open_item(
        &mut self,
        space: impl Into<DockSpaceId>,
        target_tabs: Option<DockNodeId>,
        item: impl Into<DockItemId>,
        insert_index: Option<usize>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace
            .open_item(space, target_tabs, item, insert_index)
    }

    /// Floats one item inside a dock space without creating a platform window.
    pub fn float_item_in_window(
        &mut self,
        source_space: impl Into<DockSpaceId>,
        item: impl Into<DockItemId>,
        target_space: impl Into<DockSpaceId>,
        bounds: Bounds<Pixels>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace
            .float_item_in_window(source_space, item, target_space, bounds)
    }

    /// Floats an entire tabs node inside a dock space without creating a platform window.
    pub fn float_tabs_in_window(
        &mut self,
        source_space: impl Into<DockSpaceId>,
        source_tabs: DockNodeId,
        target_space: impl Into<DockSpaceId>,
        bounds: Bounds<Pixels>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace
            .float_tabs_in_window(source_space, source_tabs, target_space, bounds)
    }

    /// Updates the bounds of an in-window floating container.
    pub fn set_floating_bounds(
        &mut self,
        space: impl Into<DockSpaceId>,
        floating: DockNodeId,
        bounds: Bounds<Pixels>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace.set_floating_bounds(space, floating, bounds)
    }

    /// Raises an in-window floating container above other floating containers.
    pub fn raise_floating(
        &mut self,
        space: impl Into<DockSpaceId>,
        floating: DockNodeId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace.raise_floating(space, floating)
    }

    /// Merges an in-window floating container into an existing tabs node.
    pub fn merge_floating_into(
        &mut self,
        space: impl Into<DockSpaceId>,
        floating: DockNodeId,
        target_tabs: DockNodeId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace
            .merge_floating_into(space, floating, target_tabs)
    }

    /// Resizes one split node by replacing its normalized fractions.
    pub fn resize_split(
        &mut self,
        split: DockNodeId,
        fractions: impl AsRef<[f32]>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace.resize_split(split, fractions)
    }

    /// Resizes multiple split nodes in one graph transaction.
    pub fn resize_splits(
        &mut self,
        updates: impl AsRef<[DockSplitResize]>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace.resize_splits(updates)
    }

    /// Applies a docking action command object through the controller's workspace.
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
    descriptors: Vec<(DockItemId, DockPanelDescriptor)>,
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
            descriptors: Vec::new(),
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

    /// Registers descriptor-only panel metadata.
    pub fn panel_descriptor(
        mut self,
        item: impl Into<DockItemId>,
        descriptor: DockPanelDescriptor,
    ) -> Self {
        self.descriptors.push((item.into(), descriptor));
        self
    }

    /// Registers a prepared panel.
    pub fn panel(mut self, item: impl Into<DockItemId>, panel: DockPanel) -> Self {
        self.panels.push((item.into(), panel));
        self
    }

    /// Registers a lazy GPUI view factory as panel content.
    pub fn panel_factory(
        self,
        item: impl Into<DockItemId>,
        title: impl Into<String>,
        factory: impl Fn(&mut open_gpui::App) -> AnyView + 'static,
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

    /// Enables or disables restoring dock-panel focus when a platform window gains focus.
    pub fn platform_focus_sets_dock_focus(mut self, enabled: bool) -> Self {
        self.policy.set_platform_focus_sets_dock_focus(enabled);
        self
    }

    /// Allows one dock class to be dropped into the given dock space.
    pub fn allow_dock_class_in_space(
        mut self,
        space: impl Into<DockSpaceId>,
        dock_class: impl Into<DockClassId>,
    ) -> Self {
        self.policy.allow_dock_class_in_space(space, dock_class);
        self
    }

    /// Builds the controller without validating a custom graph.
    ///
    /// Use [`Self::try_build`] when accepting restored or user-authored graph state.
    pub fn build(self) -> DockController {
        let mut workspace = DockWorkspace::with_options(self.space, self.graph, self.options);
        workspace.set_policy(self.policy);
        for (item, descriptor) in self.descriptors {
            workspace.register_panel_descriptor(item, descriptor);
        }
        for (item, panel) in self.panels {
            workspace.register_panel(item, panel);
        }
        DockController::new(workspace)
    }

    /// Validates reachable graph state and builds the controller.
    pub fn try_build(self) -> Result<DockController, DockGraphValidationError> {
        self.graph.validate()?;
        Ok(self.build())
    }
}
