#[cfg(test)]
use crate::debug::DockDebugInstrumentation;
use crate::{
    DockController, DockGraph, DockSpaceId, host_source::DockHostSource,
    interaction::DockInteractionRuntime, workspace::DockWorkspace,
};
use open_gpui::{Context, Entity, Pixels, px};

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

impl DockHost {
    /// Creates a host for one dock space and graph.
    ///
    /// Prefer configuring a [`DockWorkspace`] and mounting it with [`Self::from_workspace`]. This
    /// constructor remains as a compatibility path and delegates to workspace-backed state.
    #[deprecated(
        since = "0.1.0",
        note = "configure a DockWorkspace and mount it with DockHost::from_workspace"
    )]
    pub fn new(space: impl Into<DockSpaceId>, graph: DockGraph) -> Self {
        Self::from_workspace(DockWorkspace::new(space, graph))
    }

    /// Creates a host with explicit static rendering options.
    ///
    /// Prefer configuring a [`DockWorkspace`] and mounting it with [`Self::from_workspace`]. This
    /// constructor remains as a compatibility path and delegates to workspace-backed state.
    #[deprecated(
        since = "0.1.0",
        note = "configure a DockWorkspace with options and mount it with DockHost::from_workspace"
    )]
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

    pub(crate) fn source(&self) -> &DockHostSource {
        &self.source
    }

    pub(crate) fn source_mut(&mut self) -> &mut DockHostSource {
        &mut self.source
    }

    pub(crate) fn interaction(&self) -> &DockInteractionRuntime {
        &self.interaction
    }

    pub(crate) fn interaction_mut(&mut self) -> &mut DockInteractionRuntime {
        &mut self.interaction
    }

    #[cfg(test)]
    pub(crate) fn debug_instrumentation(&self) -> &DockDebugInstrumentation {
        &self.debug
    }

    #[cfg(test)]
    pub(crate) fn debug_instrumentation_mut(&mut self) -> &mut DockDebugInstrumentation {
        &mut self.debug
    }
}
