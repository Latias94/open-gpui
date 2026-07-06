#[cfg(test)]
use crate::debug::DockDebugInstrumentation;
use crate::{
    DockActionApplyError, DockActionOutcome, DockController, DockItemId, DockSpaceId,
    DockViewportFocusCommand, DockViewportFocusRequest, DockViewportPlatformFocusRestoreGate,
    DockViewportRuntimeHandle, geometry::DockDropGuideStyle,
    host_render_session::DockHostRenderSession, interaction::DockInteractionRuntime,
    presentation_scene::DockPresentationScene, transition_executor::DockTransitionExecutor,
    visual_affordance_scene::DockVisualAffordanceScene, workspace::DockWorkspace,
    zoom_state::DockZoomState,
};
use open_gpui::{
    AppContext as _, Context, Entity, FocusHandle, Pixels, Subscription, Window, WindowId, px,
};
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
    /// Style inputs used to size and hit-test dock drop guides.
    pub drop_guide_style: DockDropGuideStyle,
}

impl Default for DockHostOptions {
    fn default() -> Self {
        Self {
            empty_message: "Empty dock space".to_string(),
            missing_panel_prefix: "Missing panel".to_string(),
            split_min_size: px(96.0),
            splitter_handle_size: px(6.0),
            drop_guide_style: DockDropGuideStyle::default(),
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
    focus_handle: FocusHandle,
    viewport_runtime: DockViewportRuntimeHandle,
    viewport_activation_subscription: Option<Subscription>,
    viewport_bounds_subscription: Option<Subscription>,
    viewport_release_subscription: Option<Subscription>,
    panel_focus_trackers: HashMap<DockItemId, DockPanelFocusTracker>,
    #[cfg(test)]
    debug: DockDebugInstrumentation,
    #[cfg(test)]
    pub(crate) debug_recording_suppression_depth: usize,
    interaction: DockInteractionRuntime,
    zoom: DockZoomState,
    transitions: DockTransitionExecutor,
    visual_affordance_transitions: DockTransitionExecutor,
    last_visual_affordance_scene: Option<DockVisualAffordanceScene>,
    last_presentation_scene: Option<DockPresentationScene>,
}

impl DockHost {
    /// Creates a host that renders one dock space from a shared controller and viewport runtime.
    pub fn from_controller(
        controller: Entity<DockController>,
        space: impl Into<DockSpaceId>,
        viewport_runtime: DockViewportRuntimeHandle,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&controller, |_, _, cx| cx.notify()).detach();
        Self {
            controller,
            space: space.into(),
            focus_handle: cx.focus_handle(),
            viewport_runtime,
            viewport_activation_subscription: None,
            viewport_bounds_subscription: None,
            viewport_release_subscription: None,
            panel_focus_trackers: HashMap::new(),
            #[cfg(test)]
            debug: DockDebugInstrumentation::default(),
            #[cfg(test)]
            debug_recording_suppression_depth: 0,
            interaction: DockInteractionRuntime::default(),
            zoom: DockZoomState::default(),
            transitions: DockTransitionExecutor::default(),
            visual_affordance_transitions: DockTransitionExecutor::default(),
            last_visual_affordance_scene: None,
            last_presentation_scene: None,
        }
    }

    pub(crate) fn space(&self) -> &DockSpaceId {
        &self.space
    }

    pub(crate) fn host_focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    #[cfg(test)]
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
        self.mutate_controller_from_host_with(cx, mutate, |outcome| outcome.changed())
    }

