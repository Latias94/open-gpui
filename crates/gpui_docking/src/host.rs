#[cfg(test)]
use crate::debug::DockDebugInstrumentation;
#[cfg(test)]
use crate::interaction::{FloatingDrag, SplitterDrag};
use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockController, DockGraph, DockNodeId,
    DockPanelRegistry, DockPolicy, DockSpaceId, debug::DockDebugRegion,
    drop_target::DockDropIntent, interaction::DockInteractionRuntime, workspace::DockWorkspace,
};
use open_gpui::{AppContext as _, Bounds, Context, Entity, Pixels, Point, px};

/// Static host rendering options.
#[derive(Debug, Clone)]
pub struct DockHostOptions {
    /// Message rendered when the selected dock space has no root node.
    pub empty_message: String,
    /// Message prefix rendered when an active panel is missing from the registry.
    pub missing_panel_prefix: String,
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
            split_min_size: px(96.0),
            splitter_handle_size: px(6.0),
        }
    }
}

/// Retained GPUI host that renders one logical dock workspace.
///
/// `DockHost` is the GPUI render adapter for a dock space. Durable graph state belongs to
/// [`DockWorkspace`] or [`DockController`], while transient pointer sessions are kept behind the
/// crate's interaction runtime.
#[derive(Debug)]
pub struct DockHost {
    source: DockHostSource,
    #[cfg(test)]
    debug: DockDebugInstrumentation,
    interaction: DockInteractionRuntime,
}

