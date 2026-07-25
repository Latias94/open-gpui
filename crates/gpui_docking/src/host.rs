#[cfg(test)]
use crate::debug::DockDebugInstrumentation;
use crate::{
    DockActionApplyError, DockActionOutcome, DockController, DockItemId, DockSpaceId,
    DockViewportFocusCommand, DockViewportFocusRequest, DockViewportPlatformFocusRestoreGate,
    DockViewportRuntimeHandle, DockVisualStyle, DockVisualStyleResolver,
    geometry::DockDropGuideMetrics,
    host_render_session::DockHostRenderSession,
    interaction::{DockInteractionRuntime, DockPendingFocusCommand},
    presentation_scene::DockPresentationScene,
    surface::{
        DockSurfaceActivationHostRegistration, DockSurfaceActivationHostRegistrationStatus,
        DockSurfaceActivationOutcome, DockSurfaceActivationSettlements, DockSurfaceChangeCategory,
        DockSurfaceOwner, with_root_transaction,
    },
    transition_executor::DockTransitionExecutor,
    visual_affordance_scene::DockVisualAffordanceScene,
    workspace::DockWorkspace,
    zoom_state::DockZoomState,
};
use open_gpui::{
    App, AppContext as _, Context, Entity, FocusClaimOutcome, FocusHandle, Pixels,
    PointerCaptureHandle, PrepaintPublicationId, Subscription, WeakEntity, Window, WindowId, px,
};
use open_gpui_motion::MotionPreference;
use std::{collections::HashMap, rc::Rc};

#[derive(Debug)]
struct DockPanelFocusTracker {
    focus_handle: FocusHandle,
    _subscription: Subscription,
}

#[derive(Debug)]
struct DockPendingFocusCompletion {
    ticket: DockPendingFocusCommand,
    target: Option<FocusHandle>,
    _subscription: Subscription,
}

enum DockNoPanelFocusSettlement {
    Focus(FocusHandle),
    Blur,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockHostWindowBinding {
    window_id: WindowId,
    generation: u64,
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
    /// Structural metrics used to size and hit-test dock drop guides.
    pub drop_guide_metrics: DockDropGuideMetrics,
    /// Host-owned motion preference applied before constructing docking transition specs.
    pub motion_preference: MotionPreference,
}

impl Default for DockHostOptions {
    fn default() -> Self {
        Self {
            empty_message: "Empty dock space".to_string(),
            missing_panel_prefix: "Missing panel".to_string(),
            split_min_size: px(96.0),
            splitter_handle_size: px(6.0),
            drop_guide_metrics: DockDropGuideMetrics::default(),
            motion_preference: MotionPreference::Animated,
        }
    }
}

/// Retained GPUI host that renders one logical dock workspace.
///
/// `DockHost` is the GPUI render adapter for a dock space. Durable graph state belongs to
/// [`model::DockWorkspace`](crate::model::DockWorkspace) or
/// [`model::DockController`](crate::model::DockController), while transient pointer sessions are
/// kept behind the crate's interaction runtime.
#[derive(Debug)]
pub struct DockHost {
    controller: Entity<DockController>,
    surface_owner: Option<WeakEntity<DockSurfaceOwner>>,
    surface_activation_registration: Option<DockSurfaceActivationHostRegistration>,
    space: DockSpaceId,
    focus_handle: FocusHandle,
    viewport_runtime: DockViewportRuntimeHandle,
    visual_style_resolver: Option<DockVisualStyleResolver>,
    fallback_visual_style: Rc<DockVisualStyle>,
    bound_window_id: Option<WindowId>,
    window_binding_generation: u64,
    viewport_scene_publication: PrepaintPublicationId,
    raw_drag_pointer_capture: Option<PointerCaptureHandle>,
    viewport_activation_subscription: Option<Subscription>,
    viewport_bounds_subscription: Option<Subscription>,
    viewport_release_subscription: Option<Subscription>,
    panel_focus_trackers: HashMap<DockItemId, DockPanelFocusTracker>,
    pending_focus_completion: Option<DockPendingFocusCompletion>,
    #[cfg(test)]
    debug: DockDebugInstrumentation,
    #[cfg(test)]
    pub(crate) debug_recording_suppression_depth: usize,
    #[cfg(test)]
    last_resolved_visual_style: Option<Rc<DockVisualStyle>>,
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
        let visual_style_resolver = viewport_runtime.visual_style_resolver();
        Self::from_controller_with_optional_visual_style_resolver(
            controller,
            space,
            viewport_runtime,
            visual_style_resolver,
            None,
            cx,
        )
    }

