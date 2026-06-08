#[cfg(test)]
use crate::debug::DockDebugInstrumentation;
use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockController, DockGraph, DockNodeId,
    DockPanelRegistry, DockPolicy, DockSpaceId, debug::DockDebugRegion,
    drop_target::DockDropIntent, splitter, workspace::DockWorkspace,
};
use open_gpui::{AppContext as _, Bounds, Context, Entity, Pixels, Point, point, px};

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

#[derive(Debug, Clone)]
pub(crate) struct FloatingDrag {
    pub(crate) space: DockSpaceId,
    pub(crate) floating: DockNodeId,
    pub(crate) start_position: Point<Pixels>,
    pub(crate) initial_bounds: Bounds<Pixels>,
}

/// Retained GPUI host that renders one logical dock workspace.
#[derive(Debug)]
pub struct DockHost {
    source: DockHostSource,
    #[cfg(test)]
    debug: DockDebugInstrumentation,
    splitter_drag: Option<SplitterDrag>,
    floating_drag: Option<FloatingDrag>,
    tab_drop_intent: Option<DockDropIntent>,
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
            splitter_drag: None,
            floating_drag: None,
            tab_drop_intent: None,
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
            splitter_drag: None,
            floating_drag: None,
            tab_drop_intent: None,
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
        self.splitter_drag = Some(SplitterDrag {
            split,
            handle_index,
            start_position,
            split_extent,
            initial_fractions,
        });
    }

    pub(crate) fn update_splitter_drag(
        &mut self,
        position: Pixels,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(drag) = self.splitter_drag.as_ref() else {
            return false;
        };
        let split_min_size =
            self.with_workspace(cx, |workspace| workspace.options().split_min_size);
        let delta = position - drag.start_position;
        let Some(fractions) = splitter::resize_adjacent_fractions(
            &drag.initial_fractions,
            drag.initial_fractions.len(),
            drag.handle_index,
            drag.split_extent,
            delta,
            split_min_size,
        ) else {
            return false;
        };

        self.apply_action_from_host(
            &DockAction::ResizeSplit {
                split: drag.split,
                fractions,
            },
            cx,
        )
        .map(|outcome| outcome.changed())
        .unwrap_or(false)
    }

    pub(crate) fn finish_splitter_drag(&mut self) {
        self.splitter_drag = None;
    }

    pub(crate) fn start_floating_drag(
        &mut self,
        space: DockSpaceId,
        floating: DockNodeId,
        start_position: Point<Pixels>,
        initial_bounds: Bounds<Pixels>,
    ) {
        self.floating_drag = Some(FloatingDrag {
            space,
            floating,
            start_position,
            initial_bounds,
        });
    }

    pub(crate) fn update_floating_drag(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(drag) = self.floating_drag.as_ref() else {
            return false;
        };
        let delta = position - drag.start_position;
        let next_bounds = Bounds::new(
            point(
                drag.initial_bounds.origin.x + delta.x,
                drag.initial_bounds.origin.y + delta.y,
            ),
            drag.initial_bounds.size,
        );

        self.apply_action_from_host(
            &DockAction::SetFloatingBounds {
                space: drag.space.clone(),
                floating: drag.floating,
                bounds: next_bounds,
            },
            cx,
        )
        .map(|outcome| outcome.changed())
        .unwrap_or(false)
    }

    pub(crate) fn finish_floating_drag(&mut self) {
        self.floating_drag = None;
    }

    pub(crate) fn set_tab_drop_intent(&mut self, intent: Option<DockDropIntent>) {
        self.tab_drop_intent = intent;
    }

    pub(crate) fn tab_drop_intent(&self) -> Option<DockDropIntent> {
        self.tab_drop_intent
    }

    pub(crate) fn clear_tab_drop_intent(&mut self) {
        self.tab_drop_intent = None;
    }

    #[cfg(test)]
    pub(crate) fn splitter_drag(&self) -> Option<&SplitterDrag> {
        self.splitter_drag.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn floating_drag(&self) -> Option<&FloatingDrag> {
        self.floating_drag.as_ref()
    }
}
