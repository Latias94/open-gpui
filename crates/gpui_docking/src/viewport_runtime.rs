#[cfg(test)]
use crate::drop_runtime::DockHostDropScene;
#[cfg(test)]
use crate::viewport_registry::DockViewportRouteUnavailableReason;
pub(crate) use crate::viewport_tear_off_placement::suggested_tear_off_window_bounds;
#[cfg(test)]
use crate::viewport_window_lifecycle::DockViewportReusableWindow;
use crate::{
    DockActionApplyError, DockActionOutcome, DockController, DockDropDelivery,
    DockDropWorkspaceCommit, DockItemId, DockNodeId, DockSpaceId,
    DockViewportActivationBackendFocusApply, DockViewportActivationBackendFocusObservation,
    DockViewportActivationBackendFocusRecordEffect,
    DockViewportActivationPendingBackendFocusEffect, DockViewportActivationTransaction,
    DockViewportAdapter, DockViewportBackendFocusState, DockViewportBackendRouteRequest,
    DockViewportCloseCoordinator, DockViewportCloseOutcome, DockViewportClosePolicy,
    DockViewportCommittedTearOffMove, DockViewportDropActionOutcome, DockViewportDropRoute,
    DockViewportDropRouteOutcome, DockViewportDropRouteRequest, DockViewportDropRouteSnapshot,
    DockViewportDropRouteSnapshotRefresh, DockViewportFocusCoordinator, DockViewportFocusRequest,
    DockViewportFocusStampFallbackPermit, DockViewportFrameCoordinator, DockViewportIdentity,
    DockViewportPayloadDragBegin, DockViewportPayloadDragState, DockViewportPlacementLayout,
    DockViewportPlacementValidationError, DockViewportPlatformFocusRestoreGate,
    DockViewportPlatformFocusRestorePolicy, DockViewportPlatformSyncRecord,
    DockViewportRegisterOutcome, DockViewportResolvedDropRoute,
    DockViewportResolvedDropRouteOutcome, DockViewportResolvedDropRouteRefresh,
    DockViewportRestoreReadiness, DockViewportRoutePreview, DockViewportRoutedDropPreview,
    DockViewportRoutedDropPreviewReplacement, DockViewportRoutedDropPreviewState,
    DockViewportRuntimeHandle, DockViewportRuntimeStatus, DockViewportRuntimeUpdate,
    DockViewportShouldCloseOutcome, DockViewportShouldCloseStatus, DockViewportTearOffBeginOutcome,
    DockViewportTearOffCancelReason, DockViewportTearOffCancelled, DockViewportTearOffCompleted,
    DockViewportTearOffKey, DockViewportTearOffMachine, DockViewportTearOffOpenOutcome,
    DockViewportTearOffPending, DockViewportTearOffPlacement, DockViewportTearOffPlacementPolicy,
    DockViewportTearOffRequest, DockViewportTearOffSourceStatus, DockViewportWindowEffects,
    DockViewportWindowFacts, DockViewportWindowOwnership, DockViewportWindowRetirement,
    drag::{DockDragPayload, DockDragTearOffGeometry},
    drop_runtime::DockHostDropSceneFact,
    extend_unique_windows,
    interaction::DockRuntimeDragSession,
    resolved_drop_route_outcome,
    viewport_drop_scene::{DockViewportHostSceneFrame, DockViewportHostSceneRegistration},
    viewport_registry::DockViewportPlatformRequests,
    viewport_window_lifecycle::{
        DockViewportCloseRecoveryActivation, DockViewportClosedWindowRefresh,
        DockViewportReplacementCleanup, DockViewportReusableWindowOutcome,
        DockViewportRuntimeWindowStateCleanup, DockViewportShouldCloseRefresh,
        DockViewportSpaceFocusCleanup, DockViewportUnregisteredSpace,
        DockViewportVacatedTearOffSource, DockViewportWindowLifecycleController,
    },
    workspace_drop_transaction::DockWorkspacePayloadDropRequest,
};
#[cfg(test)]
use open_gpui::AppContext as _;
use open_gpui::{
    AnyWindowHandle, App, Bounds, Entity, Pixels, PlatformFocusedWindow, Point, WindowId,
    WindowOptions,
};

/// Internal owner for controller-backed platform viewport lifecycle.
///
/// The runtime keeps the shared [`model::DockController`](crate::model::DockController) together
/// with the low-level [`DockViewportAdapter`] so the handle does not have to pass the controller
/// into every open call or duplicate close-callback cleanup logic. The adapter remains the place
/// for window mappings, live window facts, and placement import/export.
#[derive(Debug)]
pub(crate) struct DockViewportRuntime {
    controller: Entity<DockController>,
    adapter: DockViewportAdapter,
    close_policy: DockViewportClosePolicy,
    frame_coordinator: DockViewportFrameCoordinator,
    tear_off: DockViewportTearOffMachine,
    next_tear_off_space_index: u64,
    payload_drag: DockViewportPayloadDragState,
    window_ownership: DockViewportWindowOwnership,
    focus: DockViewportFocusCoordinator,
    backend_focus: DockViewportBackendFocusState,
    close_coordinator: DockViewportCloseCoordinator,
    routed_drop_preview: DockViewportRoutedDropPreviewState,
    status: DockViewportRuntimeStatus,
}

#[derive(Debug)]
pub(crate) struct DockViewportRuntimeRegistration {
    pub(crate) outcome: DockViewportRegisterOutcome,
    window_effects: DockViewportWindowEffects,
}

impl DockViewportRuntimeRegistration {
    pub(crate) fn window_effects(&self) -> DockViewportWindowEffects {
        self.window_effects.clone()
    }
}

#[derive(Debug, Default)]
struct DockViewportVacatedPayloadDropSource {
    windows: Vec<AnyWindowHandle>,
    affected_windows: Vec<AnyWindowHandle>,
}

#[derive(Debug)]
pub(crate) struct DockViewportPreparedTearOffDrop {
    request: DockViewportTearOffRequest,
    target_space: DockSpaceId,
    focus_item: Option<DockItemId>,
    options: WindowOptions,
}

impl DockViewportPreparedTearOffDrop {
    fn new(
        request: DockViewportTearOffRequest,
        target_space: DockSpaceId,
        focus_item: Option<DockItemId>,
        options: WindowOptions,
    ) -> Self {
        Self {
            request,
            target_space,
            focus_item,
            options,
        }
    }

    pub(crate) fn target_space(&self) -> &DockSpaceId {
        &self.target_space
    }

    #[cfg(test)]
    pub(crate) fn focus_item(&self) -> Option<&DockItemId> {
        self.focus_item.as_ref()
    }
}

pub(crate) struct DockViewportPreparedTearOffWindow {
    pub(crate) pending: DockViewportTearOffPending,
    pub(crate) options: WindowOptions,
}

pub(crate) enum DockViewportPreparedTearOffBegin {
    Pending(DockViewportPreparedTearOffWindow),
    Duplicate(DockViewportTearOffPending),
}

impl DockViewportRuntime {
    /// Creates a runtime with the default close policy.
    pub(crate) fn new(controller: Entity<DockController>) -> Self {
        Self::with_close_policy(controller, DockViewportClosePolicy::default())
    }

    /// Creates a runtime with an explicit close policy.
    pub(crate) fn with_close_policy(
        controller: Entity<DockController>,
        close_policy: DockViewportClosePolicy,
    ) -> Self {
        Self {
            controller,
            adapter: DockViewportAdapter::new(),
            close_policy,
            frame_coordinator: DockViewportFrameCoordinator::default(),
            tear_off: DockViewportTearOffMachine::default(),
            next_tear_off_space_index: 0,
            payload_drag: DockViewportPayloadDragState::default(),
            window_ownership: DockViewportWindowOwnership::default(),
            focus: DockViewportFocusCoordinator::default(),
            backend_focus: DockViewportBackendFocusState::default(),
            close_coordinator: DockViewportCloseCoordinator::default(),
            routed_drop_preview: DockViewportRoutedDropPreviewState::default(),
            status: DockViewportRuntimeStatus::default(),
        }
    }

    /// Creates a runtime from an existing adapter.
    #[cfg(test)]
    pub(crate) fn from_adapter(
        controller: Entity<DockController>,
        adapter: DockViewportAdapter,
        close_policy: DockViewportClosePolicy,
    ) -> Self {
        Self {
            controller,
            adapter,
            close_policy,
            frame_coordinator: DockViewportFrameCoordinator::default(),
            tear_off: DockViewportTearOffMachine::default(),
            next_tear_off_space_index: 0,
            payload_drag: DockViewportPayloadDragState::default(),
            window_ownership: DockViewportWindowOwnership::default(),
            focus: DockViewportFocusCoordinator::default(),
            backend_focus: DockViewportBackendFocusState::default(),
            close_coordinator: DockViewportCloseCoordinator::default(),
            routed_drop_preview: DockViewportRoutedDropPreviewState::default(),
            status: DockViewportRuntimeStatus::default(),
        }
    }

