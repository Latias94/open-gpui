use crate::{
    DockGraph, DockGraphMutationError, DockItemId, DockNodeId, DockOp, DockPanel,
    DockPanelDescriptor, DockPanelRegistry, DockPolicy, DockSpaceId, DockViewportDropPayload,
    drag::DockDragPayload, host::DockHostOptions,
    workspace_drop_transaction::DockWorkspaceDropPayload,
};

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

    pub(crate) fn apply_op_checked(&mut self, op: &DockOp) -> Result<bool, DockGraphMutationError> {
        self.graph.apply_op_checked(op)
    }

    /// Returns the panel registry.
    pub fn panels(&self) -> &DockPanelRegistry {
        &self.panels
    }

    pub(crate) fn panels_mut(&mut self) -> &mut DockPanelRegistry {
        &mut self.panels
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
        view: impl Into<open_gpui::AnyView>,
    ) -> Option<DockPanel> {
        self.panels
            .register(item, DockPanel::new(title, view.into()))
    }

    /// Registers a focusable GPUI view as panel content for a dock item.
    pub fn register_focusable_panel_view<V>(
        &mut self,
        item: impl Into<DockItemId>,
        title: impl Into<String>,
        view: open_gpui::Entity<V>,
    ) -> Option<DockPanel>
    where
        V: open_gpui::Focusable + open_gpui::Render,
    {
        self.panels
            .register(item, DockPanel::focusable(title, view))
    }

    /// Registers a GPUI view factory as lazy panel content for a dock item.
    pub fn register_panel_factory(
        &mut self,
        item: impl Into<DockItemId>,
        title: impl Into<String>,
        factory: impl Fn(&mut open_gpui::App) -> open_gpui::AnyView + 'static,
    ) -> Option<DockPanel> {
        self.panels.register(item, DockPanel::lazy(title, factory))
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

    pub(crate) fn activation_focus_item_for_workspace_payload(
        &self,
        payload: &DockWorkspaceDropPayload<'_>,
        target_space: Option<&DockSpaceId>,
        frozen_focus_item: Option<&DockItemId>,
    ) -> Option<DockItemId> {
        let item = match payload {
            DockWorkspaceDropPayload::Item { item, .. } => (*item).clone(),
            DockWorkspaceDropPayload::Tabs { .. } | DockWorkspaceDropPayload::Floating { .. } => {
                frozen_focus_item?.clone()
            }
        };

        match target_space {
            Some(space) if self.graph.find_item_in_space(space, &item).is_some() => Some(item),
            Some(_) => None,
            None => Some(item),
        }
    }

    pub(crate) fn activation_focus_item_for_viewport_payload(
        &self,
        payload: &DockViewportDropPayload,
        source_node: DockNodeId,
        frozen_focus_item: Option<&DockItemId>,
    ) -> Option<DockItemId> {
        let workspace_payload = payload.as_workspace_payload(source_node);
        self.resolve_payload_focus_item(&workspace_payload, None, frozen_focus_item)
    }

    pub(crate) fn drag_focus_item_for_payload(
        &self,
        payload: &DockDragPayload,
        recorded_focus_item: Option<&DockItemId>,
    ) -> Option<DockItemId> {
        let workspace_payload = payload.as_workspace_payload();
        self.resolve_payload_focus_item(&workspace_payload, None, recorded_focus_item)
    }

    fn resolve_payload_focus_item(
        &self,
        payload: &DockWorkspaceDropPayload<'_>,
        target_space: Option<&DockSpaceId>,
        frozen_focus_item: Option<&DockItemId>,
    ) -> Option<DockItemId> {
        let item = match payload {
            DockWorkspaceDropPayload::Item { item, .. } => (*item).clone(),
            DockWorkspaceDropPayload::Tabs { .. } | DockWorkspaceDropPayload::Floating { .. } => {
                let item = frozen_focus_item?;
                self.payload_contains_workspace_focus_item(payload, item)
                    .then_some(item.clone())?
            }
        };

        match target_space {
            Some(space) if self.graph.find_item_in_space(space, &item).is_some() => Some(item),
            Some(_) => None,
            None => Some(item),
        }
    }

    fn payload_contains_workspace_focus_item(
        &self,
        payload: &DockWorkspaceDropPayload<'_>,
        item: &DockItemId,
    ) -> bool {
        match payload {
            DockWorkspaceDropPayload::Item {
                source_tabs: _,
                item: payload_item,
            } => *payload_item == item,
            DockWorkspaceDropPayload::Tabs { source_tabs } => self
                .graph
                .collect_items_in_subtree(*source_tabs)
                .iter()
                .any(|candidate| candidate == item),
            DockWorkspaceDropPayload::Floating { floating } => self
                .graph
                .collect_items_in_subtree(*floating)
                .iter()
                .any(|candidate| candidate == item),
        }
    }
}
