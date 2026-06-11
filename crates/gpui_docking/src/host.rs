#[cfg(test)]
use crate::debug::DockDebugInstrumentation;
use crate::{
    DockActionApplyError, DockActionOutcome, DockController, DockItemId, DockNodeId, DockSpaceId,
    DockViewportRuntimeHandle, interaction::DockInteractionRuntime, workspace::DockWorkspace,
    workspace_transaction::DockWorkspacePayloadDropRequest,
};
use open_gpui::{AppContext as _, Bounds, Context, Entity, Pixels, Subscription, Window, px};

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
    controller: Entity<DockController>,
    space: DockSpaceId,
    viewport_runtime: Option<DockViewportRuntimeHandle>,
    viewport_activation_subscription: Option<Subscription>,
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

    pub(crate) fn commit_resolved_payload_drop_from_host(
        &mut self,
        request: DockWorkspacePayloadDropRequest<'_>,
        cx: &mut Context<Self>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.mutate_controller_from_host(cx, |controller| {
            controller.commit_resolved_payload_drop(request)
        })
    }

    pub(crate) fn commit_select_tab_from_host(
        &mut self,
        tabs: DockNodeId,
        item: &DockItemId,
        cx: &mut Context<Self>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.mutate_controller_from_host(cx, |controller| controller.commit_select_tab(tabs, item))
    }

    pub(crate) fn commit_close_item_from_host(
        &mut self,
        item: &DockItemId,
        cx: &mut Context<Self>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        let space = self.space().clone();
        self.mutate_controller_from_host(cx, |controller| {
            controller.commit_close_item(&space, item)
        })
    }

    pub(crate) fn commit_resize_split_from_host(
        &mut self,
        split: DockNodeId,
        fractions: &[f32],
        cx: &mut Context<Self>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.mutate_controller_from_host(cx, |controller| {
            controller.commit_resize_split(split, fractions)
        })
    }

    pub(crate) fn commit_set_floating_bounds_from_host(
        &mut self,
        space: &DockSpaceId,
        floating: DockNodeId,
        bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.mutate_controller_from_host(cx, |controller| {
            controller.commit_set_floating_bounds(space, floating, bounds)
        })
    }

    pub(crate) fn commit_raise_floating_from_host(
        &mut self,
        space: &DockSpaceId,
        floating: DockNodeId,
        cx: &mut Context<Self>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.mutate_controller_from_host(cx, |controller| {
            controller.commit_raise_floating(space, floating)
        })
    }

    fn mutate_controller_from_host(
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