    /// Wraps this runtime in a cloneable handle for GPUI application callbacks.
    pub(crate) fn into_handle(self) -> DockViewportRuntimeHandle {
        DockViewportRuntimeHandle::from_runtime(self)
    }

    pub(crate) fn controller_entity(&self) -> Entity<DockController> {
        self.controller.clone()
    }

    /// Returns the low-level viewport adapter.
    pub(crate) fn adapter(&self) -> &DockViewportAdapter {
        &self.adapter
    }

    #[cfg(test)]
    pub(crate) fn unregister_adapter_window_for_test(&mut self, window_id: WindowId) {
        let _ = self.adapter.unregister_window_id_snapshot(window_id);
    }

    #[cfg(test)]
    pub(crate) fn viewport_route_ready(&self, space: &DockSpaceId) -> bool {
        self.adapter.route_ready(space)
    }

    #[cfg(test)]
    pub(crate) fn viewport_route_unavailable_reason(
        &self,
        space: &DockSpaceId,
    ) -> Option<DockViewportRouteUnavailableReason> {
        self.adapter.route_unavailable_reason(space)
    }

    /// Returns the latest read-only runtime diagnostic snapshot.
    pub(crate) fn runtime_status(&self) -> DockViewportRuntimeStatus {
        let status = self.status.clone();
        status.with_viewport_lifecycle(self.adapter.viewport_lifecycle_records())
    }

    #[cfg(test)]
    pub(crate) fn pending_activation(&self) -> Option<&DockViewportActivationTransaction> {
        self.backend_focus.pending_activation()
    }

    #[cfg(test)]
    pub(crate) fn begin_payload_drag(
        &mut self,
        payload: &DockDragPayload,
    ) -> DockRuntimeDragSession {
        self.begin_payload_drag_with_pointer_sync_and_focus(payload, None)
            .session
    }

    pub(crate) fn begin_payload_drag_with_pointer_sync_and_focus(
        &mut self,
        payload: &DockDragPayload,
        focus_item: Option<DockItemId>,
    ) -> DockViewportPayloadDragBegin {
        let source_window = self
            .adapter
            .window_for_space(payload.identity().source_space());
        let source_window_accepts_pointer_input =
            source_window.and_then(|_| self.source_window_accepts_pointer_input(payload));
        let begin = self.payload_drag.begin(
            payload,
            focus_item,
            source_window,
            source_window_accepts_pointer_input,
        );
        self.clear_routed_drop_preview();
        begin
    }

    pub(crate) fn update_payload_drag_tear_off_geometry(
        &mut self,
        session: &DockRuntimeDragSession,
        geometry: DockDragTearOffGeometry,
    ) -> bool {
        self.payload_drag
            .update_tear_off_geometry(session, geometry)
    }

