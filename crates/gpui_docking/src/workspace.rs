use crate::{
    DockGraph, DockItemId, DockOp, DockOpApplyError, DockPanel, DockPanelAttachError,
    DockPanelDescriptor, DockPanelRegistration, DockPanelRegistry, DockPolicy, DockSpaceId,
    host::DockHostOptions,
};
use open_gpui::{AnyView, Entity, Focusable, Render};

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
    policy: DockPolicy,
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
            policy: DockPolicy::default(),
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

    pub(crate) fn apply_op_checked(&mut self, op: &DockOp) -> Result<bool, DockOpApplyError> {
        self.graph.apply_op_checked(op)
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

    /// Registers panel metadata without binding GPUI view lifecycle state.
    pub fn register_panel_descriptor(
        &mut self,
        item: impl Into<DockItemId>,
        descriptor: DockPanelDescriptor,
    ) -> Option<DockPanelDescriptor> {
        self.panels.register_descriptor(item, descriptor)
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

    /// Registers a focusable GPUI view as panel content for a dock item.
    pub fn register_focusable_panel_view<V>(
        &mut self,
        item: impl Into<DockItemId>,
        title: impl Into<String>,
        view: Entity<V>,
    ) -> Option<DockPanel>
    where
        V: Focusable + Render,
    {
        self.panels.register_focusable_view(item, title, view)
    }

    /// Registers a GPUI view factory as lazy panel content for a dock item.
    pub fn register_panel_factory(
        &mut self,
        item: impl Into<DockItemId>,
        title: impl Into<String>,
        factory: impl Fn(&mut open_gpui::App) -> AnyView + 'static,
    ) -> Option<DockPanel> {
        self.panels.register_factory(item, title, factory)
    }

    /// Registers a focusable GPUI view factory as lazy panel content for a dock item.
    pub fn register_focusable_panel_factory<V>(
        &mut self,
        item: impl Into<DockItemId>,
        title: impl Into<String>,
        factory: impl Fn(&mut open_gpui::App) -> Entity<V> + 'static,
    ) -> Option<DockPanel>
    where
        V: Focusable + Render,
    {
        self.panels.register_focusable_factory(item, title, factory)
    }

    /// Attaches GPUI view content to existing panel metadata.
    pub fn attach_panel_view(
        &mut self,
        item: impl Into<DockItemId>,
        view: impl Into<AnyView>,
    ) -> Result<Option<DockPanelRegistration>, DockPanelAttachError> {
        self.panels.attach_view(item, view)
    }

    /// Attaches focusable GPUI view content to existing panel metadata.
    pub fn attach_focusable_panel_view<V>(
        &mut self,
        item: impl Into<DockItemId>,
        view: Entity<V>,
    ) -> Result<Option<DockPanelRegistration>, DockPanelAttachError>
    where
        V: Focusable + Render,
    {
        self.panels.attach_focusable_view(item, view)
    }

    /// Attaches a lazy GPUI view factory to existing panel metadata.
    pub fn attach_panel_factory(
        &mut self,
        item: impl Into<DockItemId>,
        factory: impl Fn(&mut open_gpui::App) -> AnyView + 'static,
    ) -> Result<Option<DockPanelRegistration>, DockPanelAttachError> {
        self.panels.attach_factory(item, factory)
    }

    /// Attaches a lazy focusable GPUI view factory to existing panel metadata.
    pub fn attach_focusable_panel_factory<V>(
        &mut self,
        item: impl Into<DockItemId>,
        factory: impl Fn(&mut open_gpui::App) -> Entity<V> + 'static,
    ) -> Result<Option<DockPanelRegistration>, DockPanelAttachError>
    where
        V: Focusable + Render,
    {
        self.panels.attach_focusable_factory(item, factory)
    }

    /// Returns the workspace rendering options.
    pub fn options(&self) -> &DockHostOptions {
        &self.options
    }

    /// Returns mutable workspace rendering options.
    pub fn options_mut(&mut self) -> &mut DockHostOptions {
        &mut self.options
    }

    /// Returns the workspace docking interaction policy.
    pub fn policy(&self) -> &DockPolicy {
        &self.policy
    }

    /// Returns mutable workspace docking interaction policy.
    pub fn policy_mut(&mut self) -> &mut DockPolicy {
        &mut self.policy
    }

    /// Replaces the workspace docking interaction policy.
    pub fn set_policy(&mut self, policy: DockPolicy) {
        self.policy = policy;
    }
}