#[derive(Debug)]
enum DockHostSource {
    Owned(Box<DockWorkspace>),
    Controller {
        controller: Entity<DockController>,
        space: DockSpaceId,
    },
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
            source: DockHostSource::Owned(Box::new(workspace)),
            #[cfg(test)]
            debug: DockDebugInstrumentation::default(),
            interaction: DockInteractionRuntime::default(),
        }
    }

    /// Creates a host that renders one dock space from a shared controller.
    pub fn from_controller(
        controller: Entity<DockController>,
        space: impl Into<DockSpaceId>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&controller, |_, _, cx| cx.notify()).detach();
        Self {
            source: DockHostSource::Controller {
                controller,
                space: space.into(),
            },
            #[cfg(test)]
            debug: DockDebugInstrumentation::default(),
            interaction: DockInteractionRuntime::default(),
        }
    }

    /// Returns the workspace rendered by this host.
    ///
    /// This accessor is available for compatibility hosts created with [`Self::from_workspace`].
    pub fn workspace(&self) -> &DockWorkspace {
        match &self.source {
            DockHostSource::Owned(workspace) => workspace,
            DockHostSource::Controller { .. } => {
                panic!("controller-backed hosts expose workspace state through DockController")
            }
        }
    }

    /// Returns the shared controller when this host is controller-backed.
    pub fn controller(&self) -> Option<&Entity<DockController>> {
        match &self.source {
            DockHostSource::Owned(_) => None,
            DockHostSource::Controller { controller, .. } => Some(controller),
        }
    }

    /// Applies a docking action through the host's owned workspace.
    ///
    /// Controller-backed hosts apply UI actions through their shared [`DockController`] during
    /// rendering callbacks. Call [`DockController::apply_action`] directly for external commits.
    pub fn apply_action(
        &mut self,
        action: &DockAction,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        match &mut self.source {
            DockHostSource::Owned(workspace) => workspace.apply_action(action),
            DockHostSource::Controller { .. } => Err(DockActionApplyError::ControllerBackedHost),
        }
    }

    /// Returns the logical dock space rendered by this host.
    pub fn space(&self) -> &DockSpaceId {
        match &self.source {
            DockHostSource::Owned(workspace) => workspace.space(),
            DockHostSource::Controller { space, .. } => space,
        }
    }

    /// Returns the host graph.
    pub fn graph(&self) -> &DockGraph {
        match &self.source {
            DockHostSource::Owned(workspace) => workspace.graph(),
            DockHostSource::Controller { .. } => {
                panic!("controller-backed hosts expose graph state through DockController")
            }
        }
    }

    /// Returns the panel registry.
    pub fn panels(&self) -> &DockPanelRegistry {
        match &self.source {
            DockHostSource::Owned(workspace) => workspace.panels(),
            DockHostSource::Controller { .. } => {
                panic!("controller-backed hosts expose panels through DockController")
            }
        }
    }

    /// Returns the static rendering options.
    pub fn options(&self) -> &DockHostOptions {
        match &self.source {
            DockHostSource::Owned(workspace) => workspace.options(),
            DockHostSource::Controller { .. } => {
                panic!("controller-backed hosts expose options through DockController")
            }
        }
    }

    /// Returns the docking interaction policy.
    pub fn policy(&self) -> &DockPolicy {
        match &self.source {
            DockHostSource::Owned(workspace) => workspace.policy(),
            DockHostSource::Controller { .. } => {
                panic!("controller-backed hosts expose policy through DockController")
            }
        }
    }

    pub(crate) fn with_workspace<R>(
        &self,
        cx: &Context<Self>,
        read: impl FnOnce(&DockWorkspace) -> R,
    ) -> R {
        match &self.source {
            DockHostSource::Owned(workspace) => read(workspace),
            DockHostSource::Controller { controller, .. } => {
                cx.read_entity(controller, |controller, _| read(controller.workspace()))
            }
        }
    }

    pub(crate) fn apply_action_from_host(
        &mut self,
        action: &DockAction,
        cx: &mut Context<Self>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        match &mut self.source {
            DockHostSource::Owned(workspace) => workspace.apply_action(action),
            DockHostSource::Controller { controller, .. } => {
                let controller = controller.clone();
                cx.update_entity(&controller, |controller, cx| {
                    let outcome = controller.apply_action(action);
                    if outcome
                        .as_ref()
                        .map(|outcome| outcome.changed())
                        .unwrap_or(false)
                    {
                        cx.notify();
                    }
                    outcome
                })
            }
        }
    }

    /// Returns a debug selector emitted for a test region during the most recent render.
    #[cfg(test)]
    pub(crate) fn debug_selector(&self, region: &DockDebugRegion) -> Option<&str> {
        self.debug.selector(region)
    }

    pub(crate) fn clear_debug_selectors(&mut self) {
        #[cfg(test)]
        self.debug.clear();
    }

    pub(crate) fn record_debug_selector(
        &mut self,
        region: DockDebugRegion,
        selector: String,
    ) -> String {
        #[cfg(test)]
        {
            self.debug.record(region, selector)
        }
        #[cfg(not(test))]
        {
            let _ = region;
            selector
        }
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
        self.interaction.start_splitter_drag(
            split,
            handle_index,
            start_position,
            split_extent,
            initial_fractions,
        );
    }

    pub(crate) fn update_splitter_drag(
        &mut self,
        position: Pixels,
        cx: &mut Context<Self>,
    ) -> bool {
        let split_min_size =
            self.with_workspace(cx, |workspace| workspace.options().split_min_size);
        let Some(action) = self
            .interaction
            .resize_split_action(position, split_min_size)
        else {
            return false;
        };

        self.apply_action_from_host(&action, cx)
            .map(|outcome| outcome.changed())
            .unwrap_or(false)
    }

    pub(crate) fn finish_splitter_drag(&mut self) {
        self.interaction.finish_splitter_drag();
    }

    pub(crate) fn start_floating_drag(
        &mut self,
        space: DockSpaceId,
        floating: DockNodeId,
        start_position: Point<Pixels>,
        initial_bounds: Bounds<Pixels>,
    ) {
        self.interaction
            .start_floating_drag(space, floating, start_position, initial_bounds);
    }

    pub(crate) fn update_floating_drag(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(action) = self.interaction.set_floating_bounds_action(position) else {
            return false;
        };

        self.apply_action_from_host(&action, cx)
            .map(|outcome| outcome.changed())
            .unwrap_or(false)
    }

    pub(crate) fn finish_floating_drag(&mut self) {
        self.interaction.finish_floating_drag();
    }

    pub(crate) fn update_tabs_drop_intent(
        &mut self,
        target_tabs: DockNodeId,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        cx: &Context<Self>,
    ) -> bool {
        let policy = self.with_workspace(cx, |workspace| *workspace.policy());
        self.interaction
            .update_tabs_drop_intent(target_tabs, bounds, position, &policy)
    }

    pub(crate) fn update_tab_reorder_drop_intent(
        &mut self,
        target_tabs: DockNodeId,
        target_index: usize,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        cx: &Context<Self>,
    ) -> bool {
        let policy = self.with_workspace(cx, |workspace| *workspace.policy());
        self.interaction.update_tab_reorder_drop_intent(
            target_tabs,
            target_index,
            bounds,
            position,
            &policy,
        )
    }

    pub(crate) fn take_tab_drop_intent(
        &mut self,
        target_tabs: DockNodeId,
    ) -> Option<DockDropIntent> {
        self.interaction.take_tab_drop_intent(target_tabs)
    }

    pub(crate) fn tab_drop_preview_bounds(
        &self,
        target_tabs: DockNodeId,
    ) -> Option<Bounds<Pixels>> {
        self.interaction.tab_drop_preview_bounds(target_tabs)
    }

    #[cfg(test)]
    pub(crate) fn splitter_drag(&self) -> Option<&SplitterDrag> {
        self.interaction.splitter_drag()
    }

    #[cfg(test)]
    pub(crate) fn floating_drag(&self) -> Option<&FloatingDrag> {
        self.interaction.floating_drag()
    }
}