    pub(crate) fn active_payload_drag_tear_off_geometry(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<DockDragTearOffGeometry> {
        self.payload_drag.tear_off_geometry(session)
    }

    pub(crate) fn active_payload_drag_session(
        &self,
        payload: &DockDragPayload,
    ) -> Option<DockRuntimeDragSession> {
        self.payload_drag.active_session_for_payload(payload)
    }

    fn source_window_accepts_pointer_input(&self, payload: &DockDragPayload) -> Option<bool> {
        let Some(snapshot) = self.adapter.snapshot(payload.identity().source_space()) else {
            return Some(true);
        };
        Some(snapshot.input_mask.drag_restore_accepts_pointer_input())
    }

    fn window_for_viewport_identity(
        &self,
        identity: &DockViewportIdentity,
    ) -> Option<AnyWindowHandle> {
        let window = self.adapter.window_for_space(identity.space())?;
        (window.window_id() == identity.window_id()).then_some(window)
    }

    pub(crate) fn record_payload_drag_hovered_viewport(
        &mut self,
        session: &DockRuntimeDragSession,
        space: DockSpaceId,
        window_id: WindowId,
    ) -> bool {
        if !self.payload_drag.matches_session(Some(session)) {
            return false;
        }
        if !self.adapter.is_live_window_for_space(&space, window_id) {
            return false;
        }
        self.payload_drag
            .record_last_hovered_viewport_identity(Some(DockViewportIdentity::new(
                space, window_id,
            )))
    }

    #[cfg(test)]
    pub(crate) fn finish_payload_drag(
        &mut self,
        session: &DockRuntimeDragSession,
    ) -> DockViewportRuntimeUpdate {
        self.finish_payload_drag_with_pointer_sync(session)
    }

    pub(crate) fn finish_payload_drag_with_pointer_sync(
        &mut self,
        session: &DockRuntimeDragSession,
    ) -> DockViewportRuntimeUpdate {
        let Some(finish) = self.payload_drag.finish(session) else {
            return DockViewportRuntimeUpdate::default();
        };
        let last_routed_window = finish
            .last_routed_viewport_identity()
            .and_then(|identity| self.window_for_viewport_identity(identity));
        let last_hovered_window = finish
            .last_hovered_viewport_identity()
            .and_then(|identity| self.window_for_viewport_identity(identity));
        let mut update = self.clear_routed_drop_preview_for_drag_session(Some(session));
        update.extend_windows(last_routed_window);
        update.extend_windows(last_hovered_window);
        update.mark_changed(true);
        update.set_pointer_input_sync(finish.pointer_input_sync());
        update
    }

    pub(crate) fn validate_payload_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Result<(), DockActionApplyError> {
        self.payload_drag.validate_session(session)
    }

    fn record_confirmed_backend_focused_window(&mut self, window_id: WindowId) -> Option<bool> {
        let adapter = &self.adapter;
        self.backend_focus
            .record_confirmed_backend_focused_window(window_id, |candidate| {
                adapter.space_for_window_id(candidate).is_some()
                    && !adapter.window_close_requested(candidate)
            })
            .map(|focus_record| focus_record.changed())
    }

    pub(crate) fn record_confirmed_backend_focus_for_window(
        &mut self,
        window_id: WindowId,
    ) -> bool {
        self.record_confirmed_backend_focused_window(window_id)
            .unwrap_or(false)
    }

    pub(crate) fn record_confirmed_backend_focus_signal(
        &mut self,
        focus: PlatformFocusedWindow,
    ) -> bool {
        match focus {
            PlatformFocusedWindow::Window(window) => {
                self.record_confirmed_backend_focus_for_window(window.window_id())
            }
            PlatformFocusedWindow::NoWindow => false,
            PlatformFocusedWindow::Unavailable => false,
        }
    }

    pub(crate) fn reconcile_backend_window_focus(&mut self, cx: &mut App) -> bool {
        self.record_confirmed_backend_focus_signal(cx.focused_window())
    }

    pub(crate) fn apply_activation_backend_focus(
        &mut self,
        activation: &DockViewportActivationTransaction,
        backend_focus: DockViewportActivationBackendFocusObservation,
    ) -> DockViewportActivationBackendFocusApply {
        let backend_focus_recorded_changed = if backend_focus.target_focused() {
            self.record_confirmed_backend_focus_for_window(activation.window_id())
        } else {
            false
        };
        let pending_backend_focus = activation.requests_window_activation()
            && !backend_focus.target_focused()
            && self.record_pending_activation(activation.clone());
        let pending_backend_focus_cleared = if backend_focus.target_focused() {
            self.clear_pending_activation_for(activation.space(), activation.window_id())
        } else {
            false
        };
        DockViewportActivationBackendFocusApply::new(
            DockViewportActivationBackendFocusRecordEffect::from_changed(
                backend_focus_recorded_changed,
            ),
            if backend_focus.target_focused() {
                DockViewportActivationPendingBackendFocusEffect::from_cleared(
                    pending_backend_focus_cleared,
                )
            } else {
                DockViewportActivationPendingBackendFocusEffect::from_recorded(
                    pending_backend_focus,
                )
            },
        )
    }

    pub(crate) fn record_pending_activation(
        &mut self,
        activation: DockViewportActivationTransaction,
    ) -> bool {
        self.backend_focus.record_pending_activation(activation)
    }

    pub(crate) fn clear_pending_activation_for(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> bool {
        self.backend_focus
            .clear_pending_activation_for(space, window_id)
    }

    #[cfg(test)]
    pub(crate) fn focus_command_for_confirmed_backend_window_focus(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
        mouse_down: bool,
        cx: &mut App,
    ) -> Option<crate::DockViewportFocusCommand> {
        self.confirmed_backend_window_focus_outcome(
            space,
            window_id,
            DockViewportPlatformFocusRestoreGate::from_mouse_down(mouse_down),
            cx,
        )
        .into_focus_command()
    }

    pub(crate) fn confirmed_backend_window_focus_outcome(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
        platform_focus_restore_gate: DockViewportPlatformFocusRestoreGate,
        cx: &mut App,
    ) -> crate::DockViewportConfirmedBackendFocusOutcome {
        let backend_focused = match cx.focused_window() {
            PlatformFocusedWindow::Window(window) => window.window_id() == window_id,
            PlatformFocusedWindow::NoWindow => false,
            PlatformFocusedWindow::Unavailable => {
                return crate::DockViewportConfirmedBackendFocusOutcome::default();
            }
        };
        if !backend_focused || !self.adapter.is_live_window_for_space(space, window_id) {
            return crate::DockViewportConfirmedBackendFocusOutcome::default();
        }

        let focus_record_changed = self
            .record_confirmed_backend_focused_window(window_id)
            .expect("backend focus was already validated as a live docking window");
        let platform_focus_restore_policy =
            DockViewportPlatformFocusRestorePolicy::from_platform_focus_sets_dock_focus(
                self.controller
                    .read(cx)
                    .policy()
                    .platform_focus_sets_dock_focus(),
            );
        let focus_outcome = self.backend_focus.confirmed_backend_window_focus_outcome(
            &self.focus,
            space,
            window_id,
            platform_focus_restore_gate,
            platform_focus_restore_policy,
        );
        focus_outcome.with_additional_changed(focus_record_changed)
    }

    pub(crate) fn record_panel_focus(&mut self, space: DockSpaceId, item: DockItemId) {
        self.focus.record_panel_focus(space, item);
    }

    pub(crate) fn record_no_panel_focus(&mut self, space: &DockSpaceId) {
        self.focus.record_no_panel_focus(space);
    }

    pub(crate) fn recorded_panel_focus_matches(
        &self,
        space: &DockSpaceId,
        item: &DockItemId,
    ) -> bool {
        self.focus.focused_panel(space) == Some(item)
    }

    #[cfg(test)]
    pub(crate) fn recorded_had_panel_focus_for_test(&self, space: &DockSpaceId) -> Option<bool> {
        self.focus.had_panel_focus(space)
    }

    fn retire_window(&mut self, window_id: WindowId) -> DockViewportWindowRetirement {
        self.window_ownership.retire_window(window_id)
    }

    fn retire_runtime_window_for_close(
        &mut self,
        window: AnyWindowHandle,
    ) -> DockViewportWindowRetirement {
        self.retire_window(window.window_id())
    }

    pub(crate) fn record_render_passthrough_pointer_input(&mut self, window_id: WindowId) -> bool {
        self.window_ownership
            .record_render_passthrough_pointer_input(window_id)
    }

    pub(crate) fn take_render_passthrough_pointer_input(&mut self, window_id: WindowId) -> bool {
        self.window_ownership
            .take_render_passthrough_pointer_input(window_id)
    }

    /// Returns the close policy used by [`handle_window_should_close`](Self::handle_window_should_close).
    pub(crate) fn close_policy(&self) -> DockViewportClosePolicy {
        self.close_policy.clone()
    }

    /// Replaces the close policy used by [`handle_window_should_close`](Self::handle_window_should_close).
    pub(crate) fn set_close_policy(&mut self, close_policy: DockViewportClosePolicy) {
        self.close_policy = close_policy;
    }

    #[cfg(test)]
    pub(crate) fn pending_tear_off_len(&self) -> usize {
        self.tear_off.len()
    }

    /// Updates display, window, and host bounds for a registered viewport.
    ///
    /// Returns true when the stored runtime snapshot changed.
    pub(crate) fn update_viewport_snapshot(
        &mut self,
        space: &DockSpaceId,
        window_facts: DockViewportWindowFacts,
        host_bounds: Bounds<Pixels>,
    ) -> bool {
        self.adapter
            .update_snapshot(space, window_facts, host_bounds)
    }

    pub(crate) fn platform_requests_for_space(
        &self,
        space: &DockSpaceId,
    ) -> DockViewportPlatformRequests {
        self.adapter.platform_requests_for_space(space)
    }

    pub(crate) fn mark_viewport_window_snapshot_stale(
        &mut self,
        window_id: WindowId,
    ) -> DockViewportRuntimeUpdate {
        let mut update = DockViewportRuntimeUpdate::default();
        update.mark_changed(self.adapter.mark_window_snapshot_stale(window_id));
        update.merge(self.clear_preview_for_unready_window_route(window_id));
        update
    }

    pub(crate) fn apply_platform_window_facts(
        &mut self,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
    ) -> DockViewportRuntimeUpdate {
        let mut update = DockViewportRuntimeUpdate::default();
        update.mark_changed(
            self.adapter
                .apply_platform_window_facts(window_id, window_facts),
        );
        update.merge(self.clear_preview_for_unready_window_route(window_id));
        update
    }

    fn mark_viewport_window_close_requested(
        &mut self,
        window_id: WindowId,
    ) -> DockViewportRuntimeUpdate {
        let mut update = DockViewportRuntimeUpdate::default();
        update.mark_changed(self.adapter.mark_window_close_requested(window_id));
        if let Some(space) = self.adapter.space_for_window_id(window_id).cloned() {
            self.status.clear_window_references(&space, window_id);
            update.merge(self.finish_payload_drag_for_source_space(&space));
        }
        self.frame_coordinator.unregister_window_scene(window_id);
        update.merge(self.clear_routed_drop_preview_if_window_matches(window_id));
        update
    }

    pub(crate) fn cancel_window_close_request(
        &mut self,
        window_id: WindowId,
    ) -> DockViewportRuntimeUpdate {
        let close_plan_effect = self.close_coordinator.cancel_window(window_id);
        let changed = self.adapter.cancel_window_close_requested(window_id);
        if !changed {
            let mut update = DockViewportRuntimeUpdate::default();
            update.mark_changed(close_plan_effect.changed());
            return update;
        }
        let mut update = DockViewportRuntimeUpdate::default();
        update.mark_changed(true);
        let windows: Vec<AnyWindowHandle> = self
            .adapter
            .space_for_window_id(window_id)
            .and_then(|space| self.adapter.window_for_space(space))
            .into_iter()
            .collect();
        update.extend_windows(windows);
        update
    }

    pub(crate) fn reconcile_viewport_frame<C: open_gpui::AppContext>(
        &mut self,
        cx: &mut C,
    ) -> DockViewportRuntimeUpdate {
        self.reconcile_viewport_frame_except_window(None, cx)
    }

    pub(crate) fn reconcile_viewport_frame_except_window<C: open_gpui::AppContext>(
        &mut self,
        skip_window_id: Option<WindowId>,
        cx: &mut C,
    ) -> DockViewportRuntimeUpdate {
        let changed_windows = self
            .adapter
            .refresh_registered_window_facts_except_window(cx, skip_window_id);
        let mut update = DockViewportRuntimeUpdate::default();
        update.mark_changed(!changed_windows.is_empty());
        for window in changed_windows {
            update.extend_windows([window]);
            update.merge(self.clear_preview_for_unready_window_route(window.window_id()));
        }
        update
    }

    fn clear_preview_for_unready_window_route(
        &mut self,
        window_id: WindowId,
    ) -> DockViewportRuntimeUpdate {
        if self.adapter.window_route_ready(window_id) == Some(false) {
            self.clear_routed_drop_preview_if_window_matches(window_id)
        } else {
            DockViewportRuntimeUpdate::default()
        }
    }

    #[cfg(test)]
    pub(crate) fn begin_viewport_host_scene(
        &mut self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        host_bounds: Bounds<Pixels>,
        host_position: Point<Pixels>,
    ) -> bool {
        self.begin_viewport_host_scene_frame(
            space,
            window_id,
            window_facts,
            host_bounds,
            host_position,
            crate::DockDropGuideStyle::default(),
        )
        .is_some_and(|registration| registration.changed)
    }

    #[cfg(test)]
    pub(crate) fn begin_viewport_host_scene_frame(
        &mut self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        host_bounds: Bounds<Pixels>,
        host_position: Point<Pixels>,
        drop_guide_style: crate::DockDropGuideStyle,
    ) -> Option<DockViewportHostSceneRegistration> {
        self.begin_viewport_host_scene_frame_with_facts(
            space,
            window_id,
            window_facts,
            host_bounds,
            host_position,
            drop_guide_style,
            Vec::new(),
        )
    }

    pub(crate) fn begin_viewport_host_scene_frame_with_facts(
        &mut self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        host_bounds: Bounds<Pixels>,
        host_position: Point<Pixels>,
        drop_guide_style: crate::DockDropGuideStyle,
        initial_facts: Vec<DockHostDropSceneFact>,
    ) -> Option<DockViewportHostSceneRegistration> {
        let space = space.into();
        let window = self.adapter.window_for_space(&space)?;
        let current_identity = DockViewportIdentity::new(space.clone(), window.window_id());
        if !current_identity.matches(&space, window_id) {
            return None;
        }
        let close_cancelled = if self.adapter.window_close_requested(window_id) {
            self.cancel_window_close_request(window_id).changed()
        } else {
            false
        };
        let changed = self.update_viewport_snapshot(&space, window_facts, host_bounds);
        let mut registration = self.frame_coordinator.register_host_scene_with_facts(
            space,
            window_id,
            window_facts,
            host_bounds,
            host_position,
            drop_guide_style,
            initial_facts,
        );
        registration.changed |= changed || close_cancelled;
        Some(registration)
    }

    #[cfg(test)]
    pub(crate) fn push_viewport_host_scene_fact(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
        fact: DockHostDropSceneFact,
    ) -> bool {
        self.frame_coordinator.push_fact(space, window_id, fact)
    }

    pub(crate) fn push_viewport_host_scene_frame_fact(
        &mut self,
        frame: &DockViewportHostSceneFrame,
        fact: DockHostDropSceneFact,
    ) -> Option<DockViewportHostSceneFrame> {
        self.frame_coordinator.push_frame_fact(frame, fact)
    }

    pub(crate) fn rendered_leaf_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
    ) -> Option<Bounds<Pixels>> {
        self.frame_coordinator
            .leaf_bounds_for_tabs(space, window_id, tabs)
    }

    pub(crate) fn rendered_tab_bar_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
    ) -> Option<Bounds<Pixels>> {
        self.frame_coordinator
            .tab_bar_bounds_for_tabs(space, window_id, tabs)
    }