    pub(crate) fn mutate_controller_from_host_with<R>(
        &mut self,
        cx: &mut Context<Self>,
        mutate: impl FnOnce(&mut DockController) -> Result<R, DockActionApplyError>,
        changed: impl FnOnce(&R) -> bool,
    ) -> Result<R, DockActionApplyError> {
        let controller = self.controller.clone();
        cx.update_entity(&controller, |controller, cx| {
            let outcome = mutate(controller);
            if outcome.as_ref().map(changed).unwrap_or(false) {
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

    pub(crate) fn zoom_state(&self) -> &DockZoomState {
        &self.zoom
    }

    pub(crate) fn zoom_state_mut(&mut self) -> &mut DockZoomState {
        &mut self.zoom
    }

    pub(crate) fn transition_executor_mut(&mut self) -> &mut DockTransitionExecutor {
        &mut self.transitions
    }

    pub(crate) fn visual_affordance_transition_executor_mut(
        &mut self,
    ) -> &mut DockTransitionExecutor {
        &mut self.visual_affordance_transitions
    }

    pub(crate) fn visual_affordance_transition_executor_for_debug(
        &self,
    ) -> &DockTransitionExecutor {
        &self.visual_affordance_transitions
    }

    pub(crate) fn last_visual_affordance_scene(&self) -> Option<&DockVisualAffordanceScene> {
        self.last_visual_affordance_scene.as_ref()
    }

    pub(crate) fn set_last_visual_affordance_scene(&mut self, scene: DockVisualAffordanceScene) {
        self.last_visual_affordance_scene = Some(scene);
    }

    pub(crate) fn clear_last_visual_affordance_scene(&mut self) -> bool {
        self.last_visual_affordance_scene.take().is_some()
    }

    pub(crate) fn last_presentation_scene(&self) -> Option<&DockPresentationScene> {
        self.last_presentation_scene.as_ref()
    }

    pub(crate) fn set_last_presentation_scene(&mut self, scene: DockPresentationScene) {
        self.last_presentation_scene = Some(scene);
    }

    pub(crate) fn viewport_runtime(&self) -> &DockViewportRuntimeHandle {
        &self.viewport_runtime
    }

    pub(crate) fn ensure_viewport_activation_subscription(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.viewport_activation_subscription.is_some() {
            return;
        }

        self.viewport_activation_subscription = Some(cx.observe_window_activation(
            window,
            move |host, window, cx| {
                if window.is_window_active() {
                    host.apply_confirmed_backend_window_focus(
                        window.window_handle().window_id(),
                        DockViewportPlatformFocusRestoreGate::from_app(cx),
                        cx,
                    );
                }
            },
        ));
    }

    fn apply_confirmed_backend_window_focus(
        &mut self,
        window_id: WindowId,
        platform_focus_restore_gate: DockViewportPlatformFocusRestoreGate,
        cx: &mut Context<Self>,
    ) -> bool {
        let outcome = self
            .viewport_runtime
            .confirmed_backend_window_focus_outcome(
                self.space(),
                window_id,
                platform_focus_restore_gate,
                cx,
            );
        let changed = outcome.changed();
        let focus_command_queued = outcome
            .into_focus_command()
            .is_some_and(|command| self.request_viewport_focus_command(command));
        let applied = changed || focus_command_queued;
        if applied {
            cx.notify();
        }
        applied
    }

    pub(crate) fn ensure_viewport_bounds_subscription(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.viewport_bounds_subscription.is_some() {
            return;
        }

        let runtime = self.viewport_runtime().clone();

        self.viewport_bounds_subscription =
            Some(cx.observe_window_bounds(window, move |_, window, cx| {
                runtime.apply_platform_window_facts(
                    window.window_handle().window_id(),
                    crate::DockViewportWindowFacts::from_window(window, cx),
                    cx,
                );
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

        let runtime = self.viewport_runtime().clone();

        self.viewport_release_subscription =
            Some(cx.on_release_in(window, move |host, window, cx| {
                runtime.unregister_host_for_space_with_app(
                    host.space(),
                    window.window_handle().window_id(),
                    cx,
                );
            }));
    }

    pub(crate) fn request_viewport_focus_command(
        &mut self,
        command: DockViewportFocusCommand,
    ) -> bool {
        self.interaction.request_viewport_focus_command(command)
    }

    pub(crate) fn apply_pending_focus_from_render(
        &mut self,
        session: &DockHostRenderSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(command) = self.interaction.pending_focus_command().cloned() else {
            return;
        };
        match command.request().clone() {
            DockViewportFocusRequest::Panel(item) => {
                let should_preselect =
                    command.source() == crate::DockViewportFocusCommandSource::ViewportActivation;
                match session
                    .visible_panel_registration(&item)
                    .map(|panel| panel.request_focus(window, cx))
                {
                    Some(true) => {
                        self.clear_pending_focus_command();
                        self.remember_panel_focus(item, cx);
                    }
                    Some(false) => {
                        self.record_no_panel_focus_for_gone_platform_panel(&command, &item, cx);
                        self.clear_pending_focus_command();
                    }
                    None if should_preselect => {
                        let focus_item = item;
                        let changed = self
                            .mutate_controller_from_host(cx, |controller| {
                                controller.select_item_in_space(focus_item.clone())
                            })
                            .is_ok_and(|outcome| outcome.changed());
                        if changed {
                            cx.notify();
                        }

                        let controller = self.controller.clone();
                        let registration = cx.read_entity(&controller, |controller, _| {
                            controller
                                .workspace()
                                .panels()
                                .render_registration(&focus_item)
                        });
                        if registration.is_some_and(|panel| panel.request_focus(window, cx)) {
                            self.clear_pending_focus_command();
                            self.remember_panel_focus(focus_item, cx);
                        } else {
                            self.record_no_panel_focus_for_gone_platform_panel(
                                &command,
                                &focus_item,
                                cx,
                            );
                            self.clear_pending_focus_command();
                        }
                    }
                    None => {
                        self.record_no_panel_focus_for_gone_platform_panel(&command, &item, cx);
                        self.clear_pending_focus_command();
                    }
                }
            }
            DockViewportFocusRequest::NoPanelFocus => {
                window.blur();
                self.viewport_runtime().record_no_panel_focus(self.space());
                self.clear_pending_focus_command();
            }
        }
    }

    pub(crate) fn remember_panel_focus(&mut self, item: DockItemId, cx: &mut Context<Self>) {
        let space = self.space().clone();
        self.viewport_runtime()
            .record_panel_focus(space, item.clone());
        let _ = self.mutate_controller_from_host(cx, |controller| {
            controller.select_item_in_space(item.clone())
        });
    }

    fn record_no_panel_focus_for_gone_platform_panel(
        &self,
        command: &DockViewportFocusCommand,
        item: &DockItemId,
        cx: &mut Context<Self>,
    ) {
        if command.source() != crate::DockViewportFocusCommandSource::PlatformActivation {
            return;
        }
        if !self
            .viewport_runtime()
            .recorded_panel_focus_matches(self.space(), item)
        {
            return;
        }
        if self.panel_is_reachable_in_space(item, cx) {
            return;
        }
        self.viewport_runtime().record_no_panel_focus(self.space());
    }

    fn panel_is_reachable_in_space(&self, item: &DockItemId, cx: &mut Context<Self>) -> bool {
        let space = self.space().clone();
        let controller = self.controller.clone();
        cx.read_entity(&controller, |controller, _| {
            controller
                .graph()
                .find_item_in_space(&space, item)
                .is_some()
        })
    }

    #[cfg(test)]
    pub(crate) fn pending_focus_command(&self) -> Option<&DockViewportFocusCommand> {
        self.interaction.pending_focus_command()
    }

    pub(crate) fn clear_pending_focus_command(&mut self) {
        let _ = self.interaction.take_pending_focus_command();
    }

    #[cfg(test)]
    pub(crate) fn recorded_had_panel_focus(&self) -> Option<bool> {
        self.viewport_runtime()
            .recorded_had_panel_focus_for_test(self.space())
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
            host.clear_pending_focus_command();
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
