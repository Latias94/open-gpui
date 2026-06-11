#[cfg(test)]
use crate::workspace_move_transaction::DockWorkspaceMoveTabRequest;
use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockGraph, DockGraphValidationError,
    DockItemId, DockLayout, DockLayoutValidationError, DockNodeId, DockPanel, DockPanelAttachError,
    DockPanelDescriptor, DockPanelRegistration, DockPanelRegistry, DockPolicy, DockSpaceId,
    DockWorkspace, EditorDockLayoutSpec, host::DockHostOptions,
    workspace_transaction::DockWorkspacePayloadDropRequest,
};
use open_gpui::{AnyView, Bounds, Entity, Focusable, Pixels, Render};

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

    /// Registers panel metadata without binding GPUI view lifecycle state.
    pub fn register_panel_descriptor(
        &mut self,
        item: impl Into<crate::DockItemId>,
        descriptor: DockPanelDescriptor,
    ) -> Option<DockPanelDescriptor> {
        self.workspace.register_panel_descriptor(item, descriptor)
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

    /// Registers a focusable GPUI view as panel content for a dock item.
    pub fn register_focusable_panel_view<V>(
        &mut self,
        item: impl Into<crate::DockItemId>,
        title: impl Into<String>,
        view: Entity<V>,
    ) -> Option<DockPanel>
    where
        V: Focusable + Render,
    {
        self.workspace
            .register_focusable_panel_view(item, title, view)
    }

    /// Registers a GPUI view factory as lazy panel content for a dock item.
    pub fn register_panel_factory(
        &mut self,
        item: impl Into<crate::DockItemId>,
        title: impl Into<String>,
        factory: impl Fn(&mut open_gpui::App) -> AnyView + 'static,
    ) -> Option<DockPanel> {
        self.workspace.register_panel_factory(item, title, factory)
    }

    /// Registers a focusable GPUI view factory as lazy panel content for a dock item.
    pub fn register_focusable_panel_factory<V>(
        &mut self,
        item: impl Into<crate::DockItemId>,
        title: impl Into<String>,
        factory: impl Fn(&mut open_gpui::App) -> Entity<V> + 'static,
    ) -> Option<DockPanel>
    where
        V: Focusable + Render,
    {
        self.workspace
            .register_focusable_panel_factory(item, title, factory)
    }

    /// Attaches GPUI view content to existing panel metadata.
    pub fn attach_panel_view(
        &mut self,
        item: impl Into<crate::DockItemId>,
        view: impl Into<AnyView>,
    ) -> Result<Option<DockPanelRegistration>, DockPanelAttachError> {
        self.workspace.attach_panel_view(item, view)
    }

    /// Attaches focusable GPUI view content to existing panel metadata.
    pub fn attach_focusable_panel_view<V>(
        &mut self,
        item: impl Into<crate::DockItemId>,
        view: Entity<V>,
    ) -> Result<Option<DockPanelRegistration>, DockPanelAttachError>
    where
        V: Focusable + Render,
    {
        self.workspace.attach_focusable_panel_view(item, view)
    }

    /// Attaches a GPUI view factory to existing panel metadata.
    pub fn attach_panel_factory(
        &mut self,
        item: impl Into<crate::DockItemId>,
        factory: impl Fn(&mut open_gpui::App) -> AnyView + 'static,
    ) -> Result<Option<DockPanelRegistration>, DockPanelAttachError> {
        self.workspace.attach_panel_factory(item, factory)
    }

    /// Attaches a lazy focusable GPUI view factory to existing panel metadata.
    pub fn attach_focusable_panel_factory<V>(
        &mut self,
        item: impl Into<crate::DockItemId>,
        factory: impl Fn(&mut open_gpui::App) -> Entity<V> + 'static,
    ) -> Result<Option<DockPanelRegistration>, DockPanelAttachError>
    where
        V: Focusable + Render,
    {
        self.workspace.attach_focusable_panel_factory(item, factory)
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

    /// Applies a docking action command object through the controller's workspace.
    pub fn apply_action(
        &mut self,
        action: &DockAction,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace.apply_action(action)
    }

    pub(crate) fn commit_resolved_payload_drop(
        &mut self,
        request: DockWorkspacePayloadDropRequest<'_>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace.commit_resolved_payload_drop(request)
    }

    pub(crate) fn commit_select_tab(
        &mut self,
        tabs: DockNodeId,
        item: &DockItemId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace.commit_select_tab(tabs, item)
    }

    pub(crate) fn commit_close_item(
        &mut self,
        space: &DockSpaceId,
        item: &DockItemId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace.commit_close_item(space, item)
    }

    #[cfg(test)]
    pub(crate) fn commit_tab_move(
        &mut self,
        request: DockWorkspaceMoveTabRequest<'_>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace.commit_tab_move(request)
    }

    pub(crate) fn commit_item_to_empty_dock_space(
        &mut self,
        source_space: &DockSpaceId,
        item: &DockItemId,
        target_space: &DockSpaceId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace
            .commit_item_to_empty_dock_space(source_space, item, target_space)
    }

    pub(crate) fn commit_tabs_to_empty_dock_space(
        &mut self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        target_space: &DockSpaceId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace
            .commit_tabs_to_empty_dock_space(source_space, source_tabs, target_space)
    }

    pub(crate) fn commit_floating_to_empty_dock_space(
        &mut self,
        source_space: &DockSpaceId,
        floating: DockNodeId,
        target_space: &DockSpaceId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace
            .commit_floating_to_empty_dock_space(source_space, floating, target_space)
    }

    pub(crate) fn commit_merge_space_into(
        &mut self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace
            .commit_merge_space_into(source_space, target_space)
    }

    pub(crate) fn commit_resize_split(
        &mut self,
        split: DockNodeId,
        fractions: &[f32],
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace.commit_resize_split(split, fractions)
    }

    pub(crate) fn commit_set_floating_bounds(
        &mut self,
        space: &DockSpaceId,
        floating: DockNodeId,
        bounds: Bounds<Pixels>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace
            .commit_set_floating_bounds(space, floating, bounds)
    }

    pub(crate) fn commit_raise_floating(
        &mut self,
        space: &DockSpaceId,
        floating: DockNodeId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace.commit_raise_floating(space, floating)
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

    /// Registers a prepared panel.
    pub fn panel(mut self, item: impl Into<DockItemId>, panel: DockPanel) -> Self {
        self.panels.push((item.into(), panel));
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

    /// Registers an eager GPUI view as panel content.
    pub fn panel_view(
        self,
        item: impl Into<DockItemId>,
        title: impl Into<String>,
        view: impl Into<AnyView>,
    ) -> Self {
        self.panel(item, DockPanel::new(title, view))
    }

    /// Registers an eager focusable GPUI view as panel content.
    pub fn focusable_panel_view<V>(
        self,
        item: impl Into<DockItemId>,
        title: impl Into<String>,
        view: Entity<V>,
    ) -> Self
    where
        V: Focusable + Render,
    {
        self.panel(item, DockPanel::focusable(title, view))
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

    /// Registers a lazy focusable GPUI view factory as panel content.
    pub fn focusable_panel_factory<V>(
        self,
        item: impl Into<DockItemId>,
        title: impl Into<String>,
        factory: impl Fn(&mut open_gpui::App) -> Entity<V> + 'static,
    ) -> Self
    where
        V: Focusable + Render,
    {
        self.panel(item, DockPanel::lazy_focusable(title, factory))
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