    pub(crate) fn rendered_tab_label_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
        target_index: usize,
    ) -> Option<Bounds<Pixels>> {
        self.frame_coordinator
            .tab_label_bounds_for_tabs(space, window_id, tabs, target_index)
    }

    #[cfg(test)]
    pub(crate) fn rendered_host_drop_scene_for_window(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<DockHostDropScene> {
        self.frame_coordinator.scene_for_window(space, window_id)
    }

    pub(crate) fn routed_drop_preview_for(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<DockViewportRoutedDropPreview> {
        self.routed_drop_preview.preview_for(space, window_id)
    }

    pub(crate) fn routed_drop_route_preview_for(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<crate::drop_preview::DockDropRoutePreview> {
        self.routed_drop_preview.route_preview_for(space, window_id)
    }

    #[cfg(test)]
    pub(crate) fn has_routed_drop_preview_for_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> bool {
        self.routed_drop_preview
            .has_preview_for_drag_session(session)
    }

    #[cfg(test)]
    pub(crate) fn update_routed_drop_preview(
        &mut self,
        resolution: &DockViewportResolvedDropRoute,
        payload: &DockDragPayload,
    ) -> DockViewportRuntimeUpdate {
        self.update_routed_drop_preview_inner(resolution, payload, None, None, None)
    }

    pub(crate) fn update_host_routed_drop_preview(
        &mut self,
        resolution: &DockViewportResolvedDropRoute,
        payload: &DockDragPayload,
        host_space: DockSpaceId,
        host_window_id: WindowId,
        host_position: Point<Pixels>,
    ) -> DockViewportRuntimeUpdate {
        self.update_routed_drop_preview_inner(
            resolution,
            payload,
            Some(host_space),
            Some(host_window_id),
            Some(host_position),
        )
    }

    fn update_routed_drop_preview_inner(
        &mut self,
        resolution: &DockViewportResolvedDropRoute,
        payload: &DockDragPayload,
        host_space: Option<DockSpaceId>,
        host_window_id: Option<WindowId>,
        host_position: Option<Point<Pixels>>,
    ) -> DockViewportRuntimeUpdate {
        let active_drag_session_id = self.payload_drag.active_session_id();
        if let Some(active_drag_session) = self.payload_drag.active_session()
            && let Some(identity) = crate::last_routed_viewport_identity_from_resolution(
                resolution,
                Some(active_drag_session),
            )
        {
            self.payload_drag
                .record_last_routed_viewport_identity(Some(identity));
        }
        if let Some(active_drag_session) = self.payload_drag.active_session()
            && let Some(identity) = crate::route_selection_viewport_identity_from_resolution(
                resolution,
                Some(active_drag_session),
            )
        {
            self.payload_drag
                .record_last_hovered_viewport_identity(Some(identity));
        }
        let next = match resolution.route() {
            DockViewportDropRoute::Local { .. } | DockViewportDropRoute::KnownViewport { .. } => {
                resolution
                    .routed_preview_target_snapshot()
                    .and_then(|target| {
                        crate::routed_drop_preview_from_target(
                            target,
                            active_drag_session_id,
                            payload,
                        )
                    })
            }
            DockViewportDropRoute::Rejected(_) => resolution
                .routed_preview_target_snapshot()
                .and_then(|target| {
                    crate::routed_rejected_drop_preview_from_target(
                        target,
                        active_drag_session_id,
                        payload,
                    )
                }),
            DockViewportDropRoute::TearOff => None,
            DockViewportDropRoute::Unavailable => None,
        };
        let next_route_preview = match (host_space, host_window_id, host_position) {
            (Some(space), Some(window_id), Some(position)) => {
                crate::routed_drop_route_preview_for_host(
                    resolution,
                    space,
                    window_id,
                    position,
                    active_drag_session_id,
                )
            }
            _ => None,
        };
        let next_resolution = match resolution.route() {
            DockViewportDropRoute::Unavailable => None,
            _ => Some(resolution.clone()),
        };
        let update = self.replace_routed_drop_preview(next, next_route_preview, next_resolution);
        update
    }

    #[cfg(test)]
    pub(crate) fn resolve_payload_drop_delivery_for_request<C: open_gpui::AppContext>(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportResolvedDropRoute {
        self.resolve_payload_drop_delivery_for_request_with_outcome(request, cx)
            .outcome
            .into_resolution()
    }

    pub(crate) fn resolve_payload_drop_delivery_for_request_with_outcome<
        C: open_gpui::AppContext,
    >(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportResolvedDropRouteRefresh {
        let refresh = self.resolve_payload_drop_delivery_with_outcome(request, cx);
        let DockViewportResolvedDropRouteRefresh {
            outcome,
            window_effects,
        } = refresh;
        let changed = outcome.changed();
        let resolution = outcome.into_resolution();
        let outcome = DockViewportResolvedDropRouteOutcome::new(resolution, changed);
        DockViewportResolvedDropRouteRefresh {
            outcome,
            window_effects,
        }
    }

    fn replace_routed_drop_preview(
        &mut self,
        next: Option<DockViewportRoutedDropPreview>,
        next_route_preview: Option<DockViewportRoutePreview>,
        next_resolution: Option<DockViewportResolvedDropRoute>,
    ) -> DockViewportRuntimeUpdate {
        let replacement =
            self.routed_drop_preview
                .replace(next, next_route_preview, next_resolution);
        let mut update = DockViewportRuntimeUpdate::default();
        update.mark_changed(replacement.has_changed());
        update.extend_windows(self.windows_for_routed_preview_replacement(&replacement));
        update
    }

    fn windows_for_routed_preview_replacement(
        &self,
        replacement: &DockViewportRoutedDropPreviewReplacement,
    ) -> Vec<AnyWindowHandle> {
        let mut windows = Vec::new();
        for space in replacement.affected_spaces() {
            crate::push_unique_window(&mut windows, self.adapter.window_for_space(space));
        }
        windows
    }

    pub(crate) fn clear_routed_drop_preview(&mut self) -> DockViewportRuntimeUpdate {
        self.replace_routed_drop_preview(None, None, None)
    }

    fn clear_routed_drop_preview_if_window_matches(
        &mut self,
        window_id: WindowId,
    ) -> DockViewportRuntimeUpdate {
        self.payload_drag
            .clear_last_viewport_identity_if_window_matches(window_id);
        if self.routed_drop_preview.targets_window(window_id) {
            self.replace_routed_drop_preview(None, None, None)
        } else {
            DockViewportRuntimeUpdate::default()
        }
    }

    fn clear_routed_drop_preview_for_drag_session(
        &mut self,
        session: Option<&DockRuntimeDragSession>,
    ) -> DockViewportRuntimeUpdate {
        let Some(session) = session else {
            return DockViewportRuntimeUpdate::default();
        };
        self.payload_drag
            .clear_last_viewport_identity_for_session(session);
        let replacement = self
            .routed_drop_preview
            .clear_for_drag_session(Some(session));
        let mut update = DockViewportRuntimeUpdate::default();
        update.mark_changed(replacement.has_changed());
        update.extend_windows(self.windows_for_routed_preview_replacement(&replacement));
        update
    }

    fn clear_runtime_window_state(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
        cleanup: DockViewportRuntimeWindowStateCleanup,
    ) -> DockViewportRuntimeUpdate {
        let mut update = DockViewportRuntimeUpdate::default();
        update.merge(self.clear_routed_drop_preview_if_window_matches(window_id));
        if cleanup.discard_close_plan() {
            update.mark_changed(self.close_coordinator.discard_window(window_id).changed());
        }
        self.window_ownership.clear_window_state(window_id);
        self.backend_focus.discard_window(window_id);
        self.frame_coordinator.unregister_space(space);
        self.clear_pending_activation_for(space, window_id);
        self.status.clear_window_references(space, window_id);
        if cleanup.focus_cleanup() == DockViewportSpaceFocusCleanup::Remove {
            self.focus.remove_space(space);
        }
        update.merge(self.finish_payload_drag_for_source_space(space));
        update
    }

    fn finish_payload_drag_for_source_space(
        &mut self,
        space: &DockSpaceId,
    ) -> DockViewportRuntimeUpdate {
        self.finish_payload_drag_for_source_space_with_pointer_sync(space)
            .without_pointer_input_sync()
    }

    fn finish_payload_drag_for_source_space_with_pointer_sync(
        &mut self,
        space: &DockSpaceId,
    ) -> DockViewportRuntimeUpdate {
        let Some(session) = self
            .payload_drag
            .active_session()
            .filter(|session| session.source_space() == space)
            .cloned()
        else {
            return DockViewportRuntimeUpdate::default();
        };
        self.finish_payload_drag_with_pointer_sync(&session)
    }

    fn unregister_space_runtime_state(
        &mut self,
        space: &DockSpaceId,
    ) -> Option<DockViewportUnregisteredSpace> {
        let snapshot = self.adapter.unregister_space(space)?;
        let window = snapshot.window;
        let affected_windows = self
            .clear_runtime_window_state(
                space,
                window.window_id(),
                DockViewportRuntimeWindowStateCleanup::SpaceUnregistered,
            )
            .into_windows();
        Some(DockViewportUnregisteredSpace {
            window,
            affected_windows,
        })
    }

    #[cfg(test)]
    pub(crate) fn unregister_host_for_space(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> bool {
        self.unregister_host_for_space_with_pointer_sync(space, window_id)
            .changed()
    }

    pub(crate) fn unregister_host_for_space_with_pointer_sync(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> DockViewportRuntimeUpdate {
        if self
            .adapter
            .window_for_space(space)
            .is_none_or(|window| window.window_id() != window_id)
        {
            return DockViewportRuntimeUpdate::default();
        }
        let mut update = self.finish_payload_drag_for_source_space_with_pointer_sync(space);
        if let Some(unregistered) = self.unregister_space_runtime_state(space) {
            update.mark_changed(true);
            update.extend_windows(unregistered.affected_windows);
            self.retire_window(unregistered.window.window_id());
        }
        update
    }

    #[cfg(test)]
    pub(crate) fn reusable_window_for_space(
        &mut self,
        space: &DockSpaceId,
        cx: &mut App,
    ) -> DockViewportReusableWindow {
        self.reusable_window_for_space_with_cleanup(space, cx)
            .into_parts()
            .0
    }

    pub(crate) fn reusable_window_for_space_with_cleanup(
        &mut self,
        space: &DockSpaceId,
        cx: &mut App,
    ) -> DockViewportReusableWindowOutcome {
        let Some(window) = self.adapter.window_for_space(space) else {
            return DockViewportReusableWindowOutcome::missing();
        };
        if self.adapter.window_close_requested(window.window_id()) {
            return DockViewportReusableWindowOutcome::stale();
        }
        if window.update(cx, |_, _, _| ()).is_ok() {
            return DockViewportReusableWindowOutcome::reused(window);
        }

        let mut affected_windows = Vec::new();
        if let Some(unregistered) = self.unregister_space_runtime_state(space) {
            affected_windows = unregistered.affected_windows;
            self.retire_window(unregistered.window.window_id());
        }
        DockViewportReusableWindowOutcome::stale_with_affected_windows(affected_windows)
    }

    #[cfg(test)]
    pub(crate) fn register_opened_viewport(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
    ) -> Vec<AnyWindowHandle> {
        self.register_opened_viewport_with_cleanup(space, window)
            .window_effects
            .close_now()
            .to_vec()
    }

    pub(crate) fn register_opened_viewport_with_cleanup(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
    ) -> DockViewportRuntimeRegistration {
        self.register_runtime_viewport(space, window)
    }

    fn register_runtime_viewport(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
    ) -> DockViewportRuntimeRegistration {
        let outcome = self
            .adapter
            .register_viewport_with_outcome(space.clone(), window);
        let cleanup = self.clear_replaced_viewport_mappings(&outcome, &space, window);
        self.window_ownership
            .register_runtime_window(window.window_id());
        self.backend_focus
            .record_viewport_created(window.window_id());
        DockViewportRuntimeRegistration {
            outcome,
            window_effects: DockViewportWindowEffects::new(
                cleanup.replaced_windows,
                cleanup.affected_windows,
                Vec::new(),
            ),
        }
    }

    fn clear_replaced_viewport_mappings(
        &mut self,
        outcome: &DockViewportRegisterOutcome,
        registered_space: &DockSpaceId,
        registered_window: AnyWindowHandle,
    ) -> DockViewportReplacementCleanup {
        let mut cleanup = DockViewportReplacementCleanup::default();
        for removed in outcome.replaced() {
            let affected_windows = self
                .clear_runtime_window_state(
                    &removed.space,
                    removed.window.window_id(),
                    if &removed.space == registered_space {
                        DockViewportRuntimeWindowStateCleanup::ReplacedSameSpaceMapping
                    } else {
                        DockViewportRuntimeWindowStateCleanup::ReplacedDifferentSpaceMapping
                    },
                )
                .into_windows();
            extend_unique_windows(&mut cleanup.affected_windows, affected_windows);
            if removed.window != registered_window
                && self
                    .retire_runtime_window_for_close(removed.window)
                    .should_close_window()
                && !cleanup.replaced_windows.contains(&removed.window)
            {
                cleanup.replaced_windows.push(removed.window);
            }
        }
        cleanup
    }

    #[cfg(test)]
    pub(crate) fn register_rendered_host_viewport(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
    ) -> bool {
        self.register_rendered_host_viewport_with_cleanup(space, window)
            .changed()
    }

    pub(crate) fn register_rendered_host_viewport_with_cleanup(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
    ) -> DockViewportRuntimeUpdate {
        if self.window_ownership.is_retired(window.window_id()) {
            return DockViewportRuntimeUpdate::default();
        }
        match self.adapter.window_for_space(&space) {
            Some(existing) if existing == window => DockViewportRuntimeUpdate::default(),
            Some(_) => DockViewportRuntimeUpdate::default(),
            None => {
                let outcome = self
                    .adapter
                    .register_viewport_with_outcome(space.clone(), window);
                let cleanup = self.clear_replaced_viewport_mappings(&outcome, &space, window);
                self.backend_focus
                    .record_viewport_created(window.window_id());
                let mut update = DockViewportRuntimeUpdate::default();
                update.mark_changed(true);
                update.extend_windows(cleanup.affected_windows);
                update.extend_windows(cleanup.replaced_windows);
                update
            }
        }
    }

    pub(crate) fn deliver_drop_commit_delivery_with_outcome(
        &mut self,
        delivery: DockDropDelivery,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let result = self.deliver_payload_drop_inner(delivery, cx);
        self.status.record_drop_result(&result);
        result
    }

    #[cfg(test)]
    pub(crate) fn validate_payload_drop_delivery(
        &self,
        delivery: &DockDropDelivery,
        cx: &App,
    ) -> Result<(), DockActionApplyError> {
        self.validate_payload_drag_session(delivery.drag_session())?;
        let controller = self.controller.read(cx);
        delivery.validate_current_workspace_target(
            &self.adapter,
            self.frame_coordinator.host_scenes(),
            controller.workspace(),
        )
    }

    pub(crate) fn record_drop_route_result(
        &mut self,
        result: &Result<DockViewportDropRouteOutcome, DockActionApplyError>,
    ) {
        self.status.record_drop_result(result);
    }

    pub(crate) fn record_tear_off_outcome(&mut self, outcome: &DockViewportTearOffOpenOutcome) {
        self.status.record_tear_off(outcome);
    }

    pub(crate) fn record_platform_sync(&mut self, record: DockViewportPlatformSyncRecord) {
        self.status.record_platform_sync(record);
    }

    pub(crate) fn record_visual_affordance_status(
        &mut self,
        space: DockSpaceId,
        window_id: WindowId,
        summary: crate::DockVisualAffordanceDebugSummary,
    ) {
        self.status
            .record_visual_affordance(space, window_id, summary);
    }

    pub(crate) fn clear_visual_affordance_status(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) {
        self.status.clear_visual_affordance(space, window_id);
    }

    fn deliver_payload_drop_inner(
        &mut self,
        delivery: DockDropDelivery,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        self.validate_payload_drag_session(delivery.drag_session())?;
        let DockDropWorkspaceCommit {
            source_space,
            source_node,
            payload,
            target,
            drag_session,
        } = {
            let controller = self.controller.read(cx);
            delivery.into_workspace_commit(
                &self.adapter,
                self.frame_coordinator.host_scenes(),
                controller.workspace(),
            )?
        };

        let target_space = target.target_space().clone();
        let frozen_focus_item = drag_session
            .as_ref()
            .and_then(|session| session.focus_item())
            .cloned();
        let drop_outcome = self.controller.update(cx, |controller, cx| {
            let outcome = controller.workspace_mut().commit_resolved_payload_drop(
                DockWorkspacePayloadDropRequest {
                    source_space: &source_space,
                    payload: payload.as_workspace_payload(source_node),
                    target,
                    frozen_focus_item: frozen_focus_item.as_ref(),
                },
            );
            if outcome
                .as_ref()
                .map(|outcome| outcome.changed())
                .unwrap_or(false)
            {
                cx.notify();
            }
            outcome
        })?;
        let focus_item = drag_session
            .as_ref()
            .and_then(|session| session.focus_item())
            .and_then(|focus_item| {
                self.controller
                    .read(cx)
                    .graph()
                    .find_item_in_space(&target_space, focus_item)?;
                Some(focus_item.clone())
            })
            .or_else(|| drop_outcome.focus_item().cloned());
        let focus_request = focus_item
            .map(DockViewportFocusRequest::panel)
            .unwrap_or_else(DockViewportFocusRequest::no_panel_focus);
        let (activation, reusable_effects) = DockViewportWindowLifecycleController::drop_activation(
            self.reusable_window_for_space_with_cleanup(&target_space, cx),
            target_space.clone(),
            focus_request,
        );
        let vacated_source =
            self.vacate_empty_payload_drop_source_viewport(&source_space, &target_space, cx);
        let window_effects = reusable_effects.merge(DockViewportWindowEffects::new(
            Vec::new(),
            vacated_source.affected_windows,
            vacated_source.windows,
        ));
        Ok(DockViewportDropRouteOutcome::Action(
            DockViewportDropActionOutcome::new(drop_outcome.action(), activation)
                .with_window_effects(window_effects),
        ))
    }

    pub(crate) fn vacate_empty_payload_drop_source_viewport_with_cleanup(
        &mut self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        cx: &App,
    ) -> DockViewportWindowEffects {
        let vacated_source =
            self.vacate_empty_payload_drop_source_viewport(source_space, target_space, cx);
        DockViewportWindowEffects::new(
            Vec::new(),
            vacated_source.affected_windows,
            vacated_source.windows,
        )
    }

    fn vacate_empty_payload_drop_source_viewport(
        &mut self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        cx: &App,
    ) -> DockViewportVacatedPayloadDropSource {
        if source_space == target_space {
            return DockViewportVacatedPayloadDropSource::default();
        }
        let source_is_empty = {
            let controller = self.controller.read(cx);
            controller
                .graph()
                .collect_items_in_space(source_space)
                .is_empty()
        };
        if !source_is_empty {
            return DockViewportVacatedPayloadDropSource::default();
        }
        let Some(unregistered) = self.unregister_space_runtime_state(source_space) else {
            return DockViewportVacatedPayloadDropSource::default();
        };
        let windows = if self
            .retire_runtime_window_for_close(unregistered.window)
            .should_close_window()
        {
            vec![unregistered.window]
        } else {
            Vec::new()
        };
        DockViewportVacatedPayloadDropSource {
            windows,
            affected_windows: unregistered.affected_windows,
        }
    }

    pub(crate) fn prepare_tear_off_drop_delivery(
        &mut self,
        request: DockViewportTearOffRequest,
        cx: &mut App,
    ) -> Result<DockViewportPreparedTearOffDrop, DockActionApplyError> {
        self.validate_payload_drag_session(request.drag_session())?;
        self.prepare_tear_off_drop_route(request, cx)
    }

    pub(crate) fn prepare_tear_off_drop_route(
        &mut self,
        request: DockViewportTearOffRequest,
        cx: &App,
    ) -> Result<DockViewportPreparedTearOffDrop, DockActionApplyError> {
        let options = self.tear_off_window_options(&request)?;
        let target_space = self.next_tear_off_space(&request, cx);
        {
            let controller = self.controller.read(cx);
            crate::preflight_tear_off_move(controller.workspace(), &request, &target_space)?;
        }
        let focus_item = self.focus_item_for_request(&request, cx);
        Ok(DockViewportPreparedTearOffDrop::new(
            request,
            target_space,
            focus_item,
            options,
        ))
    }

    #[cfg(test)]
    pub(crate) fn prepare_tear_off_drop_route_for_test(
        &mut self,
        request: DockViewportTearOffRequest,
        target_space: DockSpaceId,
        options: WindowOptions,
        cx: &App,
    ) -> Result<DockViewportPreparedTearOffDrop, DockActionApplyError> {
        {
            let controller = self.controller.read(cx);
            crate::preflight_tear_off_move(controller.workspace(), &request, &target_space)?;
        }
        let focus_item = self.focus_item_for_request(&request, cx);
        Ok(DockViewportPreparedTearOffDrop::new(
            request,
            target_space,
            focus_item,
            options,
        ))
    }

    pub(crate) fn next_tear_off_space(
        &mut self,
        request: &DockViewportTearOffRequest,
        cx: &App,
    ) -> DockSpaceId {
        loop {
            let space_index = self.next_tear_off_space_index();
            let space = DockSpaceId::new(format!(
                "{}:tear-off:{}:{}",
                request.source_space(),
                request.payload().label(),
                space_index
            ));
            let graph_has_space = self
                .controller
                .read(cx)
                .graph()
                .spaces()
                .iter()
                .any(|known| known == &space);
            if !graph_has_space && self.adapter.window_for_space(&space).is_none() {
                return space;
            }
        }
    }

    pub(crate) fn tear_off_window_options(
        &self,
        request: &DockViewportTearOffRequest,
    ) -> Result<WindowOptions, DockActionApplyError> {
        let window_bounds = self
            .tear_off_window_placement(request)
            .ok_or(DockActionApplyError::TearOffViewportPlacementUnavailable)?
            .window_bounds();

        Ok(WindowOptions {
            window_bounds: Some(window_bounds),
            // Tear-off viewports are activated after graph commit and runtime registration, so
            // panel focus restoration flows through the explicit activation transaction.
            focus: false,
            ..Default::default()
        })
    }

    pub(crate) fn tear_off_window_placement(
        &self,
        request: &DockViewportTearOffRequest,
    ) -> Option<DockViewportTearOffPlacement> {
        DockViewportTearOffPlacementPolicy::default().resolve(request)
    }

    #[cfg(test)]
    pub(crate) fn last_host_scene_screen_position(
        &self,
        space: &DockSpaceId,
    ) -> Option<Point<Pixels>> {
        self.frame_coordinator.screen_position(space)
    }

    #[cfg(test)]
    pub(crate) fn resolve_host_scene_target(
        &self,
        space: &DockSpaceId,
        host_position: Point<Pixels>,
        cx: &App,
    ) -> Option<crate::drop_target::DockResolvedDropTarget> {
        let window = self.adapter.window_for_space(space)?;
        if self
            .adapter
            .snapshot_facts_generation(space, window.window_id())
            .is_none()
        {
            return None;
        }
        let policy = self.controller.read(cx).workspace().policy().clone();
        self.frame_coordinator.host_scenes().resolve_for_window(
            space,
            Some(window.window_id()),
            host_position,
            &policy,
            None,
        )
    }

    /// Resolves a rendered payload release into route and delivery facts from one snapshot.
    #[cfg(test)]
    pub(crate) fn resolve_payload_drop_delivery<C: open_gpui::AppContext>(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportResolvedDropRoute {
        self.resolve_payload_drop_delivery_with_outcome(request, cx)
            .outcome
            .into_resolution()
    }

    pub(crate) fn resolve_payload_drop_delivery_with_outcome<C: open_gpui::AppContext>(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportResolvedDropRouteRefresh {
        let mut update = DockViewportRuntimeUpdate::default();
        let policy = cx.read_entity(&self.controller, |controller, _| {
            controller.workspace().policy().to_owned()
        });
        let initial_route_request =
            self.backend_route_request_without_target_context_resample(request, cx);
        update.mark_changed(initial_route_request.changed);

        let DockViewportDropRouteSnapshotRefresh {
            snapshot: resampled_snapshot,
            changed: resampled_changed,
            window_effects: resampled_effects,
        } = self.resampled_drop_route_snapshot(request, &policy, cx);
        update.mark_changed(resampled_changed);
        update.extend_windows(resampled_effects.refresh().iter().cloned());
        let selection = resampled_snapshot.into_route_selection();
        let unavailable_reason = selection.route_resolution.unavailable_reason();
        let route = selection.route_resolution.into_route();
        let resolution =
            self.resolve_payload_drop_delivery_resolution(&selection.request, route, cx);
        self.status
            .record_route(&selection.request, resolution.route(), unavailable_reason);
        resolved_drop_route_outcome(resolution, update)
    }

    fn resampled_drop_route_snapshot<C: open_gpui::AppContext>(
        &mut self,
        request: &DockViewportDropRouteRequest,
        policy: &crate::DockPolicy,
        cx: &mut C,
    ) -> DockViewportDropRouteSnapshotRefresh {
        let frame_update =
            self.reconcile_viewport_frame_except_window(request.event_receiver_window(), cx);
        let route_request = self.resampled_backend_route_request(request, cx);
        let frame_changed = frame_update.changed();
        DockViewportDropRouteSnapshotRefresh {
            snapshot: DockViewportDropRouteSnapshot::resolve(
                &self.adapter,
                route_request.request,
                policy,
            ),
            changed: frame_changed || route_request.changed,
            window_effects: DockViewportWindowEffects::refresh_only(frame_update.into_windows()),
        }
    }

    fn backend_route_request_without_target_context_resample<C: open_gpui::AppContext>(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportBackendRouteRequest {
        let backend_focus = cx.read_entity(&self.controller, |_, app| app.focused_window());
        let changed = self.record_confirmed_backend_focus_signal(backend_focus);
        let request = request.clone().with_focus_stamp_fallback_permit(
            DockViewportFocusStampFallbackPermit::from_backend_focus(backend_focus),
        );
        DockViewportBackendRouteRequest {
            request: self.with_runtime_fallback_route_context(request),
            changed,
        }
    }

    fn resampled_backend_route_request<C: open_gpui::AppContext>(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportBackendRouteRequest {
        let (request, backend_focus) = cx.read_entity(&self.controller, |_, app| {
            (
                request
                    .clone()
                    .with_resampled_platform_target_context_from_app(app),
                app.focused_window(),
            )
        });
        let changed = self.record_confirmed_backend_focus_signal(backend_focus);
        let request = request.with_focus_stamp_fallback_permit(
            DockViewportFocusStampFallbackPermit::from_backend_focus(backend_focus),
        );
        DockViewportBackendRouteRequest {
            request: self.with_runtime_fallback_route_context(request),
            changed,
        }
    }

    fn with_runtime_fallback_route_context(
        &self,
        request: DockViewportDropRouteRequest,
    ) -> DockViewportDropRouteRequest {
        let request = self.with_drag_last_hovered_viewport_context(request);
        self.with_focus_stamp_fallback_context(request)
    }

    fn with_drag_last_hovered_viewport_context(
        &self,
        request: DockViewportDropRouteRequest,
    ) -> DockViewportDropRouteRequest {
        if request.release_origin() != crate::interaction::DockPayloadDropReleaseOrigin::HoveredHost
        {
            return request;
        }
        let Some(drag_session) = request.drag_session() else {
            return request;
        };
        if !self.payload_drag.matches_session(Some(drag_session)) {
            return request;
        }
        let Some(identity) = self
            .payload_drag
            .last_hovered_viewport_identity(Some(drag_session))
        else {
            return request;
        };
        let Some(window) = self.adapter.window_for_space(identity.space()) else {
            return request;
        };
        if window.window_id() != identity.window_id() {
            return request;
        }
        if self.adapter.window_route_ready(identity.window_id()) != Some(true) {
            return request;
        }
        request.with_drag_last_hovered_viewport_window(identity.window_id())
    }

    fn with_focus_stamp_fallback_context(
        &self,
        request: DockViewportDropRouteRequest,
    ) -> DockViewportDropRouteRequest {
        if !request.allows_focus_stamp_fallback()
            || request.release_origin()
                == crate::interaction::DockPayloadDropReleaseOrigin::SourceOnly
            || !matches!(
                request.target_context().trusted_hovered_signal(),
                crate::DockViewportTrustedHoveredSignal::Unavailable
            )
            || request.target_context().has_hover_fallback_window_stack()
        {
            return request;
        }
        let focused_windows = self
            .backend_focus
            .front_to_back_z_order_windows(|window_id| {
                self.adapter.window_can_route_hover_hit(window_id) == Some(true)
            });
        if focused_windows.is_empty() {
            return request;
        }
        request.with_focus_stamp_window_stack(focused_windows)
    }

    fn resolve_payload_drop_delivery_resolution<C: open_gpui::AppContext>(
        &self,
        request: &DockViewportDropRouteRequest,
        route: DockViewportDropRoute,
        cx: &mut C,
    ) -> DockViewportResolvedDropRoute {
        cx.read_entity(&self.controller, |controller, _| {
            let workspace = controller.workspace();
            let payload_classes = workspace.payload_dock_classes_for_viewport_payload(
                request.payload(),
                request.source_node(),
            );
            self.resolve_payload_drop_delivery_resolution_with_workspace(
                request,
                route,
                workspace,
                &payload_classes,
            )
        })
    }

    fn resolve_payload_drop_delivery_resolution_with_workspace(
        &self,
        request: &DockViewportDropRouteRequest,
        route: DockViewportDropRoute,
        workspace: &crate::DockWorkspace,
        payload_classes: &crate::workspace_move_validation::DockPayloadDockClasses,
    ) -> DockViewportResolvedDropRoute {
        let workspace_target = crate::resolve_workspace_target_for_route(
            &self.adapter,
            self.frame_coordinator.host_scenes(),
            &route,
            request,
            workspace,
            payload_classes,
        );
        DockViewportResolvedDropRoute::from_workspace_route_target(request, route, workspace_target)
    }

    #[cfg(test)]
    pub(crate) fn resolve_payload_drop_route_for_test(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &mut App,
    ) -> DockViewportDropRoute {
        self.reconcile_viewport_frame(cx);
        let policy = cx.read_entity(&self.controller, |controller, _| {
            controller.workspace().policy().to_owned()
        });
        self.adapter
            .resolve_payload_drop_route_resolution(request, &policy)
            .into_route()
    }

    #[cfg(test)]
    pub(crate) fn last_routed_viewport_identity_for_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<DockViewportIdentity> {
        let session = session?;
        self.payload_drag
            .last_routed_viewport_identity(Some(session))
            .cloned()
    }

    #[cfg(test)]
    pub(crate) fn last_hovered_viewport_identity_for_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<DockViewportIdentity> {
        self.payload_drag
            .last_hovered_viewport_identity(session)
            .cloned()
    }

    #[cfg(test)]
    pub(crate) fn begin_tear_off_request(
        &mut self,
        request: DockViewportTearOffRequest,
        target_space: impl Into<DockSpaceId>,
        cx: &App,
    ) -> DockViewportTearOffBeginOutcome {
        let focus_item = self.focus_item_for_request(&request, cx);
        self.begin_tear_off_request_with_focus(request, target_space, focus_item)
    }

    pub(crate) fn begin_tear_off_request_with_focus(
        &mut self,
        request: DockViewportTearOffRequest,
        target_space: impl Into<DockSpaceId>,
        focus_item: Option<DockItemId>,
    ) -> DockViewportTearOffBeginOutcome {
        let source_window = self.adapter.window_for_space(request.source_space());
        self.tear_off
            .begin(request, target_space.into(), source_window, focus_item)
    }

    pub(crate) fn begin_prepared_tear_off_drop(
        &mut self,
        prepared: DockViewportPreparedTearOffDrop,
    ) -> DockViewportPreparedTearOffBegin {
        match self.begin_tear_off_request_with_focus(
            prepared.request,
            prepared.target_space,
            prepared.focus_item,
        ) {
            DockViewportTearOffBeginOutcome::Pending(pending) => {
                DockViewportPreparedTearOffBegin::Pending(DockViewportPreparedTearOffWindow {
                    pending,
                    options: prepared.options,
                })
            }
            DockViewportTearOffBeginOutcome::Duplicate(pending) => {
                DockViewportPreparedTearOffBegin::Duplicate(pending)
            }
        }
    }

    pub(crate) fn cancel_tear_off_request(
        &mut self,
        key: &DockViewportTearOffKey,
        reason: DockViewportTearOffCancelReason,
    ) -> Option<DockViewportTearOffCancelled> {
        self.tear_off.cancel(key, reason)
    }

    pub(crate) fn commit_prepared_tear_off_move(
        &mut self,
        pending: &DockViewportTearOffPending,
        cx: &mut App,
    ) -> Result<DockViewportCommittedTearOffMove, DockActionApplyError> {
        if !self.tear_off.is_current_pending(pending) {
            return Err(DockActionApplyError::DropTargetUnavailable);
        }
        let action = self.commit_tear_off_move(pending, cx)?;
        let Some(committed) = self.tear_off.take_committed(pending, action) else {
            return Err(DockActionApplyError::DropTargetUnavailable);
        };
        Ok(committed)
    }

    pub(crate) fn complete_committed_tear_off_window(
        &mut self,
        committed: DockViewportCommittedTearOffMove,
        window: impl Into<AnyWindowHandle>,
        cx: &App,
    ) -> DockViewportTearOffCompleted {
        self.complete_tear_off_registration(committed, window.into(), cx)
    }

    pub(crate) fn cancel_tear_off_if_source_unavailable(
        &mut self,
        pending: &DockViewportTearOffPending,
        key: &DockViewportTearOffKey,
        cx: &App,
    ) -> Option<DockViewportTearOffCancelled> {
        match self.tear_off_source_status(pending, cx) {
            DockViewportTearOffSourceStatus::Ready => None,
            DockViewportTearOffSourceStatus::Unavailable => self
                .cancel_tear_off_request(key, DockViewportTearOffCancelReason::SourceUnavailable),
        }
    }

    fn complete_tear_off_registration(
        &mut self,
        committed: DockViewportCommittedTearOffMove,
        window: AnyWindowHandle,
        cx: &App,
    ) -> DockViewportTearOffCompleted {
        let commit = committed.into_commit();
        let vacated_source = self.vacate_empty_tear_off_source_viewport(&commit.pending, cx);
        let registration =
            self.register_runtime_viewport(commit.pending.target_space().clone(), window);
        let DockViewportRuntimeRegistration {
            outcome,
            window_effects,
        } = registration;
        DockViewportTearOffCompleted::new(
            commit.pending,
            outcome,
            window_effects.close_now().to_vec(),
            window_effects.refresh().to_vec(),
            vacated_source.windows,
            vacated_source.affected_windows,
            commit.action,
        )
    }

    fn vacate_empty_tear_off_source_viewport(
        &mut self,
        pending: &DockViewportTearOffPending,
        cx: &App,
    ) -> DockViewportVacatedTearOffSource {
        let source_space = pending.request().source_space();
        if source_space == pending.target_space() {
            return DockViewportVacatedTearOffSource::default();
        }
        let source_is_empty = {
            let controller = self.controller.read(cx);
            controller
                .graph()
                .collect_items_in_space(source_space)
                .is_empty()
        };
        if !source_is_empty {
            return DockViewportVacatedTearOffSource::default();
        }
        let (window, affected_windows) =
            if let Some(unregistered) = self.unregister_space_runtime_state(source_space) {
                (Some(unregistered.window), unregistered.affected_windows)
            } else {
                (pending.source_window(), Vec::new())
            };
        let Some(window) = window else {
            return DockViewportVacatedTearOffSource {
                windows: Vec::new(),
                affected_windows,
            };
        };
        let windows = if self
            .retire_runtime_window_for_close(window)
            .should_close_window()
        {
            vec![window]
        } else {
            Vec::new()
        };
        DockViewportVacatedTearOffSource {
            windows,
            affected_windows,
        }
    }

    fn focus_item_for_request(
        &self,
        request: &DockViewportTearOffRequest,
        cx: &App,
    ) -> Option<DockItemId> {
        self.controller
            .read(cx)
            .workspace()
            .activation_focus_item_for_viewport_payload(
                request.payload(),
                request.source_node(),
                request
                    .drag_session()
                    .and_then(DockRuntimeDragSession::focus_item),
            )
    }

    pub(crate) fn drag_focus_item(
        &self,
        payload: &DockDragPayload,
        cx: &App,
    ) -> Option<DockItemId> {
        let focused_item = self.focus.focused_panel(&payload.source_space)?;
        self.controller
            .read(cx)
            .workspace()
            .drag_focus_item_for_payload(payload, Some(focused_item))
    }

    fn focus_item_for_space(&self, space: &DockSpaceId, cx: &App) -> Option<DockItemId> {
        let focused_item = self.focus.focused_panel(space)?.clone();
        let controller = self.controller.read(cx);
        let graph = controller.graph();
        graph
            .find_item_in_space(space, &focused_item)
            .is_some()
            .then_some(focused_item)
    }

    /// Handles a GPUI window-closed notification by removing stale runtime mapping.
    ///
    /// Close policy is applied by [`Self::handle_window_should_close`] before GPUI accepts a close.
    /// Once a closed notification arrives, the platform window is already gone and docking must
    /// discard the runtime mapping even when the current policy is [`DockViewportClosePolicy::Prevent`].
    #[cfg(test)]
    pub(crate) fn handle_window_closed(&mut self, window_id: WindowId) -> DockViewportCloseOutcome {
        let close = self.cleanup_closed_window(window_id);
        self.status.record_close(&close.outcome);
        close.outcome
    }

    fn cleanup_closed_window(&mut self, window_id: WindowId) -> DockViewportClosedWindowRefresh {
        self.retire_window(window_id);
        let outcome = self.adapter.handle_window_closed(window_id);
        let affected_windows = if let Some(space) = outcome.space().cloned() {
            self.clear_runtime_window_state(
                &space,
                window_id,
                DockViewportRuntimeWindowStateCleanup::ClosedWindow,
            )
            .into_windows()
        } else {
            self.frame_coordinator.unregister_window_scene(window_id);
            self.clear_routed_drop_preview_if_window_matches(window_id)
                .into_windows()
        };
        DockViewportClosedWindowRefresh::new(
            outcome,
            DockViewportWindowEffects::refresh_only(affected_windows),
        )
    }

    #[cfg(test)]
    pub(crate) fn handle_window_closed_with_app(
        &mut self,
        window_id: WindowId,
        cx: &mut App,
    ) -> DockViewportCloseOutcome {
        self.handle_window_closed_with_app_and_refresh(window_id, cx)
            .outcome
    }

    pub(crate) fn handle_window_closed_with_app_and_refresh(
        &mut self,
        window_id: WindowId,
        cx: &mut App,
    ) -> DockViewportClosedWindowRefresh {
        let pending_state = self.close_coordinator.take_window_close_state(window_id);
        let close = DockViewportWindowLifecycleController::complete_pending_close_plan(
            self.cleanup_closed_window(window_id),
            pending_state,
            |plan| crate::commit_prevalidated_merge_back_plan(&self.controller, plan, cx),
        );
        self.status.record_close(&close.outcome);
        close
    }

    #[cfg(test)]
    pub(crate) fn activation_transaction_after_close(
        &mut self,
        outcome: &DockViewportCloseOutcome,
        cx: &mut App,
    ) -> Option<DockViewportActivationTransaction> {
        self.activation_transaction_after_close_with_cleanup(outcome, cx)
            .activation
    }

    pub(crate) fn activation_transaction_after_close_with_cleanup(
        &mut self,
        outcome: &DockViewportCloseOutcome,
        cx: &mut App,
    ) -> DockViewportCloseRecoveryActivation {
        let Some(target_space) = outcome.merge_target_space().cloned() else {
            return DockViewportCloseRecoveryActivation::none();
        };
        DockViewportWindowLifecycleController::close_recovery_activation(
            outcome,
            self.reusable_window_for_space_with_cleanup(&target_space, cx),
        )
    }

    pub(crate) fn handle_window_should_close_with_app_and_refresh(
        &mut self,
        window_id: WindowId,
        cx: &mut App,
    ) -> DockViewportShouldCloseRefresh {
        if self.adapter.window_close_requested(window_id) {
            let outcome = self.allowed_should_close_outcome(window_id);
            self.status.record_should_close(&outcome);
            return DockViewportShouldCloseRefresh::new(
                outcome,
                DockViewportWindowEffects::default(),
            );
        }
        let outcome = self
            .adapter
            .should_close_viewport(window_id, self.close_policy());
        let focus_item = outcome
            .space
            .as_ref()
            .and_then(|space| self.focus_item_for_space(space, cx));
        let outcome = self.close_coordinator.apply_should_close_plan(
            outcome,
            self.close_policy(),
            focus_item,
            &self.controller,
            cx,
        );
        let affected_windows = self
            .apply_allowed_should_close_route_invalidation(&outcome)
            .into_windows();
        self.status.record_should_close(&outcome);
        DockViewportShouldCloseRefresh::new(
            outcome,
            DockViewportWindowEffects::refresh_only(affected_windows),
        )
    }

    fn allowed_should_close_outcome(&self, window_id: WindowId) -> DockViewportShouldCloseOutcome {
        DockViewportShouldCloseOutcome {
            space: self.adapter.space_for_window_id(window_id).cloned(),
            window_id,
            status: DockViewportShouldCloseStatus::Allowed,
        }
    }

    fn apply_allowed_should_close_route_invalidation(
        &mut self,
        outcome: &DockViewportShouldCloseOutcome,
    ) -> DockViewportRuntimeUpdate {
        if outcome.status == crate::DockViewportShouldCloseStatus::Allowed {
            return self.mark_viewport_window_close_requested(outcome.window_id);
        }
        DockViewportRuntimeUpdate::default()
    }

    fn next_tear_off_space_index(&mut self) -> u64 {
        let index = self.next_tear_off_space_index;
        self.next_tear_off_space_index = self.next_tear_off_space_index.saturating_add(1);
        index
    }

    fn tear_off_source_status(
        &self,
        pending: &DockViewportTearOffPending,
        cx: &App,
    ) -> DockViewportTearOffSourceStatus {
        crate::tear_off_source_status(self.controller.read(cx).graph(), pending)
    }

    fn commit_tear_off_move(
        &self,
        pending: &DockViewportTearOffPending,
        cx: &mut App,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.controller.update(cx, |controller, cx| {
            let outcome = crate::commit_tear_off_move(controller.workspace_mut(), pending);
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

    /// Exports serializable placement snapshots from the adapter.
    pub(crate) fn export_placement(&self) -> DockViewportPlacementLayout {
        self.adapter.export_placement()
    }

    /// Checks saved placement snapshots against registered viewport windows.
    pub(crate) fn check_placement_restore(
        &mut self,
        placement: &DockViewportPlacementLayout,
    ) -> Result<DockViewportRestoreReadiness, DockViewportPlacementValidationError> {
        let readiness = self.adapter.check_placement_restore(placement)?;
        self.status.record_placement_restore(Some(readiness));
        Ok(readiness)
    }
}
