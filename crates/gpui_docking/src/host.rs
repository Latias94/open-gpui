#[cfg(test)]
use crate::debug::DockDebugInstrumentation;
use crate::{
    DockActionApplyError, DockActionOutcome, DockController, DockItemId, DockSpaceId,
    DockViewportRuntimeHandle, interaction::DockInteractionRuntime, workspace::DockWorkspace,
};
use open_gpui::{AppContext as _, Context, Entity, Pixels, Subscription, Window, px};

/// Static host rendering options.
#[derive(Debug, Clone)]
pub struct DockHostOptions {
    /// Message rendered when the selected dock space has no root node.
    pub empty_message: String,
    /// Message prefix rendered when a selected panel is missing from the registry.
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
    controller: Entity<DockController>,
    space: DockSpaceId,
    viewport_runtime: Option<DockViewportRuntimeHandle>,
    viewport_activation_subscription: Option<Subscription>,
    viewport_bounds_subscription: Option<Subscription>,
    pending_panel_focus: Option<DockItemId>,
    #[cfg(test)]
    debug: DockDebugInstrumentation,
    interaction: DockInteractionRuntime,
}

impl DockHost {
    /// Creates a host that renders one dock space from a shared controller.
    pub fn from_controller(
        controller: Entity<DockController>,
        space: impl Into<DockSpaceId>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&controller, |_, _, cx| cx.notify()).detach();
        Self {
            controller,
            space: space.into(),
            viewport_runtime: None,
            viewport_activation_subscription: None,
            viewport_bounds_subscription: None,
            pending_panel_focus: None,
            #[cfg(test)]
            debug: DockDebugInstrumentation::default(),
            interaction: DockInteractionRuntime::default(),
        }
    }

    pub(crate) fn with_viewport_runtime(
        controller: Entity<DockController>,
        space: impl Into<DockSpaceId>,
        viewport_runtime: DockViewportRuntimeHandle,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut host = Self::from_controller(controller, space, cx);
        host.viewport_runtime = Some(viewport_runtime);
        host
    }

    pub(crate) fn space(&self) -> &DockSpaceId {
        &self.space
    }

    pub(crate) fn with_workspace<R>(
        &self,
        cx: &Context<Self>,
        read: impl FnOnce(&DockWorkspace) -> R,
    ) -> R {
        cx.read_entity(&self.controller, |controller, _| {
            read(controller.workspace())
        })
    }

    pub(crate) fn mutate_controller_from_host(
        &mut self,
        cx: &mut Context<Self>,
        mutate: impl FnOnce(&mut DockController) -> Result<DockActionOutcome, DockActionApplyError>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        let controller = self.controller.clone();
        cx.update_entity(&controller, |controller, cx| {
            let outcome = mutate(controller);
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

    pub(crate) fn interaction(&self) -> &DockInteractionRuntime {
        &self.interaction
    }

    pub(crate) fn interaction_mut(&mut self) -> &mut DockInteractionRuntime {
        &mut self.interaction
    }

    pub(crate) fn viewport_runtime(&self) -> Option<&DockViewportRuntimeHandle> {
        self.viewport_runtime.as_ref()
    }

    pub(crate) fn ensure_viewport_activation_subscription(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.viewport_activation_subscription.is_some() {
            return;
        }

        let Some(runtime) = self.viewport_runtime().cloned() else {
            return;
        };

        let activation_runtime = runtime.clone();
        self.viewport_activation_subscription =
            Some(cx.observe_window_activation(window, move |_, window, _| {
                if window.is_window_active() {
                    activation_runtime.record_window_focus(window.window_handle().window_id());
                }
            }));
    }

    pub(crate) fn ensure_viewport_bounds_subscription(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.viewport_bounds_subscription.is_some() {
            return;
        }

        let Some(runtime) = self.viewport_runtime().cloned() else {
            return;
        };

        self.viewport_bounds_subscription =
            Some(cx.observe_window_bounds(window, move |_, window, cx| {
                runtime.mark_viewport_window_snapshot_stale(window.window_handle().window_id(), cx);
            }));
    }

    pub(crate) fn request_panel_focus(&mut self, item: DockItemId) -> bool {
        if self.pending_panel_focus.as_ref() == Some(&item) {
            return false;
        }
        self.pending_panel_focus = Some(item);
        true
    }

    pub(crate) fn take_pending_panel_focus(&mut self) -> Option<DockItemId> {
        self.pending_panel_focus.take()
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
