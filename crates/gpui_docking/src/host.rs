#[cfg(test)]
use crate::debug::DockDebugInstrumentation;
use crate::{
    DockActionApplyError, DockActionOutcome, DockController, DockItemId, DockSpaceId,
    DockViewportFocusRequest, DockViewportRuntimeHandle, interaction::DockInteractionRuntime,
    workspace::DockWorkspace,
};
use open_gpui::{AppContext as _, Context, Entity, FocusHandle, Pixels, Subscription, Window, px};
use std::collections::HashMap;

#[derive(Debug)]
struct DockPanelFocusTracker {
    focus_handle: FocusHandle,
    _subscription: Subscription,
}

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
    viewport_release_subscription: Option<Subscription>,
    pending_focus_request: Option<DockViewportFocusRequest>,
    panel_focus_trackers: HashMap<DockItemId, DockPanelFocusTracker>,
    last_focused_panel: Option<DockItemId>,
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
            viewport_release_subscription: None,
            pending_focus_request: None,
            panel_focus_trackers: HashMap::new(),
            last_focused_panel: None,
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
        let space = space.into();
        let mut host = Self::from_controller(controller, space, cx);
        host.viewport_runtime = Some(viewport_runtime);
        host
    }

    pub(crate) fn space(&self) -> &DockSpaceId {
        &self.space
    }

    pub(crate) fn controller(&self) -> &Entity<DockController> {
        &self.controller
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
        self.viewport_activation_subscription = Some(cx.observe_window_activation(
            window,
            move |host, window, cx| {
                if window.is_window_active() {
                    activation_runtime.record_window_focus(window.window_handle().window_id());
                    let _ = host.request_viewport_focus_restore_if_idle();
                    cx.notify();
                }
            },
        ));
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

    pub(crate) fn ensure_viewport_release_subscription(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.viewport_release_subscription.is_some() {
            return;
        }

        let Some(runtime) = self.viewport_runtime().cloned() else {
            return;
        };

        self.viewport_release_subscription =
            Some(cx.on_release_in(window, move |host, window, _| {
                runtime.unregister_host_for_space(host.space(), window.window_handle().window_id());
            }));
    }

    pub(crate) fn request_viewport_focus(&mut self, request: DockViewportFocusRequest) -> bool {
        if self.pending_focus_request.as_ref() == Some(&request) {
            return false;
        }
        self.pending_focus_request = Some(request);
        true
    }

    pub(crate) fn request_panel_focus(&mut self, item: DockItemId) -> bool {
        self.request_viewport_focus(DockViewportFocusRequest::panel(item))
    }

    pub(crate) fn request_viewport_focus_restore(&mut self) -> bool {
        self.request_viewport_focus(DockViewportFocusRequest::restore_last_focused())
    }

    pub(crate) fn request_viewport_focus_restore_if_idle(&mut self) -> bool {
        if self.pending_focus_request.is_some() {
            return false;
        }
        self.request_viewport_focus_restore()
    }

    pub(crate) fn remember_panel_focus(&mut self, item: DockItemId, cx: &mut Context<Self>) {
        let space = self.space().clone();
        if let Some(runtime) = self.viewport_runtime().cloned() {
            runtime.record_panel_focus(space, item.clone());
        }
        let controller = self.controller.clone();
        let space = self.space().clone();
        cx.update_entity(&controller, |controller, _| {
            if let Some((tabs, _)) = controller.graph().find_item_in_space(&space, &item) {
                controller
                    .workspace_mut()
                    .refresh_tab_selected_stamp(tabs, &item);
            }
        });
        if self.last_focused_panel.as_ref() != Some(&item) {
            self.last_focused_panel = Some(item);
        }
    }

    pub(crate) fn pending_focus_request(&self) -> Option<&DockViewportFocusRequest> {
        self.pending_focus_request.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn has_pending_panel_focus(&self) -> bool {
        matches!(
            self.pending_focus_request,
            Some(DockViewportFocusRequest::Panel(_))
        )
    }

    #[cfg(test)]
    pub(crate) fn pending_panel_focus(&self) -> Option<&DockItemId> {
        self.pending_focus_request
            .as_ref()
            .and_then(DockViewportFocusRequest::panel_item)
    }

    pub(crate) fn clear_pending_focus_request(&mut self) {
        self.pending_focus_request = None;
    }

    pub(crate) fn ensure_panel_focus_tracker(
        &mut self,
        item: &DockItemId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> FocusHandle {
        if let Some(tracker) = self.panel_focus_trackers.get(item) {
            return tracker.focus_handle.clone();
        }

        let focus_handle = cx.focus_handle();
        let focus_item = item.clone();
        let subscription = cx.on_focus_in(&focus_handle, window, move |host, _window, cx| {
            host.remember_panel_focus(focus_item.clone(), cx);
            host.clear_pending_focus_request();
            cx.notify();
        });
        self.panel_focus_trackers.insert(
            item.clone(),
            DockPanelFocusTracker {
                focus_handle: focus_handle.clone(),
                _subscription: subscription,
            },
        );
        focus_handle
    }

    pub(crate) fn restore_panel_focus_target(
        &self,
        visible_items: &[DockItemId],
    ) -> Option<DockItemId> {
        self.last_focused_panel
            .as_ref()
            .filter(|item| visible_items.contains(item))
            .cloned()
    }

    pub(crate) fn visible_focused_panel_item(
        &self,
        visible_items: &[DockItemId],
        window: &Window,
        cx: &open_gpui::App,
    ) -> Option<DockItemId> {
        visible_items.iter().find_map(|item| {
            self.panel_focus_trackers.get(item).and_then(|tracker| {
                tracker
                    .focus_handle
                    .contains_focused(window, cx)
                    .then(|| item.clone())
            })
        })
    }

    pub(crate) fn sync_panel_focus_trackers(
        &mut self,
        visible_items: &[DockItemId],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.panel_focus_trackers
            .retain(|item, _| visible_items.contains(item));

        for item in visible_items {
            self.ensure_panel_focus_tracker(item, window, cx);
        }
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
