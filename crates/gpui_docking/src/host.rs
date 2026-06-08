use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockGraph, DockItemId, DockNodeId, DockOp,
    DockPanel, DockPanelRegistry, DockSpaceId,
    debug::{DockDebugInstrumentation, DockDebugRegion},
    splitter,
    workspace::DockWorkspace,
};
use open_gpui::{AnyView, Pixels, px};

/// Static host rendering options.
#[derive(Debug, Clone)]
pub struct DockHostOptions {
    /// Message rendered when the selected dock space has no root node.
    pub empty_message: String,
    /// Message prefix rendered when an active panel is missing from the registry.
    pub missing_panel_prefix: String,
    /// Message rendered for in-window floating nodes during Phase 2.
    pub deferred_floating_message: String,
    /// Minimum rendered size for a split pane during splitter resizing.
    pub split_min_size: Pixels,
    /// Hit target and visual thickness for rendered splitter handles.
    pub splitter_handle_size: Pixels,
}

impl Default for DockHostOptions {
    fn default() -> Self {
        Self {
            empty_message: "Empty dock space".to_string(),
            missing_panel_prefix: "Missing panel".to_string(),
            deferred_floating_message: "Floating panels render in a later phase".to_string(),
            split_min_size: px(96.0),
            splitter_handle_size: px(6.0),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SplitterDrag {
    pub(crate) split: DockNodeId,
    pub(crate) handle_index: usize,
    pub(crate) start_position: Pixels,
    pub(crate) split_extent: Pixels,
    pub(crate) initial_fractions: Vec<f32>,
}

/// Retained GPUI host that renders one logical dock workspace.
#[derive(Debug)]
pub struct DockHost {
    workspace: DockWorkspace,
    debug: DockDebugInstrumentation,
    splitter_drag: Option<SplitterDrag>,
}

impl DockHost {
    /// Creates a host for one dock space and graph.
    ///
    /// Prefer configuring a [`DockWorkspace`] and mounting it with [`Self::from_workspace`]. This
    /// constructor remains as a compatibility path and delegates to workspace-backed state.
    pub fn new(space: impl Into<DockSpaceId>, graph: DockGraph) -> Self {
        Self::with_options(space, graph, DockHostOptions::default())
    }

    /// Creates a host with explicit static rendering options.
    ///
    /// Prefer configuring a [`DockWorkspace`] and mounting it with [`Self::from_workspace`]. This
    /// constructor remains as a compatibility path and delegates to workspace-backed state.
    pub fn with_options(
        space: impl Into<DockSpaceId>,
        graph: DockGraph,
        options: DockHostOptions,
    ) -> Self {
        Self::from_workspace(DockWorkspace::with_options(space, graph, options))
    }

    /// Creates a host that renders a configured workspace.
    pub fn from_workspace(workspace: DockWorkspace) -> Self {
        Self {
            workspace,
            debug: DockDebugInstrumentation::default(),
            splitter_drag: None,
        }
    }

    /// Returns the workspace rendered by this host.
    pub fn workspace(&self) -> &DockWorkspace {
        &self.workspace
    }

    /// Returns the workspace rendered by this host for owner-level mutation.
    pub fn workspace_mut(&mut self) -> &mut DockWorkspace {
        &mut self.workspace
    }

    /// Applies a docking action through the host's workspace.
    pub fn apply_action(
        &mut self,
        action: &DockAction,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.workspace.apply_action(action)
    }

    /// Returns the logical dock space rendered by this host.
    pub fn space(&self) -> &DockSpaceId {
        self.workspace.space()
    }

    /// Returns the host graph.
    pub fn graph(&self) -> &DockGraph {
        self.workspace.graph()
    }

    /// Returns the host graph for compatibility mutation by application code.
    ///
    /// Prefer applying operations or docking actions through [`DockWorkspace`].
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

    /// Returns the panel registry for compatibility mutation by application code.
    ///
    /// Prefer registering panels through [`DockWorkspace`] before mounting the host.
    pub fn panels_mut(&mut self) -> &mut DockPanelRegistry {
        self.workspace.panels_mut()
    }

    /// Registers a panel for a dock item, returning any previous registration.
    ///
    /// Prefer registering panels through [`DockWorkspace`] before mounting the host.
    pub fn register_panel(
        &mut self,
        item: impl Into<DockItemId>,
        panel: DockPanel,
    ) -> Option<DockPanel> {
        self.workspace.register_panel(item, panel)
    }

    /// Registers a GPUI view as panel content for a dock item.
    ///
    /// Prefer registering panels through [`DockWorkspace`] before mounting the host.
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
    #[cfg(test)]
    pub(crate) fn debug_selector(&self, region: &DockDebugRegion) -> Option<&str> {
        self.debug.selector(region)
    }

    pub(crate) fn clear_debug_selectors(&mut self) {
        self.debug.clear();
    }

    pub(crate) fn record_debug_selector(
        &mut self,
        region: DockDebugRegion,
        selector: String,
    ) -> String {
        self.debug.record(region, selector)
    }

    pub(crate) fn selector_prefix(&self) -> String {
        format!("dock:{}", self.space())
    }

    pub(crate) fn start_splitter_drag(
        &mut self,
        split: DockNodeId,
        handle_index: usize,
        start_position: Pixels,
        split_extent: Pixels,
        initial_fractions: Vec<f32>,
    ) {
        self.splitter_drag = Some(SplitterDrag {
            split,
            handle_index,
            start_position,
            split_extent,
            initial_fractions,
        });
    }

    pub(crate) fn update_splitter_drag(&mut self, position: Pixels) -> bool {
        let Some(drag) = self.splitter_drag.as_ref() else {
            return false;
        };
        let delta = position - drag.start_position;
        let Some(fractions) = splitter::resize_adjacent_fractions(
            &drag.initial_fractions,
            drag.initial_fractions.len(),
            drag.handle_index,
            drag.split_extent,
            delta,
            self.options().split_min_size,
        ) else {
            return false;
        };

        self.workspace
            .apply_op_checked(&DockOp::SetSplitFractions {
                split: drag.split,
                fractions,
            })
            .unwrap_or(false)
    }

    pub(crate) fn finish_splitter_drag(&mut self) {
        self.splitter_drag = None;
    }

    #[cfg(test)]
    pub(crate) fn splitter_drag(&self) -> Option<&SplitterDrag> {
        self.splitter_drag.as_ref()
    }
}