    /// Creates a low-level host with an explicit immutable visual-style resolver.
    pub fn from_controller_with_visual_style_resolver(
        controller: Entity<DockController>,
        space: impl Into<DockSpaceId>,
        viewport_runtime: DockViewportRuntimeHandle,
        visual_style_resolver: DockVisualStyleResolver,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::from_controller_with_optional_visual_style_resolver(
            controller,
            space,
            viewport_runtime,
            Some(visual_style_resolver),
            None,
            cx,
        )
    }

    pub(crate) fn from_surface_owner(
        controller: Entity<DockController>,
        space: impl Into<DockSpaceId>,
        viewport_runtime: DockViewportRuntimeHandle,
        surface_owner: &Entity<DockSurfaceOwner>,
        cx: &mut Context<Self>,
    ) -> Self {
        let visual_style_resolver = viewport_runtime.visual_style_resolver();
        Self::from_controller_with_optional_visual_style_resolver(
            controller,
            space,
            viewport_runtime,
            visual_style_resolver,
            Some(surface_owner.downgrade()),
            cx,
        )
    }

    fn from_controller_with_optional_visual_style_resolver(
        controller: Entity<DockController>,
        space: impl Into<DockSpaceId>,
        viewport_runtime: DockViewportRuntimeHandle,
        visual_style_resolver: Option<DockVisualStyleResolver>,
        surface_owner: Option<WeakEntity<DockSurfaceOwner>>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&controller, |_, _, cx| cx.notify()).detach();
        if let Some(surface_owner) = surface_owner.as_ref().and_then(WeakEntity::upgrade) {
            cx.observe(&surface_owner, |_, _, cx| cx.notify()).detach();
        }
        Self {
            controller,
            surface_owner,
            surface_activation_registration: None,
            space: space.into(),
            focus_handle: cx.focus_handle(),
            viewport_runtime,
            visual_style_resolver,
            fallback_visual_style: Rc::new(DockVisualStyle::built_in()),
            bound_window_id: None,
            window_binding_generation: 0,
            viewport_scene_publication: PrepaintPublicationId::new(),
            raw_drag_pointer_capture: None,
            viewport_activation_subscription: None,
            viewport_bounds_subscription: None,
            viewport_release_subscription: None,
            panel_focus_trackers: HashMap::new(),
            pending_focus_completion: None,
            #[cfg(test)]
            debug: DockDebugInstrumentation::default(),
            #[cfg(test)]
            debug_recording_suppression_depth: 0,
            #[cfg(test)]
            last_resolved_visual_style: None,
            interaction: DockInteractionRuntime::default(),
            zoom: DockZoomState::default(),
            transitions: DockTransitionExecutor::default(),
            visual_affordance_transitions: DockTransitionExecutor::default(),
            last_visual_affordance_scene: None,
            last_presentation_scene: None,
        }
    }

    pub(crate) fn resolve_visual_style(
        &self,
        window: &mut Window,
        cx: &mut open_gpui::App,
    ) -> Rc<DockVisualStyle> {
        self.visual_style_resolver
            .as_ref()
            .map(|resolver| resolver.resolve(window, cx))
            .unwrap_or_else(|| self.fallback_visual_style.clone())
    }

    #[cfg(test)]
    pub(crate) fn last_resolved_visual_style(&self) -> Option<&DockVisualStyle> {
        self.last_resolved_visual_style.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn record_resolved_visual_style_for_test(&mut self, style: Rc<DockVisualStyle>) {
        self.last_resolved_visual_style = Some(style);
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

    pub(crate) fn controller_entity(&self) -> Entity<DockController> {
        self.controller.clone()
    }

    pub(crate) fn surface_owner_entity(&self) -> Option<Entity<DockSurfaceOwner>> {
        self.surface_owner.as_ref().and_then(WeakEntity::upgrade)
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
        categories: &[DockSurfaceChangeCategory],
        mutate: impl FnOnce(&mut DockController) -> Result<DockActionOutcome, DockActionApplyError>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.mutate_controller_from_host_with(cx, categories, mutate, |outcome| outcome.changed())
    }

    pub(crate) fn mutate_controller_from_host_with<R>(
        &mut self,
        cx: &mut Context<Self>,
        categories: &[DockSurfaceChangeCategory],
        mutate: impl FnOnce(&mut DockController) -> Result<R, DockActionApplyError>,
        changed: impl FnOnce(&R) -> bool,
    ) -> Result<R, DockActionApplyError> {
        let controller = self.controller.clone();
        let Some(owner) = self.surface_owner.as_ref().and_then(WeakEntity::upgrade) else {
            return cx.update_entity(&controller, |controller, cx| {
                let outcome = mutate(controller);
                let did_change = outcome.as_ref().map(changed).unwrap_or(false);
                if did_change {
                    cx.notify();
                }
                outcome
            });
        };
        with_root_transaction(&owner, cx, |owner, transaction, cx| {
            let (outcome, did_change) = cx.update_entity(&controller, |controller, cx| {
                let outcome = mutate(controller);
                let did_change = outcome.as_ref().map(changed).unwrap_or(false);
                if did_change {
                    cx.notify();
                }
                (outcome, did_change)
            });
            if did_change {
                owner.record_changes(transaction, categories.iter().copied());
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

    pub(crate) fn clear_last_presentation_scene(&mut self) -> bool {
        self.last_presentation_scene.take().is_some()
    }

    pub(crate) fn viewport_runtime(&self) -> &DockViewportRuntimeHandle {
        &self.viewport_runtime
    }

    pub(crate) fn viewport_scene_publication(&self) -> PrepaintPublicationId {
        self.viewport_scene_publication
    }

    pub(crate) fn motion_preference(&self, cx: &Context<Self>) -> MotionPreference {
        self.with_workspace(cx, |workspace| workspace.options().motion_preference)
    }

    pub(crate) fn current_window_binding(&self) -> Option<DockHostWindowBinding> {
        self.bound_window_id.map(|window_id| DockHostWindowBinding {
            window_id,
            generation: self.window_binding_generation,
        })
    }

    fn is_current_window_binding(
        &self,
        binding: DockHostWindowBinding,
        window_id: WindowId,
    ) -> bool {
        self.bound_window_id == Some(binding.window_id)
            && self.window_binding_generation == binding.generation
            && window_id == binding.window_id
    }

    pub(crate) fn accepts_window_callback(
        &self,
        binding: Option<DockHostWindowBinding>,
        window_id: WindowId,
    ) -> bool {
        binding.is_none_or(|binding| self.is_current_window_binding(binding, window_id))
    }

    pub(crate) fn accepts_bound_window(&self, binding: Option<DockHostWindowBinding>) -> bool {
        binding.is_none_or(|binding| {
            self.bound_window_id == Some(binding.window_id)
                && self.window_binding_generation == binding.generation
        })
    }

    fn is_current_bound_window_id(&self, window_id: WindowId) -> bool {
        self.bound_window_id.is_none_or(|bound| bound == window_id)
    }

    fn release_surface_activation_registration(
        &mut self,
        registration: DockSurfaceActivationHostRegistration,
        cx: &mut App,
    ) {
        let status = registration.status();
        let space = registration.space().clone();
        let window_id = registration.window().window_id();
        if let Some(owner) = self.surface_owner.as_ref().and_then(WeakEntity::upgrade) {
            let settlements = cx.update_entity(&owner, |owner, owner_cx| {
                let settlements = owner.activation_mut().release_host(&registration);
                owner_cx.notify();
                settlements
            });
            Self::defer_activation_settlements(settlements, cx);
        }
        if status == DockSurfaceActivationHostRegistrationStatus::Committed {
            self.viewport_runtime
                .unregister_host_for_space_with_app(&space, window_id, cx);
        }
    }

    fn release_window_bound_state(&mut self, window_id: WindowId, cx: &mut App) {
        let registration = self.surface_activation_registration.take();
        let pending_command = self.interaction.take_pending_focus_command();
        self.pending_focus_completion = None;
        self.panel_focus_trackers.clear();
        self.interaction.reset_window_bound_state();
        self.raw_drag_pointer_capture = None;
        self.last_visual_affordance_scene = None;
        self.last_presentation_scene = None;

        if let Some(registration) = registration {
            self.release_surface_activation_registration(registration, cx);
        } else if self
            .surface_owner
            .as_ref()
            .and_then(WeakEntity::upgrade)
            .is_none()
        {
            self.viewport_runtime
                .unregister_host_for_space_with_app(self.space(), window_id, cx);
        }

        if let Some(command) = pending_command {
            if let Some(binding) = command.surface_activation_binding() {
                binding.settle(DockSurfaceActivationOutcome::Unavailable, cx);
            }
        }
    }

    fn ensure_window_binding(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let window_id = window.window_handle().window_id();
        if self.bound_window_id == Some(window_id) {
            return;
        }

        let previous_window_id = self.bound_window_id;
        let previous_activation_subscription = self.viewport_activation_subscription.take();
        let previous_bounds_subscription = self.viewport_bounds_subscription.take();
        let previous_release_subscription = self.viewport_release_subscription.take();
        let previous_focus_completion = self.pending_focus_completion.take();
        let previous_panel_focus_trackers = std::mem::take(&mut self.panel_focus_trackers);

        self.window_binding_generation = self.window_binding_generation.wrapping_add(1).max(1);
        self.bound_window_id = Some(window_id);

        let Some(previous_window_id) = previous_window_id else {
            return;
        };

        // Change the binding before dropping observers so a callback queued by the old window
        // cannot mutate state belonging to the new window.
        drop(previous_activation_subscription);
        drop(previous_bounds_subscription);
        drop(previous_release_subscription);
        drop(previous_focus_completion);
        drop(previous_panel_focus_trackers);
        self.release_window_bound_state(previous_window_id, cx);
    }

    pub(crate) fn ensure_pointer_session(&mut self, window: &mut Window) -> PointerCaptureHandle {
        let window_id = window.window_handle().window_id();
        if let Some(handle) = self
            .raw_drag_pointer_capture
            .filter(|handle| handle.window_id() == window_id)
        {
            return handle;
        }

        let handle = window.new_pointer_capture_handle();
        self.raw_drag_pointer_capture = Some(handle);
        handle
    }

    pub(crate) fn ensure_viewport_activation_subscription(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.viewport_activation_subscription.is_some() {
            return;
        }

        let binding = self.current_window_binding();
        self.viewport_activation_subscription = Some(cx.observe_window_activation(
            window,
            move |host, window, cx| {
                let window_id = window.window_handle().window_id();
                if !host.accepts_window_callback(binding, window_id) {
                    return;
                }
                if window.is_window_active() {
                    host.apply_confirmed_backend_window_focus(
                        window_id,
                        DockViewportPlatformFocusRestoreGate::from_app(cx),
                        cx,
                    );
                }
            },
        ));
    }

    pub(crate) fn ensure_surface_activation_host_registration(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ensure_window_binding(window, cx);
        let Some(owner) = self.surface_owner.as_ref().and_then(WeakEntity::upgrade) else {
            return;
        };
        let window_handle = window.window_handle();
        let host_id = cx.entity().entity_id();
        let registration_status = self
            .surface_activation_registration
            .as_ref()
            .filter(|registration| {
                registration.host_id() == host_id
                    && registration.space() == self.space()
                    && registration.window() == window_handle
            })
            .map(DockSurfaceActivationHostRegistration::status);
        match registration_status {
            Some(DockSurfaceActivationHostRegistrationStatus::Committed) => return,
            Some(DockSurfaceActivationHostRegistrationStatus::DuplicateHostConflict) => {
                let host = cx.entity().downgrade();
                let space = self.space().clone();
                let result = cx.update_entity(&owner, |owner, _| {
                    owner
                        .activation_mut()
                        .register_host(space, host, window_handle)
                });
                let (registration, settlements) = result.into_parts();
                self.surface_activation_registration = Some(registration);
                Self::defer_activation_settlements(settlements, cx);
                return;
            }
            None => {}
        }

        if let Some(previous) = self.surface_activation_registration.take() {
            self.release_surface_activation_registration(previous, cx);
        }

        let host = cx.entity().downgrade();
        let space = self.space().clone();
        let result = cx.update_entity(&owner, |owner, _| {
            owner
                .activation_mut()
                .register_host(space, host, window_handle)
        });
        let (registration, settlements) = result.into_parts();
        self.surface_activation_registration = Some(registration);
        Self::defer_activation_settlements(settlements, cx);
    }

    fn apply_confirmed_backend_window_focus(
        &mut self,
        window_id: WindowId,
        platform_focus_restore_gate: DockViewportPlatformFocusRestoreGate,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.is_current_bound_window_id(window_id) {
            return false;
        }
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
            .is_some_and(|command| self.request_viewport_focus_command_in_context(command, cx));
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
        let binding = self.current_window_binding();

        self.viewport_bounds_subscription =
            Some(cx.observe_window_bounds(window, move |host, window, cx| {
                let window_id = window.window_handle().window_id();
                if !host.accepts_window_callback(binding, window_id) {
                    return;
                }
                runtime.apply_platform_window_facts(
                    window_id,
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

        let binding = self.current_window_binding();
        self.viewport_release_subscription =
            Some(cx.on_release_in(window, move |host, window, cx| {
                let window_id = window.window_handle().window_id();
                if !host.accepts_window_callback(binding, window_id) {
                    return;
                }
                host.bound_window_id = None;
                host.window_binding_generation =
                    host.window_binding_generation.wrapping_add(1).max(1);
                host.release_window_bound_state(window_id, cx);
            }));
    }

    fn defer_activation_settlements(settlements: DockSurfaceActivationSettlements, cx: &mut App) {
        if settlements.is_empty() {
            return;
        }
        cx.defer(move |cx| settlements.deliver(cx));
    }

    #[cfg(test)]
    pub(crate) fn request_viewport_focus_command(
        &mut self,
        command: DockViewportFocusCommand,
    ) -> bool {
        let changed = self.interaction.request_viewport_focus_command(command);
        if changed {
            self.pending_focus_completion = None;
        }
        changed
    }

    pub(crate) fn request_viewport_focus_command_in_context(
        &mut self,
        command: DockViewportFocusCommand,
        cx: &mut Context<Self>,
    ) -> bool {
        if command
            .surface_activation_binding()
            .is_some_and(|binding| !binding.is_current(cx))
        {
            if let Some(binding) = command.surface_activation_binding() {
                binding.settle(DockSurfaceActivationOutcome::Unavailable, cx);
            }
            return false;
        }

        let previous = self.interaction.pending_focus_command_ticket();
        let requested_binding = command.surface_activation_binding().cloned();
        let changed = self.interaction.request_viewport_focus_command(command);
        if changed {
            self.pending_focus_completion = None;
            if let Some(binding) = previous
                .as_ref()
                .and_then(|ticket| ticket.command().surface_activation_binding())
            {
                binding.settle(DockSurfaceActivationOutcome::Superseded, cx);
            }
        } else if let Some(binding) = requested_binding {
            let same_request_is_pending = self
                .interaction
                .pending_focus_command_ticket()
                .as_ref()
                .and_then(|ticket| ticket.command().surface_activation_binding())
                == Some(&binding);
            if !same_request_is_pending {
                binding.settle(DockSurfaceActivationOutcome::Rejected, cx);
            }
        }
        changed
    }

    pub(crate) fn prepare_pending_focus_selection_from_render(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_current_bound_window_id(window.window_handle().window_id()) {
            return;
        }
        let Some(ticket) = self.interaction.pending_focus_command_ticket() else {
            return;
        };
        let ticket_generation = ticket.generation();
        if ticket
            .command()
            .surface_activation_binding()
            .is_some_and(|binding| !binding.is_current(cx))
        {
            self.settle_pending_focus_command_generation(
                ticket_generation,
                DockSurfaceActivationOutcome::Unavailable,
                cx,
            );
            return;
        }
        if !window.subtree_presentation().is_interactive() {
            self.settle_pending_focus_command_generation(
                ticket_generation,
                DockSurfaceActivationOutcome::Rejected,
                cx,
            );
            return;
        }
        let command = ticket.command();
        if command.source() != crate::DockViewportFocusCommandSource::ViewportActivation {
            return;
        }
        let DockViewportFocusRequest::Panel(item) = command.request() else {
            return;
        };
        let space = self.space().clone();

        if self
            .mutate_controller_from_host(
                cx,
                &[DockSurfaceChangeCategory::Selection],
                |controller| {
                    controller
                        .workspace_mut()
                        .select_item_in_space(space, item.clone())
                },
            )
            .is_err()
        {
            self.settle_pending_focus_command_generation(
                ticket_generation,
                DockSurfaceActivationOutcome::Unavailable,
                cx,
            );
        }
    }

    pub(crate) fn apply_pending_focus_from_render(
        &mut self,
        session: &DockHostRenderSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_current_bound_window_id(window.window_handle().window_id()) {
            return;
        }
        let Some(ticket) = self.interaction.pending_focus_command_ticket() else {
            return;
        };
        let ticket_generation = ticket.generation();
        if ticket
            .command()
            .surface_activation_binding()
            .is_some_and(|binding| !binding.is_current(cx))
        {
            self.settle_pending_focus_command_generation(
                ticket_generation,
                DockSurfaceActivationOutcome::Unavailable,
                cx,
            );
            return;
        }
        if !window.subtree_presentation().is_interactive() {
            self.settle_pending_focus_command_generation(
                ticket_generation,
                DockSurfaceActivationOutcome::Rejected,
                cx,
            );
            return;
        }
        let command = ticket.command();
        match command.request().clone() {
            DockViewportFocusRequest::Panel(item) => {
                match session
                    .visible_panel_registration(&item)
                    .and_then(|panel| panel.focus_handle(cx))
                {
                    Some(focus_handle) => {
                        let focus_target = window
                            .committed_focus(cx)
                            .filter(|focused| focus_handle.contains(focused, window))
                            .unwrap_or(focus_handle);
                        self.ensure_pending_panel_focus_completion(
                            &ticket,
                            &item,
                            &focus_target,
                            window,
                            cx,
                        );
                    }
                    None => {
                        self.record_no_panel_focus_for_gone_platform_panel(command, &item, cx);
                        self.settle_pending_focus_command_generation(
                            ticket_generation,
                            DockSurfaceActivationOutcome::Unavailable,
                            cx,
                        );
                    }
                }
            }
            DockViewportFocusRequest::NoPanelFocus => {
                match self.no_panel_focus_settlement(window, cx) {
                    DockNoPanelFocusSettlement::Focus(focus_handle) => {
                        self.ensure_pending_no_panel_focus_completion(
                            &ticket,
                            Some(&focus_handle),
                            window,
                            cx,
                        );
                    }
                    DockNoPanelFocusSettlement::Blur => {
                        self.ensure_pending_no_panel_focus_completion(&ticket, None, window, cx);
                    }
                }
            }
        }
    }

    pub(crate) fn remember_panel_focus(&mut self, item: DockItemId, cx: &mut Context<Self>) {
        let space = self.space().clone();
        self.viewport_runtime()
            .record_panel_focus(space.clone(), item.clone());
        let _ = self.mutate_controller_from_host(
            cx,
            &[DockSurfaceChangeCategory::Selection],
            |controller| {
                controller
                    .workspace_mut()
                    .select_item_in_space(space, item.clone())
            },
        );
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

    fn take_pending_focus_command_generation(
        &mut self,
        generation: u64,
    ) -> Option<DockViewportFocusCommand> {
        let command = self
            .interaction
            .take_pending_focus_command_if_generation(generation);
        if command.is_some() {
            self.pending_focus_completion = None;
        }
        command
    }

    fn settle_pending_focus_command_generation(
        &mut self,
        generation: u64,
        outcome: DockSurfaceActivationOutcome,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(command) = self.take_pending_focus_command_generation(generation) else {
            return false;
        };
        if let Some(binding) = command.surface_activation_binding() {
            binding.settle(outcome, cx);
        }
        true
    }

    fn ensure_pending_panel_focus_completion(
        &mut self,
        ticket: &DockPendingFocusCommand,
        item: &DockItemId,
        focus_handle: &FocusHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .pending_focus_completion
            .as_ref()
            .is_some_and(|completion| {
                completion.ticket == *ticket && completion.target.as_ref() == Some(focus_handle)
            })
        {
            return;
        }

        self.pending_focus_completion = None;
        let focus_ticket = ticket.clone();
        let focus_item = item.clone();
        let completion_generation = ticket.generation();
        let completion_binding = self.current_window_binding();
        let completion_window_id = window.window_handle().window_id();
        let subscription = cx.focus_with_completion(
            focus_handle,
            window,
            move |outcome, host, callback_window, cx| {
                if !host.accepts_window_callback(
                    completion_binding,
                    callback_window.window_handle().window_id(),
                ) || !host.is_current_bound_window_id(completion_window_id)
                {
                    return;
                }
                let Some(ticket) = host.interaction.pending_focus_command_ticket() else {
                    return;
                };
                if ticket.generation() != completion_generation {
                    return;
                }
                if ticket
                    .command()
                    .surface_activation_binding()
                    .is_some_and(|binding| !binding.is_current(cx))
                {
                    host.settle_pending_focus_command_generation(
                        completion_generation,
                        DockSurfaceActivationOutcome::Unavailable,
                        cx,
                    );
                    return;
                }
                let Some(command) =
                    host.take_pending_focus_command_generation(completion_generation)
                else {
                    return;
                };
                if outcome == FocusClaimOutcome::Committed {
                    host.remember_panel_focus(focus_item, cx);
                }
                if let Some(binding) = command.surface_activation_binding() {
                    binding.settle(outcome.into(), cx);
                }
                cx.notify();
            },
        );
        if self.interaction.pending_focus_command_ticket().as_ref() != Some(ticket) {
            drop(subscription);
            return;
        }
        self.pending_focus_completion = Some(DockPendingFocusCompletion {
            ticket: focus_ticket,
            target: Some(focus_handle.clone()),
            _subscription: subscription,
        });
    }

    fn ensure_pending_no_panel_focus_completion(
        &mut self,
        ticket: &DockPendingFocusCommand,
        target: Option<&FocusHandle>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = target.cloned();
        if self
            .pending_focus_completion
            .as_ref()
            .is_some_and(|completion| completion.ticket == *ticket && completion.target == target)
        {
            return;
        }

        self.pending_focus_completion = None;
        let focus_ticket = ticket.clone();
        let completion_generation = ticket.generation();
        let completion_binding = self.current_window_binding();
        let completion_window_id = window.window_handle().window_id();
        let completion = move |outcome: FocusClaimOutcome,
                               host: &mut DockHost,
                               window: &mut Window,
                               cx: &mut Context<DockHost>| {
            if !host.accepts_window_callback(completion_binding, window.window_handle().window_id())
                || !host.is_current_bound_window_id(completion_window_id)
            {
                return;
            }
            let Some(ticket) = host.interaction.pending_focus_command_ticket() else {
                return;
            };
            if ticket.generation() != completion_generation {
                return;
            }
            if ticket
                .command()
                .surface_activation_binding()
                .is_some_and(|binding| !binding.is_current(cx))
            {
                host.settle_pending_focus_command_generation(
                    completion_generation,
                    DockSurfaceActivationOutcome::Unavailable,
                    cx,
                );
                return;
            }
            let Some(command) = host.take_pending_focus_command_generation(completion_generation)
            else {
                return;
            };
            let committed_focus = window.committed_focus(cx);
            if !host.focus_belongs_to_panel(committed_focus.as_ref(), window) {
                host.viewport_runtime().record_no_panel_focus(host.space());
            }
            if let Some(binding) = command.surface_activation_binding() {
                binding.settle(outcome.into(), cx);
            }
            cx.notify();
        };
        let subscription = match target.as_ref() {
            Some(target) => cx.focus_with_completion(target, window, completion),
            None => cx.blur_with_completion(window, completion),
        };
        if self.interaction.pending_focus_command_ticket().as_ref() != Some(ticket) {
            drop(subscription);
            return;
        }
        self.pending_focus_completion = Some(DockPendingFocusCompletion {
            ticket: focus_ticket,
            target,
            _subscription: subscription,
        });
    }

    fn no_panel_focus_settlement(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> DockNoPanelFocusSettlement {
        let current_focus = window.focused(cx);
        let committed_focus = window.committed_focus(cx);
        let current_is_panel = self.focus_belongs_to_panel(current_focus.as_ref(), window);

        match current_focus {
            Some(current) if !current_is_panel => DockNoPanelFocusSettlement::Focus(current),
            Some(_) => committed_focus
                .filter(|focus| !self.focus_belongs_to_panel(Some(focus), window))
                .map(DockNoPanelFocusSettlement::Focus)
                .unwrap_or(DockNoPanelFocusSettlement::Blur),
            None => DockNoPanelFocusSettlement::Blur,
        }
    }

    fn focus_belongs_to_panel(&self, focus: Option<&FocusHandle>, window: &Window) -> bool {
        focus.is_some_and(|focus| {
            self.panel_focus_trackers
                .values()
                .any(|tracker| tracker.focus_handle.contains(focus, window))
        })
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
        let binding = self.current_window_binding();
        let subscription = cx.on_focus_in(&focus_handle, window, move |host, window, cx| {
            if !host.accepts_window_callback(binding, window.window_handle().window_id()) {
                return;
            }
            host.remember_panel_focus(focus_item.clone(), cx);
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
